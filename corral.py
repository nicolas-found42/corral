"""The Corral: five tables of four cheap 2024 Meta Llama 3.1 8B instances, talking
at once about one seed, refereed live by TypeSafe Jev -- and occasionally
overhearing each other.

    uv run corral.py                     # asks for a seed, then runs
    uv run corral.py -t "your seed"      # run straight from a seed (any text)
    uv run corral.py -t "..." -n 6       # six rounds then stop
    uv run corral.py --headless -t "..." # no TUI: stream all five chats (scriptable)
    uv run corral.py --once              # render one frame and exit (no network)

Keys: q quit · space pause · 1-5 open a table · o overview · Tab/. cycle · t new
seed · s save · f flat · h help.
"""
from __future__ import annotations

import argparse
import asyncio
import os
import sys
import threading
import time

try:
    from rich.console import Console
    from rich.live import Live

    from room import Room, empty_room
    from session import Session
    from tui import compose

    console = Console()
except ModuleNotFoundError as e:  # a beginner running `python3 corral.py` with no toolbox
    sys.stderr.write(
        "\n  The Corral needs its own small toolbox (a Python 'virtual environment')\n"
        "  that isn't loaded when you run it this way.\n\n"
        "  Start it the easy way instead:\n\n"
        "      uv run corral.py\n\n"
        "  or double-click the file  “The Corral.command”  in this folder.\n\n"
        f"  (missing Python package: {e.name})\n\n"
    )
    raise SystemExit(1)


# --------------------------------------------------------------------- key input
class KeyReader:
    """Put stdin in cbreak mode and hand each key to a callback. macOS/Linux only."""

    def __init__(self, on_key) -> None:
        self.on_key = on_key
        self._stop = threading.Event()
        self._fd = sys.stdin.fileno()
        self._saved = None

    def __enter__(self) -> "KeyReader":
        try:
            import termios
            import tty

            self._saved = termios.tcgetattr(self._fd)
            tty.setcbreak(self._fd)
        except Exception:
            self._saved = None  # not a tty (e.g. piped): run without keys
        self._active = True
        self._t = threading.Thread(target=self._loop, daemon=True)
        self._t.start()
        return self

    def __exit__(self, *a) -> None:
        self._stop.set()
        self._active = False
        if self._saved is not None:
            try:
                import termios

                termios.tcsetattr(self._fd, termios.TCSADRAIN, self._saved)
            except Exception:
                pass

    def suspend(self) -> None:
        """Hand the terminal back in normal, echoing mode for a text prompt."""
        if self._saved is None:
            return
        self._active = False
        try:
            import termios

            termios.tcsetattr(self._fd, termios.TCSADRAIN, self._saved)
            termios.tcflush(self._fd, termios.TCIFLUSH)
        except Exception:
            pass

    def resume(self) -> None:
        if self._saved is None:
            return
        try:
            import tty

            tty.setcbreak(self._fd)
        except Exception:
            pass
        self._active = True

    def _loop(self) -> None:
        import select

        esc = False  # an ESC was just seen; arrow/Fn keys arrive as ESC + more bytes
        while not self._stop.is_set():
            if not self._active:  # suspended: the prompt owns the terminal
                esc = False
                time.sleep(0.02)
                continue
            try:
                r, _, _ = select.select([self._fd], [], [], 0.15)
            except Exception:
                return
            if not r:
                if esc:  # a lone ESC -> treat as a real Escape
                    esc = False
                    try:
                        self.on_key("\x1b\x1b")
                    except Exception:
                        pass
                continue
            try:
                ch = os.read(self._fd, 1).decode(errors="ignore")
            except Exception:
                return
            if not ch:
                continue
            if ch == "\x1b":
                try:
                    r2, _, _ = select.select([self._fd], [], [], 0.02)
                except Exception:
                    r2 = []
                if r2:
                    try:
                        os.read(self._fd, 8)  # eat the rest of an arrow / function key
                    except Exception:
                        pass
                    continue
                esc = True
                continue
            try:
                self.on_key(ch)
            except Exception:
                pass


