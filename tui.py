"""The Corral's UI (rich). Two zoom levels over five tables of four llamas.

  overview (focus = -1):  five table cards, each with its one-line summary, its
                          latest lines, and who is speaking; below them the
                          eavesdrop map showing which table heard which.
  detail   (focus = 0..4): one table's full chat, its judge panel and its table.

Keys: 1-5 focus a table · o overview · Tab / . cycle · f flat · h help · q quit.

Identity never rests on colour alone: every llama carries a sigil + name.
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
AMBER = "#ffb454"
INDIGO = "#5b6ee1"
EMBER = "#ff7a3d"
GOOD, WARN, BAD, MUTED = "#8fd694", "#ffd75f", "#ff6b6b", "#7a7f9a"
HEARD = "#c98cff"  # the eavesdrop colour
DUSK_WARM = ["#3a3f7a", "#6a4a86", "#b0605f", "#e08a4a", "#ffc46b"]
DUSK_COOL = ["#2b2f52", "#343a55", "#3d4260", "#4a4f68", "#575c72"]
ENDS = {"heat": ("quiet", "lively"), "consensus": ("split", "agreed"),
        "drift": ("on seed", "off seed"), "novelty": ("stale", "fresh")}

_FLAT = False  # set by compose(); a flat, colourless high-contrast mode


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
    return text if len(text) <= n else text[: n - 1].rstrip() + "…"


def horizon(width: int, heat: float = 0.5) -> Text:
    """A living dusk horizon: warm when the room is hot, cool when it is quiet."""
    if width <= 0:
        return Text("")
    if _FLAT:
        return Text("─" * width, style="dim")
    stops = [_lerp(c1, c2, max(0.0, min(1.0, heat))) for c1, c2 in zip(DUSK_COOL, DUSK_WARM)]
    t = Text()
    for i, c in enumerate(ramp(stops, width)):
        t.append("▔" if i % 2 else "━", style=c)
    return t


# ------------------------------------------------------------------------ header
def header(room: Room, cfg: dict, width: int) -> Panel:
    st_col = {"discussing": AMBER, "paused": WARN, "idle": MUTED, "stopped": GOOD, "error": BAD}.get(room.status, "white")
    heat = sum(t.gauges.get("heat", 0.5) for t in room.tables) / max(1, len(room.tables))
    t = Text()
    t.append("◆ THE CORRAL ", style="bold reverse" if _FLAT else f"bold black on {AMBER}")
    t.append("  five tables of four llamas, refereed by Jev  ", style=MUTED)
    t.append(f" {room.status.upper()} ", style=f"bold {st_col}")
    t.append(f"  round {room.turn}", style="bold")
    t.append(f"  ⏱ {mmss(room.elapsed)}", style="white")
    t.append("  $", style=MUTED)
    t.append(f"{room.cost:.4f}", style="bold")
    t.append(f"  eavesdrops {len(room.leaks)}", style=HEARD)
    body = Text()
    body.append_text(t)
    body.append("\n")
    if room.seed:
        body.append("seed ", style=MUTED)
        body.append(f"“{shorten(room.seed, max(20, width - 26))}”", style=f"bold {EMBER}")
    if room.phase:
        body.append("   · ", style=MUTED)
        body.append(room.phase, style=MUTED)
    if room.last_error:
        body.append("\n")
        body.append(f"last error: {room.last_error}", style=BAD)
    body.append("\n")
    body.append_text(horizon(width - 4, heat))
    return Panel(body, box=box.HEAVY, border_style="white" if _FLAT else AMBER, padding=(0, 1))


def tab_bar(room: Room) -> Text:
    t = Text()
    for tb in room.tables:
        on = room.focus == tb.id - 1
        style = ("bold reverse" if _FLAT else f"bold black on {tb.colour}") if on else (MUTED if not _FLAT else "")
        t.append(f" {tb.id} {tb.name} ", style=style)
        t.append(" ")
    t.append(" [ o overview ] ", style="bold reverse" if (_FLAT and room.focus < 0) else (f"bold black on {AMBER}" if room.focus < 0 else MUTED))
    return t


def footer(room: Room, cfg: dict) -> Text:
    t = Text()
    view = "overview" if room.focus < 0 else f"table {room.focus + 1}"
    t.append(f" {view} ", style="bold reverse" if _FLAT else f"bold black on {AMBER}")
    for k, label in [("1-5", "table"), ("o", "overview"), ("t", "seed"), ("s", "save"), ("f", "flat"), ("h", "help"), ("q", "quit")]:
        t.append(f" {k} ", style="bold reverse" if _FLAT else f"bold black on {AMBER}")
        t.append(f" {label} ", style=MUTED)
    if room.status in ("stopped", "error"):
        t.append("  ·  the tables have closed", style=GOOD if room.status == "stopped" else BAD)
    return t


# -------------------------------------------------------------------- overview
def _card_body(room: Room, table: Table, width: int) -> Text:
    body = Text()
    who = " ".join(f"{BY_ID[m].sigil}{BY_ID[m].name[:4]}" for m in table.members)
    body.append("seated: ", style=MUTED)
    body.append(who + "\n", style="" if _FLAT else table.colour)
    if table.summary:
        body.append("on: ", style=MUTED)
        body.append(f"“{shorten(table.summary, max(16, width - 10))}”\n", style="bold" if _FLAT else f"bold {EMBER}")
    writ = next((m for m in table.members if room.llamas[m].speaking), None)
    if writ:
        body.append(f"✎ {BY_ID[writ].name} is writing…\n", style="italic" if _FLAT else f"italic {EMBER}")
    for m in table.messages[-2:]:
        p = m.persona
        if m.heard:
            body.append("⟪heard⟫ ", style=HEARD)
        body.append(f"{p.sigil} ", style="" if _FLAT else p.colour)
        body.append(f"{p.name}: ", style="bold" if _FLAT else f"bold {p.colour}")
        body.append(f"{shorten(m.text, max(14, width - 22))}\n", style="white")
    if not table.messages:
        body.append("(just sitting down)\n", style=MUTED)
    return body


def table_card(room: Room, table: Table, width: int) -> Panel:
    title = f"[b]{table.name}[/b] [dim]({len(table.messages)} lines"
    if table.leaks_in:
        title += f" · heard {table.leaks_in}"
    title += ")[/dim]"
    return Panel(_card_body(room, table, width), title=title, title_align="left",
                 border_style="white" if _FLAT else table.colour, box=box.ROUNDED, padding=(0, 1))


def overview(room: Room, width: int, height: int) -> Panel:
    grid = RichTable.grid(expand=True, padding=(0, 1))
    grid.add_column(ratio=1)
    grid.add_column(ratio=1)
    cards = [table_card(room, t, (width // 2) - 4) for t in room.tables]
    for i in range(0, len(cards), 2):
        pair = cards[i:i + 2]
        if len(pair) == 1:
            pair.append(Text(""))  # type: ignore[arg-type]
        grid.add_row(*pair)
    return Panel(grid, title="[b]the five tables[/b] [dim]· press 1-5 to open one[/dim]",
                 title_align="left", border_style="white" if _FLAT else "#8a6a4a", box=box.ROUNDED, padding=(0, 0))


def leak_map(room: Room, width: int) -> Panel:
    """The visual for the eavesdrops: which table's words reached which."""
    if not room.leaks:
        return Panel(Text("no one has overheard anyone yet", style=MUTED),
                     title="[b]the wire[/b] [dim]· eavesdrops between tables[/dim]", title_align="left",
                     border_style="white" if _FLAT else HEARD, box=box.ROUNDED, padding=(0, 1))
    body = Text()
    for lk in room.leaks[-4:]:
        src_t, dst_t = room.table(lk.src), room.table(lk.dst)
        body.append(f"T{lk.src}", style="" if _FLAT or src_t is None else src_t.colour)
        body.append("  ⟶  ", style="bold" if _FLAT else HEARD)
        body.append(f"T{lk.dst}", style="" if _FLAT or dst_t is None else dst_t.colour)
        body.append(f"   round {lk.round}   ", style=MUTED)
        body.append(f"{BY_ID[lk.speaker].name}: ", style=MUTED)
        body.append(f"“{shorten(lk.line, max(16, width - 40))}”\n", style="white")
    return Panel(body, title=f"[b]the wire[/b] [dim]· {len(room.leaks)} eavesdrops, one-way[/dim]",
                 title_align="left", border_style="white" if _FLAT else HEARD, box=box.ROUNDED, padding=(0, 1))


