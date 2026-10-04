"""The Corral's UI (rich). Two zoom levels over five tables of four llamas.

  overview (focus = -1):  five full-width ticker rows, one per table, each with
                          its roster, its one-line summary and its newest line;
                          below them the wire -- the eavesdrop lattice.
  detail   (focus = 0..4): one table, arranged as three bands: the judge band
                          across the top, the transcript in the middle, and the
                          roster and beliefs along the bottom, with the wire
                          keeping its own full-width strip beneath.

Keys: 1-5 focus a table · o overview · Tab / . cycle · f flat · h help · q quit.

Design (every fork went to TypeSafe Jev; probabilities in README):
  * one metaphor per layer, never two in one glyph -- the *dusk parlour* supplies
    the ground and the light (palette, horizon rule, lamplight), the *loom*
    supplies only the wire (threads visibly crossing between table columns), and
    the *observatory* supplies only the speaking flare (the seat now writing
    glows and pulses).
  * identity never rests on colour alone: every llama carries a sigil + name, and
    every line carries a *move glyph* (one glyph, one variable).
  * nothing starves: every band is sized from the terminal with a hard floor, and
    the transcript is bottom-anchored so the newest line is always the visible one.
"""
from __future__ import annotations

import textwrap

from rich import box
from rich.console import Group
from rich.layout import Layout
from rich.panel import Panel
from rich.table import Table as RichTable
from rich.text import Text

from personas import BY_ID, FAMILY_COLOUR
from room import GAUGE_LABEL, GAUGES, MOVE_COLOUR, PROPOSITIONS, Room, Table

BLOCKS = " ▏▎▍▌▋▊▉█"
SPARK = "▁▂▃▄▅▆▇█"

# ------------------------------------------------------------------- the palette
# Ember dusk: a deep indigo ground lit by one warm amber. (Jev 0.80.)
INK = "#12102b"
INK2 = "#1c1940"
AMBER = "#ffb454"
EMBER = "#ff7a3d"
GOLD = "#ffe0a0"
INDIGO = "#5b6ee1"
GOOD, WARN, BAD, MUTED = "#8fd694", "#ffd75f", "#ff6b6b", "#8a86ad"
HEARD = "#c98cff"  # the eavesdrop colour
# the horizon ramp: cold indigo -> rose -> pale gold, as the room heats up
DUSK = ["#2a2b52", "#4a3a6a", "#8a4a6a", "#d06a4a", "#ffb454", "#ffe0a0"]

# one glyph, one variable: the discourse move is a glyph, the speaker is a colour.
# The move glyphs are chosen from outside the twenty sigils so a move can never be
# mistaken for a speaker.
MOVE_GLYPH = {"claim": "●", "evidence": "▹", "question": "?", "rebuttal": "⊗",
              "analogy": "≈", "concession": "✓", "tangent": "∿"}
ENDS = {"heat": ("quiet", "lively"), "consensus": ("split", "agreed"),
        "drift": ("on seed", "off seed"), "novelty": ("stale", "fresh")}
# the three belief propositions under their short, canonical names (Jev 0.46)
BELIEF_SHORT = {PROPOSITIONS[0]: "agreed", PROPOSITIONS[1]: "divided", PROPOSITIONS[2]: "novel"}
QUALITY_DIMS = ("originality", "clarity", "insight")  # fixed order for the legend
QUALITY_COLOUR = {"originality": "#ffb454", "clarity": "#6fb7e0", "insight": "#c98cff"}

_FLAT = False   # set by compose(); a flat, colourless high-contrast mode
_FRAME = 0      # set by compose(); an animating counter for the flare (never a clock)


def _hex(c: str) -> tuple[int, int, int]:
    c = c.lstrip("#")
    return int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16)


def _lerp(a: str, b: str, t: float) -> str:
    (r1, g1, b1), (r2, g2, b2) = _hex(a), _hex(b)
    return "#%02x%02x%02x" % (round(r1 + (r2 - r1) * t), round(g1 + (g2 - g1) * t), round(b1 + (b2 - b1) * t))


def ramp(stops: list[str], width: int) -> list[str]:
    if width <= 0:
        return []
    if width == 1:
        return [stops[0]]
    out, segs = [], len(stops) - 1
    for i in range(width):
        pos = i / (width - 1) * segs
        k = min(int(pos), segs - 1)
        out.append(_lerp(stops[k], stops[k + 1], pos - k))
    return out


def bar(frac: float, width: int = 10) -> str:
    full = max(0.0, min(1.0, frac)) * width
    n = int(full)
    tail = BLOCKS[int((full - n) * 8)] if n < width else ""
    return ("█" * n + tail).ljust(width)


def two_ended(v: float, width: int = 12) -> str:
    """A value as a position on a scale: '├───•────┤'."""
    w = max(4, width - 2)
    v = max(0.0, min(1.0, v))
    pos = int(round(v * (w - 1)))
    return "├" + "─" * pos + "•" + "─" * (w - 1 - pos) + "┤"


