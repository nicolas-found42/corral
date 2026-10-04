"""Jev on itself: which PRIMITIVE and which PHRASING answer the same question best?

    uv run method_compare.py

Two meta-questions, both answered from the corpus, both judged by Jev:

 1. PRIMITIVE. For the same judgement -- "is this a good line?" -- ask it three
    ways: a Noul ("does this line move the thread?"), a Score (three ordered
    levels) and a Choice (better than the median line, or not). Then measure which
    one spreads the lines out most, and ask Jev which answer it trusts.

 2. PHRASING. For the same judgement asked three different ways, measure how far
    the answers move. A judgement whose answer swings with the wording is not
    ready to gate anything.

No language-model generation: every number is a probability, a score or a label.
"""
from __future__ import annotations

import asyncio
import json
import os
import statistics
import time
from pathlib import Path
from typing import Any

import corpus

SYSTEMONE_BASE = "https://openrouter.ai/api"
JEV_MODEL = "jev-1.13"
HERE = Path(__file__).parent
OUT = HERE / "results"

SCORE_LEVELS = ["says nothing new", "a useful observation", "a sharp contribution that moves the thread"]
CHOICE_CRITERIA = {"strong": "a genuinely strong line: sharp, specific and it moves the thread",
                   "ordinary": "an ordinary line: plausible but it says little that is new"}

# three deliberately different ways to ask ONE thing
PHRASINGS = {
    "move_thread": "Does this line move the discussion forward, rather than restating what was already said?",
    "worth_reading": "Is this line worth reading on its own, for someone following the discussion?",
    "restates": "Does this line merely restate or rephrase something already on the table?",
}
INVERTED_START = {"restates"}


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


def spread(vals: list[float]) -> float:
    """How much the instrument separates the items: stdev of its readings."""
    return round(statistics.pstdev(vals), 3) if len(vals) > 1 else 0.0


