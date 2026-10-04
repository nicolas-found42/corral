//! Jev on itself: which PRIMITIVE and which PHRASING answer the same question best?
//!
//!     cargo run --bin method_compare
//!
//! Two meta-questions, both answered from the corpus, both judged by Jev:
//!
//! 1. PRIMITIVE. For the same judgement -- "is this a good line?" -- ask it three
//!    ways: a Noul ("does this line move the thread?"), a Score (three ordered
//!    levels) and a Choice (better than the median line, or not). Then measure which
//!    one spreads the lines out most, and ask Jev which answer it trusts.
//!
//! 2. PHRASING. For the same judgement asked three different ways, measure how far
//!    the answers move. A judgement whose answer swings with the wording is not
//!    ready to gate anything.
//!
//! No language-model generation: every number is a probability, a score or a label.

use serde_json::{json, Map, Value};

use corral::judge::Conductor;

const SCORE_LEVELS: [&str; 3] = [
    "says nothing new",
    "a useful observation",
    "a sharp contribution that moves the thread",
];

const PHRASINGS: [(&str, &str); 3] = [
    (
        "move_thread",
        "Does this line move the discussion forward, rather than restating what was already said?",
    ),
    (
        "worth_reading",
        "Is this line worth reading on its own, for someone following the discussion?",
    ),
    (
        "restates",
        "Does this line merely restate or rephrase something already on the table?",
    ),
];

