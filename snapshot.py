"""Run a short session and render both views (overview + one table) to SVG + text.

    uv run snapshot.py -t "..." -n 8 -o corral.svg

Produces a still of The Corral from a real discussion: the zoomed-out five-table
overview and the zoomed-in detail of the busiest table, one after the other.
"""
from __future__ import annotations

import argparse
import asyncio
import os
from typing import Any

from rich.console import Console

from room import Room, empty_room
from session import Session
from tui import compose


async def _run(seed: str, rounds: int, draws: int) -> tuple[Room, dict[str, Any]]:
    room = empty_room(seed)
    cfg: dict[str, Any] = {"draws": draws, "max_rounds": rounds, "seed": 7}
    await Session(room, turns=draws, max_rounds=rounds).run()
    return room, cfg


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("-t", "--seed", default="is a crowd wiser than any one of us, or just louder?")
    ap.add_argument("-n", "--rounds", type=int, default=8)
    ap.add_argument("--draws", type=int, default=2)
    ap.add_argument("-o", "--out", default="corral.svg")
    ap.add_argument("--width", type=int, default=150)
    ap.add_argument("--height", type=int, default=45)
    args = ap.parse_args()

    if not os.environ.get("OPENROUTER_API_KEY"):
        print("OPENROUTER_API_KEY is not set")
        return 2

    room, cfg = asyncio.run(_run(args.seed, args.rounds, args.draws))
    cfg["max_rounds"] = args.rounds
    busy = max(room.tables, key=lambda t: len(t.messages))

    rec = Console(record=True, width=args.width, height=args.height, force_terminal=True, color_system="truecolor")
    room.focus = -1
    rec.print(compose(room, cfg, (args.width, args.height)))
    room.focus = busy.id - 1
    rec.print(compose(room, cfg, (args.width, args.height)))
    rec.save_svg(args.out, title="The Corral")

    txt = Console(record=True, width=args.width, height=args.height, force_terminal=False)
    room.focus = -1
    txt.print(compose(room, cfg, (args.width, args.height)))
    room.focus = busy.id - 1
    txt.print(compose(room, cfg, (args.width, args.height)))
    with open(args.out.rsplit(".", 1)[0] + ".txt", "w") as fh:
        fh.write(txt.export_text())
    print(f"wrote {args.out} · {room.turn} rounds · {room.messages} lines · {len(room.leaks)} eavesdrops · "
          f"${room.cost:.4f} · jev {room.calls_jev} · llama {room.calls_llama}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