async def main() -> int:
    from typesafe_sdk import AsyncTypeSafeClient, Choice, Noul, Score

    lines = [(t, sp, tx) for (t, sp, tx, _s, _h) in corpus.LINES]
    key = os.environ["OPENROUTER_API_KEY"]
    cost, calls = 0.0, 0
    stamp = time.strftime("%Y%m%d-%H%M%S")

    async with AsyncTypeSafeClient(api_key=key, base_url=SYSTEMONE_BASE, model=JEV_MODEL) as client:
        # ---- 1. primitive comparison, one request
        q: dict[str, Any] = {}
        for i, (_t, _sp, tx) in enumerate(lines):
            q[f"l{i}_noul"] = Noul(instructions=f"{PHRASINGS['move_thread']} The line: “{tx}”")
            q[f"l{i}_score"] = Score(instructions=f"How good a contribution is this line to the discussion? “{tx}”", criteria=SCORE_LEVELS)
            q[f"l{i}_choice"] = Choice(instructions=f"Is this a strong line or an ordinary one? “{tx}”", criteria=CHOICE_CRITERIA)
        r1 = await client.system_one({"seed": corpus.SEED}, q)
        calls += 1
        cost += _cost(r1)

        by_prim: dict[str, list[float]] = {"noul": [], "score": [], "choice": []}
        ranks: dict[str, list[tuple[float, str]]] = {"noul": [], "score": [], "choice": []}
        for i, (_t, _sp, tx) in enumerate(lines):
            v_n = _noul(r1.nouls[f"l{i}_noul"])
            v_s = float(r1.scores[f"l{i}_score"].score) / (len(SCORE_LEVELS) - 1) if f"l{i}_score" in r1.scores else 0.5
            v_c = float((getattr(r1.choices[f"l{i}_choice"], "probabilities", None) or {}).get("strong", 0.0))
            by_prim["noul"].append(v_n)
            by_prim["score"].append(v_s)
            by_prim["choice"].append(v_c)
            for k, v in (("noul", v_n), ("score", v_s), ("choice", v_c)):
                ranks[k].append((v, tx[:46]))

        prim_stats = {k: {"mean": round(statistics.mean(v), 3), "spread": spread(v),
                          "range": round(max(v) - min(v), 3),
                          "top": max(ranks[k])[1], "bottom": min(ranks[k])[1]} for k, v in by_prim.items()}

        # ---- 2. phrasing stability, one request
        q2: dict[str, Any] = {}
        for i, (_t, _sp, tx) in enumerate(lines):
            for key_, instr in PHRASINGS.items():
                q2[f"l{i}_{key_}"] = Noul(instructions=f"{instr} The line: “{tx}”")
        r2 = await client.system_one({"seed": corpus.SEED}, q2)
        calls += 1
        cost += _cost(r2)

        swings = []
        per_line: dict[str, list[float]] = {k: [] for k in PHRASINGS}
        for i, (_t, _sp, tx) in enumerate(lines):
            vals = {}
            for key_ in PHRASINGS:
                v = _noul(r2.nouls[f"l{i}_{key_}"])
                vals[key_] = 1 - v if key_ in INVERTED_START else v
                per_line[key_].append(vals[key_])
            swings.append({"text": tx[:50], "vals": {k: round(v, 3) for k, v in vals.items()},
                           "max_swing": round(max(vals.values()) - min(vals.values()), 3)})
        mean_swing = round(statistics.mean([s["max_swing"] for s in swings]), 3)
        worst = sorted(swings, key=lambda s: -s["max_swing"])[:5]

        # ---- 3. let Jev judge its own instruments
        r3 = await client.system_one(
            {"primitive_stats": json.dumps(prim_stats, ensure_ascii=False),
             "phrasing": {"mean_max_swing": mean_swing, "worst": json.dumps(worst[:3], ensure_ascii=False)}},
            {
                "best_primitive": Choice(instructions="For judging how good a line is, which instrument gives the most usable answer?",
                                         criteria={"noul": "a single probability of yes",
                                                    "score": "a position on three ordered levels",
                                                    "choice": "a pick between strong and ordinary"}),
                "phrasing_ok": Noul(instructions="Is the question about line quality answerable stably enough, given how far the answers moved when the wording changed?"),
                "ready_to_gate": Noul(instructions="Is a judgement that swings this much with wording safe to use as a gate that decides what the app does?"),
            },
        )
        calls += 1
        cost += _cost(r3)

    best_prim = _pick(getattr(r3, "choices", {}).get("best_primitive"))
    probs = getattr(getattr(r3, "choices", {}).get("best_primitive"), "probabilities", {}) or {}
    OUT.mkdir(exist_ok=True)
    (OUT / f"method-{stamp}.json").write_text(json.dumps(
        {"primitive_stats": prim_stats, "phrasing_mean_swing": mean_swing, "phrasing_worst": worst,
         "best_primitive": best_prim, "primitive_probabilities": probs,
         "phrasing_ok": round(_noul(r3.nouls["phrasing_ok"]), 3),
         "ready_to_gate": round(_noul(r3.nouls["ready_to_gate"]), 3)}, indent=1, ensure_ascii=False))

    print("PRIMITIVE: three ways to ask 'is this a good line?'\n")
    print(f"  {'instrument':10} {'mean':>6} {'spread':>7} {'range':>6}   best line read")
    for k, v in prim_stats.items():
        print(f"  {k:10} {v['mean']:6.2f} {v['spread']:7.3f} {v['range']:6.2f}   {v['top'][:44]}")
    print(f"\n  Jev picks: {best_prim}  " + " ".join(f"{k}={float(v):.2f}" for k, v in sorted(probs.items(), key=lambda kv: -float(kv[1]))))

    print(f"\nPHRASING: the same judgement, three wordings\n")
    print(f"  mean swing per line: {mean_swing}  (0 = wording never matters, 1 = it dominates)")
    for s in worst:
        print(f"    swing {s['max_swing']:.2f}  {s['vals']}  {s['text'][:44]}")
    print(f"\n  wording is stable enough to trust:  {_noul(r3.nouls['phrasing_ok']):.2f}")
    print(f"  safe to use as a gate:              {_noul(r3.nouls['ready_to_gate']):.2f}")
    print(f"\n{calls} Jev calls · ${cost:.4f} · method-{stamp}.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
