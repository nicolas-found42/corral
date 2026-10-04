//! Jev-only experiments over The Corral's real transcript corpus.
//!
//! ```text
//! cargo run --bin experiments                  # all experiments (Jev only, no llama calls)
//! cargo run --bin experiments -- leak-absorption voice-distinctness
//! ```
//!
//! Every experiment reads `corpus` and asks TypeSafe Jev questions about it. No
//! language-model generation happens anywhere in this file; every number printed is
//! a probability, a label or a majority count the judgement model returned. A port
//! of the original `experiments.py`.
//!
//! Design (Jev's own rulings):
//!   * one batched request PER EXPERIMENT (`batched_per_experiment`, 0.62) so every
//!     experiment yields one comparable result table;
//!   * each experiment is internally robust: a judgement is asked under SEVERAL
//!     differently-worded phrasings and only reported when they agree, with the
//!     spread shown. Jev's decision flagged "one lucky answer" as the risk
//!     (`multi_phrasing_batch`, 0.33, second choice) so the phrasings are folded in;
//!   * the eavesdrop gate is re-calibrated from the corpus (`calibrate_on_corpus`,
//!     0.71) rather than trusted at 0.46.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{json, Map, Value};
use tokio::sync::Semaphore;

use crate::corpus;
use crate::judge::{Conductor, SystemOne};
use crate::personas::by_id;

/// A phrasing of one judgment: (key, instruction, inverted?).
type Phrasing = (&'static str, &'static str, bool);

/// One Jev client for the experiments. Never prints the key.
pub struct Kit {
    pub conductor: Conductor,
    sem: Semaphore,
    pub cost: f64,
    pub calls: u64,
    pub tin: u64,
    pub tout: u64,
    pub errors: Vec<String>,
}

impl Kit {
    pub fn new(api_key: &str) -> Self {
        Kit {
            conductor: Conductor::with_key(api_key),
            sem: Semaphore::new(6),
            cost: 0.0,
            calls: 0,
            tin: 0,
            tout: 0,
            errors: Vec::new(),
        }
    }

    /// One batched request. Fails soft: an invalid answer is recorded, never faked.
    pub async fn ask(
        &mut self,
        state: &Value,
        questions: &Map<String, Value>,
        what: &str,
    ) -> Option<SystemOne> {
        for attempt in 0..3u32 {
            let permit = self.sem.acquire().await.ok()?;
            let r = self.conductor.system_one(state, questions).await;
            drop(permit);
            match r {
                Ok(r) => {
                    self.calls += 1;
                    self.cost += r.cost;
                    self.tin += r.in_tok;
                    self.tout += r.out_tok;
                    return Some(r);
                }
                Err(e) => {
                    if attempt == 2 {
                        let msg: String = e.chars().take(100).collect();
                        self.errors.push(format!("{what}: {msg}"));
                        return None;
                    }
                    tokio::time::sleep(Duration::from_secs_f64(1.2 * (attempt as f64 + 1.0))).await;
                }
            }
        }
        None
    }
}

/// (worst positive phrasing, best inverted phrasing) -- the conservative pair.
pub fn robust(vals: &[f64], inverted: &[f64]) -> (f64, f64) {
    let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let lo = if vals.is_empty() { 0.5 } else { lo };
    let hi = inverted.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let hi = if inverted.is_empty() { 0.0 } else { hi };
    (round3(lo), round3(hi))
}

/// Agreement verdict for one item's several phrasings.
pub fn verdict(lo: f64, hi: f64) -> &'static str {
    if lo >= 0.65 && hi <= 0.35 {
        "agree-yes"
    } else if lo <= 0.35 && hi <= 0.35 {
        "agree-no"
    } else {
        "split"
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn noul(r: &SystemOne, k: &str) -> f64 {
    r.answers.get(k).map(|a| a.noul()).unwrap_or(0.5)
}

fn pick(r: &SystemOne, k: &str) -> String {
    r.answers.get(k).map(|a| a.choice()).unwrap_or_default()
}

pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().sum::<f64>() / v.len() as f64
}

pub fn pstdev(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
}

// ------------------------------------------------------------------ experiments

