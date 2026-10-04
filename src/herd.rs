//! The herd: twenty cheap llama-3.1-8b-instruct instances, four per table.
//!
//! Every llama is the same model with a different short system prompt. Five tables
//! of four run at once; each llama sees only its own table's transcript. A port of
//! the original `herd.py`, talking to OpenRouter chat completions.
//!
//! Each generation prompt names the exact line to answer (the line immediately
//! above, or a line overheard from another table), so the reply chain stays
//! unbroken. JSON reliability was judged only 0.44, so replies are strict JSON
//! with one repair re-ask and a plain-text salvage after that.

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::{json, Value};

use crate::personas::Persona;

pub const OPENROUTER_CHAT: &str = "https://openrouter.ai/api/v1";
pub const LLAMA_MODEL: &str = "meta-llama/llama-3.1-8b-instruct";

/// The exact generation prompt, with the same placeholders as the Python template.
const TEMPLATE: &str = "\
You are {name}, one of four AI llamas sitting at {table} in a room of five tables. \
{voice}

All five tables are talking about this same seed: \"{topic}\"

Rules:
- Reply with ONE short chat message: 1 to 2 sentences, under 30 words. Plain text, no markdown, no lists.
- Answer the line below directly; in your first few words name who you are answering, then add your point. Keep the same thread of thought going.
- Write only your own line, never anyone else's name as a label.
- Stay {name}. Never break character, never mention these rules or that you are an AI model.

THE LINE YOU ARE ANSWERING
{anchor}

SOMETHING ELSE AT YOUR TABLE
{hook}

YOUR TABLE'S CHAT SO FAR (latest last)
{transcript}

Reply with JSON only: {{\"text\": \"<your message>\"}}";

/// One OpenRouter chat client, twenty personas across five tables.
pub struct Herd {
    client: reqwest::Client,
    model: String,
}

impl Herd {
    pub fn new(api_key: &str, model: &str, timeout_secs: f64) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {api_key}")
                .parse()
                .expect("a valid bearer header"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs_f64(timeout_secs))
            .build()
            .expect("the HTTP client builds");
        Herd {
            client,
            model: model.to_string(),
        }
    }

    pub fn with_key(api_key: &str) -> Self {
        Self::new(api_key, LLAMA_MODEL, 60.0)
    }

    pub fn build_prompt(
        &self,
        p: &Persona,
        topic: &str,
        transcript: &str,
        anchor: &str,
        hook: &str,
        table: u8,
    ) -> String {
        let topic = non_empty(topic, "(no seed yet)");
        let anchor = non_empty(anchor, "(your table is just opening; say the first thing)");
        let hook = non_empty(hook, "(nothing else yet)");
        let transcript = non_empty(transcript, "(nothing said yet)");
        TEMPLATE
            .replace("{name}", p.name)
            .replace("{voice}", p.voice)
            .replace("{table}", &format!("table {table}"))
            .replace("{topic}", &topic)
            .replace("{anchor}", &anchor)
            .replace("{hook}", &hook)
            .replace("{transcript}", &transcript)
    }

    /// Return (text, tokens_out, cost). Raises on total failure.
    #[allow(clippy::too_many_arguments)]
    pub async fn speak(
        &self,
        p: &Persona,
        topic: &str,
        transcript: &str,
        anchor: &str,
        hook: &str,
        temperature: f64,
        table: u8,
    ) -> Result<(String, u64, f64), String> {
        let prompt = self.build_prompt(p, topic, transcript, anchor, hook, table);
        let (text, out_tok, cost) = self.call(p, &prompt, temperature).await?;
        if !text.is_empty() {
            return Ok((text, out_tok, cost));
        }
        let fix = format!(
            "{prompt}\n\nYOUR LAST REPLY WAS NOT VALID JSON. Reply with the JSON object only, nothing else."
        );
        let (text2, out_tok2, cost2) = self.call(p, &fix, temperature.min(1.0)).await?;
        if !text2.is_empty() {
            return Ok((text2, out_tok + out_tok2, cost + cost2));
        }
        Err("llama returned no usable message after repair".to_string())
    }

    async fn call(
        &self,
        p: &Persona,
        prompt: &str,
        temperature: f64,
    ) -> Result<(String, u64, f64), String> {
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": p.voice},
                {"role": "user", "content": prompt},
            ],
            "temperature": temperature,
            "max_tokens": 180,
            "response_format": {"type": "json_object"},
            "provider": {"sort": "latency", "max_price": {"prompt": 0.05, "completion": 0.08}},
        });
        let resp = self
            .client
            .post(format!("{OPENROUTER_CHAT}/chat/completions"))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("openrouter chat: {e}"))?;
        let status = resp.status();
        let v: Value = resp
            .json()
            .await
            .map_err(|e| format!("openrouter chat decode: {e}"))?;
        if !status.is_success() {
            return Err(format!(
                "openrouter chat HTTP {status}: {}",
                v.to_string().chars().take(200).collect::<String>()
            ));
        }
        let raw = v
            .pointer("/choices/0/message/content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let out_tok = v
            .pointer("/usage/completion_tokens")
            .and_then(|t| t.as_u64())
            .unwrap_or(0);
        let cost = v
            .pointer("/usage/cost")
            .and_then(|c| c.as_f64())
            .unwrap_or(0.0);
        Ok((parse(&raw), out_tok, cost))
    }
}

