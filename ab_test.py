"""A/B: Jev picks which eavesdrop policy is better, judged on the corpus.

    uv run ab_test.py

The leak threshold is a policy the app owns (a constant in code). Two policies are
set against each other on the SAME corpus:

  A  strict  (gate 0.50)  -- few, unmistakably striking lines carry
  B  broad   (gate 0.40)  -- more lines carry, including ordinary remarks

Jev judges: which policy's admitted set serves the app's goal better, which leaks
from policy B are not worth carrying, and whether the stricter set is better
material for a hearing table. All Choices and Nouls; no generation.
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

POLICIES = {"A_strict": 0.50, "B_broad": 0.40}


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


def _prob(o: Any, k: str) -> float:
    try:
        return float((getattr(o, "probabilities", None) or {}).get(k, 0.0))
    except Exception:
        return 0.0


async def main() -> int:
    files = sorted(glob.glob(str(OUT / "experiments-*.json")))
    if not files:
        print("run experiments.py first")
        return 2
    data = {r["name"]: r for r in json.loads(Path(files[-1]).read_text())}
    gate = data.get("leak-gate")
    if not gate:
        print("the leak-gate experiment has not been run yet")
        return 2
    rows = gate["rows"]

    admitted = {k: [x for x in rows if x["score"] >= thr] for k, thr in POLICIES.items()}
    from typesafe_sdk import AsyncTypeSafeClient, Choice, Noul

    key = os.environ["OPENROUTER_API_KEY"]
    cost, calls = 0.0, 0
    stamp = time.strftime("%Y%m%d-%H%M%S")
    async with AsyncTypeSafeClient(api_key=key, base_url=SYSTEMONE_BASE, model=JEV_MODEL) as client:
        # 1) per-line: is a line admitted by the broad policy but not the strict one
        #    actually worth carrying?
        marginal = [x for x in admitted["B_broad"] if x["score"] < POLICIES["A_strict"]]
        q: dict[str, Any] = {}
        for i, x in enumerate(marginal):
            q[f"m{i}_worth"] = Noul(instructions=f"Line: “{x['text']}” (said at table {x['table']} in a room of five tables all on one seed). Would a DIFFERENT table, overhearing this, have something worth picking up?")
            q[f"m{i}_ignore"] = Noul(instructions=f"Line: “{x['text']}”. Is this an ordinary table-local remark that another table would simply ignore?")
        r1 = await client.system_one({"seed": gate.get("seed", "one shared seed")}, q)
        calls += 1
        cost += _cost(r1)
        marginal_scored = [{"text": x["text"][:60], "score": x["score"], "carried": x["carried"],
                            "worth": round(min(_noul(r1.nouls[f"m{i}_worth"]), 1 - _noul(r1.nouls[f"m{i}_ignore"])), 3)}
                           for i, x in enumerate(marginal)]

        # 2) which policy serves the app better
        r2 = await client.system_one(
            {
                "goal": "Five tables of four AI llamas talk at once on one shared seed. Occasionally one table overhears a line from another and that line changes what the hearing table does next. A leak must be visible and must matter; a leak that decorates is a failure.",
                "A_strict": [x["text"] for x in admitted["A_strict"]],
                "B_broad": [x["text"] for x in admitted["B_broad"] if x["score"] < POLICIES["A_strict"]],
                "marginal_worth": marginal_scored,
            },
            {
                "policy": Choice(instructions="Which eavesdrop policy serves the stated goal better?", criteria={
                    "A_strict": "fewer lines carry, but every one of them is unmistakably striking",
                    "B_broad": "more lines carry, including ordinary remarks that sit just above a lower gate",
                }),
                "stricter_better": Noul(instructions="Given the goal that a leak must matter and must not merely decorate, is the STRICTER policy the better one?"),
                "more_leaks_better": Noul(instructions="Would more frequent eavesdrops make The Corral better to watch, even if some of them carry ordinary remarks?"),
            },
        )
        calls += 1
        cost += _cost(r2)

    OUT.mkdir(exist_ok=True)
    pol = _pick(getattr(r2, "choices", {}).get("policy"))
    (OUT / f"ab-{stamp}.json").write_text(json.dumps({
        "policies": POLICIES,
        "admitted": {k: [x["text"] for x in v] for k, v in admitted.items()},
        "marginal": marginal_scored,
        "policy": pol, "probabilities": getattr(getattr(r2, "choices", {}).get("policy"), "probabilities", {}),
        "stricter_better": round(_noul(r2.nouls["stricter_better"]), 3),
        "more_leaks_better": round(_noul(r2.nouls["more_leaks_better"]), 3),
    }, indent=1, ensure_ascii=False))

    print("A/B on the eavesdrop gate, judged by Jev\n")
    print(f"  policy A (gate {POLICIES['A_strict']}) admits {len(admitted['A_strict'])} of {len(rows)} lines")
    print(f"  policy B (gate {POLICIES['B_broad']}) admits {len(admitted['B_broad'])} of {len(rows)} lines")
    print(f"  the {len(marginal)} lines B adds that A rejects:")
    for x in marginal_scored:
        print(f"    worth {x['worth']:.2f}  (gate score {x['score']:.2f})  {x['text']}")
    print(f"\n  Jev picks: {pol}  " + " ".join(f"{k}={_prob(getattr(r2, 'choices', {}).get('policy'), k):.2f}" for k in POLICIES))
    print(f"  stricter policy is better:        {_noul(r2.nouls['stricter_better']):.2f}")
    print(f"  more frequent leaks would help:   {_noul(r2.nouls['more_leaks_better']):.2f}")
    print(f"\n{calls} Jev calls · ${cost:.4f} · ab-{stamp}.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
