//! The Corral's state: five tables of four llamas, each a continuous conversation,
//! plus the eavesdrops between them.
//!
//! Pure data and derived metrics, no network. The TUI renders this; the session
//! writes it. A port of the original `room.py`.
//!
//! Design, all from Jev judgements:
//!   * five tables of four (twenty agents in one conversation is too many to follow);
//!   * every reply answers the line immediately above (cohesion, Jev 0.94);
//!   * a one-way eavesdrop is a *judged event*: a striking line lands in another
//!     table's transcript, marked, and its next reply answers it;
//!   * each table's one-sentence summary is the single most representative line,
//!     picked by Jev -- no prose generation.

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::personas::{by_id, Persona, LLAMAS};

pub const GAUGES: [&str; 4] = ["heat", "consensus", "drift", "novelty"];
pub const MOVES: [&str; 7] = [
    "claim",
    "evidence",
    "question",
    "rebuttal",
    "analogy",
    "concession",
    "tangent",
];
pub const STANCES: [&str; 4] = ["support", "challenge", "build", "deflect"];
pub const QUALITY_DIMS: [&str; 3] = ["originality", "clarity", "insight"];

/// Composite scoring weights, kept in code.
pub const QUALITY_WEIGHTS: [(&str, f64); 3] =
    [("originality", 0.4), ("clarity", 0.2), ("insight", 0.4)];

/// Composite scoring weights, kept in code.
pub fn quality_weights() -> [(&'static str, f64); 3] {
    QUALITY_WEIGHTS
}

pub const PROPOSITIONS: [&str; 3] = [
    "this table now agrees on a single answer to the seed",
    "this table has become more divided over the last few messages",
    "this table's latest thinking would be new to someone who read only the seed",
];

/// The discourse-move palette.
pub fn move_colour(mv: &str) -> &'static str {
    match mv {
        "claim" => "#ffb454",
        "evidence" => "#6fb7e0",
        "question" => "#c98cff",
        "rebuttal" => "#ff7a90",
        "analogy" => "#e8c07a",
        "concession" => "#8fd694",
        "tangent" => "#7a7f9a",
        _ => "#8a86ad",
    }
}

/// Which four llamas sit at each table, hand-spread across voice families so no
/// table is four of a kind.
pub const TABLE_SETS: [(u8, [&str; 4]); 5] = [
    (1, ["byte", "gus", "juniper", "kestrel"]),
    (2, ["dexter", "nimbus", "cosmo", "orson"]),
    (3, ["vera", "marisol", "onyx", "paloma"]),
    (4, ["bram", "sable", "fern", "tilly"]),
    (5, ["quill", "pip", "wren", "ada"]),
];

pub const TABLE_COLOURS: [&str; 5] = ["#6fb7e0", "#e8c07a", "#c98cff", "#8fd694", "#ff9a6b"];

/// Seconds since the epoch, as Python's `time.time()`.
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn figure_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\d+(?:[.,]\d+)?\s?(?:%|percent|x|times|million|billion|thousand)?")
            .unwrap()
    })
}

fn word_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[a-z0-9']+").unwrap())
}

/// Candidate figures for Jev to choose among (regex proposes, Jev selects).
pub fn figures_in(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for m in figure_re().find_iter(text) {
        let s = m.as_str().trim();
        let low = s.to_lowercase();
        if !s.is_empty() && !seen.contains(&low) {
            seen.push(low);
            out.push(s.to_string());
        }
        if out.len() >= 8 {
            break;
        }
    }
    out.truncate(8);
    out
}

/// The word bag used by the duplicate guard.
pub fn words(text: &str) -> Vec<String> {
    word_re()
        .find_iter(&text.to_lowercase())
        .map(|m| m.as_str().to_string())
        .collect()
}

pub fn jaccard(
    a: &std::collections::BTreeSet<String>,
    b: &std::collections::BTreeSet<String>,
) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    inter / union
}

#[derive(Debug, Clone)]
pub struct Message {
    pub turn: u32,
    /// Persona id of the source speaker, even for a heard line.
    pub llama: String,
    pub text: String,
    pub ts: f64,
    /// If set, this line was overheard from table N.
    pub heard_from: Option<u8>,
    pub family: String,
    /// List index within its table.
    pub reply_to: Option<usize>,
    pub move_: String,
    pub quality: BTreeMap<String, f64>,
    pub composite: f64,
    pub figure: String,
    pub asserts_claim: bool,
    pub stance: String,
    pub cohesion: f64,
}

