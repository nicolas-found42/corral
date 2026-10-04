//! The meta-experiment: Jev judging its own experiments.
//!
//!     cargo run --bin experiment_review
//!
//! Reads the newest `results/experiments-*.json` and asks Jev, per experiment:
//!   * did it produce a signal (a result that discriminates), versus mush?
//!   * is its conclusion trustworthy given the size of the corpus?
//!   * does it say something a reader would not already assume?
//!   * is it worth running again on a bigger corpus?
//!
//! And then, in a second request, which experiments to KEEP, which to DROP, and what
//! to build next -- as Choices with probabilities, plus an escape hatch.
//!
//! Still Jev-only: no language-model generation anywhere.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use corral::judge::Conductor;

const NEXT_CANDIDATES: [(&str, &str); 8] = [
    (
        "bigger_corpus",
        "collect a much larger transcript corpus first, then re-run every experiment",
    ),
    (
        "ablate_leak",
        "run the app with eavesdrops switched off, then compare the two corpora",
    ),
    (
        "perturb_seed",
        "run the same seed five times and measure how much the conversations differ run to run",
    ),
    (
        "score_calibration",
        "measure whether one table's own quality scores predict a reader's ranking",
    ),
    (
        "cross_table_agreement",
        "ask whether the five tables end up agreeing with each other's conclusions",
    ),
    (
        "agent_reliability",
        "measure how often a table produces a line that contradicts its own earlier line",
    ),
    (
        "question_phrasing",
        "test several differently-phrased versions of each question for answer stability",
    ),
    (
        "primitive_choice",
        "for the same judgement, compare Noul vs Score vs Choice and keep the sharpest",
    ),
];

