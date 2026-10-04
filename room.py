"""The Corral's state: five tables of four llamas, each a continuous conversation,
plus the eavesdrops between them.

Pure data and derived metrics, no network. The TUI renders this; the session
writes it. Everything lives on one asyncio event loop, so reads from the renderer
never race writes from the session.

Design, all from Jev judgements:
  * five tables of four (Jev: 20 agents in one conversation is too many to follow);
  * every reply answers the line immediately above (cohesion, Jev 0.94);
  * a one-way eavesdrop is a *judged event* (``judged_leak_quote``, 0.80): a line
    striking enough to carry to another table lands in that table's transcript,
    marked, and its next reply answers it;
  * each table's one-sentence summary is the single most representative line,
    picked by Jev (``jev_representative_line``, 0.67) -- no prose generation.
"""
from __future__ import annotations

import re
import time
from dataclasses import dataclass, field

from personas import BY_ID, LLAMAS, Persona

GAUGES = ("heat", "consensus", "drift", "novelty")
GAUGE_LABEL = {"heat": "heat", "consensus": "consensus", "drift": "drift", "novelty": "novelty"}

MOVES = ("claim", "evidence", "question", "rebuttal", "analogy", "concession", "tangent")
MOVE_COLOUR = {
    "claim": "#ffb454", "evidence": "#6fb7e0", "question": "#c98cff", "rebuttal": "#ff7a90",
    "analogy": "#e8c07a", "concession": "#8fd694", "tangent": "#7a7f9a",
}
STANCES = ("support", "challenge", "build", "deflect")
QUALITY_DIMS = ("originality", "clarity", "insight")
QUALITY_WEIGHTS = {"originality": 0.4, "clarity": 0.2, "insight": 0.4}  # composite scoring, weights in code

PROPOSITIONS = (
    "this table now agrees on a single answer to the seed",
    "this table has become more divided over the last few messages",
    "this table's latest thinking would be new to someone who read only the seed",
)

# Which four llamas sit at each table. Hand-spread across voice families so no
# table is four of a kind (analytical / narrative / systems / other).
TABLE_SETS: tuple[tuple[int, tuple[str, ...]], ...] = (
    (1, ("byte", "gus", "juniper", "kestrel")),
    (2, ("dexter", "nimbus", "cosmo", "orson")),
    (3, ("vera", "marisol", "onyx", "paloma")),
    (4, ("bram", "sable", "fern", "tilly")),
    (5, ("quill", "pip", "wren", "ada")),
)
TABLE_COLOURS = ("#6fb7e0", "#e8c07a", "#c98cff", "#8fd694", "#ff9a6b")

_WORD = re.compile(r"[a-z0-9']+")
_FIGURE = re.compile(r"\d+(?:[.,]\d+)?\s?(?:%|percent|x|times|million|billion|thousand)?", re.I)


def figures_in(text: str) -> list[str]:
    """Candidate figures for Jev to choose among (regex proposes, Jev selects)."""
    out, seen = [], set()
    for m in _FIGURE.finditer(text):
        s = m.group(0).strip()
        if s and s.lower() not in seen:
            seen.add(s.lower())
            out.append(s)
    return out[:8]


def words(text: str) -> list[str]:
    return _WORD.findall(text.lower())


def jaccard(a: set[str], b: set[str]) -> float:
    if not a or not b:
        return 0.0
    return len(a & b) / len(a | b)


@dataclass
class Message:
    turn: int
    llama: str  # persona id (the source speaker, even for a heard line)
    text: str
    ts: float = field(default_factory=time.time)
    heard_from: int | None = None  # if set, this line was overheard from table N
    family: str = ""
    reply_to: int | None = None  # list index within its table
    move: str = ""
    quality: dict[str, float] = field(default_factory=dict)
    composite: float = 0.0
    figure: str = ""
    asserts_claim: bool = False
    stance: str = ""
    cohesion: float = 0.0

    @property
    def persona(self) -> Persona:
        return BY_ID[self.llama]

    @property
    def heard(self) -> bool:
        return self.heard_from is not None


