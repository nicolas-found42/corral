"""Jev-only experiments over The Corral's real transcript corpus.

    uv run experiments.py                 # all experiments (Jev only, no llama calls)
    uv run experiments.py leak-absorption voice-distinctness

Every experiment reads corpus.py and asks TypeSafe Jev questions about it. No
language-model generation happens anywhere in this file; every number printed is
a probability, a label or a majority count the judgement model returned.

Design (Jev's own rulings):
  * one batched request PER EXPERIMENT (`batched_per_experiment`, 0.62) so every
    experiment yields one comparable result table;
  * each experiment is internally robust: a judgement is asked under SEVERAL
    differently-worded phrasings and only reported when they agree, with the
    spread shown. Jev's decision flagged "one lucky answer" as the risk
    (`multi_phrasing_batch`, 0.33, second choice) so the phrasings are folded in;
  * the eavesdrop gate is re-calibrated from the corpus (`calibrate_on_corpus`,
    0.71) rather than trusted at 0.46.

Phrasings: every Noul below is asked 3 ways, once inverted where inversion is
meaningful, and scored as min(positive phrasings) and 1 - max(inverted phrasing).
"""
from __future__ import annotations

import asyncio
import json
import os
import statistics
import sys
import time
from pathlib import Path
from typing import Any

import corpus
from personas import BY_ID
from room import PROPOSITIONS

SYSTEMONE_BASE = "https://openrouter.ai/api"
JEV_MODEL = "jev-1.13"
OUT = Path(__file__).parent / "results"

# each entry: (key, instruction, inverted?)
Phrasing = tuple[str, str, bool]


def _cost(r: Any) -> float:
    try:
        return float(r.raw_http_response.json()["usage"]["cost"])
    except Exception:
        return 0.0


def _noul(o: Any) -> float:
    try:
        return float(o.noul)
    except Exception:
        return 0.5


def _pick(o: Any) -> str:
    for a in ("choice", "selected", "value"):
        v = getattr(o, a, None)
        if v:
            return str(v)
    return ""


def _score_norm(o: Any, levels: int) -> float:
    try:
        return float(o.score) / max(1, levels - 1)
    except Exception:
        return 0.5


def _usage_of(r: Any) -> tuple[int, int]:
    """Token usage, read from whichever shape OpenRouter returned."""
    try:
        u = r.raw_http_response.json()["usage"]
        return int(u.get("prompt_tokens", u.get("input_tokens", 0))), int(u.get("completion_tokens", u.get("output_tokens", 0)))
    except Exception:
        return 0, 0


class Kit:
    """One Jev client for the experiments. Never prints the key."""

    def __init__(self) -> None:
        from typesafe_sdk import AsyncTypeSafeClient

        key = os.environ["OPENROUTER_API_KEY"]
        self.client = AsyncTypeSafeClient(api_key=key, base_url=SYSTEMONE_BASE, model=JEV_MODEL)
        self.cost = 0.0
        self.calls = 0
        self.tin = 0
        self.tout = 0
        self.errors: list[str] = []
        self.sem = asyncio.Semaphore(6)

    async def __aenter__(self) -> "Kit":
        await self.client.__aenter__()
        return self

    async def __aexit__(self, *a: Any) -> None:
        await self.client.__aexit__(*a)

    async def ask(self, state: dict, questions: dict, what: str = "ask", retries: int = 3) -> Any | None:
        """One batched request. Fails soft: an invalid answer is recorded, never faked."""
        for attempt in range(retries):
            try:
                async with self.sem:
                    r = await self.client.system_one(state, questions)
                self.calls += 1
                self.cost += _cost(r)
                ti, to = _usage_of(r)
                self.tin += ti
                self.tout += to
                return r
            except Exception as e:
                if attempt == retries - 1:
                    self.errors.append(f"{what}: {type(e).__name__}: {str(e)[:100]}")
                    return None
                await asyncio.sleep(1.2 * (attempt + 1))
        return None


# --------------------------------------------------------------------- helpers
def robust(vals: list[float], inverted: list[float]) -> tuple[float, float]:
    """(worst positive phrasing, best inverted phrasing) -- the conservative pair."""
    lo = min(vals) if vals else 0.5
    hi = max(inverted) if inverted else 0.0
    return round(lo, 3), round(hi, 3)