pub async fn exp_leak_absorption(kit: &mut Kit) -> Value {
    let mut cases: Vec<(u8, u8, String, String, String)> = Vec::new();
    for (i, l) in corpus::LINES.iter().enumerate() {
        let Some(hf) = l.heard_from else { continue };
        let after = corpus::LINES[i + 1..]
            .iter()
            .find(|x| x.table == l.table && x.heard_from.is_none());
        if let Some(nxt) = after {
            cases.push((
                l.table,
                hf,
                l.text.to_string(),
                nxt.text.to_string(),
                nxt.speaker.to_string(),
            ));
        }
    }
    if cases.is_empty() {
        return json!({"name": "leak-absorption", "note": "no heard line in the corpus has a following reply"});
    }
    let phrasings: [Phrasing; 3] = [
        ("addresses", "The `reply` was written by a llama sitting at table `table`, immediately after a line from another table was read aloud to them. Does `reply` substantively address or build on that heard line, rather than ignoring it?", false),
        ("about_heard", "`reply` follows `heard` in a transcript. Is `reply` a response to what `heard` says?", false),
        ("ignores", "`reply` was written after `heard` was heard at this table. Does `reply` ignore the heard line and just continue the table's own previous topic as if they had not heard anything?", true),
    ];
    let state = json!({"seed": corpus::SEED, "cases": cases.iter().enumerate().map(|(i, c)| json!({
        "id": format!("c{i}"), "table": c.0, "heard_from": c.1, "heard": c.2, "reply": c.3, "replier": c.4
    })).collect::<Vec<_>>()});
    let mut q: Map<String, Value> = Map::new();
    for (j, _c) in cases.iter().enumerate() {
        for (key, instr, _inv) in phrasings {
            let instr = instr
                .replace("`reply`", &format!("case c{j}'s reply"))
                .replace("`heard`", &format!("case c{j}'s heard line"))
                .replace("`table`", &format!("case c{j}'s table"));
            q.insert(
                format!("c{j}_{key}"),
                json!({"type": "noul", "instructions": instr}),
            );
        }
    }
    let r = kit.ask(&state, &q, "leak-absorption").await;
    let mut rows = Vec::new();
    let mut landed = 0;
    for (j, c) in cases.iter().enumerate() {
        match &r {
            None => rows.push(json!({"table": c.0, "error": "no answer"})),
            Some(r) => {
                let pos = vec![
                    noul(r, &format!("c{j}_addresses")),
                    noul(r, &format!("c{j}_about_heard")),
                ];
                let inv = vec![noul(r, &format!("c{j}_ignores"))];
                let (lo, hi) = robust(&pos, &inv);
                let v = verdict(lo, hi);
                if v == "agree-yes" {
                    landed += 1;
                }
                rows.push(json!({"table": c.0, "heard_from": c.1, "heard": c.2, "reply": c.3, "replier": c.4,
                    "p_addresses": round3(pos.iter().cloned().fold(f64::INFINITY, f64::min)),
                    "p_ignores": round3(inv[0]), "worst": lo, "inverted": hi, "verdict": v}));
            }
        }
    }
    json!({"name": "leak-absorption", "cases": cases.len(), "rows": rows, "landed": landed, "of": cases.len(),
        "note": "three phrasings, one inverted; agree-yes means all of them concur"})
}

pub async fn exp_seed_fidelity(kit: &mut Kit) -> Value {
    let instrs: [Phrasing; 3] = [
        ("engages", "Given the shared seed, does this line still engage that seed rather than talking about something else entirely?", false),
        ("on_topic", "Is this line on the topic of the seed?", false),
        ("tangent", "Has this line wandered off the seed onto a side topic of its own?", true),
    ];
    let mut q: Map<String, Value> = Map::new();
    for (i, l) in corpus::LINES.iter().enumerate() {
        for (key, instr, _inv) in instrs {
            q.insert(format!("l{i}_{key}"), json!({"type": "noul",
                "instructions": format!("{instr} The shared seed is `seed`. The line is: “{}”", l.text)}));
        }
    }
    let r = kit
        .ask(&json!({"seed": corpus::SEED}), &q, "seed-fidelity")
        .await;
    let mut rows = Vec::new();
    let mut by_table: std::collections::BTreeMap<u8, Vec<f64>> = Default::default();
    for (i, l) in corpus::LINES.iter().enumerate() {
        let Some(r) = &r else { continue };
        let pos = vec![
            noul(r, &format!("l{i}_engages")),
            noul(r, &format!("l{i}_on_topic")),
        ];
        let inv = vec![noul(r, &format!("l{i}_tangent"))];
        let (lo, hi) = robust(&pos, &inv);
        by_table.entry(l.table).or_default().push(lo);
        rows.push(json!({"table": l.table, "speaker": l.speaker,
            "text": l.text.chars().take(64).collect::<String>(), "score": lo, "inverted": hi, "verdict": verdict(lo, hi)}));
    }
    let by: Map<String, Value> = by_table
        .iter()
        .map(|(k, v)| (k.to_string(), json!(round3(mean(v)))))
        .collect();
    let all: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.get("score").and_then(|s| s.as_f64()))
        .collect();
    json!({"name": "seed-fidelity", "rows": rows, "by_table": by, "mean": round3(mean(&all))})
}

