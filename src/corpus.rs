//! The corpus: 36 real lines The Corral produced, captured off disk and from
//! live runs. NO llama calls here -- the experiments read this and ask Jev
//! questions about it. Captured before the Rust port; the pre-port source is in
//! the commit history.
#![allow(clippy::redundant_static_lifetimes)]

/// One real transcript line.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub table: u8,
    pub speaker: &'static str,
    pub text: &'static str,
    /// "disk" (captured in corral.txt) or "live" (a real headless run).
    pub source: &'static str,
    /// If set, this line was overheard from table N.
    pub heard_from: Option<u8>,
}

pub const SEED: &str = "is a crowd wiser than any one of us, or just louder?";

/// The 36 real lines.
pub const LINES: [Line; 36] = [
    Line { table: 1, speaker: "gus", text: "A quiet beaver builds a strong dam, not a loud w…", source: "disk", heard_from: None },
    Line { table: 1, speaker: "dexter", text: "I've monitored past conversations about group dynamics; it seems that when the noise decreases, insight incre…", source: "disk", heard_from: Some(2) },
    Line { table: 2, speaker: "nimbus", text: "Storm clouds gather, Orson's words a thunderous…", source: "disk", heard_from: None },
    Line { table: 2, speaker: "dexter", text: "I've monitored past conversations about group dy…", source: "disk", heard_from: None },
    Line { table: 2, speaker: "cosmo", text: "Nimbus, Orson's words are symptoms, not roots. What's the wound a crowd's fervor aims to heal?", source: "disk", heard_from: None },
    Line { table: 2, speaker: "byte", text: "50% threshold doesn't account for information redundancy", source: "live", heard_from: None },
    Line { table: 3, speaker: "marisol", text: "What if that single individual's keen eye wasn't…", source: "disk", heard_from: None },
    Line { table: 3, speaker: "onyx", text: "Their empathetic influence could create an envir…", source: "disk", heard_from: None },
    Line { table: 3, speaker: "vera", text: "Glad we're discussing this, I think we need to differentiate between 'group intelligence' and 'collective loudness'.", source: "live", heard_from: None },
    Line { table: 3, speaker: "marisol", text: "I'm Livia here, I recall a time when our group at the art studio decided to create a mural, and through collective brainstorming, we came up with a beautiful and cohesive piece.", source: "live", heard_from: None },
    Line { table: 3, speaker: "vera", text: "Not to diminish your mural, but collective loudness merely aggregates individual opinions, whereas group intelligence might involve synergy and shared problem-solving.", source: "live", heard_from: None },
    Line { table: 3, speaker: "paloma", text: "Marisol, I think Vera's point is still relevant, yet your experience at the art studio showcases how collective input can lead to unexpected and creative solutions.", source: "live", heard_from: None },
    Line { table: 3, speaker: "onyx", text: "Livia, collective input can indeed yield unique solutions, but the true challenge lies in distinguishing genuine group intelligence from superficial concurrence.", source: "live", heard_from: None },
    Line { table: 3, speaker: "marisol", text: "As I listened to the sound of 12 hammers tapping together in perfect rhythm at the art studio, I realized harmony was born not from individual loudness, but from shared vision.", source: "live", heard_from: None },
    Line { table: 4, speaker: "sable", text: "Caspian's enthusiasm is a stark contrast to the room's collective skepticism, but I'm not holding my breath", source: "disk", heard_from: None },
    Line { table: 4, speaker: "fern", text: "The feeling in the room is indeed a mix of excitement and tension, with Bram's caution setting a more measured tone", source: "disk", heard_from: None },
    Line { table: 4, speaker: "cosmo", text: "Nimbus, Orson's words are symptoms, not roots. What's the wound a crowd's fervor aims to heal?", source: "disk", heard_from: Some(2) },
    Line { table: 4, speaker: "bram", text: "Bram, the crowd's fervor may be a desperate cry for meaning or a desperate attempt to fill a void left by failed institutions.", source: "disk", heard_from: None },
    Line { table: 4, speaker: "tilly", text: "Sable thinks crowds are just a noisy distraction, but I feel like Bram's getting at something important here.", source: "disk", heard_from: None },
    Line { table: 4, speaker: "ada", text: "As a systems thinker, I believe that's where the layers of a crowd's wisdom come into play: the surface level of volume, the middle layer of diversity, and the load-bearing foundation of intention", source: "disk", heard_from: Some(5) },
    Line { table: 4, speaker: "sable", text: "Intention sounds like a euphemism for 'whatever the loudest person says'", source: "disk", heard_from: None },
    Line { table: 4, speaker: "bram", text: "Bram, that's a common critique, but intention is more about collective goals than just the loudest voice", source: "disk", heard_from: None },
    Line { table: 4, speaker: "bram", text: "As I see it, we need to look at crowds from a historical perspective, where the loudest voices often led to chaos.", source: "live", heard_from: None },
    Line { table: 4, speaker: "sable", text: "Crowds are like the aunt at family reunions, always making a scene but not necessarily knowing anything.", source: "live", heard_from: None },
    Line { table: 4, speaker: "fern", text: "Tension, the mood matches Sable's wry remark.", source: "live", heard_from: None },
    Line { table: 4, speaker: "tilly", text: "Actually, I think I just spotted Luna at table 2, looking so wise, and she was nodding in agreement, you know, that wise look of hers really means something.", source: "live", heard_from: None },
    Line { table: 4, speaker: "bram", text: "That wise look of hers means nothing, it's the outcome of her thoughts we should be worried about, not the manner in which they're delivered.", source: "live", heard_from: None },
    Line { table: 4, speaker: "sable", text: "Historical precedent is all well and good, but what does it matter when the outcome is still someone's ego inflating?", source: "live", heard_from: None },
    Line { table: 5, speaker: "wren", text: "How do you feel when a crowd's intentions aren't…", source: "disk", heard_from: None },
    Line { table: 5, speaker: "ada", text: "As a systems thinker, the surface level is where…", source: "disk", heard_from: None },
    Line { table: 5, speaker: "ada", text: "I'm Ada. A crowd's collective wisdom comes from individual perspectives, not just noise.", source: "live", heard_from: None },
    Line { table: 5, speaker: "wren", text: "How does being part of a crowd make you feel, rather than the wisdom itself?", source: "live", heard_from: None },
    Line { table: 5, speaker: "ada", text: "I see the crowd as interconnected nodes, with wisdom emerging from the network as a whole, but one vulnerable node can bring down the whole.", source: "live", heard_from: None },
    Line { table: 5, speaker: "wren", text: "Being part of a crowd can be overwhelming and make me feel lost in the sea of voices.", source: "live", heard_from: None },
    Line { table: 5, speaker: "ada", text: "In such networks, each node's influence varies widely, with some holding greater sway than others, and the system's strength lies in its redundancy.", source: "live", heard_from: None },
    Line { table: 5, speaker: "wren", text: "Does the thought of redundancy make you feel safer, Ada, or more anxious about the potential for failure?", source: "live", heard_from: None },
];

