# AGENTS.md

Guidance for any AI agent working in this repository.

## What this is

**The Corral** — five tables of four `meta-llama/llama-3.1-8b-instruct` agents
talking at once, all on one shared seed, refereed live by **TypeSafe Jev 1.13**
through the TypeSafe Python SDK (OpenRouter's System One endpoint). The judgement
model picks every speaker, grades every line, steers the room's thread, and decides
when one table overhears another.

The full design and provenance live in [`README.md`](./README.md). What the
experiments found lives in [`FINDINGS.md`](./FINDINGS.md).

## Getting set up

```sh
uv sync --python 3.11 --all-extras   # Python 3.11 is pinned deliberately
uv run corral.py                     # asks for a seed, then runs
uv run corral.py --once              # render one frame, no network
```

`OPENROUTER_API_KEY` must be in the environment (it lives in `~/.zshenv` on the
maintainer's machine). **Never print, commit or echo the key** — it is passed only
to the SDK clients.

## Layout

| file | what |
| --- | --- |
| `personas.py` | the twenty llamas: sigil, colour, voice family, voice |
| `room.py` | `Room` (five tables, the wire), `Table` (one conversation), composite weights, duplicate guard |
| `judge.py` | the Conductor: per-table batched Jev fan-out, eavesdrop picker, seed screening, verdicts |
| `herd.py` | the llama writers, strict-JSON replies with repair |
| `session.py` | the round loop: five tables in parallel, then the eavesdrop check |
| `tui.py` | overview (five cards + the wire) and detail (one table) |
| `corral.py` | entry point, key input, headless mode |
| `corpus.py` | the 36 real transcript lines the experiments read |
| `experiments.py` | the eight Jev-only experiments |
| `experiment_review.py` | Jev grading its own experiments; keep / drop / next |
| `ab_test.py` | A/B of two eavesdrop gate policies |
| `method_compare.py` | primitive comparison and phrasing-stability measurement |

## Commands

```sh
uv run pytest -q                     # the offline suite (no network, no spend)
uv run corral.py --headless -t "..." -n 6   # scriptable; streams all five chats
uv run snapshot.py -t "..." -n 10 -o corral.svg
uv run experiments.py                # Jev-only; no llama calls
uv run experiment_review.py
uv run ab_test.py
uv run method_compare.py
```

## Conventions

- **Every judgement the app makes is TypeSafe Jev's, not a rule in code.** If a new
  decision is semantic — who speaks, what a line means, whether a table is
  drifting — it belongs in `judge.py` as a typed question, batched into the
  existing per-turn fan-out rather than added as a new request.
- **Costs are measured, never assumed.** The llama is the cheap part; the judgement
  model is the point. A run is ~$0.012 for 10 rounds.
- **Thresholds are calibrated from data.** `judge.LEAK_WORTH` carries a comment
  explaining how it was measured and re-checked; keep that discipline for any new
  threshold.
- **Identity never rests on colour alone** — every llama carries a sigil + name;
  `f` toggles a flat, colourless mode.
- Tests are offline by design. Anything that would spend money stays out of
  `tests/`.

## Git

Conventional Commits, using the catalogs in the `conventional-commits` skill
(note: this project uses `doc`, not `docs`). One type and at most one context per
commit.

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `nicolas-found42/corral`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