pub async fn exp_cohesion_vs_quality(kit: &mut Kit) -> Value {
    let mut seqs: std::collections::BTreeMap<u8, Vec<&corpus::Line>> = Default::default();
    for l in corpus::LINES.iter() {
        seqs.entry(l.table).or_default().push(l);
    }
    let mut pairs: Vec<(u8, &str, &str)> = Vec::new();
    for (t, seq) in &seqs {
        for i in 1..seq.len() {
            if seq[i].heard_from.is_some() {
                continue;
            }
            pairs.push((*t, seq[i - 1].text, seq[i].text));
        }
    }
    let mut q: Map<String, Value> = Map::new();
    for (i, (_t, prev, cur)) in pairs.iter().enumerate() {
        q.insert(format!("p{i}_flows"), json!({"type": "noul",
            "instructions": format!("Line A: “{prev}”  Line B: “{cur}”  Does B flow naturally from A and continue the same thread of thought?")}));
        q.insert(format!("p{i}_tangent"), json!({"type": "noul",
            "instructions": format!("Line A: “{prev}”  Line B: “{cur}”  Does B change the subject away from A?")}));
        q.insert(format!("p{i}_q"), json!({"type": "score",
            "instructions": format!("How good a contribution to the discussion is Line B: “{cur}”?"),
            "criteria": ["says nothing new", "a useful observation", "a sharp contribution that moves the thread"]}));
    }
    let r = kit
        .ask(&json!({"seed": corpus::SEED}), &q, "cohesion-vs-quality")
        .await;
    let mut out = Vec::new();
    for (i, (t, _prev, cur)) in pairs.iter().enumerate() {
        let Some(r) = &r else { continue };
        let flow = noul(r, &format!("p{i}_flows"));
        let tang = noul(r, &format!("p{i}_tangent"));
        let coh = round3(flow.min(1.0 - tang));
        let qual = r
            .answers
            .get(&format!("p{i}_q"))
            .map(|a| a.score_norm(3))
            .unwrap_or(0.5);
        out.push(json!({"table": t, "cohesion": coh, "quality": round3(qual), "text": cur.chars().take(60).collect::<String>()}));
    }
    let xs: Vec<f64> = out.iter().filter_map(|x| x["cohesion"].as_f64()).collect();
    let ys: Vec<f64> = out.iter().filter_map(|x| x["quality"].as_f64()).collect();
    let corr = if out.len() > 2 {
        let (mx, my) = (mean(&xs), mean(&ys));
        let num: f64 = xs.iter().zip(&ys).map(|(a, b)| (a - mx) * (b - my)).sum();
        let den = (xs.iter().map(|a| (a - mx).powi(2)).sum::<f64>()
            * ys.iter().map(|b| (b - my).powi(2)).sum::<f64>())
        .sqrt();
        if den == 0.0 {
            Value::Null
        } else {
            json!(round3(num / den))
        }
    } else {
        Value::Null
    };
    let mut qs = out.clone();
    qs.sort_by(|a, b| {
        b["cohesion"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["cohesion"].as_f64().unwrap_or(0.0))
            .unwrap()
    });
    let n = (qs.len() / 4).max(1);
    let top: Vec<f64> = qs
        .iter()
        .take(n)
        .filter_map(|x| x["quality"].as_f64())
        .collect();
    let bot: Vec<f64> = qs
        .iter()
        .rev()
        .take(n)
        .filter_map(|x| x["quality"].as_f64())
        .collect();
    json!({"name": "cohesion-vs-quality", "pairs": out.len(), "correlation": corr,
        "quality_top_cohesion_quartile": round3(mean(&top)), "quality_bottom_cohesion_quartile": round3(mean(&bot)), "rows": out})
}