@dataclass
class LlamaState:
    id: str
    spoken: int = 0
    eagerness: float = 0.0
    last_spoke_turn: int = -1
    speaking: bool = False

    @property
    def persona(self) -> Persona:
        return BY_ID[self.id]


@dataclass
class Judgement:
    """One turn's judgement for a table."""
    chosen: str = ""
    fit: float = 0.0
    fit_map: dict[str, float] = field(default_factory=dict)
    pick_confidence: float = 0.0
    reply_to: int | None = None
    reason: str = ""
    family: str = ""
    move: str = ""
    stance: str = ""
    cohesion: float = 0.0
    quality: dict[str, float] = field(default_factory=dict)
    composite: float = 0.0
    figure: str = ""
    asserts_claim: bool = False
    gauges: dict[str, float] = field(default_factory=dict)
    thread: str = ""
    tactic: str = ""
    tactic_confidence: float = 0.0
    summary: str = ""  # the representative line Jev picked for the overview
    summary_from: str = ""
    beliefs: dict[str, float] = field(default_factory=dict)
    verdict: str = ""
    calls_last: int = 0


@dataclass
class Leak:
    """A one-way eavesdrop: table ``src`` was overheard by table ``dst``."""
    round: int
    src: int
    dst: int
    line: str
    speaker: str  # persona id of whoever said the overheard line
    why: str = ""       # Jev's label for why it carried
    effect: str = ""    # Jev's label for how it should land
    landed: bool = False  # has the hearing table replied to it yet


@dataclass
class Table:
    id: int
    members: tuple[str, ...]  # four llama ids, fixed
    colour: str = "#6fb7e0"
    messages: list[Message] = field(default_factory=list)
    gauges: dict[str, float] = field(default_factory=lambda: {g: 0.5 for g in GAUGES})
    judgement: Judgement = field(default_factory=Judgement)
    thread: str = ""
    threads: list[str] = field(default_factory=list)
    summary: str = ""
    summary_from: str = ""
    history: dict[str, list[float]] = field(default_factory=lambda: {g: [] for g in GAUGES})
    belief_history: dict[str, list[float]] = field(default_factory=lambda: {p: [] for p in PROPOSITIONS})
    leaks_in: int = 0  # how many eavesdrops this table has heard

    @property
    def name(self) -> str:
        return f"Table {self.id}"

    def member_states(self, room: "Room") -> list[LlamaState]:
        return [room.llamas[m] for m in self.members]

    def tail_text(self, room: "Room", n: int = 16) -> str:
        out = []
        for m in self.messages[-n:]:
            if m.heard:
                out.append(f"(heard from table {m.heard_from}) {m.persona.name}: {m.text}")
            else:
                out.append(f"{m.persona.name}: {m.text}")
        return "\n".join(out)

    def thread_choices(self) -> dict[str, str]:
        out: dict[str, str] = {"seed": "the shared seed itself"}
        for i, t in enumerate(self.threads[-4:]):
            out[f"th{i}"] = t
        out["new"] = "a genuinely new sub-question this table has not asked yet"
        return out

    def set_thread(self, thread: str) -> None:
        if not thread:
            return
        if thread.startswith("seed"):
            self.thread = ""
            return
        if thread.startswith("th") and thread[2:].isdigit():
            i = int(thread[2:])
            if 0 <= i < len(self.threads):
                self.thread = self.threads[i]
            return
        if self.messages:
            sub = self.messages[-1].text.strip()
            sub = (sub[:72] + "…") if len(sub) > 72 else sub
            self.thread = f"whether {sub.rstrip('.').lower()}" if sub else ""
            if self.thread:
                self.threads.append(self.thread)
                del self.threads[:-4]

    def note_gauges(self) -> None:
        for g in GAUGES:
            h = self.history.setdefault(g, [])
            h.append(round(self.gauges.get(g, 0.5), 3))
            if len(h) > 120:
                del h[:-120]

    def note_beliefs(self) -> None:
        for p in PROPOSITIONS:
            h = self.belief_history.setdefault(p, [])
            h.append(round(self.judgement.beliefs.get(p, 0.5), 3))
            if len(h) > 120:
                del h[:-120]

    def is_near_duplicate(self, text: str, lookback: int = 10, threshold: float = 0.62) -> bool:
        a = set(words(text))
        if len(a) < 4:
            return False
        return any(jaccard(a, set(words(m.text))) >= threshold for m in self.messages[-lookback:])

    def roster(self, room: "Room") -> list[LlamaState]:
        return sorted(self.member_states(room), key=lambda s: (-s.eagerness, s.last_spoke_turn))