impl Message {
    pub fn new(turn: u32, llama: &str, text: &str) -> Self {
        Message {
            turn,
            llama: llama.to_string(),
            text: text.to_string(),
            ts: now(),
            heard_from: None,
            family: String::new(),
            reply_to: None,
            move_: String::new(),
            quality: BTreeMap::new(),
            composite: 0.0,
            figure: String::new(),
            asserts_claim: false,
            stance: String::new(),
            cohesion: 0.0,
        }
    }

    pub fn persona(&self) -> &'static Persona {
        by_id(&self.llama).expect("every message names a seated llama")
    }

    pub fn heard(&self) -> bool {
        self.heard_from.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct LlamaState {
    pub id: String,
    pub spoken: u32,
    pub eagerness: f64,
    pub last_spoke_turn: i32,
    pub speaking: bool,
}

impl LlamaState {
    pub fn new(id: &str) -> Self {
        LlamaState {
            id: id.to_string(),
            spoken: 0,
            eagerness: 0.0,
            last_spoke_turn: -1,
            speaking: false,
        }
    }

    pub fn persona(&self) -> &'static Persona {
        by_id(&self.id).expect("a llama state names a seated llama")
    }
}

/// One turn's judgement for a table.
#[derive(Debug, Clone, Default)]
pub struct Judgement {
    pub chosen: String,
    pub fit: f64,
    pub fit_map: BTreeMap<String, f64>,
    pub pick_confidence: f64,
    pub reply_to: Option<usize>,
    pub reason: String,
    pub family: String,
    pub move_: String,
    pub stance: String,
    pub cohesion: f64,
    pub quality: BTreeMap<String, f64>,
    pub composite: f64,
    pub figure: String,
    pub asserts_claim: bool,
    pub gauges: BTreeMap<String, f64>,
    pub thread: String,
    pub tactic: String,
    pub tactic_confidence: f64,
    /// The representative line Jev picked for the overview.
    pub summary: String,
    pub summary_from: String,
    pub beliefs: BTreeMap<String, f64>,
    pub verdict: String,
    pub calls_last: u32,
}

/// A one-way eavesdrop: table `src` was overheard by table `dst`.
#[derive(Debug, Clone)]
pub struct Leak {
    pub round: u32,
    pub src: u8,
    pub dst: u8,
    pub line: String,
    /// Persona id of whoever said the overheard line.
    pub speaker: String,
    /// Jev's label for why it carried.
    pub why: String,
    /// Jev's label for how it should land.
    pub effect: String,
    /// Has the hearing table replied to it yet.
    pub landed: bool,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub id: u8,
    /// Four llama ids, fixed.
    pub members: [&'static str; 4],
    pub colour: String,
    pub messages: Vec<Message>,
    pub gauges: BTreeMap<String, f64>,
    pub judgement: Judgement,
    pub thread: String,
    pub threads: Vec<String>,
    pub summary: String,
    pub summary_from: String,
    pub history: BTreeMap<String, Vec<f64>>,
    pub belief_history: BTreeMap<String, Vec<f64>>,
    /// How many eavesdrops this table has heard.
    pub leaks_in: u32,
}

impl Table {
    pub fn new(id: u8, members: [&'static str; 4], colour: &str) -> Self {
        let gauges = GAUGES.iter().map(|g| (g.to_string(), 0.5)).collect();
        let history = GAUGES.iter().map(|g| (g.to_string(), Vec::new())).collect();
        let belief_history = PROPOSITIONS
            .iter()
            .map(|p| (p.to_string(), Vec::new()))
            .collect();
        Table {
            id,
            members,
            colour: colour.to_string(),
            messages: Vec::new(),
            gauges,
            judgement: Judgement::default(),
            thread: String::new(),
            threads: Vec::new(),
            summary: String::new(),
            summary_from: String::new(),
            history,
            belief_history,
            leaks_in: 0,
        }
    }

    pub fn name(&self) -> String {
        format!("Table {}", self.id)
    }

    pub fn member_states<'a>(&self, room: &'a Room) -> Vec<&'a LlamaState> {
        self.members
            .iter()
            .filter_map(|m| room.llamas.get(*m))
            .collect()
    }