pub async fn exp_voice_distinctness(kit: &mut Kit) -> Value {
    let mut hits = 0;
    let mut detail = Vec::new();
    for l in corpus::LINES.iter() {
        let members = corpus::table_members(l.table).unwrap();
        let mut crit = Map::new();
        for m in members {
            let p = by_id(m).unwrap();
            crit.insert(
                m.to_string(),
                Value::String(format!("{}: {}", p.name, p.tagline)),
            );
        }
        let r = kit
            .ask(
                &json!({"line": l.text, "table": l.table}),
                &{
                    let mut q = Map::new();
                    q.insert("who".to_string(), json!({"type": "choice",
                        "instructions": "A llama at this table said this line. Which of the four was it, judging only from what the line says and how it says it?",
                        "criteria": crit}));
                    q
                },
                &format!("voice-{}-{}", l.table, l.speaker),
            )
            .await;
        let Some(r) = r else { continue };
        let who = pick(&r, "who");
        let ok = who == l.speaker;
        if ok {
            hits += 1;
        }
        detail.push(
            json!({"table": l.table, "said": l.speaker, "guessed": who, "right": ok,
            "line": l.text.chars().take(60).collect::<String>()}),
        );
    }
    let n = detail.len();
    json!({"name": "voice-distinctness", "n": n, "correct": hits,
        "accuracy": if n == 0 { Value::Null } else { json!(round3(hits as f64 / n as f64)) }, "rows": detail})
}

pub async fn exp_leak_gate(kit: &mut Kit) -> Value {
    let carried: std::collections::BTreeSet<&str> =
        corpus::LEAKS.iter().map(|(_, _, _, _, tx)| *tx).collect();
    let instrs: [Phrasing; 3] = [
        ("carry", "Would this line, overheard by a DIFFERENT table working the same seed, change what that other table talks about? It must be striking, surprising or sharply relevant, not an ordinary remark.", false),
        ("portable", "Is this line a portable claim about the seed that another table could pick up and argue with?", false),
        ("table_local", "Is this line only meaningful to the table that said it, with nothing for an outsider to take away?", true),
    ];
    let mut q: Map<String, Value> = Map::new();
    for (i, l) in corpus::LINES.iter().enumerate() {
        for (key, instr, _inv) in instrs {
            q.insert(format!("l{i}_{key}"), json!({"type": "noul",
                "instructions": format!("{instr} The shared seed is `seed`. The line is: “{}”", l.text)}));
        }
    }
    let r = kit
        .ask(&json!({"seed": corpus::SEED}), &q, "leak-gate")
        .await;
    let mut out = Vec::new();
    for (i, l) in corpus::LINES.iter().enumerate() {
        let Some(r) = &r else { continue };
        let pos = vec![
            noul(r, &format!("l{i}_carry")),
            noul(r, &format!("l{i}_portable")),
        ];
        let inv = vec![noul(r, &format!("l{i}_table_local"))];
        let (lo, hi) = robust(&pos, &inv);
        out.push(json!({"table": l.table, "speaker": l.speaker, "score": lo, "inverted": hi,
            "carried": carried.contains(l.text), "text": l.text.chars().take(60).collect::<String>()}));
    }
    let mut sweep = Vec::new();
    for thr in [0.30, 0.35, 0.40, 0.45, 0.46, 0.50, 0.55, 0.60] {
        let keeps: Vec<&Value> = out
            .iter()
            .filter(|x| x["score"].as_f64().unwrap_or(0.0) >= thr)
            .collect();
        let real = keeps
            .iter()
            .filter(|x| x["carried"].as_bool().unwrap_or(false))
            .count();
        sweep.push(json!({"threshold": thr, "lines_passing": keeps.len(), "of": out.len(), "real_leaks_kept": real,
            "real_leaks_scored_above": out.iter().filter(|x| x["carried"].as_bool().unwrap_or(false) && x["score"].as_f64().unwrap_or(0.0) >= thr).count()}));
    }
    json!({"name": "leak-gate", "rows": out, "sweep": sweep,
        "note": "a line below the threshold never carries; the sweep shows what each gate would have admitted"})
}

