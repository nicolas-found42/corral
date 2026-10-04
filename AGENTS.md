# AGENTS.md

Guidance for any AI agent working in this repository.

## What this is

**The Corral** — five tables of four `meta-llama/llama-3.1-8b-instruct` agents
talking at once, all on one shared seed, refereed live by **TypeSafe Jev 1.13**
on OpenRouter's System One endpoint. The judgement model picks every speaker,
grades every line, steers the room's thread, and decides when one table overhears
another.

The full design and provenance live in [`README.md`](./README.md). What the
experiments found lives in [`FINDINGS.md`](./FINDINGS.md).

## Getting set up

```sh
cargo build --release                # builds the app and its binaries
cargo test                           # the offline suite (no network, no spend)
cargo run                            # asks for a seed, then runs
cargo run -- --once                  # render one frame, no network
git config core.hooksPath .githooks  # enable the pre-commit hook (fmt · clippy · test)
```

`.github/workflows/ci.yml` runs the same three checks. Re-run them by hand with
`cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test`.

`OPENROUTER_API_KEY` must be in the environment (it lives in `~/.zshenv` on the
maintainer's machine). **Never print, commit or echo the key** — it is read once in
`lib.rs::api_key()` and passed only to the two HTTP clients.

## Layout

| file | what |
| --- | --- |
| `src/personas.rs` | the twenty llamas: sigil, colour, voice family, voice |
| `src/room.rs` | `Room` (five tables, the wire), `Table` (one conversation), composite weights, duplicate guard |
| `src/judge.rs` | the Conductor: per-table batched Jev fan-out, eavesdrop picker, seed screening, verdicts |
| `src/herd.rs` | the llama writers, strict-JSON replies with repair |
| `src/session.rs` | the round loop: five tables in parallel, then the eavesdrop check |
| `src/tui.rs` | overview (heat ribbon + five table bands + the wire lattice) and detail (judge band · transcript · roster/beliefs · wire) as one-sided hairlines, the braille wire pulse, the three-tier palette |
| `src/bin/` | `corral` (entry point, key input, headless, `--once`), `snapshot`, `experiments`, `experiment_review`, `ab_test`, `method_compare` |
| `src/rng.rs` | CPython's Mersenne Twister, ported exactly, so `--rng` reproduces the leaks |
| `src/corpus.rs` | the 36 real transcript lines the experiments read |
| `src/experiments.rs` | the eight Jev-only experiments |
| `tests/port.rs` | the offline suite ported from the original |

## Commands

```sh
cargo test                           # the offline suite (no network, no spend)
cargo run -- --headless -t "..." -n 6   # scriptable; streams all five chats
cargo run --bin snapshot -- --demo -o corral.svg   # offline still from the corpus
cargo run --bin experiments          # Jev-only; no llama calls
cargo run --bin experiment_review
cargo run --bin ab_test
cargo run --bin method_compare
```

## Conventions

- **Every judgement the app makes is TypeSafe Jev's, not a rule in code.** If a new
  decision is semantic — who speaks, what a line means, whether a table is
  drifting — it belongs as a typed question, batched into the existing per-turn
  fan-out rather than added as a new request.
- **Costs are measured, never assumed.** The llama is the cheap part; the judgement
  model is the point. A run is ~$0.012 for 10 rounds.
- **Thresholds are calibrated from data.** `LEAK_WORTH` carries a comment explaining
  how it was measured and re-checked; keep that discipline for any new threshold.
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