def verdict(lo: float, hi: float) -> str:
    """Agreement verdict for one item's several phrasings."""
    if lo >= 0.65 and hi <= 0.35:
        return "agree-yes"
    if lo <= 0.35 and hi <= 0.35:
        return "agree-no"
    return "split"


# ------------------------------------------------------------------ experiments
async def exp_leak_absorption(kit: Kit) -> dict:
    """Did the eavesdrop land? For every reply written right after a heard line,
    ask whether it addresses that heard line -- under three phrasings, one inverted."""
    from typesafe_sdk import Noul

    cases: list[dict] = []
    for (t, sp, tx, _src, heard) in corpus.LINES:
        if heard is None:
            continue
        table = corpus.TABLES[t]
        seq = [x for x in corpus.LINES if x[0] == t]
        i = seq.index((t, sp, tx, _src, heard))
        after = [x for x in seq[i + 1:] if x[4] is None]
        if not after:
            continue
        nxt = after[0]
        cases.append({"table": t, "heard_from": heard, "heard": tx, "reply": nxt[2], "replier": nxt[1]})

    if not cases:
        return {"name": "leak-absorption", "note": "no heard line in the corpus has a following reply"}

    phrasings: list[Phrasing] = [
        ("addresses", "The `reply` was written by a llama sitting at table `table`, immediately after a line from another table was read aloud to them. Does `reply` substantively address or build on that heard line, rather than ignoring it?", False),
        ("about_heard", "`reply` follows `heard` in a transcript. Is `reply` a response to what `heard` says?", False),
        ("ignores", "`reply` was written after `heard` was heard at this table. Does `reply` ignore the heard line and just continue the table's own previous topic as if they had not heard anything?", True),
    ]
    state = {"seed": corpus.SEED, "cases": [{"id": f"c{i}", **c} for i, c in enumerate(cases)]}
    q: dict[str, Any] = {}
    for j, c in enumerate(cases):
        for key, instr, inv in phrasings:
            q[f"c{j}_{key}"] = Noul(instructions=instr.replace("`reply`", f'case c{j}\'s reply').replace("`heard`", f"case c{j}'s heard line").replace("`table`", f"case c{j}'s table"))
    r = await kit.ask(state, q, "leak-absorption")
    rows = []
    for j, c in enumerate(cases):
        if r is None:
            rows.append({**c, "error": "no answer"})
            continue
        pos = [_noul(r.nouls[f"c{j}_addresses"]), _noul(r.nouls[f"c{j}_about_heard"])]
        inv = [_noul(r.nouls[f"c{j}_ignores"])]
        lo, hi = robust(pos, inv)
        rows.append({**c, "p_addresses": round(min(pos), 3), "p_ignores": round(max(inv), 3),
                     "worst": lo, "inverted": hi, "verdict": verdict(lo, hi)})
    landed = sum(1 for x in rows if x.get("verdict") == "agree-yes")
    return {"name": "leak-absorption", "cases": cases, "rows": rows, "landed": landed,
            "of": len(rows), "note": "three phrasings, one inverted; agree-yes means all of them concur"}


async def exp_seed_fidelity(kit: Kit) -> dict:
    """Does every line still engage the shared seed?"""
    from typesafe_sdk import Noul

    rows: list[tuple[int, str, str, str, int | None]] = list(corpus.LINES)
    instrs: list[Phrasing] = [
        ("engages", "Given the shared seed, does this line still engage that seed rather than talking about something else entirely?", False),
        ("on_topic", "Is this line on the topic of the seed?", False),
        ("tangent", "Has this line wandered off the seed onto a side topic of its own?", True),
    ]
    q: dict[str, Any] = {}
    for i, (_t, _sp, tx, _s, _h) in enumerate(rows):
        for key, instr, _inv in instrs:
            q[f"l{i}_{key}"] = Noul(instructions=f"{instr} The shared seed is `seed`. The line is: “{tx}”")
    r = await kit.ask({"seed": corpus.SEED}, q, "seed-fidelity")
    out = []
    for i, (t, sp, tx, _s, _h) in enumerate(rows):
        if r is None:
            continue
        pos = [_noul(r.nouls[f"l{i}_engages"]), _noul(r.nouls[f"l{i}_on_topic"])]
        inv = [_noul(r.nouls[f"l{i}_tangent"])]
        lo, hi = robust(pos, inv)
        out.append({"table": t, "speaker": sp, "text": tx[:64], "score": lo, "inverted": hi, "verdict": verdict(lo, hi)})
    by_table: dict[int, list[float]] = {}
    for x in out:
        by_table.setdefault(x["table"], []).append(x["score"])
    return {"name": "seed-fidelity", "rows": out,
            "by_table": {k: round(statistics.mean(v), 3) for k, v in sorted(by_table.items())},
            "mean": round(statistics.mean([x["score"] for x in out]), 3) if out else None}