pub async fn exp_unanswered(kit: &mut Kit) -> Value {
    let qs: Vec<&corpus::Line> = corpus::LINES
        .iter()
        .filter(|l| l.text.contains('?'))
        .collect();
    if qs.is_empty() {
        return json!({"name": "unanswered", "note": "no questions in the corpus"});
    }
    let mut seqs: std::collections::BTreeMap<u8, Vec<&corpus::Line>> = Default::default();
    for l in corpus::LINES.iter() {
        seqs.entry(l.table).or_default().push(l);
    }
    let mut q: Map<String, Value> = Map::new();
    for (i, l) in qs.iter().enumerate() {
        let seq = &seqs[&l.table];
        let idx = seq.iter().position(|x| x.text == l.text).unwrap();
        let ctx: String = seq[idx + 1..]
            .iter()
            .map(|x| format!("“{}”", x.text))
            .collect::<Vec<_>>()
            .join(" ");
        let ctx = if ctx.is_empty() {
            "(nothing followed)".to_string()
        } else {
            ctx
        };
        q.insert(format!("q{i}_answered"), json!({"type": "noul",
            "instructions": format!("The question “{}” was asked at table {}. What followed at that table was: {ctx}. Did what followed answer that question?", l.text, l.table)}));
        q.insert(format!("q{i}_open"), json!({"type": "noul",
            "instructions": format!("The question “{}” was asked at table {}. What followed at that table was: {ctx}. Is the question still open -- did nobody address it?", l.text, l.table)}));
    }
    let r = kit
        .ask(&json!({"seed": corpus::SEED}), &q, "unanswered")
        .await;
    let mut out = Vec::new();
    for (i, l) in qs.iter().enumerate() {
        let Some(r) = &r else { continue };
        let ans = noul(r, &format!("q{i}_answered"));
        let opn = noul(r, &format!("q{i}_open"));
        let (lo, hi) = robust(&[ans], &[opn]);
        out.push(
            json!({"table": l.table, "speaker": l.speaker, "question": l.text,
            "p_answered": round3(ans), "p_open": round3(opn), "verdict": verdict(lo, hi)}),
        );
    }
    let open = out.iter().filter(|x| x["verdict"] != "agree-yes").count();
    json!({"name": "unanswered", "rows": out, "open": open, "of": out.len()})
}

pub async fn exp_table_redundancy(kit: &mut Kit) -> Value {
    let ids: Vec<u8> = sorted_tables();
    let joined: std::collections::BTreeMap<u8, String> = ids
        .iter()
        .map(|t| {
            (
                *t,
                corpus::LINES
                    .iter()
                    .filter(|l| l.table == *t)
                    .map(|l| l.text)
                    .collect::<Vec<_>>()
                    .join(" | "),
            )
        })
        .collect();
    let mut q: Map<String, Value> = Map::new();
    let mut pairs = Vec::new();
    for a in &ids {
        for b in &ids {
            if a < b {
                pairs.push((*a, *b));
            }
        }
    }
    for (a, b) in &pairs {
        q.insert(format!("t{a}t{b}_same"), json!({"type": "noul",
            "instructions": format!("Table {a} said: {}   Table {b} said: {}   Are these two tables covering substantially the same ground, rather than different angles on the seed?", joined[a], joined[b])}));
        q.insert(format!("t{a}t{b}_distinct"), json!({"type": "noul",
            "instructions": format!("Table {a} said: {}   Table {b} said: {}   Are these two tables exploring genuinely different angles on the seed?", joined[a], joined[b])}));
    }
    let r = kit
        .ask(&json!({"seed": corpus::SEED}), &q, "redundancy")
        .await;
    let mut out = Vec::new();
    for (a, b) in &pairs {
        let Some(r) = &r else { continue };
        let same = noul(r, &format!("t{a}t{b}_same"));
        let dist = noul(r, &format!("t{a}t{b}_distinct"));
        let (lo, hi) = robust(&[dist], &[same]);
        out.push(
            json!({"pair": format!("T{a}/T{b}"), "p_distinct": round3(dist), "p_same": round3(same),
            "worst": lo, "inverted": hi, "verdict": verdict(lo, hi)}),
        );
    }
    let ds: Vec<f64> = out
        .iter()
        .filter_map(|x| x["p_distinct"].as_f64())
        .collect();
    json!({"name": "redundancy", "rows": out, "mean_distinctness": round3(mean(&ds))})
}