fn inverted(key: &str) -> bool {
    key == "restates"
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), String> {
    let key = corral::api_key()?;
    let client = Conductor::with_key(&key);
    let lines: Vec<(&str, &str)> = corral::corpus::LINES
        .iter()
        .map(|l| (l.speaker, l.text))
        .collect();
    let mut cost = 0.0;
    let mut calls = 0u64;

    // ---- 1. primitive comparison, one request
    let mut q: Map<String, Value> = Map::new();
    for (i, (_sp, tx)) in lines.iter().enumerate() {
        q.insert(
            format!("l{i}_noul"),
            json!({"type": "noul",
            "instructions": format!("{} The line: “{tx}”", PHRASINGS[0].1)}),
        );
        q.insert(format!("l{i}_score"), json!({"type": "score",
            "instructions": format!("How good a contribution is this line to the discussion? “{tx}”"),
            "criteria": SCORE_LEVELS}));
        let mut crit = Map::new();
        crit.insert(
            "strong".to_string(),
            json!("a genuinely strong line: sharp, specific and it moves the thread"),
        );
        crit.insert(
            "ordinary".to_string(),
            json!("an ordinary line: plausible but it says little that is new"),
        );
        q.insert(format!("l{i}_choice"), json!({"type": "choice",
            "instructions": format!("Is this a strong line or an ordinary one? “{tx}”"), "criteria": crit}));
    }
    let r1 = client
        .system_one(&json!({"seed": corral::corpus::SEED}), &q)
        .await?;
    cost += r1.cost;
    calls += 1;

    let mut by_prim: std::collections::BTreeMap<&str, Vec<f64>> = Default::default();
    for (i, (_sp, _tx)) in lines.iter().enumerate() {
        let v_n = r1
            .answers
            .get(&format!("l{i}_noul"))
            .map(|a| a.noul())
            .unwrap_or(0.5);
        let v_s = r1
            .answers
            .get(&format!("l{i}_score"))
            .map(|a| a.score_norm(SCORE_LEVELS.len()))
            .unwrap_or(0.5);
        let v_c = r1
            .answers
            .get(&format!("l{i}_choice"))
            .map(|a| a.probability("strong"))
            .unwrap_or(0.0);
        by_prim.entry("noul").or_default().push(v_n);
        by_prim.entry("score").or_default().push(v_s);
        by_prim.entry("choice").or_default().push(v_c);
    }
    let mut prim_stats: Map<String, Value> = Map::new();
    for (k, v) in &by_prim {
        let (mut top, mut bot) = (f64::NEG_INFINITY, f64::INFINITY);
        let mut top_i = 0;
        let mut bot_i = 0;
        for (i, x) in v.iter().enumerate() {
            if *x > top {
                top = *x;
                top_i = i;
            }
            if *x < bot {
                bot = *x;
                bot_i = i;
            }
        }
        prim_stats.insert(
            k.to_string(),
            json!({"mean": round3(corral::experiments::mean(v)), "spread": round3(corral::experiments::pstdev(v)),
                   "range": round3(top - bot),
                   "top": lines[top_i].1.chars().take(46).collect::<String>(),
                   "bottom": lines[bot_i].1.chars().take(46).collect::<String>()}),
        );
    }

    // ---- 2. phrasing stability, one request
    let mut q2: Map<String, Value> = Map::new();
    for (i, (_sp, tx)) in lines.iter().enumerate() {
        for (key_, instr) in PHRASINGS {
            q2.insert(
                format!("l{i}_{key_}"),
                json!({"type": "noul", "instructions": format!("{instr} The line: “{tx}”")}),
            );
        }
    }
    let r2 = client
        .system_one(&json!({"seed": corral::corpus::SEED}), &q2)
        .await?;
    cost += r2.cost;
    calls += 1;

    let mut swings = Vec::new();
    for (i, (_sp, tx)) in lines.iter().enumerate() {
        let mut vals: Vec<(&str, f64)> = Vec::new();
        for (key_, _instr) in PHRASINGS {
            let v = r2
                .answers
                .get(&format!("l{i}_{key_}"))
                .map(|a| a.noul())
                .unwrap_or(0.5);
            vals.push((key_, if inverted(key_) { 1.0 - v } else { v }));
        }
        let mn = vals.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
        let mx = vals
            .iter()
            .map(|(_, v)| *v)
            .fold(f64::NEG_INFINITY, f64::max);
        swings.push((
            round3(mx - mn),
            tx.chars().take(44).collect::<String>(),
            format!("{vals:?}"),
        ));
    }
    let mean_swing = round3(corral::experiments::mean(
        &swings.iter().map(|s| s.0).collect::<Vec<_>>(),
    ));
    swings.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let worst: Vec<Value> = swings
        .iter()
        .take(5)
        .map(|(s, t, v)| json!({"max_swing": s, "text": t, "vals": v}))
        .collect();

    // ---- 3. let Jev judge its own instruments
    let r3 = client
        .system_one(
            &json!({"primitive_stats": serde_json::to_string(&prim_stats).unwrap_or_default(),
                    "phrasing": {"mean_max_swing": mean_swing, "worst": serde_json::to_string(&worst.iter().take(3).collect::<Vec<_>>()).unwrap_or_default()}}),
            &{
                let mut q = Map::new();
                let mut crit = Map::new();
                crit.insert("noul".to_string(), json!("a single probability of yes"));
                crit.insert("score".to_string(), json!("a position on three ordered levels"));
                crit.insert("choice".to_string(), json!("a pick between strong and ordinary"));
                q.insert("best_primitive".to_string(), json!({"type": "choice",
                    "instructions": "For judging how good a line is, which instrument gives the most usable answer?", "criteria": crit}));
                q.insert("phrasing_ok".to_string(), json!({"type": "noul",
                    "instructions": "Is the question about line quality answerable stably enough, given how far the answers moved when the wording changed?"}));
                q.insert("ready_to_gate".to_string(), json!({"type": "noul",
                    "instructions": "Is a judgement that swings this much with wording safe to use as a gate that decides what the app does?"}));
                q
            },
        )
        .await?;
    cost += r3.cost;
    calls += 1;

    let best_prim = r3
        .answers
        .get("best_primitive")
        .map(|a| a.choice())
        .unwrap_or_default();
    let probs = r3
        .answers
        .get("best_primitive")
        .map(|a| a.probabilities.clone())
        .unwrap_or_default();
    std::fs::create_dir_all("results").map_err(|e| e.to_string())?;
    let stamp = corral::experiments::stamp();
    let out = json!({"primitive_stats": prim_stats, "phrasing_mean_swing": mean_swing, "phrasing_worst": worst,
        "best_primitive": best_prim, "primitive_probabilities": probs,
        "phrasing_ok": round3(r3.answers.get("phrasing_ok").map(|a| a.noul()).unwrap_or(0.5)),
        "ready_to_gate": round3(r3.answers.get("ready_to_gate").map(|a| a.noul()).unwrap_or(0.5))});
    std::fs::write(
        format!("results/method-{stamp}.json"),
        serde_json::to_string_pretty(&out).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;

    println!("PRIMITIVE: three ways to ask 'is this a good line?'\n");
    println!(
        "  {:10} {:>6} {:>7} {:>6}   best line read",
        "instrument", "mean", "spread", "range"
    );
    for (k, v) in &prim_stats {
        println!(
            "  {:10} {:6.2} {:7.3} {:6.2}   {}",
            k,
            v["mean"].as_f64().unwrap_or(0.0),
            v["spread"].as_f64().unwrap_or(0.0),
            v["range"].as_f64().unwrap_or(0.0),
            v["top"].as_str().unwrap_or("")
        );
    }
    println!("\n  Jev picks: {best_prim}");
    println!("\nPHRASING: the same judgement, three wordings\n");
    println!("  mean swing per line: {mean_swing}");
    for (s, t, v) in swings.iter().take(5) {
        println!("    swing {s:.2}  {v}  {t}");
    }
    println!(
        "\n  wording is stable enough to trust:  {:.2}",
        r3.answers
            .get("phrasing_ok")
            .map(|a| a.noul())
            .unwrap_or(0.5)
    );
    println!(
        "  safe to use as a gate:              {:.2}",
        r3.answers
            .get("ready_to_gate")
            .map(|a| a.noul())
            .unwrap_or(0.5)
    );
    println!("\n{calls} Jev calls · ${cost:.4} · method-{stamp}.json");
    Ok(())
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
