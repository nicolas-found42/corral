"""The corpus: real lines The Corral actually produced, captured off disk and from
live runs. NO llama calls here -- every experiment in experiments.py reads this and
asks TypeSafe Jev questions about it.

Provenance tags:
  "disk"  -- captured in corral.txt (the rendered snapshot of a real 10-round run)
  "live"  -- printed by a real headless run in this session
Lines ending in "…" were truncated by the display, not by us.
"""
from __future__ import annotations

SEED = "is a crowd wiser than any one of us, or just louder?"

# (table, speaker_id, text, source, heard_from)
LINES: list[tuple[int, str, str, str, int | None]] = [
    # ---- Table 1: byte / gus / juniper / kestrel
    (1, "gus", "A quiet beaver builds a strong dam, not a loud w…", "disk", None),
    (1, "dexter", "I've monitored past conversations about group dynamics; it seems that when the noise decreases, insight incre…", "disk", 2),
    # ---- Table 2: dexter / nimbus / cosmo / orson
    (2, "nimbus", "Storm clouds gather, Orson's words a thunderous…", "disk", None),
    (2, "dexter", "I've monitored past conversations about group dy…", "disk", None),
    (2, "cosmo", "Nimbus, Orson's words are symptoms, not roots. What's the wound a crowd's fervor aims to heal?", "disk", None),
    (2, "byte", "50% threshold doesn't account for information redundancy", "live", None),
    # ---- Table 3: vera / marisol / onyx / paloma
    (3, "marisol", "What if that single individual's keen eye wasn't…", "disk", None),
    (3, "onyx", "Their empathetic influence could create an envir…", "disk", None),
    (3, "vera", "Glad we're discussing this, I think we need to differentiate between 'group intelligence' and 'collective loudness'.", "live", None),
    (3, "marisol", "I'm Livia here, I recall a time when our group at the art studio decided to create a mural, and through collective brainstorming, we came up with a beautiful and cohesive piece.", "live", None),
    (3, "vera", "Not to diminish your mural, but collective loudness merely aggregates individual opinions, whereas group intelligence might involve synergy and shared problem-solving.", "live", None),
    (3, "paloma", "Marisol, I think Vera's point is still relevant, yet your experience at the art studio showcases how collective input can lead to unexpected and creative solutions.", "live", None),
    (3, "onyx", "Livia, collective input can indeed yield unique solutions, but the true challenge lies in distinguishing genuine group intelligence from superficial concurrence.", "live", None),
    (3, "marisol", "As I listened to the sound of 12 hammers tapping together in perfect rhythm at the art studio, I realized harmony was born not from individual loudness, but from shared vision.", "live", None),
    # ---- Table 4: bram / sable / fern / tilly
    (4, "sable", "Caspian's enthusiasm is a stark contrast to the room's collective skepticism, but I'm not holding my breath", "disk", None),
    (4, "fern", "The feeling in the room is indeed a mix of excitement and tension, with Bram's caution setting a more measured tone", "disk", None),
    (4, "cosmo", "Nimbus, Orson's words are symptoms, not roots. What's the wound a crowd's fervor aims to heal?", "disk", 2),
    (4, "bram", "Bram, the crowd's fervor may be a desperate cry for meaning or a desperate attempt to fill a void left by failed institutions.", "disk", None),
    (4, "tilly", "Sable thinks crowds are just a noisy distraction, but I feel like Bram's getting at something important here.", "disk", None),
    (4, "ada", "As a systems thinker, I believe that's where the layers of a crowd's wisdom come into play: the surface level of volume, the middle layer of diversity, and the load-bearing foundation of intention", "disk", 5),
    (4, "sable", "Intention sounds like a euphemism for 'whatever the loudest person says'", "disk", None),
    (4, "bram", "Bram, that's a common critique, but intention is more about collective goals than just the loudest voice", "disk", None),
    (4, "bram", "As I see it, we need to look at crowds from a historical perspective, where the loudest voices often led to chaos.", "live", None),
    (4, "sable", "Crowds are like the aunt at family reunions, always making a scene but not necessarily knowing anything.", "live", None),
    (4, "fern", "Tension, the mood matches Sable's wry remark.", "live", None),
    (4, "tilly", "Actually, I think I just spotted Luna at table 2, looking so wise, and she was nodding in agreement, you know, that wise look of hers really means something.", "live", None),
    (4, "bram", "That wise look of hers means nothing, it's the outcome of her thoughts we should be worried about, not the manner in which they're delivered.", "live", None),
    (4, "sable", "Historical precedent is all well and good, but what does it matter when the outcome is still someone's ego inflating?", "live", None),
    # ---- Table 5: quill / pip / wren / ada
    (5, "wren", "How do you feel when a crowd's intentions aren't…", "disk", None),
    (5, "ada", "As a systems thinker, the surface level is where…", "disk", None),
    (5, "ada", "I'm Ada. A crowd's collective wisdom comes from individual perspectives, not just noise.", "live", None),
    (5, "wren", "How does being part of a crowd make you feel, rather than the wisdom itself?", "live", None),
    (5, "ada", "I see the crowd as interconnected nodes, with wisdom emerging from the network as a whole, but one vulnerable node can bring down the whole.", "live", None),
    (5, "wren", "Being part of a crowd can be overwhelming and make me feel lost in the sea of voices.", "live", None),
    (5, "ada", "In such networks, each node's influence varies widely, with some holding greater sway than others, and the system's strength lies in its redundancy.", "live", None),
    (5, "wren", "Does the thought of redundancy make you feel safer, Ada, or more anxious about the potential for failure?", "live", None),
]

TABLES: dict[int, tuple[str, ...]] = {
    1: ("byte", "gus", "juniper", "kestrel"),
    2: ("dexter", "nimbus", "cosmo", "orson"),
    3: ("vera", "marisol", "onyx", "paloma"),
    4: ("bram", "sable", "fern", "tilly"),
    5: ("quill", "pip", "wren", "ada"),
}

# the three eavesdrops the app actually logged (round, src, dst, speaker, line)
LEAKS: list[tuple[int, int, int, str, str]] = [
    (6, 2, 4, "cosmo", "Nimbus, Orson's words are symptoms, not roots. What's the wound a crowd's fervor aims to heal?"),
    (8, 5, 4, "ada", "As a systems thinker, I believe that's where the layers of a crowd's wisdom come into play: the surface level…"),
    (10, 2, 1, "dexter", "I've monitored past conversations about group dynamics; it seems that when the noise decreases, insight incre…"),
]


def lines_of(table: int) -> list[tuple[str, str]]:
    return [(sp, tx) for (t, sp, tx, _src, _h) in LINES if t == table]


def all_lines() -> list[tuple[int, str, str]]:
    return [(t, sp, tx) for (t, sp, tx, _src, _h) in LINES]


def heard_lines() -> list[tuple[int, str, str, int]]:
    return [(t, sp, tx, h) for (t, sp, tx, _s, h) in LINES if h]


def corpus_size() -> int:
    return len(LINES)
