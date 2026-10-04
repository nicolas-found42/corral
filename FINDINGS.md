# What Jev found in The Corral's conversations

Eight experiments, one A/B and one method comparison, all run **Jev-only** — no
language-model generation anywhere. Every number below is a probability, a score
or a label the judgement model returned, over a corpus of **36 real transcript
lines** from five concurrent four-agent conversations. Total spend: **under two
cents** (~$0.003 across ~60 Jev calls).

Reproduce:

```
uv run experiments.py        # the eight experiments
uv run experiment_review.py  # Jev grades its own experiments
uv run ab_test.py            # A/B of two eavesdrop policies
uv run method_compare.py     # which primitive, which phrasing
```

## The findings

### 1. Most questions the room asks go unanswered — `4 of 5` still open
The strongest signal of the set (Jev's own scorecard put it top, composite 0.63).
A question is asked, the table talks past it, and nobody comes back. Example:
*"Historical precedent is all well and good, but what does it matter when the
outcome is still someone's ego inflating?"* — Jev scored it answered **0.04**,
open **0.78**.

**What to do with it:** a "did anyone answer that?" Noul is cheap and would let
the app *nudge* a table back to an open question. This is the single best
candidate for a new feature.

### 2. Eavesdrops land, but only just — `1 of 2` heard lines got a reply that addresses them
The line from table 2 → table 4 drew a reply Jev scored **0.92 addresses / 0.07
ignores** (`Bram: "…a desperate cry for meaning…"`). The one from table 5 → table
4 **split** (addresses 0.63, ignores 0.12). So a leak sometimes just colours the
next line instead of changing it.

### 3. The five tables are genuinely distinct — mean distinctness `0.67`
No pair scored as redundant (closest was T2/T4 at distinct 0.58 / same 0.57).
Five tables really do explore different ground rather than echoing each other.

### 4. Voice attribution from text alone: `64%` (chance = 25%)
You can often tell which llama said a line without being told. The misses cluster
in **table 3** (Vera / Marisol / Onyx / Paloma were confused for each other five
times) — the diplomatic and analytical voices overlap. **Actionable:** sharpen
those four personas. Table 4's voices (Bram / Sable / Fern / Tilly) were never
confused, so that seating works.

### 5. Flowing ≠ good — cohesion/quality correlation only `0.28`
But the most cohesive quartile of lines scored **0.556** on quality against
**0.342** for the least cohesive. So fluency helps *somewhat*, and the app's
cohesion score is a weak proxy for line quality, not a substitute.

### 6. Seed fidelity is mediocre — mean `0.62`, table 2 worst (`0.552`)
The lowest-scoring lines are atmospheric rather than substantive: *"Storm clouds
gather, Orson's words a thunderous…"* (0.11), *"Tension, the mood matches Sable's
wry remark"* (0.13). **Actionable:** the poetic/empathic personas drift from the
seed more than the analytical ones.

### 7. The eavesdrop gate is set correctly — and it was worth checking
A sweep of the real corpus:

| gate | lines admitted | logged leaks kept |
| --- | --- | --- |
| 0.40 | 17/36 | 3/3 |
| **0.45** | **15/36** | **3/3** |
| 0.46 | 13/36 | 2/3 |
| 0.50 | 10/36 | 2/3 |

Jev picked the **stricter** policy **0.84 vs 0.16**, and the seven lines a looser
gate would add score only **0.42–0.52** on "would a different table pick this up"
— ordinary remarks like *"Crowds are like the aunt at family reunions…"*. So the
gate stays at **0.46**; loosening it would decorate, not matter.

### 8. Best line per table, judged against the seed
T1 Dexter · T2 Byte · T3 Vera · T4 Ada · T5 Ada — **T4 won across all five**. But
Jev's scorecard rated this experiment *worst* (composite 0.34, signal 0.18): the
answer isn't discriminating enough to act on. It got dropped.

## Jev grading its own experiments

| experiment | signal | trust | surprise | rerun | composite |
| --- | --- | --- | --- | --- | --- |
| unanswered | 0.76 | 0.46 | 0.48 | 0.67 | **0.63** |
| leak-gate | 0.65 | 0.44 | 0.46 | 0.69 | 0.58 |
| seed-fidelity | 0.60 | 0.45 | 0.43 | 0.70 | 0.56 |
| voice-distinctness | 0.53 | 0.47 | 0.48 | 0.71 | 0.54 |
| cohesion-vs-quality | 0.54 | 0.43 | 0.44 | 0.69 | 0.53 |
| leak-absorption | 0.55 | 0.36 | 0.44 | 0.65 | 0.51 |
| redundancy | 0.45 | 0.44 | 0.43 | 0.69 | 0.49 |
| best-line | 0.18 | 0.36 | 0.43 | 0.53 | 0.34 |

* **keep:** `seed-fidelity` · **drop:** `best-line`
* **next:** `ablate_leak` (0.30) — run the app with eavesdrops switched off and
  compare the two corpora.

Note the **trust** column: every experiment sits near 0.45. Jev is telling us the
*corpus* is the limiting factor, not the questions — 36 lines is thin. That is the
honest headline of this whole exercise.

## Which primitive, and which wording (method comparison)

**The same question — "is this a good line?" — asked three ways:**

| instrument | mean | spread | range | Jev's trust |
| --- | --- | --- | --- | --- |
| Noul | 0.69 | 0.156 | 0.66 | 0.15 |
| Score | 0.53 | 0.206 | 0.87 | **0.67** |
| Choice | 0.27 | **0.270** | **0.91** | 0.18 |

A real tension: **Choice separates the lines most** (spread 0.270, range 0.91) but
Jev says the **Score** is the more usable instrument. Range and usability are not
the same thing.

**The same question in three wordings:** mean swing **0.237** per line. Jev says
that is *not* stable enough (0.35) and **not safe to use as a gate (0.10)**.

That is the most consequential finding here: **the app's per-turn gauges are
single-phrased Nouls.** This experiment says a single wording should not be
treated as precise — which is exactly why the experiments above ask every Noul
three ways and report the conservative minimum.

## Files

| file | what |
| --- | --- |
| `corpus.py` | the 36 real lines, tagged by table/speaker/source, plus the 3 logged eavesdrops |
| `experiments.py` | the eight Jev-only experiments (batched per experiment, multi-phrased) |
| `experiment_review.py` | Jev grading its own experiments, and picking keep / drop / next |
| `ab_test.py` | A/B of two eavesdrop gate policies on the same corpus |
| `method_compare.py` | primitive comparison and phrasing-stability measurement |
| `results/` | every raw result as JSON |