# ---------------------------------------------------------------------- detail
def _detail_lines(room: Room, table: Table, width: int) -> list[list[Text]]:
    blocks: list[list[Text]] = []
    for i, m in enumerate(table.messages):
        p = m.persona
        head = Text()
        if m.heard:
            head.append(f"⟪heard from table {m.heard_from}⟫ ", style=HEARD)
        head.append(f"{p.sigil} ", style="" if _FLAT else p.colour)
        head.append(p.name, style="bold" if _FLAT else f"bold {p.colour}")
        if m.move:
            head.append(f" [{m.move}]", style="" if _FLAT else MOVE_COLOUR.get(m.move, MUTED))
        if m.composite >= 0.6:
            head.append(" ★", style="" if _FLAT else "#ffd75f")
        if m.figure:
            head.append(f" ⛭{m.figure}", style=MUTED)
        if m.reply_to is not None and 0 <= m.reply_to < len(table.messages) and m.reply_to != i:
            src = table.messages[m.reply_to]
            tag = f"table {src.heard_from}" if src.heard else src.persona.name
            head.append(f"   ▸ answering {tag}: “{shorten(src.text, max(10, width // 4))}”",
                        style=HEARD if src.heard else MUTED)
        body = [Text("  " + ln, style="white") for ln in (textwrap.wrap(m.text, max(20, width - 4)) or [""])]
        blocks.append([head, *body, Text("")])
    return blocks