def spark(vals: list[float], lo: float = 0.0, hi: float = 1.0) -> str:
    if not vals:
        return ""
    span = (hi - lo) or 1.0
    return "".join(SPARK[min(7, max(0, int((v - lo) / span * 7.999)))] for v in vals)


def mmss(t: float) -> str:
    return f"{int(t // 60):02d}:{int(t % 60):02d}"


def shorten(text: str, n: int) -> str:
    text = " ".join(text.split())
    if n <= 1:
        return "…" if text else ""
    return text if len(text) <= n else text[: n - 1].rstrip() + "…"


def _title(bold: str, dim: str, width: int) -> str:
    """A panel title that can never be truncated mid-word: the bold part is kept
    whole and the dim subtitle is dropped, then clipped, to fit the panel width."""
    full = f"[b]{bold}[/b]"
    if dim:
        room_for = width - len(bold) - 6
        if room_for >= len(dim):
            return full + f" [dim]{dim}[/dim]"
        if room_for >= 4:
            return full + f" [dim]{shorten(dim, room_for)}[/dim]"
    return full



def horizon(width: int, heat: float = 0.5) -> Text:
    """A living dusk horizon: warm when the room is hot, cool when it is quiet."""
    if width <= 0:
        return Text("")
    if _FLAT:
        return Text("─" * width, style="dim")
    # heat picks the top of the ramp; the rule always runs cool-to-warm at its left edge
    t = Text()
    for i, c in enumerate(ramp(DUSK, width)):
        t.append("▔" if i % 2 else "━", style=c)
    return t


# ------------------------------------------------------------------------ header
def masthead(room: Room, cfg: dict, width: int) -> Panel:
    """Row 1: identity + status + metrics. Row 2: the seed and the phase."""
    inner = max(10, width - 4)
    st_col = {"discussing": AMBER, "paused": WARN, "idle": MUTED, "stopped": GOOD, "error": BAD}.get(room.status, "white")
    status = room.status.upper()

    # ---- row 1, assembled greedily so it can never wrap ----
    t = Text()
    t.append(" ◆ THE CORRAL ", style="bold reverse" if _FLAT else f"bold black on {AMBER}")
    used = len(" ◆ THE CORRAL ")
    sub = "five tables, one seed, and Jev between them"
    if inner - used > 52:  # the subtitle is the first thing to go
        t.append("  " + sub + "  ", style=MUTED)
        used += len(sub) + 4
    t.append(f" {status} ", style=f"bold {st_col}")
    used += len(status) + 2
    metrics = f"  round {room.turn}   ⏱ {mmss(room.elapsed)}   ${room.cost:.4f}   ⇄ {len(room.leaks)}"
    if inner - used > len(metrics):
        t.append(f"  round ", style=MUTED)
        t.append(f"{room.turn}", style="bold")
        t.append("   ⏱ ", style=MUTED)
        t.append(f"{mmss(room.elapsed)}", style="white")
        t.append("   $", style=MUTED)
        t.append(f"{room.cost:.4f}", style="bold")
        t.append("   ⇄ ", style=MUTED)
        t.append(f"{len(room.leaks)}", style=f"bold {HEARD}")

    body = Text()
    body.append_text(t)
    body.append("\n")
    # ---- row 2: seed + phase ----
    if room.seed:
        body.append("seed ", style=MUTED)
        body.append(f"“{shorten(room.seed, max(18, inner - 20))}”", style=f"bold {EMBER if not _FLAT else ''}")
    if room.phase and len(room.seed) + len(room.phase) + 14 < inner:
        body.append("   · ", style=MUTED)
        body.append(room.phase, style=MUTED)
    if room.last_error:
        body.append("   · ", style=MUTED)
        body.append(f"last error: {shorten(room.last_error, max(12, inner // 3))}", style=BAD)
    body.append("\n")
    body.append_text(horizon(inner, heat_of(room)))
    return Panel(body, box=box.HEAVY, border_style="white" if _FLAT else AMBER,
                 padding=(0, 1), style="" if _FLAT else f"on {INK}")


def heat_of(room: Room) -> float:
    return sum(t.gauges.get("heat", 0.5) for t in room.tables) / max(1, len(room.tables))



def tab_bar(room: Room) -> Text:
    t = Text()
    for tb in room.tables:
        on = room.focus == tb.id - 1
        style = ("bold reverse" if _FLAT else f"bold black on {tb.colour}") if on else (MUTED if not _FLAT else "")
        t.append(f" {tb.id} {tb.name} ", style=style)
        t.append(" ")
    t.append(" [ o overview ] ", style="bold reverse" if (_FLAT and room.focus < 0)
             else (f"bold black on {AMBER}" if room.focus < 0 else MUTED))
    return t