async def exp_cohesion_vs_quality(kit: Kit) -> dict:
    """Do fluent (highly cohesive) lines also score as the better lines?"""
    from typesafe_sdk import Noul, Score

    seqs: dict[int, list] = {}
    for row in corpus.LINES:
        seqs.setdefault(row[0], []).append(row)
    pairs = []
    for t, seq in seqs.items():
        for i in range(1, len(seq)):
            if seq[i][4] is not None:
                continue  # skip heard lines: their cohesion is not a table-internal reply
            pairs.append((t, seq[i - 1][2], seq[i][2]))
    q: dict[str, Any] = {}
    for i, (_t, prev, cur) in enumerate(pairs):
        q[f"p{i}_flows"] = Noul(instructions=f"Line A: “{prev}”  Line B: “{cur}”  Does B flow naturally from A and continue the same thread of thought?")
        q[f"p{i}_tangent"] = Noul(instructions=f"Line A: “{prev}”  Line B: “{cur}”  Does B change the subject away from A?")
        q[f"p{i}_q"] = Score(instructions=f"How good a contribution to the discussion is Line B: “{cur}”?",
                             criteria=["says nothing new", "a useful observation", "a sharp contribution that moves the thread"])
    r = await kit.ask({"seed": corpus.SEED}, q, "cohesion-vs-quality")
    out = []
    for i, (t, prev, cur) in enumerate(pairs):
        if r is None:
            continue
        flow, tang = _noul(r.nouls[f"p{i}_flows"]), _noul(r.nouls[f"p{i}_tangent"])
        coh = round(min(flow, 1 - tang), 3)
        qual = round(_score_norm(r.scores[f"p{i}_q"], 3), 3)
        out.append({"table": t, "cohesion": coh, "quality": qual, "text": cur[:60]})
    # correlation by hand (no scipy): Pearson on the pairs
    corr = None
    if len(out) > 2:
        xs, ys = [x["cohesion"] for x in out], [x["quality"] for x in out]
        mx, my = statistics.mean(xs), statistics.mean(ys)
        num = sum((a - mx) * (b - my) for a, b in zip(xs, ys))
        den = (sum((a - mx) ** 2 for a in xs) * sum((b - my) ** 2 for b in ys)) ** 0.5
        corr = round(num / den, 3) if den else None
    # do the most cohesive quartile beat the least cohesive quartile on quality?
    qs = sorted(out, key=lambda x: -x["cohesion"])
    top = statistics.mean([x["quality"] for x in qs[: max(1, len(qs) // 4)]]) if qs else None
    bot = statistics.mean([x["quality"] for x in qs[-max(1, len(qs) // 4):]]) if qs else None
    return {"name": "cohesion-vs-quality", "pairs": len(out), "correlation": corr,
            "quality_top_cohesion_quartile": round(top, 3) if top else None,
            "quality_bottom_cohesion_quartile": round(bot, 3) if bot else None, "rows": out}


async def exp_voice_distinctness(kit: Kit) -> dict:
    """Can Jev tell which of a table's four llamas said a line, from the text alone?"""
    from typesafe_sdk import Choice

    rows = [(t, sp, tx) for (t, sp, tx, _s, _h) in corpus.LINES]
    hits = 0
    detail = []
    for t, sp, tx in rows:  # one request per line keeps the candidate set unambiguous
        members = corpus.TABLES[t]
        criteria = {m: f"{BY_ID[m].name}: {BY_ID[m].tagline}" for m in members}
        r = await kit.ask({"line": tx, "table": t},
                          {"who": Choice(instructions="A llama at this table said this line. Which of the four was it, judging only from what the line says and how it says it?",
                                         criteria=criteria)}, f"voice-{t}-{sp}")
        if r is None:
            continue
        who = _pick(getattr(r, "choices", {}).get("who"))
        ok = who == sp
        hits += 1 if ok else 0
        detail.append({"table": t, "said": sp, "guessed": who, "right": ok, "line": tx[:60]})
    n = len(detail)
    return {"name": "voice-distinctness", "n": n, "correct": hits,
            "accuracy": round(hits / n, 3) if n else None, "rows": detail}


async def exp_leak_gate(kit: Kit) -> dict:
    """Re-calibrate the eavesdrop gate from the corpus: score every line's worth and
    see which threshold separates the lines that actually carried."""
    from typesafe_sdk import Noul

    carried = {tx for (_r, _s, _d, _sp, tx) in corpus.LEAKS}
    rows = [(t, sp, tx) for (t, sp, tx, _s, _h) in corpus.LINES]
    instrs: list[Phrasing] = [
        ("carry", "Would this line, overheard by a DIFFERENT table working the same seed, change what that other table talks about? It must be striking, surprising or sharply relevant, not an ordinary remark.", False),
        ("portable", "Is this line a portable claim about the seed that another table could pick up and argue with?", False),
        ("table_local", "Is this line only meaningful to the table that said it, with nothing for an outsider to take away?", True),
    ]
    q: dict[str, Any] = {}
    for i, (_t, _sp, tx) in enumerate(rows):
        for key, instr, _inv in instrs:
            q[f"l{i}_{key}"] = Noul(instructions=f"{instr} The shared seed is `seed`. The line is: “{tx}”")
    r = await kit.ask({"seed": corpus.SEED}, q, "leak-gate")
    out = []
    for i, (t, sp, tx) in enumerate(rows):
        if r is None:
            continue
        pos = [_noul(r.nouls[f"l{i}_carry"]), _noul(r.nouls[f"l{i}_portable"])]
        inv = [_noul(r.nouls[f"l{i}_table_local"])]
        lo, hi = robust(pos, inv)
        out.append({"table": t, "speaker": sp, "score": lo, "inverted": hi, "carried": tx in carried, "text": tx[:60]})
    # sweep thresholds: for each, how many of the real leaks it keeps, and how many lines it lets through
    sweep = []
    for thr in [0.30, 0.35, 0.40, 0.45, 0.46, 0.50, 0.55, 0.60]:
        keeps = [x for x in out if x["score"] >= thr]
        real = [x for x in keeps if x["carried"]]
        sweep.append({"threshold": thr, "lines_passing": len(keeps), "of": len(out),
                      "real_leaks_kept": len(real), "real_leaks_scored_above": sum(1 for x in out if x["carried"] and x["score"] >= thr)})
    return {"name": "leak-gate", "rows": out, "sweep": sweep,
            "note": "a line below the threshold never carries; the sweep shows what each gate would have admitted"}


async def exp_unanswered(kit: Kit) -> dict:
    """Every question asked in the corpus, and whether the table ever answered it."""
    from typesafe_sdk import Noul

    qs = []
    for (t, sp, tx, _s, _h) in corpus.LINES:
        if "?" in tx:
            qs.append((t, sp, tx))
    if not qs:
        return {"name": "unanswered", "note": "no questions in the corpus"}
    seqs: dict[int, list] = {}
    for row in corpus.LINES:
        seqs.setdefault(row[0], []).append(row)
    q: dict[str, Any] = {}
    for i, (t, sp, tx) in enumerate(qs):
        seq = seqs[t]
        idx = next(j for j, x in enumerate(seq) if x[2] == tx)
        after = [x[2] for x in seq[idx + 1:]]
        ctx = " ".join(f"“{a}”" for a in after) or "(nothing followed)"
        q[f"q{i}_answered"] = Noul(instructions=f"The question “{tx}” was asked at table {t}. What followed at that table was: {ctx}. Did what followed answer that question?")
        q[f"q{i}_open"] = Noul(instructions=f"The question “{tx}” was asked at table {t}. What followed at that table was: {ctx}. Is the question still open -- did nobody address it?")
    r = await kit.ask({"seed": corpus.SEED}, q, "unanswered")
    out = []
    for i, (t, sp, tx) in enumerate(qs):
        if r is None:
            continue
        ans, opn = _noul(r.nouls[f"q{i}_answered"]), _noul(r.nouls[f"q{i}_open"])
        lo, hi = robust([ans], [opn])
        out.append({"table": t, "speaker": sp, "question": tx, "p_answered": round(ans, 3),
                    "p_open": round(opn, 3), "verdict": verdict(lo, hi)})
    open_q = [x for x in out if x["verdict"] != "agree-yes"]
    return {"name": "unanswered", "rows": out, "open": len(open_q), "of": len(out)}


async def exp_table_redundancy(kit: Kit) -> dict:
    """Do the five tables explore different ground, or repeat each other?"""
    from typesafe_sdk import Noul

    ids = sorted(corpus.TABLES)
    q: dict[str, Any] = {}
    pairs = [(a, b) for a in ids for b in ids if a < b]
    joined = {t: " | ".join(tx for (_t, _sp, tx, _s, _h) in corpus.LINES if _t == t) for t in ids}
    for a, b in pairs:
        q[f"t{a}t{b}_same"] = Noul(instructions=f"Table {a} said: {joined[a]}   Table {b} said: {joined[b]}   Are these two tables covering substantially the same ground, rather than different angles on the seed?")
        q[f"t{a}t{b}_distinct"] = Noul(instructions=f"Table {a} said: {joined[a]}   Table {b} said: {joined[b]}   Are these two tables exploring genuinely different angles on the seed?")
    r = await kit.ask({"seed": corpus.SEED}, q, "redundancy")
    out = []
    for a, b in pairs:
        if r is None:
            continue
        same, dist = _noul(r.nouls[f"t{a}t{b}_same"]), _noul(r.nouls[f"t{a}t{b}_distinct"])
        lo, hi = robust([dist], [same])
        out.append({"pair": f"T{a}/T{b}", "p_distinct": round(dist, 3), "p_same": round(same, 3),
                    "worst": lo, "inverted": hi, "verdict": verdict(lo, hi)})
    return {"name": "redundancy", "rows": out,
            "mean_distinctness": round(statistics.mean([x["p_distinct"] for x in out]), 3) if out else None}


async def exp_best_line(kit: Kit) -> dict:
    """The best line per table, and then the best of the five."""
    from typesafe_sdk import Choice

    per = {}
    for t in sorted(corpus.TABLES):
        lines = corpus.lines_of(t)
        if not lines:
            continue
        crit = {f"l{i}": f"{BY_ID[sp].name}: {tx}" for i, (sp, tx) in enumerate(lines)}
        r = await kit.ask({"seed": corpus.SEED},
                          {"best": Choice(instructions="Which single line best answers the shared seed?", criteria=crit)}, f"best-{t}")
        if r is None:
            continue
        k = _pick(getattr(r, "choices", {}).get("best"))
        if k.startswith("l") and k[1:].isdigit():
            i = int(k[1:])
            if 0 <= i < len(lines):
                per[t] = {"speaker": lines[i][0], "text": lines[i][1]}
    if len(per) >= 2:
        crit = {f"t{t}": f"{BY_ID[v['speaker']].name}: {v['text']}" for t, v in per.items()}
        r = await kit.ask({"seed": corpus.SEED},
                          {"best": Choice(instructions="Five tables each picked their best line. Which single line, across all five, best answers the shared seed?", criteria=crit)}, "best-of-five")
        if r is not None:
            k = _pick(getattr(r, "choices", {}).get("best"))
            if k.startswith("t") and k[1:].isdigit():
                per["winner"] = int(k[1:])
    return {"name": "best-line", "per_table": per}


EXPERIMENTS = {
    "leak-absorption": exp_leak_absorption,
    "seed-fidelity": exp_seed_fidelity,
    "cohesion-vs-quality": exp_cohesion_vs_quality,
    "voice-distinctness": exp_voice_distinctness,
    "leak-gate": exp_leak_gate,
    "unanswered": exp_unanswered,
    "redundancy": exp_table_redundancy,
    "best-line": exp_best_line,
}


async def main() -> int:
    want = sys.argv[1:] or list(EXPERIMENTS)
    unknown = [w for w in want if w not in EXPERIMENTS]
    if unknown:
        print(f"unknown experiment(s): {unknown}\navailable: {list(EXPERIMENTS)}")
        return 2
    OUT.mkdir(exist_ok=True)
    t0 = time.time()
    print(f"corpus: {corpus.corpus_size()} real lines, 5 tables, {len(corpus.LEAKS)} logged eavesdrops")
    print(f"running {len(want)} Jev-only experiment(s): {', '.join(want)}\n")
    results = []
    async with Kit() as kit:
        for name in want:
            print(f"— {name} …", flush=True)
            try:
                res = await EXPERIMENTS[name](kit)
            except Exception as e:
                res = {"name": name, "error": f"{type(e).__name__}: {str(e)[:140]}"}
            results.append(res)
            print(f"  done ({kit.calls} calls so far, ${kit.cost:.4f})", flush=True)
    stamp = time.strftime("%Y%m%d-%H%M%S")
    (OUT / f"experiments-{stamp}.json").write_text(json.dumps(results, indent=1, ensure_ascii=False))
    _report(results)
    print(f"\n{kit.calls} Jev calls · ${kit.cost:.4f} · {kit.tin:,} in / {kit.tout:,} out tokens · "
          f"{time.time() - t0:.0f}s · results/experiments-{stamp}.json")
    if kit.errors:
        print(f"errors ({len(kit.errors)}):", kit.errors[:3])
    return 0


def _report(results: list[dict]) -> None:
    for r in results:
        name = r.get("name", "?")
        print(f"\n{'=' * 78}\n{name}\n{'=' * 78}")
        if "error" in r:
            print(f"  FAILED: {r['error']}")
        elif name == "leak-absorption":
            print(f"  landed: {r.get('landed')}/{r.get('of')} heard lines got a reply that addresses them")
            for x in r.get("rows", []):
                print(f"  T{x['table']} ←T{x['heard_from']}  {x.get('verdict'):10}  addresses {x.get('p_addresses')}  ignores {x.get('p_ignores')}")
                print(f"      heard: {x['heard'][:66]}")
                print(f"      reply: {x['reply'][:66]}")
        elif name == "seed-fidelity":
            print(f"  mean {r.get('mean')}   by table {r.get('by_table')}")
            for x in sorted(r.get("rows", []), key=lambda y: y["score"])[:5]:
                print(f"  {x['score']:.2f} T{x['table']} {x['speaker']:9} {x['verdict']:10} {x['text']}")
        elif name == "cohesion-vs-quality":
            print(f"  pairs {r.get('pairs')}  correlation(cohesion, quality) = {r.get('correlation')}")
            print(f"  quality of the most cohesive quartile  {r.get('quality_top_cohesion_quartile')}")
            print(f"  quality of the least cohesive quartile {r.get('quality_bottom_cohesion_quartile')}")
        elif name == "voice-distinctness":
            print(f"  accuracy {r.get('accuracy')} ({r.get('correct')}/{r.get('n')}) — guessing would be 0.25")
            wrong = [x for x in r.get("rows", []) if not x["right"]]
            for x in wrong[:6]:
                print(f"  missed T{x['table']} said {x['said']:9} guessed {x['guessed']:9} {x['line'][:52]}")
        elif name == "leak-gate":
            print("  threshold sweep (a line at or above the gate may carry):")
            for s in r.get("sweep", []):
                print(f"    ≥{s['threshold']:.2f}  admits {s['lines_passing']:2}/{s['of']} lines · keeps {s['real_leaks_kept']} of the {len(corpus.LEAKS)} logged leaks")
            top = sorted(r.get("rows", []), key=lambda y: -y["score"])[:5]
            print("  the five most portable lines:")
            for x in top:
                print(f"    {x['score']:.2f} T{x['table']} {x['speaker']:9} {'CARRIED' if x['carried'] else '        '} {x['text'][:56]}")
        elif name in ("unanswered", "unanswered-questions"):
            print(f"  still open: {r.get('open')}/{r.get('of')} questions")
            for x in r.get("rows", []):
                if x["verdict"] != "agree-yes":
                    print(f"  {x['verdict']:10} T{x['table']} {x['speaker']:9} answered {x['p_answered']:.2f} open {x['p_open']:.2f}  {x['question'][:54]}")
        elif name in ("redundancy", "table-redundancy"):
            print(f"  mean distinctness ({r.get('mean_distinctness')}) — how far apart the five tables are")
            for x in sorted(r.get("rows", []), key=lambda y: y["p_distinct"]):
                print(f"  {x['pair']}  distinct {x['p_distinct']:.2f}  same {x['p_same']:.2f}  {x['verdict']}")
        elif name == "best-line":
            per = r.get("per_table", {})
            for t, v in sorted(per.items(), key=lambda kv: str(kv[0])):
                if isinstance(t, int):
                    print(f"  T{t}: {BY_ID[v['speaker']].name}: {v['text'][:66]}")
            if "winner" in per:
                print(f"  winner across all five: T{per['winner']}")


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