# -------------------------------------------------------------------- rendering
async def run_tui(room: Room, cfg: dict) -> None:
    """Drive the session and repaint the frame ~10x a second."""
    sess = Session(room, turns=cfg["draws"], seed=cfg["seed"], max_rounds=cfg["max_rounds"])
    cfg["max_rounds"] = sess.max_rounds
    task = asyncio.create_task(sess.run())

    commands: list[str] = []

    def on_key(ch: str) -> None:
        commands.append(ch)

    quit_requested = False
    n_tables = len(room.tables)

    def save() -> None:
        path = f"corral-{time.strftime('%Y%m%d-%H%M%S')}.txt"
        try:
            with open(path, "w") as fh:
                fh.write(f"The Corral — five tables on: {room.seed}\n{room.turn} rounds\n")
                for t in room.tables:
                    fh.write(f"\n=== {t.name} (on: {t.summary or 'the seed'}) ===\n")
                    for m in t.messages:
                        tag = f"  ⟪heard from table {m.heard_from}⟫" if m.heard else ""
                        fh.write(f"{m.persona.name}:{tag} {m.text}\n\n")
            room.log_line(f"saved every table's chat to {path}", "#8fd694")
        except OSError as e:
            room.log_line(f"could not save: {e}", "#ff6b6b")

    def ask(question: str) -> str:
        keys.suspend()
        live.stop()
        try:
            return console.input(f"[bold #ffb454]{question}[/] ").strip()
        except (EOFError, KeyboardInterrupt):
            return ""
        finally:
            live.start()
            keys.resume()

    with KeyReader(on_key) as keys, Live(console=console, screen=True, auto_refresh=False, transient=False) as live:
        while not task.done():
            live.update(compose(room, cfg, console.size), refresh=True)
            while commands:
                ch = commands.pop(0)
                if ch in ("q", "Q", "\x03", "\x1b\x1b"):
                    sess.stop()
                    quit_requested = True
                elif ch == " ":
                    paused = sess.toggle_pause()
                    room.set_status("paused" if paused else "discussing", "paused" if paused else "back at the tables")
                elif ch in ("o", "O", "0", "`"):
                    room.focus = -1
                elif ch in "12345" and int(ch) <= n_tables:
                    room.focus = int(ch) - 1
                elif ch in ("\t", ".", "]"):
                    room.focus = -1 if room.focus >= n_tables - 1 else room.focus + 1
                elif ch in ("[", ","):
                    room.focus = n_tables - 1 if room.focus <= 0 else room.focus - 1
                elif ch in ("+", "="):
                    cfg["draws"] = min(4, cfg["draws"] + 1)
                    room.log_line(f"judgement draws per turn: {cfg['draws']}", "dim")
                elif ch in ("-", "_"):
                    cfg["draws"] = max(1, cfg["draws"] - 1)
                    room.log_line(f"judgement draws per turn: {cfg['draws']}", "dim")
                elif ch in ("s", "S"):
                    save()
                elif ch in ("h", "H", "?"):
                    cfg["hint"] = not cfg.get("hint", False)
                    room.log_line("help shown" if cfg["hint"] else "help hidden", "dim")
                elif ch in ("f", "F"):
                    cfg["flat"] = not cfg.get("flat", False)
                    room.log_line("flat mode on (no colour)" if cfg["flat"] else "colour back on", "dim")
                elif ch in ("t", "T"):
                    room.set_status("paused", "type a new seed below")
                    seed = ask("New seed for all five tables (Enter to cancel) ›")
                    if seed:
                        sess.reseed(seed)
                    room.set_status("discussing", "back at the tables")
            await asyncio.sleep(0.1)
        live.update(compose(room, cfg, console.size), refresh=True)
        if quit_requested:
            sess.stop()
    if task.done():
        await task
    console.print(f"\n[bold #ffb454]The Corral[/] — {room.turn} rounds, {room.messages} lines across "
                  f"{len(room.tables)} tables, {len(room.leaks)} eavesdrops, ${room.cost:.4f}")
    if room.outcome:
        console.print(f"[dim]{room.outcome}[/dim]")


