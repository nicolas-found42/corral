"""The meta-experiment: Jev judging its own experiments.

    uv run experiment_review.py

Reads the newest results/experiments-*.json and asks Jev, per experiment:
  * did it produce a signal (a result that discriminates), versus mush?
  * is its conclusion trustworthy given the size of the corpus?
  * does it say something a reader would not already assume?
  * is it worth running again on a bigger corpus?
and then, in a second request, which experiments to KEEP, which to DROP, and what
to build next -- as Choices with probabilities, plus an escape hatch.

Still Jev-only: no language-model generation anywhere.
"""
from __future__ import annotations

import asyncio
import glob
import json
import os
import time
from pathlib import Path
from typing import Any

SYSTEMONE_BASE = "https://openrouter.ai/api"
JEV_MODEL = "jev-1.13"
HERE = Path(__file__).parent
OUT = HERE / "results"

NEXT_CANDIDATES = {
    "bigger_corpus": "collect a much larger transcript corpus first, then re-run every experiment",
    "ablate_leak": "run the app with eavesdrops switched off, then compare the two corpora",
    "perturb_seed": "run the same seed five times and measure how much the conversations differ run to run",
    "score_calibration": "measure whether one table's own quality scores predict a reader's ranking",
    "cross_table_agreement": "ask whether the five tables end up agreeing with each other's conclusions",
    "agent_reliability": "measure how often a table produces a line that contradicts its own earlier line",
    "question_phrasing": "test several differently-phrased versions of each question for answer stability",
    "primitive_choice": "for the same judgement, compare Noul vs Score vs Choice and keep the sharpest",
}


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


def _conf(o: Any) -> float:
    try:
        return float(getattr(o, "confidence", 0.0) or 0.0)
    except Exception:
        return 0.0


def summarise(r: dict) -> str:
    """A compact, factual summary of one experiment's result, for Jev to judge."""
    n = r.get("name", "?")
    if n == "leak-absorption":
        return (f"{r.get('landed')} of {r.get('of')} heard lines were judged (across three phrasings) to have drawn a reply "
                f"that addresses them. Individual probabilities: "
                + "; ".join(f"T{x['table']} addresses={x.get('p_addresses')} ignores={x.get('p_ignores')}" for x in r.get("rows", [])))
    if n == "seed-fidelity":
        return (f"mean seed-engagement {r.get('mean')} over {len(r.get('rows', []))} lines; "
                f"per table {r.get('by_table')}; lowest lines scored "
                + "; ".join(f"{x['score']} ({x['speaker']}: {x['text'][:40]})" for x in sorted(r.get("rows", []), key=lambda y: y["score"])[:3]))
    if n == "cohesion-vs-quality":
        return (f"{r.get('pairs')} adjacent-line pairs; correlation between how well a line flows and how good it is scored "
                f"{r.get('correlation')}; mean quality of the most cohesive quartile {r.get('quality_top_cohesion_quartile')} "
                f"versus least cohesive quartile {r.get('quality_bottom_cohesion_quartile')}")
    if n == "voice-distinctness":
        return (f"{r.get('correct')} of {r.get('n')} lines were attributed to the right llama from text alone "
                f"(accuracy {r.get('accuracy')}, chance 0.25)")
    if n == "leak-gate":
        return ("threshold sweep: " + "; ".join(
            f"gate {s['threshold']} admits {s['lines_passing']} of {s['of']} lines and keeps {s['real_leaks_kept']} of 3 logged leaks"
            for s in r.get("sweep", [])))
    if n == "unanswered":
        return (f"{r.get('open')} of {r.get('of')} questions in the corpus are judged still open; "
                + "; ".join(f"T{x['table']} answered={x['p_answered']} open={x['p_open']}" for x in r.get("rows", [])))
    if n == "redundancy":
        return (f"mean distinctness between the five tables {r.get('mean_distinctness')}; per pair "
                + "; ".join(f"{x['pair']} distinct={x['p_distinct']} same={x['p_same']}" for x in r.get("rows", [])))
    if n == "best-line":
        per = r.get("per_table", {})
        return "each table's best line: " + "; ".join(f"T{t}: {v['text'][:44]}" for t, v in per.items() if isinstance(t, int))
    return json.dumps(r)[:400]