@dataclass
class Room:
    """The whole app: five tables, a shared seed, and the eavesdrops between them."""
    seed: str = ""
    tables: list[Table] = field(default_factory=list)
    llamas: dict[str, LlamaState] = field(default_factory=dict)
    leaks: list[Leak] = field(default_factory=list)
    turn: int = 0
    status: str = "idle"
    phase: str = ""
    focus: int = -1  # which table is in detail; -1 = the overview
    outcome: str = ""
    started: float | None = None
    finished: float | None = None
    log: list[tuple[str, str]] = field(default_factory=list)
    cost_jev: float = 0.0
    cost_llama: float = 0.0
    tok_in: int = 0
    tok_out: int = 0
    calls_jev: int = 0
    calls_llama: int = 0
    last_error: str = ""

    def __post_init__(self) -> None:
        if not self.llamas:
            self.llamas = {p.id: LlamaState(p.id) for p in LLAMAS}
        if not self.tables:
            self.tables = [Table(id=tid, members=members, colour=TABLE_COLOURS[(tid - 1) % len(TABLE_COLOURS)])
                           for tid, members in TABLE_SETS]

    @property
    def elapsed(self) -> float:
        return 0.0 if self.started is None else ((self.finished or time.time()) - self.started)

    @property
    def cost(self) -> float:
        return self.cost_jev + self.cost_llama

    @property
    def tokens(self) -> int:
        return self.tok_in + self.tok_out

    @property
    def messages(self) -> int:
        return sum(len(t.messages) for t in self.tables)

    def table(self, tid: int) -> Table | None:
        return next((t for t in self.tables if t.id == tid), None)

    def log_line(self, text: str, style: str = "") -> None:
        self.log.append((text, style))
        if len(self.log) > 300:
            del self.log[:-300]

    def set_status(self, status: str, phase: str = "", outcome: str = "") -> None:
        self.status = status
        if phase:
            self.phase = phase
        if outcome:
            self.outcome = outcome
        if status in ("stopped", "done", "error") and self.finished is None:
            self.finished = time.time()

    def table_of(self, llama: str) -> Table | None:
        return next((t for t in self.tables if llama in t.members), None)

    def reset(self, seed: str) -> None:
        """A fresh run on a new seed: the tables and llamas stay, the talk is wiped."""
        self.seed = seed
        for t in self.tables:
            t.messages.clear()
            t.gauges = {g: 0.5 for g in GAUGES}
            t.judgement = Judgement()
            t.thread, t.threads, t.summary, t.summary_from, t.leaks_in = "", [], "", "", 0
            t.history = {g: [] for g in GAUGES}
            t.belief_history = {p: [] for p in PROPOSITIONS}
        for s in self.llamas.values():
            s.spoken, s.eagerness, s.last_spoke_turn, s.speaking = 0, 0.0, -1, False
        self.leaks.clear()
        self.turn, self.cost_jev, self.cost_llama, self.tok_in, self.tok_out = 0, 0.0, 0.0, 0, 0
        self.calls_jev, self.calls_llama, self.last_error = 0, 0, ""
        self.finished = None
        self.set_status("discussing", "a fresh seed")
        self.log_line(f'new seed: "{seed[:80]}"', "#ffd06b")


def empty_room(seed: str) -> Room:
    room = Room(seed=seed)
    room.log_line("five tables seated", "dim")
    return room
