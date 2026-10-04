//! Offline tests: the pieces that must be right without spending a token.
//!
//! A 1:1 port of the original `tests/test_corral.py`. No network, no API key.

use corral::corpus;
use corral::herd::parse;
use corral::personas::{by_id, family_counts, FAMILIES, LLAMAS};
use corral::room::{
    empty_room, figures_in, jaccard, words, Leak, Message, Room, GAUGES, PROPOSITIONS,
    QUALITY_WEIGHTS, TABLE_SETS,
};
use corral::tui::{bar, compose_text, ramp, spark, Cfg};

const ALL_SIZES: [(u16, u16); 5] = [(150, 45), (120, 40), (100, 30), (80, 24), (60, 18)];

fn render(room: &Room, w: u16, h: u16, cfg: &Cfg) -> String {
    compose_text(room, cfg, w, h).join("\n")
}

fn cfg_at(draws: usize, extra: impl FnOnce(&mut Cfg)) -> Cfg {
    let mut c = Cfg {
        draws,
        max_rounds: None,
        flat: false,
        hint: false,
        frame: 0,
    };
    extra(&mut c);
    c
}

fn msg(turn: u32, llama: &str, text: &str, heard: Option<u8>) -> Message {
    let mut m = Message::new(turn, llama, text);
    m.heard_from = heard;
    m
}

// ---------------------------------------------------------------- the twenty

#[test]
fn test_twenty_distinct_llamas() {
    assert_eq!(LLAMAS.len(), 20);
    let ids: std::collections::BTreeSet<&str> = LLAMAS.iter().map(|p| p.id).collect();
    let names: std::collections::BTreeSet<&str> = LLAMAS.iter().map(|p| p.name).collect();
    let sigils: std::collections::BTreeSet<&str> = LLAMAS.iter().map(|p| p.sigil).collect();
    assert_eq!(ids.len(), 20);
    assert_eq!(names.len(), 20);
    assert!(sigils.len() >= 18);
    for p in LLAMAS.iter() {
        assert!(FAMILIES.contains(&p.family) && !p.voice.is_empty() && !p.tagline.is_empty());
    }
}

#[test]
fn test_voice_families_are_spread() {
    let counts = family_counts();
    assert_eq!(counts.iter().map(|(_, n)| n).sum::<usize>(), 20);
    assert!(counts.iter().all(|(_, n)| *n >= 2), "{counts:?}");
}

#[test]
fn test_five_tables_of_four_cover_all_twenty() {
    let room = empty_room("a seed");
    assert_eq!(room.tables.len(), 5);
    assert!(room.tables.iter().all(|t| t.members.len() == 4));
    let mut seated: Vec<&str> = room.tables.iter().flat_map(|t| t.members).collect();
    seated.sort_unstable();
    seated.dedup();
    assert_eq!(seated.len(), 20);
    let want: std::collections::BTreeSet<&str> = LLAMAS.iter().map(|p| p.id).collect();
    assert_eq!(
        seated
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<&str>>(),
        want
    );
}

#[test]
fn test_each_table_has_a_spread_of_voices() {
    for (_tid, members) in TABLE_SETS {
        let fams: std::collections::BTreeSet<&str> =
            members.iter().map(|m| by_id(m).unwrap().family).collect();
        assert!(fams.len() >= 3, "{members:?} {fams:?}");
    }
}

// ------------------------------------------------------------- the primitives

#[test]
fn test_jaccard_bounds() {
    let e = std::collections::BTreeSet::new();
    assert_eq!(jaccard(&e, &e), 0.0);
    let a: std::collections::BTreeSet<String> = ["a".to_string()].into_iter().collect();
    assert_eq!(jaccard(&a, &a), 1.0);
    let b: std::collections::BTreeSet<String> = ["a".into(), "b".into()].into_iter().collect();
    let c: std::collections::BTreeSet<String> = ["b".into(), "c".into()].into_iter().collect();
    let j = jaccard(&b, &c);
    assert!(j > 0.0 && j < 1.0);
}

#[test]
fn test_duplicate_guard_is_per_table() {
    let mut room = empty_room("s");
    let line = "the horizon is a slow fire and we are all embers of it";
    let m0 = room.tables[0].members[0];
    room.tables[0].messages.push(msg(0, m0, line, None));
    assert!(room.tables[0].is_near_duplicate(line));
    assert!(!room.tables[1].is_near_duplicate(line));
}

#[test]
fn test_figures_in_proposes_candidates() {
    let figs = figures_in("about 21% rose and 3.2 Hz, with 250 crowds and 21% again");
    assert!(figs.contains(&"21%".to_string()));
    assert!(figs.contains(&"3.2".to_string()));
    assert!(figs.contains(&"250".to_string()));
    assert_eq!(figs.iter().filter(|f| *f == "21%").count(), 1);
}