/// Which four llamas sit at each table.
pub const TABLES: [(u8, [&'static str; 4]); 5] = [
    (1, ["byte", "gus", "juniper", "kestrel"]),
    (2, ["dexter", "nimbus", "cosmo", "orson"]),
    (3, ["vera", "marisol", "onyx", "paloma"]),
    (4, ["bram", "sable", "fern", "tilly"]),
    (5, ["quill", "pip", "wren", "ada"]),
];

/// The three eavesdrops the app actually logged (round, src, dst, speaker, line).
pub const LEAKS: [(u8, u8, u8, &'static str, &'static str); 3] = [
    (6, 2, 4, "cosmo", "Nimbus, Orson's words are symptoms, not roots. What's the wound a crowd's fervor aims to heal?"),
    (8, 5, 4, "ada", "As a systems thinker, I believe that's where the layers of a crowd's wisdom come into play: the surface level…"),
    (10, 2, 1, "dexter", "I've monitored past conversations about group dynamics; it seems that when the noise decreases, insight incre…"),
];

/// The four llamas seated at a table.
pub fn table_members(table: u8) -> Option<[&'static str; 4]> {
    TABLES.iter().find(|(id, _)| *id == table).map(|(_, m)| *m)
}

/// Every line said at one table.
pub fn lines_of(table: u8) -> Vec<(&'static str, &'static str)> {
    LINES
        .iter()
        .filter(|l| l.table == table)
        .map(|l| (l.speaker, l.text))
        .collect()
}

/// Lines that drifted in from another table, with their source.
pub fn heard_lines() -> Vec<(u8, &'static str, &'static str, u8)> {
    LINES
        .iter()
        .filter_map(|l| l.heard_from.map(|h| (l.table, l.speaker, l.text, h)))
        .collect()
}

pub fn corpus_size() -> usize {
    LINES.len()
}