def footer(room: Room, width: int) -> Text:
    """A single recessed hint row: only the keys this view needs, greedily trimmed
    so the row can never wrap. The rest live in the help overlay."""
    keys = (["1-5 open a table", "o overview", "t seed", "s save", "h help", "q quit"]
            if room.focus < 0 else
            ["o overview", "Tab next", "1-5 switch", "space pause", "s save", "h help", "q quit"])
    view = "the five tables" if room.focus < 0 else f"table {room.focus + 1}"
    t = Text()
    t.append(f" {view} ", style="bold reverse" if _FLAT else f"bold black on {AMBER}")
    budget = max(0, width - len(view) - 4)
    for k in keys:
        seg = "  ·  " + k
        if len(seg) > budget:
            break
        t.append(seg, style="dim")
        budget -= len(seg)
    if room.status in ("stopped", "error"):
        tail = "   ·  the tables have closed"
        if len(tail) <= budget:
            t.append(tail, style=GOOD if room.status == "stopped" else BAD)
    return t


# ----------------------------------------------------------------- overview: rows
def _members(room: Room, table: Table) -> Text:
    """The four seated llamas, as sigil + initial -- identity with no colour at all."""
    t = Text()
    for i, m in enumerate(table.members):
        p = BY_ID[m]
        if i:
            t.append(" ")
        t.append(p.sigil, style="" if _FLAT else f"bold {p.colour}")
        t.append(p.name[:4], style="" if _FLAT else p.colour)
    return t


def _table_row(room: Room, table: Table, width: int, depth: int) -> list[Text]:
    """One table as a ticker row. depth 0 = a single line (name · newest line);
    depth >= 1 = a header (name · roster), the summary, and up to `depth` lines."""
    inner = max(12, width - 4)
    head = Text()
    head.append(f" ◆ {table.name} ", style="bold reverse" if _FLAT else f"bold black on {table.colour}")

    if depth <= 0:  # the tiniest tier: the newest line outranks the roster
        if table.leaks_in:
            head.append(f" ⇄{table.leaks_in}", style=HEARD)
        writ = next((m for m in table.members if room.llamas[m].speaking), None)
        if writ:
            head.append(" ")
            head.append_text(flare(BY_ID[writ].sigil, BY_ID[writ].colour))
        head.append("  ")
        if table.messages:
            head.append_text(_speaker_line(room, table.messages[-1], inner, reserve=len(head.plain)))
        else:
            head.append("(just sitting down)", style=MUTED)
        return [head]

    head.append("  ")
    head.append_text(_members(room, table))
    if table.leaks_in:
        head.append("   ⇄ ", style=HEARD)
        head.append(f"heard {table.leaks_in}", style=HEARD)

    writ = next((m for m in table.members if room.llamas[m].speaking), None)
    if writ:
        head.append("   ")
        head.append_text(flare(BY_ID[writ].sigil, BY_ID[writ].colour))
        head.append(f" {BY_ID[writ].name} is writing", style="italic" if _FLAT else f"italic {AMBER}")
    elif table.messages:
        head.append(f"   {len(table.messages)} lines", style=MUTED)

    lines = [head]
    s = Text()
    s.append("   on  ", style=MUTED)
    s.append(f"“{shorten(table.summary, inner - 10)}”" if table.summary else "the seed itself",
             style="bold" if _FLAT else f"bold {EMBER}")
    lines.append(s)
    for m in table.messages[-depth:]:
        lines.append(_speaker_line(room, m, inner, prefix="   "))
    if not table.messages:
        lines.append(Text("   (just sitting down)", style=MUTED))
    return lines


def _fit_depth(avail: int) -> tuple[int, bool]:
    """How many message lines each table can show, and whether to rule between them."""
    if avail < 16:  # not enough for a summary row per table: one line each
        return 0, False
    sep = avail >= 14
    for depth in (4, 3, 2, 1):
        total = 5 * (2 + depth) + (4 if (sep and depth >= 2) else 0)
        if total <= avail:
            return depth, sep and depth >= 2
    return 1, False


def overview_rows(room: Room, width: int, avail: int) -> Panel:
    """Five peer tables as full-width rows. Nothing is ever clipped away."""
    depth, sep = _fit_depth(avail)
    blocks: list[Text] = []
    for i, t in enumerate(room.tables):
        if i and sep:
            blocks.append(Text(" " + "·" * max(0, width - 6), style="" if _FLAT else f"dim {table_edge(t)}"))
        blocks.extend(_table_row(room, t, width, depth))
    out: list[Text] = blocks[:max(1, avail - 2)]
    dropped = len(blocks) - len(out)
    if dropped > 0:
        out = [Text(f"   ▲ {dropped} rows above", style=MUTED)] + out[:max(0, avail - 3)]
    return Panel(Group(*out), title=_title("the five tables", "· press 1-5 to open one", width - 4),
                 title_align="left", border_style="white" if _FLAT else "#8a6a4a",
                 box=box.ROUNDED, padding=(0, 1))


