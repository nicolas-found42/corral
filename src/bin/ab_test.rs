//! A/B: Jev picks which eavesdrop policy is better, judged on the corpus.
//!
//!     cargo run --bin ab_test
//!
//! The leak threshold is a policy the app owns (a constant in code). Two policies are
//! set against each other on the SAME corpus:
//!
//!   A  strict  (gate 0.50)  -- few, unmistakably striking lines carry
//!   B  broad   (gate 0.40)  -- more lines carry, including ordinary remarks
//!
//! Jev judges: which policy's admitted set serves the app's goal better, which leaks
//! from policy B are not worth carrying, and whether the stricter set is better
//! material for a hearing table. All Choices and Nouls; no generation.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use corral::judge::Conductor;

const A_STRICT: f64 = 0.50;
const B_BROAD: f64 = 0.40;

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

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), String> {
    let key = corral::api_key()?;
    let path = newest("experiments-").ok_or("run `cargo run --bin experiments` first")?;
    let data: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("parse {}: {e}", path.display()))?;
    let gate = data
        .iter()
        .find(|r| r.get("name").and_then(|n| n.as_str()) == Some("leak-gate"))
        .ok_or("the leak-gate experiment has not been run yet")?;
    let rows = gate["rows"].as_array().cloned().unwrap_or_default();

    let admitted = |thr: f64| -> Vec<Value> {
        rows.iter()
            .filter(|x| x["score"].as_f64().unwrap_or(0.0) >= thr)
            .cloned()
            .collect()
    };
    let a = admitted(A_STRICT);
    let b = admitted(B_BROAD);
    let marginal: Vec<Value> = b
        .iter()
        .filter(|x| x["score"].as_f64().unwrap_or(0.0) < A_STRICT)
        .cloned()
        .collect();

    let client = Conductor::with_key(&key);
    let mut cost = 0.0;
    let mut calls = 0u64;

    // 1) per-line: is a line admitted by the broad policy but not the strict one
    //    actually worth carrying?
    let mut q: Map<String, Value> = Map::new();
    for (i, x) in marginal.iter().enumerate() {
        q.insert(format!("m{i}_worth"), json!({"type": "noul",
            "instructions": format!("Line: “{}” (said at table {} in a room of five tables all on one seed). Would a DIFFERENT table, overhearing this, have something worth picking up?", x["text"].as_str().unwrap_or(""), x["table"])}));
        q.insert(format!("m{i}_ignore"), json!({"type": "noul",
            "instructions": format!("Line: “{}”. Is this an ordinary table-local remark that another table would simply ignore?", x["text"].as_str().unwrap_or(""))}));
    }
    let r1 = client
        .system_one(
            &json!({"seed": gate.get("seed").cloned().unwrap_or(json!("one shared seed"))}),
            &q,
        )
        .await?;
    cost += r1.cost;
    calls += 1;
    let marginal_scored: Vec<Value> = marginal
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let worth = r1.answers.get(&format!("m{i}_worth")).map(|a| a.noul()).unwrap_or(0.5);
            let ignore = r1.answers.get(&format!("m{i}_ignore")).map(|a| a.noul()).unwrap_or(0.5);
            json!({"text": x["text"], "score": x["score"], "carried": x["carried"], "worth": round3(worth.min(1.0 - ignore))})
        })
        .collect();

    // 2) which policy serves the app better
    let mut crit = Map::new();
    crit.insert(
        "A_strict".to_string(),
        json!("fewer lines carry, but every one of them is unmistakably striking"),
    );
    crit.insert(
        "B_broad".to_string(),
        json!("more lines carry, including ordinary remarks that sit just above a lower gate"),
    );
    let mut q2: Map<String, Value> = Map::new();
    q2.insert(
        "policy".to_string(),
        json!({"type": "choice",
        "instructions": "Which eavesdrop policy serves the stated goal better?", "criteria": crit}),
    );
    q2.insert("stricter_better".to_string(), json!({"type": "noul",
        "instructions": "Given the goal that a leak must matter and must not merely decorate, is the STRICTER policy the better one?"}));
    q2.insert("more_leaks_better".to_string(), json!({"type": "noul",
        "instructions": "Would more frequent eavesdrops make The Corral better to watch, even if some of them carry ordinary remarks?"}));
    let r2 = client
        .system_one(
            &json!({
                "goal": "Five tables of four AI llamas talk at once on one shared seed. Occasionally one table overhears a line from another and that line changes what the hearing table does next. A leak must be visible and must matter; a leak that decorates is a failure.",
                "A_strict": a.iter().map(|x| x["text"].clone()).collect::<Vec<_>>(),
                "B_broad": marginal.iter().map(|x| x["text"].clone()).collect::<Vec<_>>(),
                "marginal_worth": marginal_scored,
            }),
            &q2,
        )
        .await?;
    cost += r2.cost;
    calls += 1;

    std::fs::create_dir_all("results").map_err(|e| e.to_string())?;
    let pol = r2
        .answers
        .get("policy")
        .map(|a| a.choice())
        .unwrap_or_default();
    let out = json!({
        "policies": {"A_strict": A_STRICT, "B_broad": B_BROAD},
        "admitted": {"A_strict": a.iter().map(|x| x["text"].clone()).collect::<Vec<_>>(),
                     "B_broad": b.iter().map(|x| x["text"].clone()).collect::<Vec<_>>()},
        "marginal": marginal_scored,
        "policy": pol,
        "probabilities": r2.answers.get("policy").map(|p| p.probabilities.clone()).unwrap_or_default(),
        "stricter_better": round3(r2.answers.get("stricter_better").map(|a| a.noul()).unwrap_or(0.5)),
        "more_leaks_better": round3(r2.answers.get("more_leaks_better").map(|a| a.noul()).unwrap_or(0.5)),
    });
    let stamp = corral::experiments::stamp();
    std::fs::write(
        format!("results/ab-{stamp}.json"),
        serde_json::to_string_pretty(&out).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;

    println!("A/B on the eavesdrop gate, judged by Jev\n");
    println!(
        "  policy A (gate {A_STRICT}) admits {} of {} lines",
        a.len(),
        rows.len()
    );
    println!(
        "  policy B (gate {B_BROAD}) admits {} of {} lines",
        b.len(),
        rows.len()
    );
    println!(
        "  the {} lines B adds that A rejects:",
        marginal_scored.len()
    );
    for x in &marginal_scored {
        println!(
            "    worth {:.2}  (gate score {:.2})  {}",
            x["worth"].as_f64().unwrap_or(0.0),
            x["score"].as_f64().unwrap_or(0.0),
            x["text"].as_str().unwrap_or("")
        );
    }
    println!(
        "\n  Jev picks: {pol}  A={:.2} B={:.2}",
        r2.answers
            .get("policy")
            .and_then(|p| p.probabilities.get("A_strict"))
            .copied()
            .unwrap_or(0.0),
        r2.answers
            .get("policy")
            .and_then(|p| p.probabilities.get("B_broad"))
            .copied()
            .unwrap_or(0.0)
    );
    println!(
        "  stricter policy is better:        {:.2}",
        r2.answers
            .get("stricter_better")
            .map(|a| a.noul())
            .unwrap_or(0.5)
    );
    println!(
        "  more frequent leaks would help:   {:.2}",
        r2.answers
            .get("more_leaks_better")
            .map(|a| a.noul())
            .unwrap_or(0.5)
    );
    println!("\n{calls} Jev calls · ${cost:.4} · ab-{stamp}.json");
    Ok(())
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
