"""The session: five tables of four llamas talking at once, with eavesdrops.

  seed in -> Jev screens it -> round robin: each table takes a turn and its next
  line is judged -> between rounds Jev decides whether a line carries to another
  table -> repeat.

A round is one message added to each table. The five tables take their turns in
parallel, then the eavesdrop check runs once for the whole round. Judgement cost
is accounted inside :class:`judge.Conductor`; llama cost here.
"""
from __future__ import annotations

import asyncio
import os
import random
import time

from herd import Herd
from judge import LEAK_EVERY, Conductor
from personas import BY_ID
from room import Message, Room, Table

WINDOW = 16   # transcript window handed to a speaker
DRAWS = 2     # independent Jev draws per table per turn


class Session:
    def __init__(self, room: Room, turns: int = DRAWS, seed: int = 7, max_rounds: int | None = None):
        self.room = room
        self.turns = turns
        self.rng = random.Random(seed)
        self.max_rounds = max_rounds
        self._stop = asyncio.Event()
        self._pause = asyncio.Event()
        self._pause.set()
        self._pending_seed: str | None = None
        key = os.environ.get("OPENROUTER_API_KEY")
        if not key:
            raise RuntimeError("OPENROUTER_API_KEY is not set")
        self.conductor = Conductor(key)
        self.herd = Herd(key)

    # ----------------------------------------------------------------- control
    def stop(self) -> None:
        self._stop.set()

    def toggle_pause(self) -> bool:
        if self._pause.is_set():
            self._pause.clear()
            return True
        self._pause.set()
        return False

    @property
    def paused(self) -> bool:
        return not self._pause.is_set()

    def reseed(self, seed: str) -> None:
        self._pending_seed = seed

    # -------------------------------------------------------------------- run
    async def run(self) -> None:
        room = self.room
        room.started = time.time()
        room.set_status("discussing", "the five tables sit down")
        async with self.conductor:
            try:
                await self._first_round()
                while not self._stop.is_set():
                    if self.max_rounds is not None and room.turn >= self.max_rounds:
                        room.set_status("stopped", outcome=f"reached {self.max_rounds} rounds")
                        break
                    if self._pause.is_set() and self._pending_seed:
                        await self._reseed()
                    await self._pause.wait()
                    if self._stop.is_set():
                        break
                    await self._round()
            except Exception as e:  # surfaced in the UI, never swallowed
                room.set_status("error", outcome=f"{type(e).__name__}: {str(e)[:160]}")
                room.log_line(f"error: {type(e).__name__}: {str(e)[:160]}", "#ff6b6b")
            finally:
                if room.status == "discussing":
                    room.set_status("stopped", outcome="stopped")
                if room.messages:
                    await self.conductor.verdicts(room)
                room.log_line(f"tables closed after {room.turn} rounds, ${room.cost:.4f}", "dim")

    # ---------------------------------------------------------------- rounds
    async def _first_round(self) -> None:
        room = self.room
        room.log_line(f'five tables, one seed: "{room.seed[:70]}"', "dim")
        await asyncio.gather(*(self._turn(t) for t in room.tables))

    async def _round(self) -> None:
        room = self.room
        room.turn += 1
        room.phase = f"round {room.turn}: all five tables talking"
        await asyncio.gather(*(self._turn(t) for t in room.tables))
        if room.turn % LEAK_EVERY == 0:
            room.phase = "listening across the tables"
            await self.conductor.pick_leak(room)

    # ------------------------------------------------------------------ turn
    async def _turn(self, table: Table) -> None:
        room = self.room
        j = await self.conductor.conduct(room, table, self.turns)
        table.judgement = j
        if j.summary:
            table.summary, table.summary_from = j.summary, j.summary_from
        if j.gauges:
            table.gauges.update(j.gauges)
            table.note_gauges()
        table.note_beliefs()
        speaker = j.chosen if j.chosen in table.members else self.conductor._least_recent(room, table)
        await self._speak(table, speaker, j.reply_to)

    async def _speak(self, table: Table, llama: str, reply_to: int | None) -> None:
        room = self.room
        p = BY_ID[llama]
        ls = room.llamas[llama]
        ls.speaking = True
        anchor, hook, answering_heard = self._anchor(table, reply_to)
        try:
            text, out_tok, cost = await self._speak_with_retry(table, room, p, anchor, hook)
        except Exception as e:
            ls.speaking = False
            room.last_error = f"{type(e).__name__}: {str(e)[:110]}"
            room.log_line(f"[table {table.id}] {p.name} could not reply: {room.last_error}", "#ffd75f")
            return
        ls.speaking = False
        if not text.strip():
            return
        room.calls_llama += 1
        room.cost_llama += cost
        room.tok_out += out_tok
        j = table.judgement
        m = Message(turn=table.messages[-1].turn + 1 if table.messages else 0, llama=llama, text=text,
                    reply_to=reply_to, family=j.family, move=j.move, quality=dict(j.quality),
                    composite=j.composite, figure=j.figure, asserts_claim=j.asserts_claim,
                    stance=j.stance, cohesion=j.cohesion)
        table.messages.append(m)
        ls.spoken += 1
        ls.last_spoke_turn = room.turn
        if answering_heard:
            room.log_line(f"[table {table.id}] {p.name} answers the line overheard from table {answering_heard}", "#c98cff")
        room.log_line(f"[t{table.id}] {p.sigil} {p.name}: {text[:78]}", table.colour)

    async def _speak_with_retry(self, table: Table, room: Room, p, anchor: str, hook: str):
        text, out_tok, cost = "", 0, 0.0
        penalty = ""
        for _ in range(3):
            text, out_tok, cost = await self.herd.speak(
                p, room.seed, table.tail_text(room, WINDOW), anchor, hook + penalty,
                temperature=self.rng.uniform(0.75, 1.05), table=table.id,
            )
            if not table.is_near_duplicate(text):
                return text, out_tok, cost
            penalty = "\n\nSomeone just said almost exactly that. Say something clearly different."
        return text, out_tok, cost

    # ---------------------------------------------------------------- anchors
    def _anchor(self, table: Table, reply_to: int | None) -> tuple[str, str, int | None]:
        if not table.messages:
            anchor = f"Your table is opening on the seed. Say the first thing."
            return anchor, "", None
        idx = reply_to if reply_to is not None and 0 <= reply_to < len(table.messages) else len(table.messages) - 1
        src = table.messages[idx]
        if src.heard_from is not None:
            anchor = (f"A line said at table {src.heard_from} drifted over to you: "
                      f"{BY_ID[src.llama].name} said “{src.text}”. You have all just heard it.")
            return anchor, "", src.heard_from
        anchor = f'{src.persona.name}: "{src.text}"'
        pool = [m for m in table.messages if m.turn != src.turn]
        hook = f'{pool[-1].persona.name}: "{pool[-1].text[:110]}"' if pool else ""
        return anchor, hook, None

    # ---------------------------------------------------------------- reseed
    async def _reseed(self) -> None:
        room = self.room
        seed = (self._pending_seed or "").strip()
        self._pending_seed = None
        if not seed:
            return
        room.phase = "screening the new seed"
        verdict, probs = await self.conductor.screen_seed(room, seed)
        room.log_line(f'seed "{seed[:50]}" -> {verdict} (injection {probs["injection"]:.2f}, topic {probs["topic"]:.2f})',
                      "dim" if verdict == "pass" else "#ffd75f")
        if verdict == "block":
            room.log_line("blocked: that reads like instructions to the llamas, not a seed", "#ff6b6b")
            return
        room.reset(seed)