def table_edge(t: Table) -> str:
    return t.colour


# ------------------------------------------------------------------- the wire
def _thread(src: int, dst: int, lw: int) -> str:
    """One eavesdrop as a thread: a ● at the source column, a ▸ at the destination,
    dashes across the gap and a ┼ wherever it crosses another table's column."""
    if lw < 6:
        return ""
    canvas = [" "] * lw
    centres = [int((i + 0.5) * lw / 5) for i in range(5)]
    a, b = centres[src - 1], centres[dst - 1]
    for x in centres:
        canvas[x] = "│"
    lo, hi = min(a, b), max(a, b)
    for x in range(lo + 1, hi):
        canvas[x] = "┼" if canvas[x] == "│" else "─"
    canvas[a] = "●"
    canvas[b] = "▸"
    return "".join(canvas)


def _col_labels(lw: int) -> str:
    canvas = [" "] * max(1, lw)
    for i in range(5):
        c = int((i + 0.5) * lw / 5)
        lab = f"T{i + 1}"
        x = max(0, c - 1)
        for k, ch in enumerate(lab):
            if x + k < len(canvas):
                canvas[x + k] = ch
    return "".join(canvas)


def _wire_title(room: Room, width: int) -> str:
    if room.leaks:
        return _title("the wire", f"· {len(room.leaks)} eavesdrops, one-way", width)
    return _title("the wire", "· eavesdrops between tables", width)