async def main() -> int:
    files = sorted(glob.glob(str(OUT / "experiments-*.json")))
    if not files:
        print("no results yet — run `uv run experiments.py` first")
        return 2
    data = json.loads(Path(files[-1]).read_text())
    data = [r for r in data if "error" not in r]
    from typesafe_sdk import AsyncTypeSafeClient, Choice, Noul

    key = os.environ["OPENROUTER_API_KEY"]
    cost, calls = 0.0, 0
    stamp = time.strftime("%Y%m%d-%H%M%S")
    async with AsyncTypeSafeClient(api_key=key, base_url=SYSTEMONE_BASE, model=JEV_MODEL) as client:
        # 1) one question set per experiment: signal, trust, surprise, rerun-worth
        state = {"corpus": f"{36} real transcript lines from five concurrent four-agent AI conversations, all on one shared seed",
                 "experiments": [{"id": r["name"], "finding": summarise(r)} for r in data]}
        q: dict[str, Any] = {}
        for r in data:
            n = r["name"]
            q[f"{n}_signal"] = Noul(instructions=f"Experiment `{n}` produced the finding shown under its id. Does that finding DISCRIMINATE -- does it separate the lines or tables into clearly different cases -- or is it mush where everything lands near the middle?")
            q[f"{n}_mush"] = Noul(instructions=f"Experiment `{n}` produced the finding shown under its id. Is that finding mush: everything scored nearly the same, so nothing is separated?")
            q[f"{n}_trust"] = Noul(instructions=f"Experiment `{n}` produced the finding shown under its id, from only about 36 transcript lines. Is that finding solid enough to act on, or too small a sample to trust?")
            q[f"{n}_surprise"] = Noul(instructions=f"Experiment `{n}` produced the finding shown under its id. Would a reader find that finding SURPRISING, or is it what anyone would already assume about a group chat?")
            q[f"{n}_rerun"] = Noul(instructions=f"Experiment `{n}` produced the finding shown under its id. Is it worth running again on a much larger corpus?")
        r1 = await client.system_one(state, q)
        calls += 1
        cost += _cost(r1)

        # 2) which to keep, which to drop, and what to do next
        def val(n: str, k: str) -> float:
            return round(min(_noul(r1.nouls[f"{n}_{k}"]), 1 - _noul(r1.nouls[f"{n}_mush"])), 3)

        table = {r["name"]: {"signal": val(r["name"], "signal"), "trust": round(_noul(r1.nouls[f'{r["name"]}_trust']), 3),
                             "surprise": round(_noul(r1.nouls[f'{r["name"]}_surprise']), 3),
                             "rerun": round(_noul(r1.nouls[f'{r["name"]}_rerun']), 3)} for r in data}
        for n, v in table.items():
            v["composite"] = round(0.4 * v["signal"] + 0.2 * v["trust"] + 0.2 * v["surprise"] + 0.2 * v["rerun"], 3)

        r2 = await client.system_one(
            {"experiments": json.dumps(table), "options": NEXT_CANDIDATES},
            {"keep": Choice(instructions="Which of these experiments should be KEPT and run as a standing check on the app?",
                            criteria={n: f"the {n} experiment" for n in table} | {"none": "none of them are worth keeping"}),
             "drop": Choice(instructions="Which one experiment should be DROPPED as not earning its cost?",
                            criteria={n: f"the {n} experiment" for n in table} | {"none": "none should be dropped"}),
             "next": Choice(instructions="What should be built or measured next to improve these conversations?",
                            criteria=NEXT_CANDIDATES | {"nothing": "nothing further is needed"})},
        )
        calls += 1
        cost += _cost(r2)

        keep = _pick(getattr(r2, "choices", {}).get("keep"))
        drop = _pick(getattr(r2, "choices", {}).get("drop"))
        nxt = _pick(getattr(r2, "choices", {}).get("next"))
        next_probs = getattr(getattr(r2, "choices", {}).get("next"), "probabilities", {}) or {}

    OUT.mkdir(exist_ok=True)
    (OUT / f"review-{stamp}.json").write_text(json.dumps(
        {"table": table, "keep": keep, "keep_confidence": _conf(getattr(r2, "choices", {}).get("keep")),
         "drop": drop, "next": nxt, "next_probabilities": next_probs}, indent=1))

    print("Jev's scorecard for its own experiments")
    print(f"{'experiment':22} {'signal':>7} {'trust':>6} {'surprise':>9} {'rerun':>6} {'composite':>10}")
    for n, v in sorted(table.items(), key=lambda kv: -kv[1]["composite"]):
        print(f"{n:22} {v['signal']:7.2f} {v['trust']:6.2f} {v['surprise']:9.2f} {v['rerun']:6.2f} {v['composite']:10.2f}")
    print(f"\nkeep: {keep} ({_conf(getattr(r2, 'choices', {}).get('keep')):.2f})")
    print(f"drop: {drop}")
    print(f"next: {nxt}")
    for k, v in sorted(next_probs.items(), key=lambda kv: -float(kv[1]))[:5]:
        print(f"   {float(v):.2f}  {NEXT_CANDIDATES.get(k, k)}")
    print(f"\n{calls} Jev calls · ${cost:.4f} · review-{stamp}.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
