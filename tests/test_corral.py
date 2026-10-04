"""Offline tests: the pieces that must be right without spending a token."""
from __future__ import annotations

import io

from rich.console import Console

from herd import _parse
from personas import FAMILIES, LLAMAS, family_counts
from room import (GAUGES, PROPOSITIONS, QUALITY_WEIGHTS, TABLE_SETS, Leak, Message,
                  Room, empty_room, figures_in, jaccard, words)
from tui import bar, compose, ramp, spark

BY = {p.id: p for p in LLAMAS}


def _render(room: Room, w: int = 150, h: int = 45, **cfg_extra) -> str:
    buf = io.StringIO()
    cfg = {"draws": 2, "max_rounds": None}
    cfg.update(cfg_extra)
    Console(file=buf, width=w, height=h, force_terminal=False).print(compose(room, cfg, (w, h)))
    return buf.getvalue()


def _msg(turn, llama, text, heard=None):
    return Message(turn=turn, llama=llama, text=text, heard_from=heard)


# ---------------------------------------------------------------- the twenty
def test_twenty_distinct_llamas():
    assert len(LLAMAS) == 20
    assert len({p.id for p in LLAMAS}) == 20
    assert len({p.name for p in LLAMAS}) == 20
    assert len({p.sigil for p in LLAMAS}) >= 18
    for p in LLAMAS:
        assert p.family in FAMILIES and p.voice and p.tagline


def test_voice_families_are_spread():
    counts = family_counts()
    assert sum(counts.values()) == 20
    assert all(counts[f] >= 2 for f in FAMILIES), counts


def test_five_tables_of_four_cover_all_twenty():
    room = empty_room("a seed")
    assert len(room.tables) == 5
    assert all(len(t.members) == 4 for t in room.tables)
    seated = [m for t in room.tables for m in t.members]
    assert len(seated) == 20 and len(set(seated)) == 20
    assert set(seated) == {p.id for p in LLAMAS}


def test_each_table_has_a_spread_of_voices():
    for _tid, members in TABLE_SETS:
        fams = {BY[m].family for m in members}
        assert len(fams) >= 3, (members, fams)  # not four of a kind


# ------------------------------------------------------------- the primitives
def test_jaccard_bounds():
    assert jaccard(set(), set()) == 0.0
    assert jaccard({"a"}, {"a"}) == 1.0
    assert 0 < jaccard({"a", "b"}, {"b", "c"}) < 1


def test_duplicate_guard_is_per_table():
    room = empty_room("s")
    t1, t2 = room.tables[0], room.tables[1]
    line = "the horizon is a slow fire and we are all embers of it"
    t1.messages.append(_msg(0, t1.members[0], line))
    assert t1.is_near_duplicate(line)
    assert not t2.is_near_duplicate(line)  # other tables never see this line


def test_figures_in_proposes_candidates():
    figs = figures_in("about 21% rose and 3.2 Hz, with 250 crowds and 21% again")
    assert "21%" in figs and "3.2" in figs and "250" in figs
    assert figs.count("21%") == 1  # deduped


def test_quality_composite_weights():
    assert abs(sum(QUALITY_WEIGHTS.values()) - 1.0) < 1e-9


def test_gauges_spark_and_ramp():
    room = empty_room("s")
    assert set(room.tables[0].gauges) == set(GAUGES)
    for v in (0.0, 0.5, 1.0):
        room.tables[0].gauges["heat"] = v
        room.tables[0].note_gauges()
    assert room.tables[0].history["heat"] == [0.0, 0.5, 1.0]
    assert len(spark(room.tables[0].history["heat"])) == 3
    assert len(bar(0.5, 10)) == 10
    cols = ramp(["#000000", "#ffffff"], 5)
    assert len(cols) == 5 and len(set(cols)) == 5


def test_belief_history_tracks_propositions():
    room = empty_room("s")
    for v in (0.3, 0.6):
        room.tables[0].judgement.beliefs = {p: v for p in PROPOSITIONS}
        room.tables[0].note_beliefs()
    for p in PROPOSITIONS:
        assert room.tables[0].belief_history[p] == [0.3, 0.6]


def test_parse_llama_json_is_forgiving():
    assert _parse('{"text": "hello there friend"}') == "hello there friend"
    assert _parse('sure! {"text": "here it is"}') == "here it is"
    assert _parse("I think the point is simply that we disagree about the premise").startswith("I think")
    assert _parse('{"text": "Byte: a short line here"}') == "a short line here"


# ----------------------------------------------------------------- the walls
def test_message_stays_inside_its_table():
    room = empty_room("s")
    for t in room.tables:
        t.messages.append(_msg(0, t.members[0], f"line from {t.name}"))
    assert all(len(t.messages) == 1 for t in room.tables)
    assert room.messages == 5