def detail_chat(room: Room, table: Table, width: int, avail: int) -> Panel:
    blocks = _detail_lines(room, table, width)
    out: list[Text] = []
    used, dropped = 0, 0
    for idx in range(len(blocks) - 1, -1, -1):
        if used + len(blocks[idx]) > max(1, avail - 1):
            dropped = idx + 1
            break
        out = blocks[idx] + out
        used += len(blocks[idx])
    lines = ([Text(f"   ▲ {dropped} older lines above", style=MUTED)] if dropped else []) + out
    writ = next((m for m in table.members if room.llamas[m].speaking), None)
    if writ:
        lines.append(Text(f"   ✎ {BY_ID[writ].name} is writing…", style="italic" if _FLAT else f"italic {EMBER}"))
    if not lines:
        lines = [Text("(nothing said yet)", style=MUTED)]
    return Panel(Group(*lines), title=f"[b]{table.name}[/b] [dim](newest at the bottom · each reply answers the line above)[/dim]",
                 title_align="left", border_style="white" if _FLAT else table.colour, box=box.ROUNDED, padding=(0, 1))


def detail_judge(room: Room, table: Table, width: int) -> Panel:
    j = table.judgement
    body = Text()
    who = BY_ID.get(j.chosen) if j.chosen in table.members else None
    body.append("next to speak  ", style=MUTED)
    if who and room.status not in ("stopped", "error"):
        body.append(f"{who.sigil} {who.name}", style="bold" if _FLAT else f"bold {who.colour}")
        if j.pick_confidence:
            how = "explore" if "explore" in (j.reason or "") else "exploit"
            body.append(f"  {j.pick_confidence:.2f} → {how}", style=WARN if how == "explore" else GOOD)
    else:
        body.append("—  the tables are closed", style=MUTED)
    if j.move:
        body.append("\nlast line      ", style=MUTED)
        body.append(j.move, style="" if _FLAT else MOVE_COLOUR.get(j.move, "white"))
        if j.stance:
            body.append(f", a {j.stance}", style=MUTED)
        if j.cohesion:
            body.append(f"   ·   flows {j.cohesion:.2f}", style=MUTED)
    if j.quality:
        body.append("\nquality        ", style=MUTED)
        for d in ("originality", "clarity", "insight"):
            if d in j.quality:
                body.append(f"{d[0].upper()}{j.quality[d]:.2f} ", style=MUTED)
        body.append(f"→ {j.composite:.2f}", style="bold" if _FLAT else f"bold {AMBER}")
    body.append("\n")
    track = max(8, min(14, max(10, width - 20)))
    for g in GAUGES:
        v = table.gauges.get(g, 0.5)
        col = "bold" if _FLAT else (BAD if g == "drift" and v > 0.6 else WARN if g == "drift" and v > 0.35 else GOOD)
        body.append(f"{GAUGE_LABEL[g]:<10}", style=MUTED)
        body.append(two_ended(v, track), style=col)
        body.append(f" {v:4.2f}\n", style=MUTED)
    body.append("\n◀ " + " · ".join(ENDS[g][0] for g in GAUGES) + "\n", style=MUTED)
    body.append("  " + " · ".join(ENDS[g][1] for g in GAUGES) + " ▶", style=MUTED)
    if j.verdict:
        body.append(f"\nJev's verdict: {j.verdict}", style="bold" if _FLAT else f"bold {AMBER}")
    return Panel(body, title=f"[b]the judge[/b] [dim]· {table.name} · jev-1.13[/dim]", title_align="left",
                 border_style="white" if _FLAT else INDIGO, box=box.ROUNDED, padding=(0, 1))


def _herd_cell(room: Room, table: Table, s) -> Text:
    p = s.persona
    chosen = s.id == table.judgement.chosen
    t = Text()
    t.append("▶" if chosen else " ", style="bold" if _FLAT else f"bold {AMBER}")
    t.append(p.sigil, style="" if _FLAT else f"bold {p.colour}")
    t.append(f" {p.name[:7]:<7}", style="bold" if (chosen or _FLAT) else ("" if _FLAT else p.colour))
    t.append(bar(s.eagerness, 5), style="" if _FLAT else FAMILY_COLOUR[p.family])
    t.append(f" {s.spoken}×", style=MUTED)
    if s.speaking:
        t.append(" ✎", style=EMBER if not _FLAT else "")
    return t


