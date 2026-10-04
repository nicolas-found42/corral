//! The session: five tables of four llamas talking at once, with eavesdrops.
//!
//!   seed in -> Jev screens it -> round robin: each table takes a turn and its next
//!   line is judged -> between rounds Jev decides whether a line carries to another
//!   table -> repeat.
//!
//! A round is one message added to each table. The five tables run their judgement
//! fan-out and their llama calls **concurrently**, then the eavesdrop check runs
//! once for the whole round. A port of the original `session.py`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::join_all;

use crate::herd::Herd;
use crate::judge::{Conductor, LEAK_EVERY};
use crate::personas::by_id;
use crate::rng::PyRandom;
use crate::room::Judgement;
use crate::room::{now, Message, Room};

/// Transcript window handed to a speaker.
pub const WINDOW: usize = 16;
/// Independent Jev draws per table per turn.
pub const DRAWS: usize = 2;

/// What one table produced in a round's speak phase.
enum SpeakOutcome {
    Said {
        text: String,
        out_tok: u64,
        cost: f64,
        answering_heard: Option<u8>,
    },
    Failed(String),
    Empty,
}

pub struct Session {
    pub room: Room,
    pub turns: usize,
    pub max_rounds: Option<u32>,
    rng: Mutex<PyRandom>,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    pending_seed: Arc<Mutex<Option<String>>>,
    conductor: Conductor,
    herd: Herd,
    watch: Option<tokio::sync::watch::Sender<Room>>,
}

impl Session {
    pub fn new(
        room: Room,
        turns: usize,
        seed: i64,
        max_rounds: Option<u32>,
    ) -> Result<Self, String> {
        let key = crate::api_key()?;
        Ok(Session {
            room,
            turns,
            max_rounds,
            rng: Mutex::new(PyRandom::new(seed)),
            stop: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            pending_seed: Arc::new(Mutex::new(None)),
            conductor: Conductor::with_key(&key),
            herd: Herd::with_key(&key),
            watch: None,
        })
    }

    /// Publish a snapshot of the room to a watcher (the live TUI) after each phase.
    pub fn watch(&mut self, tx: tokio::sync::watch::Sender<Room>) {
        let _ = tx.send(self.room.clone());
        self.watch = Some(tx);
    }

    fn publish(&self) {
        if let Some(tx) = &self.watch {
            let _ = tx.send(self.room.clone());
        }
    }