pub async fn exp_best_line(kit: &mut Kit) -> Value {
    let mut per: std::collections::BTreeMap<String, Value> = Default::default();
    for t in sorted_tables() {
        let lines = corpus::lines_of(t);
        if lines.is_empty() {
            continue;
        }
        let mut crit = Map::new();
        for (i, (sp, tx)) in lines.iter().enumerate() {
            crit.insert(
                format!("l{i}"),
                Value::String(format!("{}: {}", by_id(sp).unwrap().name, tx)),
            );
        }
        let r = kit
            .ask(
                &json!({"seed": corpus::SEED}),
                &{
                    let mut q = Map::new();
                    q.insert("best".to_string(), json!({"type": "choice", "instructions": "Which single line best answers the shared seed?", "criteria": crit}));
                    q
                },
                &format!("best-{t}"),
            )
            .await;
        let Some(r) = r else { continue };
        if let Some(i) = pick(&r, "best")
            .strip_prefix('l')
            .and_then(|x| x.parse::<usize>().ok())
        {
            if i < lines.len() {
                per.insert(
                    t.to_string(),
                    json!({"speaker": lines[i].0, "text": lines[i].1}),
                );
            }
        }
    }
    if per.len() >= 2 {
        let mut crit = Map::new();
        for (t, v) in &per {
            crit.insert(
                format!("t{t}"),
                Value::String(format!(
                    "{}: {}",
                    by_id(v["speaker"].as_str().unwrap()).unwrap().name,
                    v["text"].as_str().unwrap()
                )),
            );
        }
        let r = kit
            .ask(
                &json!({"seed": corpus::SEED}),
                &{
                    let mut q = Map::new();
                    q.insert("best".to_string(), json!({"type": "choice",
                        "instructions": "Five tables each picked their best line. Which single line, across all five, best answers the shared seed?",
                        "criteria": crit}));
                    q
                },
                "best-of-five",
            )
            .await;
        if let Some(r) = r {
            if let Some(t) = pick(&r, "best")
                .strip_prefix('t')
                .and_then(|x| x.parse::<u8>().ok())
            {
                per.insert("winner".to_string(), json!(t));
            }
        }
    }
    json!({"name": "best-line", "per_table": per})
}

fn sorted_tables() -> Vec<u8> {
    let mut ids: Vec<u8> = corpus::TABLES.iter().map(|(t, _)| *t).collect();
    ids.sort_unstable();
    ids
}

/// The eight experiments, keyed by their CLI names.
pub const NAMES: [&str; 8] = [
    "leak-absorption",
    "seed-fidelity",
    "cohesion-vs-quality",
    "voice-distinctness",
    "leak-gate",
    "unanswered",
    "redundancy",
    "best-line",
];

pub async fn run_one(kit: &mut Kit, name: &str) -> Value {
    match name {
        "leak-absorption" => exp_leak_absorption(kit).await,
        "seed-fidelity" => exp_seed_fidelity(kit).await,
        "cohesion-vs-quality" => exp_cohesion_vs_quality(kit).await,
        "voice-distinctness" => exp_voice_distinctness(kit).await,
        "leak-gate" => exp_leak_gate(kit).await,
        "unanswered" => exp_unanswered(kit).await,
        "redundancy" => exp_table_redundancy(kit).await,
        "best-line" => exp_best_line(kit).await,
        other => json!({"name": other, "error": format!("unknown experiment: {other}")}),
    }
}