def wire_panel(room: Room, width: int, avail: int) -> Panel:
    """The eavesdrop lattice: threads crossing between the five table columns."""
    title = _wire_title(room, width - 4)
    if not room.leaks:
        return Panel(Text("no one has overheard anyone yet", style=MUTED), title=title, title_align="left",
                     border_style="white" if _FLAT else HEARD, box=box.ROUNDED, padding=(0, 1))

    cap = max(20, width - 4)          # every row must fit inside this, or it wraps
    lw = max(10, min(cap * 3 // 5, 56))
    content = max(1, avail - 2)       # rows inside the border

    all_lines: list[Text] = []
    labels = Text(_col_labels(lw), style=MUTED)
    for tb in room.tables:
        x = int((tb.id - 0.5) * lw / 5)
        labels.stylize("" if _FLAT else f"bold {tb.colour}", max(0, x - 1), min(lw, x + 1))
    all_lines.append(labels)

    # the newest carried line gets a whole row of its own; older eavesdrops fill
    # whatever rows remain, and the whole list is sliced to the panel so nothing
    # ever overflows its border.
    quote_row = content >= 3
    thread_rows = max(1, content - 1 - (1 if quote_row else 0))
    for lk in room.leaks[-thread_rows:]:
        src_t, dst_t = room.table(lk.src), room.table(lk.dst)
        name = BY_ID[lk.speaker].name
        fixed = lw + 2 + len(f"T{lk.src}") + 3 + len(f"T{lk.dst}") + len(f"  round {lk.round}  ") + len(name)
        room_left = cap - fixed - 2
        row = Text()
        row.append(_thread(lk.src, lk.dst, lw), style="" if _FLAT else f"bold {HEARD}")
        row.append("  ")
        row.append(f"T{lk.src}", style="" if _FLAT or src_t is None else src_t.colour)
        row.append(" ⟶ ", style="bold" if _FLAT else HEARD)
        row.append(f"T{lk.dst}", style="" if _FLAT or dst_t is None else dst_t.colour)
        row.append(f"  round {lk.round}  ", style=MUTED)
        row.append(name, style=MUTED)
        if room_left > 8:
            row.append(f"  “{shorten(lk.line, room_left - 3)}”", style="white")
        all_lines.append(row)

    if quote_row:
        lk = room.leaks[-1]
        head = f"the newest line to cross: {BY_ID[lk.speaker].name} at table {lk.src} → table {lk.dst} — "
        tail = Text("   ↳ ", style=MUTED)
        tail.append("the newest line to cross: ", style=MUTED)
        tail.append(f"{BY_ID[lk.speaker].name} at table {lk.src} → table {lk.dst} — ", style="" if _FLAT else HEARD)
        tail.append(f"“{shorten(lk.line, cap - len(head) - 6)}”", style="white")
        all_lines.append(tail)

    return Panel(Group(*all_lines[:content]), title=_wire_title(room, width - 4), title_align="left",
                 border_style="white" if _FLAT else HEARD, box=box.ROUNDED, padding=(0, 1))


# ----------------------------------------------------------------------- detail
def flare(sigil: str, colour: str) -> Text:
    """The speaking seat: a small star that pulses. Motion rides on the frame
    counter, never on the wall clock, so the render stays pure and testable."""
    t = Text()
    t.append(sigil, style="" if _FLAT else f"bold {colour}")
    if _FLAT:
        t.append("✦")
        return t
    glyph = ("✦", "✧", "✦", "⋆")[_FRAME % 4]
    shade = (AMBER, GOLD, AMBER, EMBER)[_FRAME % 4]
    t.append(glyph, style=f"bold {shade}")
    return t


def _speaker_line(room: Room, m, width: int, prefix: str = "", reserve: int = 0) -> Text:
    """One message as a single rich line: sigil, name, move glyph, the text.

    `reserve` is the columns already spent on this row by whatever comes before us,
    so the row can never wrap."""
    p = m.persona
    t = Text(prefix)
    if m.heard:
        t.append("⟪heard⟫ ", style=HEARD)
    t.append(p.sigil, style="" if _FLAT else f"bold {p.colour}")
    t.append(" ")
    t.append(p.name, style="bold" if _FLAT else f"bold {p.colour}")
    if m.move:
        t.append(f" {MOVE_GLYPH.get(m.move, '·')}", style="" if _FLAT else MOVE_COLOUR.get(m.move, MUTED))
    if m.composite >= 0.6:
        t.append(" ★", style="" if _FLAT else GOLD)
    if m.figure:
        t.append(f" ⛭{m.figure}", style=MUTED)
    head_w = len(t.plain)
    t.append("  ")
    t.append(shorten(m.text, max(8, width - reserve - head_w - 2)), style="white")
    return t


def _detail_blocks(room: Room, table: Table, width: int) -> list[list[Text]]:
    blocks: list[list[Text]] = []
    for i, m in enumerate(table.messages):
        p = m.persona
        head = Text()
        if m.heard:
            head.append(f"⟪heard from table {m.heard_from}⟫ ", style=HEARD)
        head.append(p.sigil, style="" if _FLAT else f"bold {p.colour}")
        head.append(" ")
        head.append(p.name, style="bold" if _FLAT else f"bold {p.colour}")
        if m.move:
            head.append(f"  {MOVE_GLYPH.get(m.move, '·')} {m.move}", style="" if _FLAT else MOVE_COLOUR.get(m.move, MUTED))
        if m.composite >= 0.6:
            head.append("  ★", style="" if _FLAT else GOLD)
        if m.figure:
            head.append(f"  ⛭{m.figure}", style=MUTED)
        if m.reply_to is not None and 0 <= m.reply_to < len(table.messages) and m.reply_to != i:
            src = table.messages[m.reply_to]
            tag = f"table {src.heard_from}" if src.heard else src.persona.name
            head.append(f"   ▸ answering {tag}: “{shorten(src.text, max(10, width // 4))}”",
                        style=HEARD if src.heard else MUTED)
        body = [Text("   " + ln, style="white") for ln in (textwrap.wrap(m.text, max(18, width - 7)) or [""])]
        blocks.append([head, *body, Text("")])
    return blocks


def detail_chat(room: Room, table: Table, width: int, avail: int) -> Panel:
    """The transcript as a printed column, bottom-anchored to the newest line.
    The newest message is always placed whole or clipped, never dropped."""
    blocks = _detail_blocks(room, table, width)
    writ = next((m for m in table.members if room.llamas[m].speaking), None)
    limit = max(1, avail - (1 if writ else 0) - 1)  # one row kept for the writing line
    out: list[Text] = []
    used, dropped = 0, 0
    for idx in range(len(blocks) - 1, -1, -1):
        if out and used + len(blocks[idx]) > limit:  # always take at least the newest
            dropped = idx + 1
            break
        out = blocks[idx] + out
        used += len(blocks[idx])
    if len(out) > limit and table.messages:
        # the newest message will not fit even alone: collapse it to a single line
        # so its text is never lost, and say how much sits above it.
        m = table.messages[-1]
        out = [_speaker_line(room, m, max(18, width - 1), prefix="   ")]
        dropped = len(table.messages) - 1
    lines = ([Text(f"   ▲ {dropped} older lines above", style=MUTED)] if dropped else []) + out
    if writ:
        row = Text("   ")
        row.append_text(flare(BY_ID[writ].sigil, BY_ID[writ].colour))
        row.append(f" {BY_ID[writ].name} is writing…", style="italic" if _FLAT else f"italic {AMBER}")
        lines.append(row)
    if not lines:
        lines = [Text("   (nothing said yet)", style=MUTED)]
    return Panel(Group(*lines), title=_title(table.name, "· newest at the bottom", width - 4), title_align="left",
                 border_style="white" if _FLAT else table.colour, box=box.ROUNDED, padding=(0, 1))


def _gauge_cols(width: int) -> int:
    return 4 if width >= 96 else 2 if width >= 54 else 1


def _gauge_lines(table: Table, width: int, cols: int) -> list[Text]:
    """Four gauges: a names row over a tracks row, each track carrying both of its
    own end-words, laid out in `cols` columns."""
    cell_w = max(22, width // cols)
    track_w = max(5, cell_w - 18)
    cells: list[tuple[str, str, str, float, str]] = []
    for g in GAUGES:
        v = table.gauges.get(g, 0.5)
        lo, hi = ENDS[g]
        cells.append((GAUGE_LABEL[g], lo, hi, v, two_ended(v, track_w)))

    def colour(g: str, v: float) -> str:
        if g == "drift":
            return BAD if v > 0.6 else WARN if v > 0.35 else GOOD
        return GOOD

    lines: list[Text] = []
    for start in range(0, len(cells), cols):
        group = cells[start:start + cols]
        name_row = Text()
        track_row = Text()
        for name, lo, hi, v, track in group:
            col = "" if _FLAT else colour(name, v)
            name_row.append(f"{name:<10}", style=MUTED)
            name_row.append(f"{v:.2f}", style=MUTED)
            name_row.append(" " * max(1, cell_w - 14), style="")
            track_row.append(f"{lo:<8}", style=MUTED)
            track_row.append(track, style="bold" if _FLAT else col)
            track_row.append(f"{hi:<8}", style=MUTED)
            track_row.append(" " * max(1, cell_w - 16 - track_w), style="")
        lines.append(name_row)
        lines.append(track_row)
    return lines


def _quality_line(table: Table, width: int) -> Text:
    j = table.judgement
    q = j.quality or {}
    t = Text()
    t.append("quality ", style=MUTED)
    # one three-segment block bar: the three dimensions in a fixed order
    for d in QUALITY_DIMS:
        v = q.get(d, 0.0)
        seg = bar(v, 6)
        t.append(seg, style="" if _FLAT else QUALITY_COLOUR[d])
    t.append(f"  {j.composite:.2f}", style="bold" if _FLAT else f"bold {AMBER}")
    # the legend, naming the order (so no initial ever needs decoding)
    for d in QUALITY_DIMS:
        t.append("   ", style="")
        t.append("▍", style="" if _FLAT else QUALITY_COLOUR[d])
        t.append(f" {d} {q.get(d, 0.0):.2f}", style=MUTED)
    return t


def _gauge_compact(table: Table, width: int) -> list[Text]:
    """Two-word end labels with an arrow, two gauges to a row: the smallest form
    that still names both poles of every gauge."""
    per = 2 if width >= 64 else 1
    cells = [(GAUGE_LABEL[g], table.gauges.get(g, 0.5), *ENDS[g]) for g in GAUGES]
    rows: list[Text] = []
    for i in range(0, len(cells), per):
        row = Text()
        for name, v, lo, hi in cells[i:i + per]:
            col = "" if _FLAT else (BAD if name == "drift" and v > 0.6 else WARN if name == "drift" and v > 0.35 else GOOD)
            row.append(f"{name} ", style=MUTED)
            row.append(f"{v:.2f}", style="bold" if _FLAT else col)
            row.append(f"  {lo} → {hi}   ", style=MUTED)
        row.truncate(width, overflow="ellipsis")
        rows.append(row)
    return rows


def judge_band_lines(room: Room, table: Table, width: int, avail: int | None = None,
                     compact: bool = False) -> list[Text]:
    """The judge band's content. Full form when there is room, then compact gauges,
    then just the speaking row. A gauge block is never split across the clip."""
    j = table.judgement
    inner = max(12, width - 4)
    cols = _gauge_cols(inner)

    speak = Text()
    speak.append("next to speak  ", style=MUTED)
    who = BY_ID.get(j.chosen) if j.chosen in table.members else None
    if who and room.status not in ("stopped", "error"):
        speak.append(f"{who.sigil} ", style="" if _FLAT else f"bold {who.colour}")
        speak.append(who.name, style="bold" if _FLAT else f"bold {who.colour}")
        if j.pick_confidence:
            how = "explore" if "explore" in (j.reason or "") else "exploit"
            speak.append(f"  {j.pick_confidence:.2f} → {how}", style=WARN if how == "explore" else GOOD)
    else:
        speak.append("—  the tables are closed", style=MUTED)
    if j.move and len(speak.plain) + 24 < inner:
        speak.append("      last line  ", style=MUTED)
        speak.append(f"{MOVE_GLYPH.get(j.move, '·')} {j.move}", style="" if _FLAT else MOVE_COLOUR.get(j.move, "white"))
        if j.stance and len(speak.plain) + len(j.stance) + 6 < inner:
            speak.append(f", a {j.stance}", style=MUTED)
        if j.cohesion and len(speak.plain) + 14 < inner:
            speak.append(f"  ·  flows {j.cohesion:.2f}", style=MUTED)
    speak.truncate(inner, overflow="ellipsis")

    quality = _quality_line(table, inner)
    quality.truncate(inner, overflow="ellipsis")
    verdict = None
    if j.verdict:
        verdict = Text()
        verdict.append("Jev's verdict  ", style=MUTED)
        verdict.append(f"“{shorten(j.verdict, inner - 18)}”", style="bold" if _FLAT else f"bold {GOLD}")

    glines = _gauge_lines(table, inner, cols)
    for gl in glines:
        gl.truncate(inner, overflow="ellipsis")
    full = [speak, *glines, quality] + ([verdict] if verdict else [])
    cg = _gauge_compact(table, inner)
    compact_lines = [speak, *cg] + ([quality] if len(cg) <= 2 else [])

    if compact:
        body = compact_lines
    elif avail is None:
        return full
    else:
        body = full if len(full) <= max(1, avail - 2) else compact_lines
    if avail is None:
        return body
    content = max(1, avail - 2)
    if len(body) <= content:
        return body
    cand = [speak, *cg]
    if len(cand) <= content:
        return cand
    return cand[:content]


def _herd_cell(room: Room, table: Table, s) -> Text:
    p = s.persona
    chosen = s.id == table.judgement.chosen
    t = Text()
    t.append("▶" if chosen else " ", style="bold" if _FLAT else f"bold {AMBER}")
    t.append(p.sigil, style="" if _FLAT else f"bold {p.colour}")
    t.append(f" {p.name[:7]:<7}", style="bold" if (chosen or _FLAT) else p.colour)
    if s.speaking:
        t.append_text(flare("", p.colour))
    t.append(" ")
    t.append(bar(s.eagerness, 5), style="" if _FLAT else FAMILY_COLOUR[p.family])
    t.append(f" {s.spoken}×", style=MUTED)
    return t


def detail_herd(room: Room, table: Table, width: int) -> Panel:
    grid = RichTable.grid(padding=(0, 2))
    grid.add_column()
    grid.add_column()
    order = table.roster(room)
    half = (len(order) + 1) // 2
    for a, b in zip(order[:half], order[half:]):
        grid.add_row(_herd_cell(room, table, a), _herd_cell(room, table, b))
    return Panel(grid, title=_title("the roster", "· eagerness ▏ who has spoken", width), title_align="left",
                 border_style="white" if _FLAT else table.colour, box=box.ROUNDED, padding=(0, 1))


def detail_beliefs(table: Table, width: int) -> Panel:
    body = Text()
    for p in PROPOSITIONS:
        h = table.belief_history.get(p, [])
        if len(h) < 2:
            continue
        name = BELIEF_SHORT.get(p, p)
        body.append(f"{name:<9}", style="bold" if _FLAT else f"bold {INDIGO}")
        body.append(spark(h[-40:]), style="" if _FLAT else INDIGO)
        body.append(f" {h[-1]:.2f}\n", style=MUTED)
    return Panel(body if body.plain else Text("(no beliefs yet)", style=MUTED),
                 title=_title("what this table believes", "· h for the full sentences", width - 4),
                 title_align="left", border_style="white" if _FLAT else "#5a5f7a",
                 box=box.ROUNDED, padding=(0, 1))


# ------------------------------------------------------------------------ help
def help_panel() -> Panel:
    body = Text()
    body.append("Five tables of four llamas are talking at once, all about the same seed.\n", style="white")
    body.append("Sometimes one table overhears another; a carried line is marked ⟪heard⟫ "
                "and the hearing\ntable must answer it. The wire at the bottom shows which table's words reached which.\n\n", style="white")
    for k, label in [("1-5", "open one table in detail"),
                     ("o", "zoom out to the five tables"),
                     ("Tab / .", "next table"),
                     ("space", "pause / resume"),
                     ("t", "type a new seed (all five restart on it)"),
                     ("s", "save every table's chat"),
                     ("f", "flat mode (no colour at all)"),
                     ("h", "hide this help"),
                     ("q", "quit")]:
        body.append(f"  {k:<8}", style="bold" if _FLAT else f"bold {AMBER}")
        body.append(f"{label}\n", style=MUTED)
    body.append("\n")
    body.append("how to read the judge\n", style="bold" if _FLAT else f"bold {GOLD}")
    body.append("  gauges      ", style=MUTED)
    for g in GAUGES:
        body.append(f"{GAUGE_LABEL[g]} ", style=MUTED)
        body.append(f"({ENDS[g][0]} → {ENDS[g][1]})  ", style="white")
    body.append("\n  quality     ", style=MUTED)
    body.append("the bar is three segments — ", style="white")
    for d in QUALITY_DIMS:
        body.append("▍", style="" if _FLAT else QUALITY_COLOUR[d])
        body.append(f"{d} ", style=MUTED)
    body.append("— and the number after it is the composite.\n", style="white")
    body.append("  moves       ", style=MUTED)
    body.append(" ".join(f"{MOVE_GLYPH[k]} {k}" for k in MOVE_GLYPH) + "\n", style="white")
    body.append("  beliefs     ", style=MUTED)
    body.append("shorter names for the three propositions this table is tracked against:\n", style="white")
    for p in PROPOSITIONS:
        body.append(f"                 {BELIEF_SHORT[p]:<9}", style="bold" if _FLAT else f"bold {INDIGO}")
        body.append(f"{p}\n", style=MUTED)
    return Panel(body, title="[b]how to use this[/b] [dim](h to hide)[/dim]", title_align="left",
                 border_style="white" if _FLAT else INDIGO, box=box.ROUNDED, padding=(0, 1))


# ------------------------------------------------------------------------ layout
def _budgets(height: int, judge_h: int) -> dict[str, int]:
    """Rows for the detail view's bands. The judge band gets its content height
    where that fits; the transcript keeps a hard floor of four rows."""
    avail = max(6, height - 6)  # header 4, tabs 1, footer 1
    wire = 7 if avail >= 34 else 5 if avail >= 24 else 3
    band = 7 if avail >= 34 else 5 if avail >= 24 else 0
    judge = min(judge_h, max(3, avail - wire - band - 4))
    chat = avail - judge - wire - band
    while chat < 4 and band > 0:
        band -= 1
        chat += 1
    while chat < 4 and wire > 3:
        wire -= 1
        chat += 1
    return {"judge": judge, "wire": wire, "band": band, "chat": max(3, chat)}


def _judge_alloc(height: int, room: Room, table: Table, width: int) -> tuple[int, int]:
    """Two passes: prefer the full judge band, then drop to the compact form if the
    transcript would fall below its floor. Returns (judge rows, compact flag)."""
    full_h = len(judge_band_lines(room, table, width, None)) + 2
    b = _budgets(height, full_h)
    if b["chat"] >= 4:  # the transcript is the hero: prefer the full judge only if it fits
        return b["judge"], 0
    comp_h = len(judge_band_lines(room, table, width, None, compact=True)) + 2
    b2 = _budgets(height, comp_h)
    return b2["judge"], 1


def compose(room: Room, cfg: dict, size: tuple[int, int]) -> Layout:
    global _FLAT, _FRAME
    _FLAT = bool(cfg.get("flat"))
    _FRAME = int(cfg.get("frame", 0))
    hint = bool(cfg.get("hint"))
    width, height = size
    focus = room.focus if 0 <= room.focus < len(room.tables) else -1

    lay = Layout()
    lay.split_column(Layout(name="head", size=4), Layout(name="tabs", size=1),
                     Layout(name="body", ratio=1), Layout(name="foot", size=1))
    lay["head"].update(masthead(room, cfg, width))
    lay["tabs"].update(tab_bar(room))
    lay["foot"].update(footer(room, width))

    if focus < 0:  # ---------------- overview: five rows + the wire
        wire_h = min(6, max(3, height // 6))
        lay["body"].split_column(Layout(name="rows", ratio=1), Layout(name="wire", size=wire_h))
        rows_h = max(1, height - 6 - wire_h)
        lay["body"]["rows"].update(help_panel() if hint else overview_rows(room, width, rows_h))
        lay["body"]["wire"].update(wire_panel(room, width, wire_h))
        return lay

    # ---------------- detail: three bands + the wire
    table = room.tables[focus]
    judge_h, compactf = _judge_alloc(height, room, table, width)
    b = _budgets(height, judge_h)
    parts = [Layout(name="judge", size=b["judge"]), Layout(name="chat", ratio=1)]
    if b["band"] > 0:
        parts.append(Layout(name="band", size=b["band"]))
    parts.append(Layout(name="wire", size=b["wire"]))
    lay["body"].split_column(*parts)
    lines = judge_band_lines(room, table, width, b["judge"], compact=bool(compactf))
    lay["body"]["judge"].update(Panel(Group(*lines), title=_title("the judge", "· jev-1.13", width - 4),
                                      title_align="left", border_style="white" if _FLAT else INDIGO,
                                      box=box.ROUNDED, padding=(0, 1)))
    lay["body"]["chat"].update(help_panel() if hint else detail_chat(room, table, width, max(3, b["chat"] - 2)))
    if b["band"] > 0:
        half = max(34, min(width * 2 // 5, 64))
        lay["body"]["band"].split_row(Layout(name="roster", size=half), Layout(name="beliefs", ratio=1))
        lay["body"]["band"]["roster"].update(detail_herd(room, table, half - 4))
        lay["body"]["band"]["beliefs"].update(detail_beliefs(table, width - half - 4))
    lay["body"]["wire"].update(wire_panel(room, width, b["wire"]))
    return lay
