# Handoff — set up Matt Pocock's engineering skills in the `corral` repo

**Written:** 2026-10-03
**Next session's job:** run the `setup-matt-pocock-skills` skill in
`/Users/Nicolas/Documents/github/corral` and complete the per-repo configuration it
scaffolds (issue tracker, triage labels, domain docs).

## Suggested skills for the next agent

Load these with the Skill tool before starting:

1. **`setup-matt-pocock-skills`** (engineering) — the job itself. Prompt-driven, not
   a script: explore → present findings → confirm → write.
2. **`triage`** (engineering) — installed, and its presence decides whether Section B
   (triage labels) of the setup runs at all. It runs.
3. **`conventional-commits`** (conventional-commits) — for the commits you'll make.
   Note this project uses **`doc`**, not `docs`.

## Repo state (verified, not assumed)

- **Path:** `/Users/Nicolas/Documents/github/corral`
- **Remote:** `origin` → `https://github.com/nicolas-found42/corral` — created this
  session, **PUBLIC**, default branch `main`, pushed and tracking.
  `gh` is authenticated as `nicolas-found42` with `repo` scope.
- **Commits:** one (`feat: five tables of four llamas, refereed by Jev`). Working
  tree clean **before** the files listed under "Uncommitted" below.
- **Issue tracker today:** none configured. GitHub Issues is available and empty.
- **`glab` / GitLab:** not in play; the remote is GitHub.

### Uncommitted files from this session (commit them or fold them into the setup commit)

- `AGENTS.md` — new, full agent guidance for the repo.
- `CLAUDE.md` — new, deliberately a one-line pointer to `AGENTS.md`.

### Existing files the setup skill will inspect

- `README.md` — project overview, design provenance, cost.
- `FINDINGS.md` — what the Jev experiments found.
- `docs/` — **does not exist yet**. `docs/agents/`, `docs/adr/`, `GLOSSARY.md` are
  all absent. This is a greenfield setup.
- `.scratch/` — **does not exist**. No local-markdown tracker convention in use.
- Monorepo signals — none. This is a **single-context** repo (pure Python, one
  `pyproject.toml`, one test suite). Section C should default to single-context
  without asking.

## The decision already made for you (Section A)

**Issue tracker: GitHub.** The remote is GitHub and `gh` is authenticated, so the
skill's default posture applies and the user has stated this preference explicitly.
Write `docs/agents/issue-tracker.md` from the skill's
`issue-tracker-github.md` template.

Leave the **"PRs as a request surface"** flag **off** — the skill says not to raise
it, and there is no signal the user wants it on.

## The decision you must NOT make for them (Section A, file selection)

The setup skill's step 4 has a hard rule: *if `CLAUDE.md` exists, edit it; else if
`AGENTS.md` exists, edit it; if neither exists, **ask the user which one to create**
— never create one when the other exists.*

⚠️ **Both now exist** (see above). This puts you in a case the skill does not
explicitly cover. **Do not guess and do not delete either file.** Present the
situation to the user and ask which file should carry the `## Agent skills` block,
offering these two options:

- **(recommended) Edit `AGENTS.md`** and leave `CLAUDE.md` as its pointer. Keeps one
  source of truth and matches how the files were written; every tool that reads
  `CLAUDE.md` follows the link.
- **Edit `CLAUDE.md`** and leave `AGENTS.md` as the general doc. Matches the skill's
  "CLAUDE.md wins" ordering literally, at the cost of guidance living in two places.

Whichever they choose, if an `## Agent skills` block already exists in that file,
update it in place rather than appending a duplicate.

## Section B (triage labels) — runs

The `triage` skill **is installed**
(`~/.hermes/profiles/coder/skills/engineering/triage`), so this section runs. Ask
the one question the skill specifies: keep the default triage labels? Recommended
yes — the five canonical roles, each label string equal to its name:
`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`.

## Section C (domain docs) — single-context

No monorepo signals, so write **single-context** without asking: one `GLOSSARY.md`
plus `docs/adr/` at the repo root, using the skill's `domain.md` template.

## Concrete deliverables to produce

1. `docs/agents/issue-tracker.md` — from `issue-tracker-github.md`, PR flag off.
2. `docs/agents/triage-labels.md` — from the skill's `triage-labels.md`.
3. `docs/agents/domain.md` — from the skill's `domain.md`, single-context.
4. The `## Agent skills` block inserted into the file the user picks, containing the
   Issue tracker / Triage labels / Domain docs sub-blocks (the Triage labels
   sub-block only because `triage` is installed).
5. A commit for the above, Conventional Commits, `doc:` type (not `docs:`), e.g.
   `doc: configure the repo for the engineering skills`. Push it.

## Environment notes

- Python is pinned to **3.11** — `uv sync --python 3.11 --all-extras`. uv defaults a
  fresh worktree venv to 3.12 and the type stubs then break.
- `OPENROUTER_API_KEY` is required for the *app* to run but **not** for this setup
  task. Never print or commit it.
- `uv run pytest -q` should be **24 passed** on this tree and needs no network. Run
  it once as a sanity check before committing; the setup task itself touches no code.
- The main checkout's `.gitignore` ignores `launch.sh` and `The Corral.command`
  (they are on disk but untracked). Don't "fix" that unless the user asks.

## Out of scope

Do not extend the app, re-run any experiments, or touch `judge.py`'s thresholds —
this session is configuration only. The engineering skills (`to-spec`, `to-tickets`,
`triage`, `code-review`, `implement`, `wayfinder`) will read from the files you write;
they will not be exercised here.

## Pointers, not duplicates

- Project overview and design provenance: `README.md`
- Experiment results and their interpretation: `FINDINGS.md`
- Raw experiment data: `results/*.json`
- The app's own guidance for agents: `AGENTS.md` / `CLAUDE.md`