def test_a_heard_line_is_marked_and_verbatim():
    room = empty_room("s")
    t1, t2 = room.tables[0], room.tables[1]
    src = _msg(0, t1.members[0], "a striking line from the first table")
    t1.messages.append(src)
    heard = _msg(0, src.llama, src.text, heard=t1.id)
    t2.messages.append(heard)
    assert heard.heard and heard.heard_from == t1.id
    assert heard.text == src.text and heard.llama == src.llama  # verbatim, on purpose


def test_leak_record_shape():
    room = empty_room("s")
    room.leaks.append(Leak(round=3, src=1, dst=4, line="something striking", speaker="byte",
                           why="carried at 0.71", effect="changes what they discuss"))
    assert room.table(1) is not None and room.table(4) is not None
    assert room.messages == 0  # a leak alone adds no table lines


def test_reset_clears_tables_but_keeps_llamas_and_seating():
    room = empty_room("first")
    for t in room.tables:
        t.messages.append(_msg(0, t.members[0], "something said here"))
        room.llamas[t.members[0]].spoken = 1
    room.leaks.append(Leak(round=1, src=1, dst=2, line="x", speaker="byte"))
    room.cost_jev = room.cost_llama = 0.01
    room.reset("second")
    assert room.seed == "second"
    assert all(t.messages == [] for t in room.tables)
    assert room.leaks == [] and room.cost == 0.0
    assert len(room.tables) == 5 and all(len(t.members) == 4 for t in room.tables)
    assert all(s.spoken == 0 for s in room.llamas.values())


# ---------------------------------------------------------------------- render
def test_overview_shows_all_five_tables_and_summaries():
    room = empty_room("is a crowd wiser than any one of us?")
    for t in room.tables:
        t.summary = f"table {t.id} is chewing on the loudness question"
        t.messages.append(_msg(0, t.members[0], f"table {t.id} says something here"))
    out = _render(room, 150, 45)
    for t in room.tables:
        assert t.name in out
        assert f"table {t.id} is chewing" in out
    assert "the five tables" in out and "the wire" in out


def test_overview_marks_a_heard_line():
    room = empty_room("s")
    room.tables[1].messages.append(_msg(0, "byte", "carried across from table one", heard=1))
    assert "⟪heard⟫" in _render(room, 150, 45)


def test_detail_zooms_into_one_table():
    room = empty_room("s")
    for t in room.tables:
        t.messages.append(_msg(0, t.members[0], f"a line that only {t.name} said"))
    room.focus = 2
    out = _render(room, 150, 45)
    assert "a line that only Table 3 said" in out       # the focused table's chat
    assert "a line that only Table 5 said" not in out   # other tables are not shown
    assert "the judge" in out


def test_heard_line_is_visually_marked_in_detail():
    room = empty_room("s")
    room.tables[2].messages.append(_msg(0, "gus", "a line that drifted in here", heard=3))
    room.focus = 2
    assert "⟪heard from table 3⟫" in _render(room, 150, 45)


def test_wire_lists_the_eavesdrops():
    room = empty_room("s")
    room.leaks.append(Leak(round=2, src=2, dst=5, line="a very striking line indeed", speaker="dexter",
                           why="carried at 0.8", effect="adds a new argument"))
    out = _render(room, 150, 45)
    assert "T2" in out and "T5" in out and "⟶" in out


def test_detail_chat_is_bottom_anchored_at_every_size():
    room = empty_room("s")
    t = room.tables[0]
    for i in range(30):
        t.messages.append(_msg(i, t.members[i % 4], f"message number {i} with several words in it"))
    t.messages[-1].text = "ZZZ_THE_VERY_NEWEST_LINE_ZZZ"
    room.focus = 0
    for w, h in [(150, 45), (110, 34), (100, 30)]:
        out = _render(room, w, h)
        assert "ZZZ_THE_VERY_NEWEST_LINE_ZZZ" in out, f"newest clipped at {w}x{h}"
        assert "message number 0" not in out, f"oldest shown at {w}x{h}"


def test_gauges_read_as_two_ended_scales():
    room = empty_room("s")
    room.focus = 0
    room.tables[0].gauges["drift"] = 0.17
    out = _render(room, 150, 45)
    assert "on seed" in out and "off seed" in out  # both ends labelled
    assert "├" in out and "•" in out               # drawn as a scale


def test_flat_mode_drops_colour():
    room = empty_room("s")
    room.focus = 0
    room.tables[0].messages.append(_msg(0, "quill", "a line with several words in it"))
    colour = _render(room, 150, 45)
    flat = _render(room, 150, 45, flat=True)
    assert "◆ THE CORRAL" in colour and "◆ THE CORRAL" in flat  # words survive


def test_help_panel_lists_the_new_keys():
    room = empty_room("s")
    out = _render(room, 150, 45, hint=True)
    for token in ["open one table", "zoom out", "new seed", "quit"]:
        assert token in out, token