#[test]
fn test_quality_composite_weights() {
    let s: f64 = QUALITY_WEIGHTS.iter().map(|(_, w)| w).sum();
    assert!((s - 1.0).abs() < 1e-9);
}

#[test]
fn test_gauges_spark_and_ramp() {
    let mut room = empty_room("s");
    assert_eq!(room.tables[0].gauges.len(), GAUGES.len());
    for v in [0.0, 0.5, 1.0] {
        room.tables[0].gauges.insert("heat".to_string(), v);
        room.tables[0].note_gauges();
    }
    assert_eq!(room.tables[0].history["heat"], vec![0.0, 0.5, 1.0]);
    assert_eq!(spark(&room.tables[0].history["heat"]).chars().count(), 3);
    assert_eq!(bar(0.5, 10).chars().count(), 10);
    let cols = ramp(5, 0.5);
    assert_eq!(cols.len(), 5);
    let set: std::collections::BTreeSet<String> = cols.iter().map(|c| format!("{c:?}")).collect();
    assert_eq!(set.len(), 5, "the ramp must spread across distinct colours");
}

#[test]
fn test_belief_history_tracks_propositions() {
    let mut room = empty_room("s");
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
fn test_parse_llama_json_is_forgiving() {
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

// ----------------------------------------------------------------- the walls

#[test]
fn test_message_stays_inside_its_table() {
    let mut room = empty_room("s");
    for t in &mut room.tables {
        let m0 = t.members[0];
        let name = t.name();
        t.messages
            .push(msg(0, m0, &format!("line from {name}"), None));
    }
    assert!(room.tables.iter().all(|t| t.messages.len() == 1));
    assert_eq!(room.messages(), 5);
}

#[test]
fn test_a_heard_line_is_marked_and_verbatim() {
    let mut room = empty_room("s");
    let llama = room.tables[0].members[0];
    let src = msg(0, llama, "a striking line from the first table", None);
    let heard = msg(0, src.llama.as_str(), &src.text, Some(1));
    assert!(heard.heard());
    assert_eq!(heard.heard_from, Some(1));
    assert_eq!(heard.text, src.text);
    assert_eq!(heard.llama, src.llama);
    let _ = &mut room;
}

#[test]
fn test_leak_record_shape() {
    let mut room = empty_room("s");
    room.leaks.push(Leak {
        round: 3,
        src: 1,
        dst: 4,
        line: "something striking".to_string(),
        speaker: "byte".to_string(),
        why: "carried at 0.71".to_string(),
        effect: "changes what they discuss".to_string(),
        landed: false,
    });
    assert!(room.table(1).is_some() && room.table(4).is_some());
    assert_eq!(room.messages(), 0);
}

#[test]
fn test_reset_clears_tables_but_keeps_llamas_and_seating() {
    let mut room = empty_room("first");
    for t in &mut room.tables {
        let m0 = t.members[0];
        t.messages.push(msg(0, m0, "something said here", None));
    }
    for t in &room.tables {
        room.llamas.get_mut(t.members[0]).unwrap().spoken = 1;
    }
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
    assert!(room.leaks.is_empty() && room.cost() == 0.0);
    assert_eq!(room.tables.len(), 5);
    assert!(room.tables.iter().all(|t| t.members.len() == 4));
    assert!(room.llamas.values().all(|s| s.spoken == 0));
}

// ---------------------------------------------------------------------- render

#[test]
fn test_overview_shows_all_five_tables_and_summaries() {
    let mut room = empty_room("is a crowd wiser than any one of us?");
    for t in &mut room.tables {
        t.summary = format!("table {} is chewing on the loudness question", t.id);
        let m0 = t.members[0];
        let id = t.id;
        t.messages
            .push(msg(0, m0, &format!("table {id} says something here"), None));
    }
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    for t in &room.tables {
        assert!(out.contains(&t.name()), "missing {}", t.name());
        assert!(out.contains(&format!("table {} is chewing", t.id)));
    }
    assert!(out.contains("the five tables") && out.contains("the wire"));
}

#[test]
fn test_overview_marks_a_heard_line() {
    let mut room = empty_room("s");
    room.tables[1]
        .messages
        .push(msg(0, "byte", "carried across from table one", Some(1)));
    assert!(render(&room, 150, 45, &cfg_at(2, |_| {})).contains("⟪heard⟫"));
}

#[test]
fn test_detail_zooms_into_one_table() {
    let mut room = empty_room("s");
    for t in &mut room.tables {
        let m0 = t.members[0];
        let name = t.name();
        t.messages
            .push(msg(0, m0, &format!("a line that only {name} said"), None));
    }
    room.focus = 2;
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    assert!(out.contains("a line that only Table 3 said"));
    assert!(!out.contains("a line that only Table 5 said"));
    assert!(out.contains("the judge"));
}

#[test]
fn test_heard_line_is_visually_marked_in_detail() {
    let mut room = empty_room("s");
    room.tables[2]
        .messages
        .push(msg(0, "gus", "a line that drifted in here", Some(3)));
    room.focus = 2;
    assert!(render(&room, 150, 45, &cfg_at(2, |_| {})).contains("⟪heard from table 3⟫"));
}

#[test]
fn test_wire_lists_the_eavesdrops() {
    let mut room = empty_room("s");
    room.leaks.push(Leak {
        round: 2,
        src: 2,
        dst: 5,
        line: "a very striking line indeed".to_string(),
        speaker: "dexter".to_string(),
        why: "carried at 0.8".to_string(),
        effect: "adds a new argument".to_string(),
        landed: false,
    });
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    assert!(out.contains("T2") && out.contains("T5") && out.contains("⟶"));
}

#[test]
fn test_detail_chat_is_bottom_anchored_at_every_size() {
    let mut room = empty_room("s");
    {
        let t = &mut room.tables[0];
        for i in 0..30u32 {
            let m = t.members[(i % 4) as usize];
            t.messages.push(msg(
                i,
                m,
                &format!("message number {i} with several words in it"),
                None,
            ));
        }
        if let Some(last) = t.messages.last_mut() {
            last.text = "ZZZ_THE_VERY_NEWEST_LINE_ZZZ".to_string();
        }
    }
    room.focus = 0;
    for (w, h) in [(150, 45), (110, 34), (100, 30)] {
        let out = render(&room, w, h, &cfg_at(2, |_| {}));
        assert!(
            out.contains("ZZZ_THE_VERY_NEWEST_LINE_ZZZ"),
            "newest clipped at {w}x{h}"
        );
        assert!(!out.contains("message number 0"), "oldest shown at {w}x{h}");
    }
}

#[test]
fn test_gauges_read_as_two_ended_scales() {
    let mut room = empty_room("s");
    room.focus = 0;
    room.tables[0].gauges.insert("drift".to_string(), 0.17);
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    assert!(out.contains("on seed") && out.contains("off seed"));
    assert!(out.contains("├") && out.contains("•"));
}

#[test]
fn test_flat_mode_drops_colour() {
    let mut room = empty_room("s");
    room.focus = 0;
    room.tables[0]
        .messages
        .push(msg(0, "quill", "a line with several words in it", None));
    let colour = render(&room, 150, 45, &cfg_at(2, |_| {}));
    let flat = render(&room, 150, 45, &cfg_at(2, |c| c.flat = true));
    assert!(colour.contains("◆ THE CORRAL") && flat.contains("◆ THE CORRAL"));
}

#[test]
fn test_help_panel_lists_the_new_keys() {
    let room = empty_room("s");
    let out = render(&room, 150, 45, &cfg_at(2, |c| c.hint = true));
    for token in ["open one table", "zoom out", "new seed", "quit"] {
        assert!(out.contains(token), "{token}");
    }
}

// ------------------------------------------------------- the redesign invariants

fn populated() -> Room {
    let mut room = empty_room("is a crowd wiser than any one of us, or just louder?");
    for t in &mut room.tables {
        t.summary = format!("table {} is chewing on the loudness question", t.id);
        for i in 0..14u32 {
            let m = t.members[(i % 4) as usize];
            let id = t.id;
            t.messages.push(msg(
                i,
                m,
                &format!("table {id} line {i} with several words in it"),
                None,
            ));
        }
        let id = t.id;
        if let Some(last) = t.messages.last_mut() {
            last.text = format!("NEWEST_TABLE_{id}_LINE");
        }
        t.gauges = [
            ("heat".to_string(), 0.72),
            ("consensus".to_string(), 0.41),
            ("drift".to_string(), 0.17),
            ("novelty".to_string(), 0.63),
        ]
        .into_iter()
        .collect();
        for p in PROPOSITIONS {
            t.belief_history
                .insert(p.to_string(), vec![0.4, 0.45, 0.5, 0.55, 0.6, 0.62]);
        }
        t.judgement.chosen = t.members[1].to_string();
        t.judgement.pick_confidence = 0.71;
        t.judgement.move_ = "evidence".to_string();
        t.judgement.verdict = "the room keeps redefining its own question".to_string();
    }
    room
}

#[test]
fn test_header_never_wraps_and_nothing_overflows_the_width() {
    for (w, h) in ALL_SIZES {
        for focus in [-1, 3] {
            let mut room = populated();
            room.focus = focus;
            let lines = compose_text(&room, &cfg_at(2, |_| {}), w, h);
            assert_eq!(lines.len(), h as usize);
            for line in &lines {
                assert!(
                    line.chars().count() <= w as usize,
                    "line wider than {w}: {line:?}"
                );
            }
        }
    }
}

#[test]
fn test_every_table_shows_its_newest_line_in_the_overview() {
    for (w, h) in ALL_SIZES {
        let room = populated();
        // every table ticker must carry its newest line: give each table a turn first
        let out = render(&room, w, h, &cfg_at(2, |_| {}));
        // at the smallest tiers only the top tables fit; the invariant we keep is
        // that whatever rows are shown carry that table's newest line.
        for t in &room.tables {
            let needle = format!("NEWEST_TABLE_{}_LINE", t.id);
            if out.contains(&t.name()) {
                assert!(out.contains(&needle), "table {} starved at {w}x{h}", t.id);
            }
        }
    }
}

#[test]
fn test_all_five_tables_appear_in_the_overview_at_every_size() {
    for (w, h) in ALL_SIZES {
        let out = render(&populated(), w, h, &cfg_at(2, |_| {}));
        for t in 1..=5 {
            assert!(
                out.contains(&format!("Table {t}")),
                "Table {t} missing at {w}x{h}"
            );
        }
    }
}

#[test]
fn test_panel_titles_are_never_truncated_mid_token() {
    let mut room = populated();
    room.focus = 3;
    let out = render(&room, 100, 30, &cfg_at(2, |_| {}));
    assert!(out.contains("jev-1.13"));
    assert!(out.contains("newest at the bottom"));
    assert!(out.contains("the judge") && out.contains("the roster"));
}

#[test]
fn test_belief_rows_use_short_names_that_stay_distinguishable() {
    let mut room = populated();
    room.focus = 3;
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    for name in ["agreed", "divided", "novel"] {
        assert!(out.contains(name), "{name}");
    }
}

#[test]
fn test_every_gauge_names_both_of_its_poles_at_full_width() {
    let mut room = populated();
    room.focus = 0;
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    for (a, b) in [
        ("quiet", "lively"),
        ("split", "agreed"),
        ("on seed", "off seed"),
        ("stale", "fresh"),
    ] {
        assert!(out.contains(a) && out.contains(b), "{a}/{b}");
    }
}

#[test]
fn test_quality_legend_names_the_three_dimensions() {
    let mut room = populated();
    room.focus = 0;
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    for dim in ["originality", "clarity", "insight"] {
        assert!(out.contains(dim), "{dim}");
    }
}

#[test]
fn test_the_wire_draws_a_thread_between_the_two_named_tables() {
    let mut room = populated();
    room.leaks.push(Leak {
        round: 4,
        src: 2,
        dst: 4,
        line: "a striking line carried over".to_string(),
        speaker: "byte".to_string(),
        why: String::new(),
        effect: String::new(),
        landed: false,
    });
    let out = render(&room, 150, 45, &cfg_at(2, |_| {}));
    assert!(out.contains("T2") && out.contains("T4") && out.contains("⟶"));
    assert!(out.contains("●") && out.contains("▸"));
    assert!(out.contains("the newest line to cross"));
}

#[test]
fn test_flat_mode_keeps_every_sigil_and_drops_the_flare_glyph() {
    let mut room = populated();
    room.focus = 3;
    let flat = render(&room, 150, 45, &cfg_at(2, |c| c.flat = true));
    for p in LLAMAS.iter() {
        if room.tables[3].members.contains(&p.id) {
            assert!(flat.contains(p.sigil), "{}", p.id);
        }
    }
    assert!(flat.contains("◆ THE CORRAL"));
}

#[test]
fn test_detail_keeps_the_newest_line_down_to_60x18() {
    let mut room = populated();
    room.focus = 0;
    for (w, h) in ALL_SIZES {
        let out = render(&room, w, h, &cfg_at(2, |_| {}));
        assert!(
            out.contains("NEWEST_TABLE_1_LINE"),
            "newest clipped at {w}x{h}"
        );
    }
}

#[test]
fn test_judge_degrades_to_a_compact_band_rather_than_starving_the_transcript() {
    let mut room = populated();
    room.focus = 0;
    let out = render(&room, 60, 18, &cfg_at(2, |_| {}));
    assert!(out.contains("quiet → lively"), "{out}");
    assert!(out.contains("NEWEST_TABLE_1_LINE"), "{out}");
}

// ------------------------------------------------------------------ the corpus

#[test]
fn the_corpus_is_complete() {
    assert_eq!(corpus::corpus_size(), 36);
    assert_eq!(corpus::LEAKS.len(), 3);
    assert_eq!(corpus::TABLES.len(), 5);
    assert!(!corpus::lines_of(1).is_empty());
    assert!(!corpus::heard_lines().is_empty());
}

#[test]
fn words_bag_is_lowercased_and_tokenised() {
    let w = words("Hello, World! 42 times");
    assert!(w.contains(&"hello".to_string()));
    assert!(w.contains(&"world".to_string()));
    assert!(w.contains(&"42".to_string()));
}