fn newest(prefix: &str) -> Option<PathBuf> {
    let mut best: Option<(String, PathBuf)> = None;
    for entry in std::fs::read_dir("results").ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(prefix) && name.ends_with(".json") {
            let path = entry.path();
            if best.as_ref().map(|(n, _)| name > *n).unwrap_or(true) {
                best = Some((name, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// A compact, factual summary of one experiment's result, for Jev to judge.
fn summarise(r: &Value) -> String {
    let n = r.get("name").and_then(|x| x.as_str()).unwrap_or("?");
    match n {
        "leak-absorption" => format!(
            "{} of {} heard lines drew a reply judged to address them (three phrasings).",
            r["landed"], r["of"]
        ),
        "seed-fidelity" => format!(
            "mean seed-engagement {} over {} lines; per table {}",
            r["mean"],
            r["rows"].as_array().map(|a| a.len()).unwrap_or(0),
            r["by_table"]
        ),
        "cohesion-vs-quality" => format!(
            "{} adjacent pairs; correlation {:.3}; most-cohesive quartile quality {} vs least {}",
            r["pairs"].as_u64().unwrap_or(0),
            r["correlation"].as_f64().unwrap_or(0.0),
            r["quality_top_cohesion_quartile"],
            r["quality_bottom_cohesion_quartile"]
        ),
        "voice-distinctness" => format!(
            "{} of {} lines attributed to the right llama from text alone (accuracy {}; chance 0.25)",
            r["correct"], r["n"], r["accuracy"]
        ),
        "leak-gate" => {
            let parts: Vec<String> = r["sweep"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| {
                    format!(
                        "gate {:.2} admits {} of {} and keeps {}/3 logged leaks",
                        s["threshold"].as_f64().unwrap_or(0.0), s["lines_passing"], s["of"], s["real_leaks_kept"]
                    )
                })
                .collect();
            format!("threshold sweep: {}", parts.join("; "))
        }
        "unanswered" => format!("{} of {} questions judged still open.", r["open"], r["of"]),
        "redundancy" => format!("mean distinctness between the five tables {}", r["mean_distinctness"]),
        "best-line" => format!("per-table best lines: {}", r["per_table"]),
        _ => serde_json::to_string(r).unwrap_or_default().chars().take(400).collect(),
    }
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
    let path = match newest("experiments-") {
        Some(p) => p,
        None => return Err("no results yet — run `cargo run --bin experiments` first".to_string()),
    };
    let all: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("parse {}: {e}", path.display()))?;
    let data: Vec<Value> = all
        .into_iter()
        .filter(|r| r.get("error").is_none())
        .collect();

    let client = Conductor::with_key(&key);
    let mut cost = 0.0;
    let mut calls = 0u64;

    // 1) one question set per experiment
    let state = json!({
        "corpus": "36 real transcript lines from five concurrent four-agent AI conversations, all on one shared seed",
        "experiments": data.iter().map(|r| json!({"id": r["name"], "finding": summarise(r)})).collect::<Vec<_>>()
    });
    let mut q: Map<String, Value> = Map::new();
    for r in &data {
        let n = r["name"].as_str().unwrap();
        q.insert(format!("{n}_signal"), json!({"type": "noul", "instructions": format!("Experiment `{n}` produced the finding shown under its id. Does that finding DISCRIMINATE -- does it separate the lines or tables into clearly different cases -- or is it mush where everything lands near the middle?")}));
        q.insert(format!("{n}_mush"), json!({"type": "noul", "instructions": format!("Experiment `{n}` produced the finding shown under its id. Is that finding mush: everything scored nearly the same, so nothing is separated?")}));
        q.insert(format!("{n}_trust"), json!({"type": "noul", "instructions": format!("Experiment `{n}` produced the finding shown under its id, from only about 36 transcript lines. Is that finding solid enough to act on, or too small a sample to trust?")}));
        q.insert(format!("{n}_surprise"), json!({"type": "noul", "instructions": format!("Experiment `{n}` produced the finding shown under its id. Would a reader find that finding SURPRISING, or is it what anyone would already assume about a group chat?")}));
        q.insert(format!("{n}_rerun"), json!({"type": "noul", "instructions": format!("Experiment `{n}` produced the finding shown under its id. Is it worth running again on a much larger corpus?")}));
    }
    let r1 = client.system_one(&state, &q).await?;
    cost += r1.cost;
    calls += 1;

    // 2) which to keep, which to drop, and what to do next
    let mut table: Map<String, Value> = Map::new();
    for r in &data {
        let n = r["name"].as_str().unwrap();
        let get = |k: &str| {
            r1.answers
                .get(&format!("{n}_{k}"))
                .map(|a| a.noul())
                .unwrap_or(0.5)
        };
        let signal = get("signal").min(1.0 - get("mush"));
        let trust = get("trust");
        let surprise = get("surprise");
        let rerun = get("rerun");
        let composite = 0.4 * signal + 0.2 * trust + 0.2 * surprise + 0.2 * rerun;
        table.insert(
            n.to_string(),
            json!({"signal": round3(signal), "trust": round3(trust), "surprise": round3(surprise), "rerun": round3(rerun), "composite": round3(composite)}),
        );
    }

    let mut keep_crit = Map::new();
    let mut drop_crit = Map::new();
    for r in &data {
        let n = r["name"].as_str().unwrap();
        keep_crit.insert(n.to_string(), json!(format!("the {n} experiment")));
        drop_crit.insert(n.to_string(), json!(format!("the {n} experiment")));
    }
    keep_crit.insert("none".to_string(), json!("none of them are worth keeping"));
    drop_crit.insert("none".to_string(), json!("none should be dropped"));
    let mut next_crit = Map::new();
    for (k, v) in NEXT_CANDIDATES {
        next_crit.insert(k.to_string(), json!(v));
    }
    next_crit.insert("nothing".to_string(), json!("nothing further is needed"));
    let r2 = client
        .system_one(
            &json!({"experiments": serde_json::to_string(&table).unwrap_or_default(),
                    "options": NEXT_CANDIDATES.iter().map(|(k, v)| (*k, *v)).collect::<std::collections::BTreeMap<_, _>>()}),
            &{
                let mut q = Map::new();
                q.insert("keep".to_string(), json!({"type": "choice",
                    "instructions": "Which of these experiments should be KEPT and run as a standing check on the app?", "criteria": keep_crit}));
                q.insert("drop".to_string(), json!({"type": "choice",
                    "instructions": "Which one experiment should be DROPPED as not earning its cost?", "criteria": drop_crit}));
                q.insert("next".to_string(), json!({"type": "choice",
                    "instructions": "What should be built or measured next to improve these conversations?", "criteria": next_crit}));
                q
            },
        )
        .await?;
    cost += r2.cost;
    calls += 1;

    let keep = r2
        .answers
        .get("keep")
        .map(|a| a.choice())
        .unwrap_or_default();
    let drop = r2
        .answers
        .get("drop")
        .map(|a| a.choice())
        .unwrap_or_default();
    let nxt = r2
        .answers
        .get("next")
        .map(|a| a.choice())
        .unwrap_or_default();
    let keep_conf = r2
        .answers
        .get("keep")
        .map(|a| a.confidence())
        .unwrap_or(0.0);

    std::fs::create_dir_all("results").map_err(|e| e.to_string())?;
    let stamp = corral::experiments::stamp();
    let out = json!({"table": table, "keep": keep, "keep_confidence": round3(keep_conf), "drop": drop, "next": nxt,
        "next_probabilities": r2.answers.get("next").map(|a| a.probabilities.clone()).unwrap_or_default()});
    std::fs::write(
        format!("results/review-{stamp}.json"),
        serde_json::to_string_pretty(&out).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;

    println!("Jev's scorecard for its own experiments");
    println!(
        "{:22} {:>7} {:>6} {:>9} {:>6} {:>10}",
        "experiment", "signal", "trust", "surprise", "rerun", "composite"
    );
    let mut rows: Vec<(&String, &Value)> = table.iter().collect();
    rows.sort_by(|a, b| {
        b.1["composite"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a.1["composite"].as_f64().unwrap_or(0.0))
            .unwrap()
    });
    for (n, v) in rows {
        println!(
            "{:22} {:7.2} {:6.2} {:9.2} {:6.2} {:10.2}",
            n,
            v["signal"].as_f64().unwrap_or(0.0),
            v["trust"].as_f64().unwrap_or(0.0),
            v["surprise"].as_f64().unwrap_or(0.0),
            v["rerun"].as_f64().unwrap_or(0.0),
            v["composite"].as_f64().unwrap_or(0.0)
        );
    }
    println!("\nkeep: {keep} ({keep_conf:.2})");
    println!("drop: {drop}");
    println!("next: {nxt}");
    println!("\n{calls} Jev calls · ${cost:.4} · review-{stamp}.json");
    Ok(())
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