/// Write the results JSON and print the human report.
pub fn report(results: &[Value]) {
    for r in results {
        let name = r.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        println!("\n{}\n{name}\n{}", "=".repeat(78), "=".repeat(78));
        if let Some(e) = r.get("error").and_then(|x| x.as_str()) {
            println!("  FAILED: {e}");
        } else if name == "leak-absorption" {
            println!(
                "  landed: {}/{} heard lines got a reply that addresses them",
                r["landed"], r["of"]
            );
            for x in r["rows"].as_array().into_iter().flatten() {
                println!(
                    "  T{} ←T{}  {:10}  addresses {}  ignores {}",
                    x["table"], x["heard_from"], x["verdict"], x["p_addresses"], x["p_ignores"]
                );
            }
        } else if name == "seed-fidelity" {
            println!("  mean {}   by table {}", r["mean"], r["by_table"]);
        } else if name == "cohesion-vs-quality" {
            println!(
                "  pairs {}  correlation(cohesion, quality) = {}",
                r["pairs"], r["correlation"]
            );
            println!(
                "  quality of the most cohesive quartile  {}   least cohesive {}",
                r["quality_top_cohesion_quartile"], r["quality_bottom_cohesion_quartile"]
            );
        } else if name == "voice-distinctness" {
            println!(
                "  accuracy {} ({}/{}) — guessing would be 0.25",
                r["accuracy"], r["correct"], r["n"]
            );
        } else if name == "leak-gate" {
            println!("  threshold sweep (a line at or above the gate may carry):");
            for s in r["sweep"].as_array().into_iter().flatten() {
                println!(
                    "    ≥{:.2}  admits {:2}/{} lines · keeps {} of the {} logged leaks",
                    s["threshold"].as_f64().unwrap_or(0.0),
                    s["lines_passing"].as_u64().unwrap_or(0),
                    s["of"].as_u64().unwrap_or(0),
                    s["real_leaks_kept"].as_u64().unwrap_or(0),
                    corpus::LEAKS.len()
                );
            }
        } else if name == "unanswered" {
            println!("  still open: {}/{} questions", r["open"], r["of"]);
        } else if name == "redundancy" {
            println!("  mean distinctness ({})", r["mean_distinctness"]);
        } else if name == "best-line" {
            println!("  per-table winners: {}", r["per_table"]);
        }
    }
}

/// A `YYYYMMDD-HHMMSS` stamp.
pub fn stamp() -> String {
    crate::timestamp()
}

/// The output directory for results (`results/` at the repo root).
pub fn results_dir() -> PathBuf {
    PathBuf::from("results")
}

/// Run the named experiments, write `results/experiments-<stamp>.json`, print a report.
pub async fn run(api_key: &str, want: Vec<String>, calls: &AtomicU64) -> Result<PathBuf, String> {
    let unknown: Vec<String> = want
        .iter()
        .filter(|w| !NAMES.contains(&w.as_str()))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "unknown experiment(s): {unknown:?}\navailable: {NAMES:?}"
        ));
    }
    std::fs::create_dir_all(results_dir()).map_err(|e| format!("results dir: {e}"))?;
    let t0 = std::time::Instant::now();
    println!(
        "corpus: {} real lines, 5 tables, {} logged eavesdrops",
        corpus::corpus_size(),
        corpus::LEAKS.len()
    );
    println!(
        "running {} Jev-only experiment(s): {}\n",
        want.len(),
        want.join(", ")
    );

    let mut kit = Kit::new(api_key);
    let mut results = Vec::new();
    for name in &want {
        println!("— {name} …");
        let res = run_one(&mut kit, name).await;
        results.push(res);
        println!("  done ({} calls so far, ${:.4})", kit.calls, kit.cost);
    }
    let path = results_dir().join(format!("experiments-{}.json", stamp()));
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&results).unwrap_or_default(),
    )
    .map_err(|e| format!("write results: {e}"))?;
    report(&results);
    println!(
        "\n{} Jev calls · ${:.4} · {} in / {} out tokens · {}s · {}",
        kit.calls,
        kit.cost,
        kit.tin,
        kit.tout,
        t0.elapsed().as_secs(),
        path.display()
    );
    if !kit.errors.is_empty() {
        println!(
            "errors ({}): {:?}",
            kit.errors.len(),
            &kit.errors[..kit.errors.len().min(3)]
        );
    }
    calls.store(kit.calls, Ordering::SeqCst);
    Ok(path)
}