    /// The last `n` lines, as the transcript a speaker sees.
    pub fn tail_text(&self, _room: &Room, n: usize) -> String {
        let start = self.messages.len().saturating_sub(n);
        self.messages[start..]
            .iter()
            .map(|m| {
                if let Some(h) = m.heard_from {
                    format!("(heard from table {}) {}: {}", h, m.persona().name, m.text)
                } else {
                    format!("{}: {}", m.persona().name, m.text)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The thread-choice catalogue, in a stable order.
    pub fn thread_choices(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> =
            vec![("seed".to_string(), "the shared seed itself".to_string())];
        let start = self.threads.len().saturating_sub(4);
        for (i, t) in self.threads[start..].iter().enumerate() {
            out.push((format!("th{i}"), t.clone()));
        }
        out.push((
            "new".to_string(),
            "a genuinely new sub-question this table has not asked yet".to_string(),
        ));
        out
    }

    pub fn set_thread(&mut self, thread: &str) {
        if thread.is_empty() {
            return;
        }
        if thread.starts_with("seed") {
            self.thread = String::new();
            return;
        }
        if let Some(rest) = thread.strip_prefix("th") {
            if let Ok(i) = rest.parse::<usize>() {
                if i < self.threads.len() {
                    self.thread = self.threads[i].clone();
                }
                return;
            }
        }
        if let Some(last) = self.messages.last() {
            let sub = last.text.trim();
            let sub = if sub.chars().count() > 72 {
                let t: String = sub.chars().take(72).collect();
                format!("{t}…")
            } else {
                sub.to_string()
            };
            let t = if sub.is_empty() {
                String::new()
            } else {
                let stripped = sub.trim_end_matches('.').to_lowercase();
                format!("whether {stripped}")
            };
            if !t.is_empty() {
                self.thread = t.clone();
                self.threads.push(t);
                if self.threads.len() > 4 {
                    let drain = self.threads.len() - 4;
                    self.threads.drain(0..drain);
                }
            }
        }
    }

    pub fn note_gauges(&mut self) {
        for g in GAUGES {
            let v = ((self.gauges.get(g).copied().unwrap_or(0.5) * 1000.0).round()) / 1000.0;
            let h = self.history.entry(g.to_string()).or_default();
            h.push(v);
            if h.len() > 120 {
                let drain = h.len() - 120;
                h.drain(0..drain);
            }
        }
    }

    pub fn note_beliefs(&mut self) {
        for p in PROPOSITIONS {
            let v =
                ((self.judgement.beliefs.get(p).copied().unwrap_or(0.5) * 1000.0).round()) / 1000.0;
            let h = self.belief_history.entry(p.to_string()).or_default();
            h.push(v);
            if h.len() > 120 {
                let drain = h.len() - 120;
                h.drain(0..drain);
            }
        }
    }

    pub fn is_near_duplicate(&self, text: &str) -> bool {
        self.is_near_duplicate_with(text, 10, 0.62)
    }

    pub fn is_near_duplicate_with(&self, text: &str, lookback: usize, threshold: f64) -> bool {
        let a: std::collections::BTreeSet<String> = words(text).into_iter().collect();
        if a.len() < 4 {
            return false;
        }
        let start = self.messages.len().saturating_sub(lookback);
        self.messages[start..].iter().any(|m| {
            let b: std::collections::BTreeSet<String> = words(&m.text).into_iter().collect();
            jaccard(&a, &b) >= threshold
        })
    }

    pub fn roster<'a>(&self, room: &'a Room) -> Vec<&'a LlamaState> {
        let mut states: Vec<&LlamaState> = self.member_states(room);
        states.sort_by(|a, b| {
            b.eagerness
                .partial_cmp(&a.eagerness)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.last_spoke_turn.cmp(&b.last_spoke_turn))
        });
        states
    }
}

/// The whole app: five tables, a shared seed, and the eavesdrops between them.
#[derive(Debug, Clone)]
pub struct Room {
    pub seed: String,
    pub tables: Vec<Table>,
    pub llamas: BTreeMap<String, LlamaState>,
    pub leaks: Vec<Leak>,
    pub turn: u32,
    pub status: String,
    pub phase: String,
    /// Which table is in detail; -1 = the overview.
    pub focus: i32,
    pub outcome: String,
    pub started: Option<f64>,
    pub finished: Option<f64>,
    pub log: Vec<(String, String)>,
    pub cost_jev: f64,
    pub cost_llama: f64,
    pub tok_in: u64,
    pub tok_out: u64,
    pub calls_jev: u64,
    pub calls_llama: u64,
    pub last_error: String,
}

impl Room {
    pub fn new(seed: &str) -> Self {
        let llamas = LLAMAS
            .iter()
            .map(|p| (p.id.to_string(), LlamaState::new(p.id)))
            .collect();
        let tables = TABLE_SETS
            .iter()
            .map(|(id, members)| {
                Table::new(
                    *id,
                    *members,
                    TABLE_COLOURS[((*id as usize) - 1) % TABLE_COLOURS.len()],
                )
            })
            .collect();
        let mut room = Room {
            seed: seed.to_string(),
            tables,
            llamas,
            leaks: Vec::new(),
            turn: 0,
            status: "idle".to_string(),
            phase: String::new(),
            focus: -1,
            outcome: String::new(),
            started: None,
            finished: None,
            log: Vec::new(),
            cost_jev: 0.0,
            cost_llama: 0.0,
            tok_in: 0,
            tok_out: 0,
            calls_jev: 0,
            calls_llama: 0,
            last_error: String::new(),
        };
        room.log_line("five tables seated", "dim");
        room
    }

