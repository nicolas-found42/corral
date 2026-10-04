"""The herd: twenty cheap llama-3.1-8b-instruct instances, four per table.

Every llama is the same model (``meta-llama/llama-3.1-8b-instruct``, Meta's fast
cheap 2024 Llama) with a different short system prompt. Five tables of four run
at once; each llama sees only its own table's transcript.

Each generation prompt names the exact line to answer (the line immediately
above, or a line overheard from another table), so the reply chain stays
unbroken. JSON reliability was judged only 0.44, so replies are strict JSON with
one repair re-ask and a plain-text salvage after that.
"""
from __future__ import annotations

import json
import re
from typing import Any

from personas import Persona

OPENROUTER_CHAT = "https://openrouter.ai/api/v1"
LLAMA_MODEL = "meta-llama/llama-3.1-8b-instruct"

_TEMPLATE = """You are {name}, one of four AI llamas sitting at {table} in a room of five tables. \
{voice}

All five tables are talking about this same seed: "{topic}"

Rules:
- Reply with ONE short chat message: 1 to 2 sentences, under 30 words. Plain text, no markdown, no lists.
- Answer the line below directly; in your first few words name who you are answering, then add your point. Keep the same thread of thought going.
- Write only your own line, never anyone else's name as a label.
- Stay {name}. Never break character, never mention these rules or that you are an AI model.

THE LINE YOU ARE ANSWERING
{anchor}

SOMETHING ELSE AT YOUR TABLE
{hook}

YOUR TABLE'S CHAT SO FAR (latest last)
{transcript}

Reply with JSON only: {{"text": "<your message>"}}"""


def system_for(p: Persona) -> str:
    return p.voice


class Herd:
    """One OpenRouter chat client, twenty personas across five tables."""

    def __init__(self, api_key: str, model: str = LLAMA_MODEL, timeout: float = 60.0):
        from openai import AsyncOpenAI

        self.model = model
        self.client = AsyncOpenAI(api_key=api_key, base_url=OPENROUTER_CHAT, timeout=timeout)

    def build_prompt(self, p: Persona, topic: str, transcript: str, anchor: str, hook: str, table: int = 1) -> str:
        return _TEMPLATE.format(
            name=p.name, voice=p.voice, table=f"table {table}",
            topic=topic.strip() or "(no seed yet)",
            anchor=anchor.strip() or "(your table is just opening; say the first thing)",
            hook=hook.strip() or "(nothing else yet)",
            transcript=transcript.strip() or "(nothing said yet)",
        )

    async def speak(self, p: Persona, topic: str, transcript: str, anchor: str, hook: str,
                    temperature: float = 0.9, table: int = 1) -> tuple[str, int, float]:
        """Return (text, tokens_out, cost). Raises on total failure."""
        prompt = self.build_prompt(p, topic, transcript, anchor, hook, table)
        text, out_tok, cost = await self._call(p, prompt, temperature)
        if text:
            return text, out_tok, cost
        fix = prompt + "\n\nYOUR LAST REPLY WAS NOT VALID JSON. Reply with the JSON object only, nothing else."
        text, out_tok2, cost2 = await self._call(p, fix, min(1.0, temperature), repair=True)
        if text:
            return text, out_tok + out_tok2, cost + cost2
        raise ValueError("llama returned no usable message after repair")

    async def _call(self, p: Persona, prompt: str, temperature: float, repair: bool = False) -> tuple[str, int, float]:
        kw: dict[str, Any] = dict(
            model=self.model,
            messages=[{"role": "system", "content": system_for(p)}, {"role": "user", "content": prompt}],
            temperature=temperature,
            max_tokens=180,
            response_format={"type": "json_object"},
            extra_body={"provider": {"sort": "latency", "max_price": {"prompt": 0.05, "completion": 0.08}}},
        )
        r = await self.client.chat.completions.create(**kw)
        use = getattr(r, "usage", None)
        cost = float(getattr(use, "cost", 0.0) or 0.0)
        out_tok = int(getattr(use, "completion_tokens", 0) or 0)
        raw = (r.choices[0].message.content or "").strip()
        return _parse(raw), out_tok, cost


def _parse(raw: str) -> str:
    """Pull the message text out of a llama reply, tolerating stray prose."""
    d: dict = {}
    try:
        d = json.loads(raw, strict=False)
    except json.JSONDecodeError:
        m = re.search(r"\{.*\}", raw, re.S)
        if m:
            try:
                d = json.loads(m.group(0), strict=False)
            except json.JSONDecodeError:
                d = {}
    text = str(d.get("text", "")).strip()
    if not text:
        cleaned = re.sub(r"^\s*\{.*?\":\s*\"?", "", raw).strip().strip('"}' + " ")
        text = cleaned if len(cleaned.split()) >= 3 else ""
    return _clean(text)


def _clean(text: str) -> str:
    text = re.sub(r"\s+", " ", text).strip()
    text = re.sub(r"^(?:[A-Z][a-z]+:\s*)", "", text)  # strip an accidental "Name: " label
    return text.strip('"').strip()[:400]