# --------------------------------------------------------------------- headless
async def run_headless(room: Room, cfg: dict) -> None:
    sess = Session(room, turns=cfg["draws"], seed=cfg["seed"], max_rounds=cfg["max_rounds"])
    watcher = asyncio.create_task(_watch(room))
    await sess.run()
    watcher.cancel()
    _print_transcript(room)
    console.print(f"\n[dim]{room.turn} rounds · {room.messages} lines · {len(room.leaks)} eavesdrops · "
                  f"${room.cost:.4f} · jev {room.calls_jev} · llama {room.calls_llama}[/dim]")


async def _watch(room: Room) -> None:
    seen = {t.id: 0 for t in room.tables}
    while True:
        for t in room.tables:
            while seen[t.id] < len(t.messages):
                m = t.messages[seen[t.id]]
                tag = f" (heard from T{m.heard_from})" if m.heard else ""
                print(f"[T{t.id}] {m.persona.name}{tag}: {m.text}", flush=True)
                seen[t.id] += 1
        await asyncio.sleep(0.2)


def _print_transcript(room: Room) -> None:
    console.rule(f"five tables on: {room.seed}")
    for t in room.tables:
        console.print(f"\n[bold]{t.name}[/bold] — on: {t.summary or 'the seed'}")
        for m in t.messages:
            tag = f"[magenta](heard from T{m.heard_from})[/magenta] " if m.heard else ""
            console.print(f"  {tag}[bold]{m.persona.name}[/bold]: {m.text}")
    if room.leaks:
        console.rule("the wire")
        for lk in room.leaks:
            console.print(f"  T{lk.src} ⟶ T{lk.dst}  ({lk.effect})  {lk.line[:70]}")


# ------------------------------------------------------------------------ main
def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    ap = argparse.ArgumentParser(description="The Corral: five tables of four llama-3.1-8b agents, one seed.")
    ap.add_argument("-t", "--seed", dest="seed_text", help="the shared seed: a question, a topic, a sentence, any text")
    ap.add_argument("-n", "--rounds", type=int, default=None, help="stop after this many rounds")
    ap.add_argument("--draws", type=int, default=2, help="independent Jev draws per table per turn (1-4)")
    ap.add_argument("--rng", type=int, default=7, help="random seed, for reproducible leaks")
    ap.add_argument("--headless", action="store_true", help="no TUI: stream all five chats to stdout")
    ap.add_argument("--once", action="store_true", help="render one frame and exit (no network)")
    return ap.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.once:
        room = empty_room(args.seed_text or "a seed nobody has typed yet")
        room.set_status("discussing", "the five tables sit down")
        console.print(compose(room, {"draws": args.draws, "max_rounds": args.rounds}, console.size))
        room.focus = 0
        console.print(compose(room, {"draws": args.draws, "max_rounds": args.rounds}, console.size))
        return 0

    if not os.environ.get("OPENROUTER_API_KEY"):
        console.print("[bold red]OPENROUTER_API_KEY is not set.[/bold red] "
                      "Export it (it lives in ~/.zshenv on this machine) and try again.")
        return 2

    seed = args.seed_text
    if not seed:
        try:
            seed = console.input("[bold #ffb454]what should all five tables talk about? ›[/] ").strip()
        except (EOFError, KeyboardInterrupt):
            seed = ""
    if not seed:
        seed = "is a crowd wiser than any one of us, or just louder?"

    room = empty_room(seed)
    cfg = {"draws": max(1, min(4, args.draws)), "max_rounds": args.rounds, "seed": args.rng,
           "hint": False, "flat": False}
    try:
        if args.headless:
            asyncio.run(run_headless(room, cfg))
        else:
            asyncio.run(run_tui(room, cfg))
    except KeyboardInterrupt:
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