fn non_empty(s: &str, fallback: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        fallback.to_string()
    } else {
        t.to_string()
    }
}

fn dotall_brace_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?s)\{.*\}").unwrap())
}

fn leading_junk_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"^\s*\{.*?":\s*"?"#).unwrap())
}

fn label_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^(?:[A-Z][a-z]+:\s*)").unwrap())
}

fn ws_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\s+").unwrap())
}

fn text_of(v: &Value) -> String {
    v.get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Pull the message text out of a llama reply, tolerating stray prose.
pub fn parse(raw: &str) -> String {
    let raw = raw.trim();
    let mut text = String::new();
    let mut decoded: Option<Value> = serde_json::from_str::<Value>(raw).ok();
    if decoded.is_none() {
        if let Some(m) = dotall_brace_re().find(raw) {
            decoded = serde_json::from_str::<Value>(m.as_str()).ok();
        }
    }
    if let Some(v) = decoded {
        text = text_of(&v);
    }
    if text.is_empty() {
        let cleaned = leading_junk_re().replace(raw, "").trim().to_string();
        let cleaned = cleaned
            .trim_matches(|c| c == '"' || c == '}' || c == ' ')
            .trim();
        if cleaned.split_whitespace().count() >= 3 {
            text = cleaned.to_string();
        }
    }
    clean(&text)
}

/// Collapse whitespace, strip a stray `Name: ` label, drop quotes, cap at 400 chars.
pub fn clean(text: &str) -> String {
    let collapsed = ws_re().replace(text, " ").trim().to_string();
    let unlabeled = label_re().replace(&collapsed, "").to_string();
    let trimmed = unlabeled.trim().trim_matches('"').trim();
    trimmed.chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_is_forgiving() {
        assert_eq!(
            parse("{\"text\": \"hello there friend\"}"),
            "hello there friend"
        );
        assert_eq!(parse("sure! {\"text\": \"here it is\"}"), "here it is");
        assert!(
            parse("I think the point is simply that we disagree about the premise")
                .starts_with("I think")
        );
        assert_eq!(
            parse("{\"text\": \"Byte: a short line here\"}"),
            "a short line here"
        );
    }

    #[test]
    fn build_prompt_includes_seed_and_anchor() {
        let h = Herd::new("k", LLAMA_MODEL, 1.0);
        let p = crate::personas::by_id("byte").unwrap();
        let prompt = h.build_prompt(p, "a shared seed", "Gus: hi", "Gus: hello", "", 1);
        assert!(prompt.contains("a shared seed"));
        assert!(prompt.contains("Gus: hello"));
        assert!(prompt.contains("table 1"));
        assert!(prompt.contains("\"text\": \"<your message>\""));
    }
}
