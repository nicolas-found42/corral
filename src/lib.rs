//! The Corral: five tables of four cheap 2024 Meta Llama 3.1 8B instances, talking
//! at once about one seed, refereed live by TypeSafe Jev -- and occasionally
//! overhearing each other.
//!
//! A Rust port of the original Python app, with the same behaviour. Jev is called
//! directly over OpenRouter's System One endpoint
//! (`POST https://openrouter.ai/api/v1/systemone`, model `jev-1.13`); the llama
//! calls go to OpenRouter chat completions (`meta-llama/llama-3.1-8b-instruct`).

pub mod corpus;
pub mod experiments;
pub mod herd;
pub mod judge;
pub mod personas;
pub mod rng;
pub mod room;
pub mod session;
pub mod tui;

/// The OpenRouter key, read from the environment (it lives in `~/.zshenv`).
/// Never printed, never logged.
pub fn api_key() -> Result<String, String> {
    match std::env::var("OPENROUTER_API_KEY") {
        Ok(k) if !k.trim().is_empty() => Ok(k),
        _ => Err("OPENROUTER_API_KEY is not set".to_string()),
    }
}

/// A `YYYYMMDD-HHMMSS` stamp, from the wall clock (UTC), matching the Python
/// `time.strftime("%Y%m%d-%H%M%S")` shape used for `results/*.json` names.
pub fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = now % 86_400;
    let (y, m, d) = civil_from_days((now / 86_400) as i64);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Howard Hinnant's days-from-civil, inverted (days since the epoch -> y/m/d).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