    pub fn elapsed(&self) -> f64 {
        match self.started {
            None => 0.0,
            Some(s) => self.finished.unwrap_or_else(now) - s,
        }
    }

    pub fn cost(&self) -> f64 {
        self.cost_jev + self.cost_llama
    }

    pub fn tokens(&self) -> u64 {
        self.tok_in + self.tok_out
    }

    pub fn messages(&self) -> usize {
        self.tables.iter().map(|t| t.messages.len()).sum()
    }

    pub fn table(&self, tid: u8) -> Option<&Table> {
        self.tables.iter().find(|t| t.id == tid)
    }

    pub fn table_mut(&mut self, tid: u8) -> Option<&mut Table> {
        self.tables.iter_mut().find(|t| t.id == tid)
    }

    pub fn log_line(&mut self, text: &str, style: &str) {
        self.log.push((text.to_string(), style.to_string()));
        if self.log.len() > 300 {
            let drain = self.log.len() - 300;
            self.log.drain(0..drain);
        }
    }

    pub fn set_status(&mut self, status: &str, phase: &str, outcome: &str) {
        self.status = status.to_string();
        if !phase.is_empty() {
            self.phase = phase.to_string();
        }
        if !outcome.is_empty() {
            self.outcome = outcome.to_string();
        }
        if matches!(status, "stopped" | "done" | "error") && self.finished.is_none() {
            self.finished = Some(now());
        }
    }

    pub fn table_of(&self, llama: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.members.contains(&llama))
    }

    /// A fresh run on a new seed: the tables and llamas stay, the talk is wiped.
    pub fn reset(&mut self, seed: &str) {
        self.seed = seed.to_string();
        for t in &mut self.tables {
            t.messages.clear();
            t.gauges = GAUGES.iter().map(|g| (g.to_string(), 0.5)).collect();
            t.judgement = Judgement::default();
            t.thread = String::new();
            t.threads = Vec::new();
            t.summary = String::new();
            t.summary_from = String::new();
            t.leaks_in = 0;
            t.history = GAUGES.iter().map(|g| (g.to_string(), Vec::new())).collect();
            t.belief_history = PROPOSITIONS
                .iter()
                .map(|p| (p.to_string(), Vec::new()))
                .collect();
        }
        for s in self.llamas.values_mut() {
            s.spoken = 0;
            s.eagerness = 0.0;
            s.last_spoke_turn = -1;
            s.speaking = false;
        }
        self.leaks.clear();
        self.turn = 0;
        self.cost_jev = 0.0;
        self.cost_llama = 0.0;
        self.tok_in = 0;
        self.tok_out = 0;
        self.calls_jev = 0;
        self.calls_llama = 0;
        self.last_error = String::new();
        self.finished = None;
        self.set_status("discussing", "a fresh seed", "");
        let shown: String = seed.chars().take(80).collect();
        self.log_line(&format!("new seed: \"{}\"", shown), "#ffd06b");
    }
}