def detail_herd(room: Room, table: Table) -> Panel:
    grid = RichTable.grid(padding=(0, 2))
    grid.add_column()
    grid.add_column()
    order = table.roster(room)
    half = (len(order) + 1) // 2
    for a, b in zip(order[:half], order[half:]):
        grid.add_row(_herd_cell(room, table, a), _herd_cell(room, table, b))
    return Panel(grid, title=f"[b]the table[/b] [dim]· {len(table.members)} llamas[/dim]", title_align="left",
                 border_style="white" if _FLAT else table.colour, box=box.ROUNDED, padding=(0, 1))


def detail_beliefs(table: Table, width: int) -> Panel:
    body = Text()
    for p in PROPOSITIONS:
        h = table.belief_history.get(p, [])
        if len(h) < 2:
            continue
        body.append(shorten(p, max(14, width // 2)).ljust(max(14, width // 2)) + " ", style=MUTED)
        body.append(spark(h[-40:]), style="" if _FLAT else INDIGO)
        body.append(f" {h[-1]:.2f}\n", style=MUTED)
    return Panel(body if body.plain else Text("(no beliefs yet)", style=MUTED),
                 title="[b]what this table believes[/b]", title_align="left",
                 border_style="white" if _FLAT else "#5a5f7a", box=box.ROUNDED, padding=(0, 1))


# ------------------------------------------------------------------------ help
def help_panel() -> Panel:
    body = Text()
    body.append("Five tables of four llamas are talking at once, all about the same seed.\n", style="white")
    body.append("Sometimes one table overhears another; the line that drifts over is marked ⟪heard⟫.\n\n", style="white")
    for k, label in [("1-5", "open one table in detail"),
                     ("o", "zoom out to all five"),
                     ("Tab/.", "next table"),
                     ("t", "type a new seed (all five restart on it)"),
                     ("space", "pause / resume"),
                     ("s", "save every table's chat"),
                     ("f", "flat mode (no colour)"),
                     ("q", "quit")]:
        body.append(f"  {k:<7}", style="bold" if _FLAT else f"bold {AMBER}")
        body.append(f"{label}\n", style=MUTED)
    return Panel(body, title="[b]how to use this[/b] [dim](h to hide)[/dim]", title_align="left",
                 border_style="white" if _FLAT else INDIGO, box=box.ROUNDED, padding=(0, 1))


# ------------------------------------------------------------------------ layout
def compose(room: Room, cfg: dict, size: tuple[int, int]) -> Layout:
    global _FLAT
    _FLAT = bool(cfg.get("flat"))
    hint = bool(cfg.get("hint"))
    width, height = size
    lay = Layout()
    focus = room.focus if 0 <= room.focus < len(room.tables) else -1

    if focus < 0:  # ---------------- overview: five cards + the wire
        wire_h = min(8, max(3, height // 5))
        lay.split_column(Layout(name="header", size=4), Layout(name="tabs", size=1),
                         Layout(name="cards", ratio=1), Layout(name="wire", size=wire_h),
                         Layout(name="footer", size=1))
        lay["header"].update(header(room, cfg, width))
        lay["tabs"].update(tab_bar(room))
        lay["cards"].update(help_panel() if hint else overview(room, width, height))
        lay["wire"].update(leak_map(room, width))
        lay["footer"].update(footer(room, cfg))
        return lay

    # ---------------- detail: one table
    table = room.tables[focus]
    side = min(44, max(34, width // 3))
    wire_h = min(6, max(3, height // 8))
    lay.split_column(Layout(name="header", size=4), Layout(name="tabs", size=1),
                     Layout(name="mid", ratio=1), Layout(name="wire", size=wire_h),
                     Layout(name="footer", size=1))
    mid = height - 4 - 1 - wire_h - 1
    lay["mid"].split_row(Layout(name="chat", ratio=1), Layout(name="side", size=side))
    lay["side"].split_column(Layout(name="herd", size=6), Layout(name="judge", ratio=1), Layout(name="beliefs", size=6))
    lay["header"].update(header(room, cfg, width))
    lay["tabs"].update(tab_bar(room))
    lay["mid"]["chat"].update(help_panel() if hint else detail_chat(room, table, width - side - 6, max(4, mid - 2)))
    lay["mid"]["side"]["herd"].update(detail_herd(room, table))
    lay["mid"]["side"]["judge"].update(detail_judge(room, table, side - 4))
    lay["mid"]["side"]["beliefs"].update(detail_beliefs(table, side - 4))
    lay["wire"].update(leak_map(room, width))
    lay["footer"].update(footer(room, cfg))
    return lay
