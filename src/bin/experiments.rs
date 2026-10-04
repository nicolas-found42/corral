//! Jev-only experiments over The Corral's real transcript corpus.
//!
//!     cargo run --bin experiments                       # all, Jev only, no llama calls
//!     cargo run --bin experiments -- leak-absorption voice-distinctness
//!
//! Every number printed is a probability, a label or a majority count the judgement
//! model returned -- no language-model generation happens anywhere.

use std::sync::atomic::AtomicU64;

#[tokio::main]
async fn main() {
    let key = match corral::api_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let want: Vec<String> = std::env::args().skip(1).collect();
    let want = if want.is_empty() {
        corral::experiments::NAMES
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        want
    };
    if let Err(e) = corral::experiments::run(&key, want, &AtomicU64::new(0)).await {
        eprintln!("{e}");
        std::process::exit(2);
    }
}