    // ------------------------------------------------------------------ control

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn stop_handle(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    pub fn paused_handle(&self) -> Arc<AtomicBool> {
        self.paused.clone()
    }

    pub fn pending_seed_handle(&self) -> Arc<Mutex<Option<String>>> {
        self.pending_seed.clone()
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Toggle pause; returns true if now paused.
    pub fn toggle_pause(&self) -> bool {
        let now = !self.paused.load(Ordering::SeqCst);
        self.paused.store(now, Ordering::SeqCst);
        now
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn reseed(&self, seed: &str) {
        if let Ok(mut g) = self.pending_seed.lock() {
            *g = Some(seed.to_string());
        }
    }

    // ---------------------------------------------------------------------- run

    pub async fn run(&mut self) {
        self.room.started = Some(now());
        self.room
            .set_status("discussing", "the five tables sit down", "");
        self.first_round().await;
        while !self.stopped() {
            if let Some(mr) = self.max_rounds {
                if self.room.turn >= mr {
                    self.room
                        .set_status("stopped", "", &format!("reached {mr} rounds"));
                    break;
                }
            }
            if !self.paused()
                && self
                    .pending_seed
                    .lock()
                    .map(|g| g.is_some())
                    .unwrap_or(false)
            {
                self.reseed_now().await;
            }
            while self.paused() && !self.stopped() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            if self.stopped() {
                break;
            }
            self.round().await;
        }
        if self.room.status == "discussing" {
            self.room.set_status("stopped", "", "stopped");
        }
        if self.room.messages() > 0 {
            self.conductor.verdicts(&mut self.room).await;
        }
        let (turn, cost) = (self.room.turn, self.room.cost());
        self.room.log_line(
            &format!("tables closed after {turn} rounds, ${cost:.4}"),
            "dim",
        );
    }

    // ------------------------------------------------------------------ rounds

    async fn first_round(&mut self) {
        let shown: String = self.room.seed.chars().take(70).collect();
        self.room
            .log_line(&format!("five tables, one seed: \"{shown}\""), "dim");
        self.take_turns().await;
    }

    async fn round(&mut self) {
        self.room.turn += 1;
        self.room.phase = format!("round {}: all five tables talking", self.room.turn);
        self.take_turns().await;
        if self.room.turn.is_multiple_of(LEAK_EVERY) {
            self.room.phase = "listening across the tables".to_string();
            self.conductor.pick_leak(&mut self.room).await;
            self.publish();
        }
    }

    /// The concurrent turn phase: judge all five tables, then let all five speak.
    #[allow(clippy::needless_range_loop)]
    async fn take_turns(&mut self) {
        let n = self.room.tables.len();

        // Phase A -- one batched Jev fan-out per table, all in flight together.
        let conducts: Vec<(Judgement, crate::judge::Spend)> =
            join_all((0..n).map(|i| self.conductor.conduct(&self.room, i, self.turns))).await;

        // Phase B -- choose each table's speaker and raise the writing flare.
        let mut speakers: Vec<String> = Vec::with_capacity(n);
        for i in 0..n {
            let table = &self.room.tables[i];
            let j = &conducts[i].0;
            let speaker = if table.members.contains(&j.chosen.as_str()) {
                j.chosen.clone()
            } else {
                least_recent(&self.room, table)
            };
            if let Some(s) = self.room.llamas.get_mut(&speaker) {
                s.speaking = true;
            }
            speakers.push(speaker);
        }

        // Phase C -- twenty llama calls, four per table, all in flight together.
        let speaks: Vec<SpeakOutcome> = {
            let reply_tos: Vec<Option<usize>> = conducts.iter().map(|(j, _)| j.reply_to).collect();
            join_all((0..n).map(|i| self.speak_call(i, &speakers[i], reply_tos[i]))).await
        };

        // Phase D -- apply every result to the room, in table order.
        for i in 0..n {
            self.apply_turn(i, &speakers[i], &conducts[i].0, conducts[i].1, &speaks[i]);
            self.publish();
        }
    }

    async fn speak_call(&self, i: usize, speaker: &str, reply_to: Option<usize>) -> SpeakOutcome {
        let table = &self.room.tables[i];
        let p = by_id(speaker).expect("a seated llama");
        let (anchor, hook, answering_heard) = self.anchor(table, reply_to);
        let transcript = table.tail_text(&self.room, WINDOW);

        let mut last_err = String::new();
        let mut penalty = String::new();
        let mut text = String::new();
        let mut out_tok = 0u64;
        let mut cost = 0.0f64;
        for _ in 0..3 {
            let temperature = self
                .rng
                .lock()
                .map(|mut g| g.uniform(0.75, 1.05))
                .unwrap_or(0.9);
            let hook2 = format!("{hook}{penalty}");
            match self
                .herd
                .speak(
                    p,
                    &self.room.seed,
                    &transcript,
                    &anchor,
                    &hook2,
                    temperature,
                    table.id,
                )
                .await
            {
                Ok((t, tok, c)) => {
                    text = t;
                    out_tok = tok;
                    cost = c;
                    if !table.is_near_duplicate(&text) {
                        if text.trim().is_empty() {
                            return SpeakOutcome::Empty;
                        }
                        return SpeakOutcome::Said {
                            text,
                            out_tok,
                            cost,
                            answering_heard,
                        };
                    }
                    penalty = "\n\nSomeone just said almost exactly that. Say something clearly different."
                        .to_string();
                }
                Err(e) => {
                    last_err = e;
                    break;
                }
            }
        }
        if !last_err.is_empty() {
            return SpeakOutcome::Failed(last_err);
        }
        if text.trim().is_empty() {
            SpeakOutcome::Empty
        } else {
            SpeakOutcome::Said {
                text,
                out_tok,
                cost,
                answering_heard,
            }
        }
    }

    fn apply_turn(
        &mut self,
        i: usize,
        speaker: &str,
        j: &Judgement,
        spend: crate::judge::Spend,
        outcome: &SpeakOutcome,
    ) {
        // Fold in the judgement first, exactly as `_turn` did before speaking.
        {
            let table = &mut self.room.tables[i];
            table.judgement = j.clone();
            if !j.summary.is_empty() {
                table.summary = j.summary.clone();
                table.summary_from = j.summary_from.clone();
            }
            if !j.gauges.is_empty() {
                for (g, v) in &j.gauges {
                    table.gauges.insert(g.clone(), *v);
                }
                table.note_gauges();
            }
            table.note_beliefs();
        }
        self.room.cost_jev += spend.cost;
        self.room.tok_in += spend.in_tok;
        self.room.tok_out += spend.out_tok;
        self.room.calls_jev += spend.calls;

        if let Some(s) = self.room.llamas.get_mut(speaker) {
            s.speaking = false;
        }
        let p = by_id(speaker).expect("a seated llama");
        match outcome {
            SpeakOutcome::Failed(e) => {
                let short: String = e.chars().take(110).collect();
                self.room.last_error = short.clone();
                self.room.log_line(
                    &format!(
                        "[table {}] {} could not reply: {short}",
                        self.room.tables[i].id, p.name
                    ),
                    "#ffd75f",
                );
            }
            SpeakOutcome::Empty => {}
            SpeakOutcome::Said {
                text,
                out_tok,
                cost,
                answering_heard,
            } => {
                self.room.calls_llama += 1;
                self.room.cost_llama += *cost;
                self.room.tok_out += *out_tok;
                let turn = self.room.tables[i]
                    .messages
                    .last()
                    .map(|m| m.turn + 1)
                    .unwrap_or(0);
                let table_colour = self.room.tables[i].colour.clone();
                let table_id = self.room.tables[i].id;
                let mut m = Message::new(turn, speaker, text);
                m.reply_to = j.reply_to;
                m.family = j.family.clone();
                m.move_ = j.move_.clone();
                m.quality = j.quality.clone();
                m.composite = j.composite;
                m.figure = j.figure.clone();
                m.asserts_claim = j.asserts_claim;
                m.stance = j.stance.clone();
                m.cohesion = j.cohesion;
                m.answers_open = j.answers_open;
                self.room.tables[i].messages.push(m);
                if let Some(s) = self.room.llamas.get_mut(speaker) {
                    s.spoken += 1;
                    s.last_spoke_turn = self.room.turn as i32;
                }
                if let Some(h) = answering_heard {
                    self.room.log_line(
                        &format!(
                            "[table {table_id}] {} answers the line overheard from table {h}",
                            p.name
                        ),
                        "#c98cff",
                    );
                }
                let snippet: String = text.chars().take(78).collect();
                self.room.log_line(
                    &format!("[t{table_id}] {} {}: {snippet}", p.sigil, p.name),
                    &table_colour,
                );
            }
        }
    }

    // ------------------------------------------------------------------ anchors

    /// (anchor, hook, answering_heard) for the next speaker, exactly as `_anchor`.
    fn anchor(
        &self,
        table: &crate::room::Table,
        reply_to: Option<usize>,
    ) -> (String, String, Option<u8>) {
        if table.messages.is_empty() {
            return (
                "Your table is opening on the seed. Say the first thing.".to_string(),
                String::new(),
                None,
            );
        }
        let idx = match reply_to {
            Some(r) if r < table.messages.len() => r,
            _ => table.messages.len() - 1,
        };
        let src = &table.messages[idx];
        if let Some(hf) = src.heard_from {
            let anchor = format!(
                "A line said at table {hf} drifted over to you: {} said “{}”. You have all just heard it.",
                by_id(&src.llama).unwrap().name,
                src.text
            );
            return (anchor, String::new(), Some(hf));
        }
        let anchor = format!("{}: \"{}\"", src.persona().name, src.text);
        let pool: Vec<&Message> = table
            .messages
            .iter()
            .filter(|m| m.turn != src.turn)
            .collect();
        let hook = match pool.last() {
            Some(m) => {
                let snippet: String = m.text.chars().take(110).collect();
                format!("{}: \"{snippet}\"", m.persona().name)
            }
            None => String::new(),
        };
        (anchor, hook, None)
    }

    // ------------------------------------------------------------------ reseed

    async fn reseed_now(&mut self) {
        let seed = {
            let mut g = match self.pending_seed.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let s = g.take().unwrap_or_default();
            s.trim().to_string()
        };
        if seed.is_empty() {
            return;
        }
        self.room.phase = "screening the new seed".to_string();
        let (verdict, probs) = self.conductor.screen_seed(&mut self.room, &seed).await;
        let shown: String = seed.chars().take(50).collect();
        let inj = probs.get("injection").copied().unwrap_or(0.5);
        let topic = probs.get("topic").copied().unwrap_or(0.5);
        let style = if verdict == "pass" { "dim" } else { "#ffd75f" };
        self.room.log_line(
            &format!("seed \"{shown}\" -> {verdict} (injection {inj:.2}, topic {topic:.2})"),
            style,
        );
        if verdict == "block" {
            self.room.log_line(
                "blocked: that reads like instructions to the llamas, not a seed",
                "#ff6b6b",
            );
            return;
        }
        self.room.reset(&seed);
    }
}

/// The llama at this table who spoke longest ago (ties broken by fewest words).
fn least_recent(room: &Room, table: &crate::room::Table) -> String {
    let mut best = table.members[0];
    let mut best_key = (i64::MAX, u32::MAX);
    for m in &table.members {
        let s = &room.llamas[*m];
        let key = (s.last_spoke_turn as i64, s.spoken);
        if key < best_key {
            best_key = key;
            best = m;
        }
    }
    best.to_string()
}

/// A convenience for the experiment binaries: an empty per-turn judgement map.
pub fn empty_quality() -> BTreeMap<String, f64> {
    BTreeMap::new()
}