/// A fresh room, seated and ready.
pub fn empty_room(seed: &str) -> Room {
    Room::new(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_tables_of_four_cover_all_twenty() {
        let room = empty_room("a seed");
        assert_eq!(room.tables.len(), 5);
        assert!(room.tables.iter().all(|t| t.members.len() == 4));
        let mut seated: Vec<&str> = room.tables.iter().flat_map(|t| t.members).collect();
        seated.sort_unstable();
        seated.dedup();
        assert_eq!(seated.len(), 20);
    }

    #[test]
    fn jaccard_bounds() {
        let e = std::collections::BTreeSet::new();
        assert_eq!(jaccard(&e, &e), 0.0);
        let a: std::collections::BTreeSet<String> = ["a".to_string()].into_iter().collect();
        assert_eq!(jaccard(&a, &a), 1.0);
    }

    #[test]
    fn duplicate_guard_is_per_table() {
        let mut room = empty_room("s");
        let line = "the horizon is a slow fire and we are all embers of it";
        let t1_member = room.tables[0].members[0];
        room.tables[0]
            .messages
            .push(Message::new(0, t1_member, line));
        assert!(room.tables[0].is_near_duplicate(line));
        assert!(!room.tables[1].is_near_duplicate(line));
    }

    #[test]
    fn figures_propose_candidates() {
        let figs = figures_in("about 21% rose and 3.2 Hz, with 250 crowds and 21% again");
        assert!(figs.contains(&"21%".to_string()));
        assert!(figs.contains(&"3.2".to_string()));
        assert!(figs.contains(&"250".to_string()));
        assert_eq!(figs.iter().filter(|f| *f == "21%").count(), 1);
    }

    #[test]
    fn quality_weights_sum_to_one() {
        let s: f64 = quality_weights().iter().map(|(_, w)| w).sum();
        assert!((s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn history_tracks_gauges_and_beliefs() {
        let mut room = empty_room("s");
        for v in [0.0, 0.5, 1.0] {
            room.tables[0].gauges.insert("heat".to_string(), v);
            room.tables[0].note_gauges();
        }
        assert_eq!(room.tables[0].history["heat"], vec![0.0, 0.5, 1.0]);
        for v in [0.3, 0.6] {
            room.tables[0].judgement.beliefs =
                PROPOSITIONS.iter().map(|p| (p.to_string(), v)).collect();
            room.tables[0].note_beliefs();
        }
        for p in PROPOSITIONS {
            assert_eq!(room.tables[0].belief_history[p], vec![0.3, 0.6]);
        }
    }

    #[test]
    fn test_a_heard_line_is_marked_and_verbatim_room() {
        let room = empty_room("s");
        let llama = room.tables[0].members[0];
        let mut heard = Message::new(0, llama, "a striking line from the first table");
        heard.heard_from = Some(1);
        assert!(heard.heard());
        assert_eq!(heard.heard_from, Some(1));
    }

    #[test]
    fn reset_clears_tables_but_keeps_seating() {
        let mut room = empty_room("first");
        let m = room.tables[0].members[0];
        room.tables[0]
            .messages
            .push(Message::new(0, m, "something"));
        room.llamas.get_mut(m).unwrap().spoken = 1;
        room.leaks.push(Leak {
            round: 1,
            src: 1,
            dst: 2,
            line: "x".to_string(),
            speaker: "byte".to_string(),
            why: String::new(),
            effect: String::new(),
            landed: false,
        });
        room.cost_jev = 0.01;
        room.cost_llama = 0.01;
        room.reset("second");
        assert_eq!(room.seed, "second");
        assert!(room.tables.iter().all(|t| t.messages.is_empty()));
        assert!(room.leaks.is_empty());
        assert_eq!(room.cost(), 0.0);
        assert_eq!(room.tables.len(), 5);
        assert!(room.llamas.values().all(|s| s.spoken == 0));
    }
}
