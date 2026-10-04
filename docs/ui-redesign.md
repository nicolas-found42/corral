# The Corral UI redesign — every fork, put to Jev

**Status: design settled, implementation pending.** This is the decision record for a
redesign of `src/tui.rs`. Twenty-five judgements went to TypeSafe Jev 1.13 before any
code: twenty-five `jev_decide` calls (one bounded choice each), two `jev_noul` batches
(calibrated probabilities over stated propositions), two `jev_classify` batches
(twelve items each, one shared catalog), and two `jev_verify` calls against the actual
rendered frames and the offline test suite. Every probability below is Jev's.

The brief: **prettier, more creative and more cutting-edge, without cluttering the
screen or sacrificing readability.** So the whole design is subtractive. Every fork
below reduces ink, reduces the number of saturated hues, or reclaims rows — and the two
things it adds are the app's own premise drawn once, and one quiet row of comparable
heat.

Baseline for reference: `cargo run --bin snapshot -- --demo` renders the current UI
offline from the corpus, at 150×45, to SVG and text.

---

## The frame it is improving on

Verified against the real render with `jev_verify` (6 of 6 claims `verified`, 0
contradicted):

| claim | verdict | p(supports) |
| --- | --- | --- |
| the detail frame renders five bordered rounded panels plus a bordered masthead | verified | 0.84 |
| every piece of prose is pure white | verified | 0.90 |
| one muted grey carries gauge names, poles, counts, round numbers and key hints | verified | 1.00 |
| the overview at 150×45 leaves unused blank rows at the bottom of the five-tables panel | verified | 0.97 |
| an eavesdropped line is marked by the literal word "heard" **and** a distinct colour | verified | 1.00 |
| the wire draws one row per eavesdrop inside a single grid sharing five columns | verified | 0.88 |

A second `jev_verify` (8 of 8 `verified`) pinned the invariants that constrain the
redesign: identity never rests on colour alone; the offline suite asserts the literal
tokens `⟪heard⟫`, `⟪heard from table 3⟫`, `●`, `▸`, `⟶`, "the newest line to cross",
"the judge", "the roster", "jev-1.13", "newest at the bottom", "Table N", both gauge
poles, and the three quality dimensions — at five terminal sizes, with `lines.len() == h`
and no line wider than `w`.

---

## The forks

### 1. Direction

| fork | Jev chose | p |
| --- | --- | --- |
| overall aesthetic | **refine the existing ember-dusk parlour** (indigo ground, one warm amber, the dusk ramp) rather than replace it | 0.94 |

`calm_slate` 0.00 · `editorial_mono` 0.06 · `layered_glass` 0.00 (one requirement
**contradicted**) · `do_nothing` 0.00.

### 2. Palette — how many colours, carrying what

| fork | Jev chose | p |
| --- | --- | --- |
| palette allocation | **one accent** (amber) for attention, **one reserved hue** for the eavesdrop channel, everything else a neutral ramp; table identity moves to sigil + name | 0.93 |

`keep_table_hues` 0.00 (two requirements contradicted) · `two_tier` 0.07 (one
contradicted) · `intensity_only` 0.00.

| fork | Jev chose | p |
| --- | --- | --- |
| neutral text tiers | **off-white body just below pure white + two grey tiers** with split roles (legible mid grey for labels, darker grey for decoration only) | 0.88 |

Supporting `jev_noul` (measured, not assumed):

| proposition | p |
| --- | --- |
| on a dark ground, pure white body text glares more than an off-white | 0.86 |
| dimming with the terminal's *faint* attribute is weaker than a lower-contrast colour, because many terminals do not render faint distinctly | 0.82 |
| the current muted colour carries too many distinct meanings to stay distinguishable | 0.62 |

> **Consequence:** de-emphasis is done by *lightness*, never by the `faint` modifier.

| fork | Jev chose | p |
| --- | --- | --- |
| how colour expresses the twenty agents | the sigil's hue encodes the agent's **voice family**, not the agent: six established family colours, names neutral | 0.81 |

> First pass returned **no winner above 0.5** (sigil-only 0.38, muted pairs 0.26, fully
> neutral 0.21, accent-for-active 0.08) — Jev flagged that twenty scattered sigil hues
> each mean nothing on their own and conflict with "few hues, each with a stable
> meaning". Re-put with the families in evidence: `family_hue_sigil` **0.81**, all three
> requirements supported, no contradiction. Each family has ≥2 members and every family
> already has a source-defined colour (`analytical #6fb7e0`, `narrative #e8c07a`,
> `playful #ff9a6b`, `skeptical #c98cff`, `empathic #ff7a90`, `systems #5fd0c0`). So
> the frame carries six hues that each name a *kind of mind in the room*, and the
> twenty are told apart by sigil shape and name.

### 3. Chrome — fewer, quieter borders

| fork | Jev chose | p |
| --- | --- | --- |
| panel chrome | every panel **keeps** a border, but every non-focused border drops to a **single dim hairline**; only the focused panel is bright — **superseded by §9: one-sided hairlines** | 0.84 |

`borderless_bands` 0.07 · `focused_box_only` 0.08 (one requirement contradicted) ·
`keep_rounded` 0.00 (two contradicted). Backed by `jev_noul`: removing panel borders in
a dense multi-panel frame risks losing the panel boundaries (**0.90**).

| fork | Jev chose | p |
| --- | --- | --- |
| focus signal | raise the focused panel's border from the dim neutral to neutral-white and bold its title — **lightness, no new glyph** | 0.58 |

`caret_mark` 0.22 · `gutter_rail_mark` 0.10 · `solid_badge_block` 0.08 (two
contradicted). Low confidence; a runner-up caret is a live alternative if the border
alone does not read.

| fork | Jev chose | p |
| --- | --- | --- |
| solid-filled blocks | **exactly one in the whole frame** — the title badge (black on accent) | 1.00 |

`title_and_focus` 0.00 (one contradicted) · `block_on_focus` 0.00 (one contradicted) ·
`no_solid_blocks` 0.00 (one contradicted).

| fork | Jev chose | p |
| --- | --- | --- |
| titles & labels | **demote 11 of 12 to dim, borderless running headers**; the words survive, the box corners and title slots do not | 11× `demote_dim_header`, 1× `keep_title` |

The lone `keep_title` is **"the wire"** (keep 0.49 vs demote 0.38 — itself a `review`
decision, so the wire keeps a full title). Demotion keeps every test-pinned token
present in the render, so the offline suite is unaffected.

### 4. The header, and reclaimed rows

| fork | Jev chose | p |
| --- | --- | --- |
| the gradient rule | **half the ink**: a single-glyph hairline carrying the same Oklab dusk ramp, no alternating `━`/`▔` texture | 0.98 |

Backed by `jev_noul`: a two-glyph alternating rule is noisier than a single-glyph
hairline (**0.84**), and across a full width it can read as dashed or broken rather
than as a horizon (**0.85**).

| fork | Jev chose | p |
| --- | --- | --- |
| masthead height | **two content rows**: title badge + status + metrics on the first, seed + the gradient hairline sharing the second | 0.73 |

`one_row_no_border` 0.26 · `title_to_footer` 0.00 (two contradicted) · `keep_three`
0.00 (one contradicted). Reclaims one row of body at every size.

| fork | Jev chose | p |
| --- | --- | --- |
| footer | **three contextual key hints** only (`space pause · s save · ? keys`), right-aligned; the full map stays behind the help toggle | 0.88 |

`keys_affordance_only` 0.11 · `keep_key_list` 0.00 (two contradicted) ·
`detail_only` 0.00 (one contradicted).

### 5. The body

| fork | Jev chose | p |
| --- | --- | --- |
| overview depth at 45 rows | **three message lines per table**, borders gone | 0.70 |

`two_line_rows` 0.20 · `one_line_rows` 0.02 (one contradicted) · `max_lines` 0.02 (two
contradicted).

| fork | Jev chose | p |
| --- | --- | --- |
| transcript message shape | **inline hanging head**: speaker, move glyph and text on one row, continuation lines hanging-indented to the text column | 0.77 |

`sigil_gutter_rail` 0.16 · `stacked_dim_head` 0.03 (one contradicted) · `keep_as_is`
0.00 (two contradicted). Three rows per message become two, and every line of prose
gets one clean left edge.

| fork | Jev chose | p |
| --- | --- | --- |
| detail band arrangement | **three bands** — judge / transcript / (roster │ beliefs inside one bordered band) + the wire | 0.31 |

> **Read the warning, not just the winner.** Jev ranked
> `transcript_with_side_rail` first (0.57) while its own requirement checks contradicted
> it on "reduces the number of bright border outlines"; `three_bands` passed all three
> requirements at the same evidence and ranked second at 0.31, with overall confidence
> 0.50 and a contradiction warning attached to the pick. Following the skill's rule —
> read the distribution and the checks, not only the ranking — the band layout is
> `three_bands`, and `keep_four_unbordered` (0.09, one contradicted) is out.

### 6. The wire — the app's one genuinely novel drawing

| fork | Jev chose | p |
| --- | --- | --- |
| resting lattice | **keep the block-glyph lattice** (`● ▸ ─ │ ┼`) recoloured to the single reserved hue, crossings dimmed | 0.88 |

`list_only_pulse_pill` 0.05 · `braille_whole` 0.03 (one requirement contradicted) ·
`per_leak_rail` 0.02 (one contradicted). The earlier open question —
`braille_lattice` 0.50 vs `restrained_glyphs` 0.45, confidence 0.42, the braille option
`unknown` on readability at 60 columns — resolves once the *pulse* is the braille
element and the *resting* drawing stays glyphs. `jev_noul` on the value of braille's
2×4 sub-cell resolution returned only 0.71 (uncertain), which is why it is not spent on
the whole lattice.

| fork | Jev chose | p |
| --- | --- | --- |
| the signature live element | a **braille eavesdrop pulse** travelling the wire from source column to destination column when a line is carried over | 0.78 |

`ambient_heat` 0.12 · `speaker_breath` 0.06 (one contradicted) · `belief_sparklines`
0.02 (one contradicted) · `polish_only` 0.02.

| fork | Jev chose | p |
| --- | --- | --- |
| the motion budget | **exactly one animated element**; every other live signal is a static glyph that changes only when its state changes | 1.00 |

`pulse_breath_dissolve` 0.00 (all three contradicted) · `pulse_and_breath` 0.00 (one
contradicted) · `fully_static` 0.00. Backed by `jev_noul`: one subtle live animation
reads as more polished without harming readability (**0.55** — the weakest proposition
in that batch, so the motion budget is the thing to re-measure against the built frame).

| fork | Jev chose | p |
| --- | --- | --- |
| pulse behaviour | a **single bright braille dot** travels the path and flashes once at the destination — no trail, no lingering | 0.28 |

Jev ranked the cheaper `thread_blink` first (0.45) but at low overall confidence (0.36),
and a blink contradicts the braille rendering already chosen above at 0.78;
`comet_with_trail` 0.10 had its "does not obscure the resting lattice" requirement
**contradicted**; `sweep_brighten` 0.13. `single_dot_pass` passes all three
requirements, so the travelling dot is taken and the blink is not.

| fork | Jev chose | p |
| --- | --- | --- |
| the whole-frame dissolve | **remove it entirely** | 0.97 |

`keep` 0.00 (all three contradicted) · `slower_subtler` 0.00 (two contradicted) ·
`wire_only_dissolve` 0.00 (two contradicted). The frame is redrawn on state change; the
pulse is the only motion.

| fork | Jev chose | p |
| --- | --- | --- |
| an overheard line in the transcript | neutral prose + a marker in the reserved hue + a **short vertical rail in the left gutter** down its rows | 0.91 |

`marker_only` 0.07 · `no_transcript_treatment` 0.01 · `whole_line_tinted` 0.00 (all
three contradicted — washing the whole line in colour makes the one line harder to read
than its neighbours). The literal tokens stay, because the suite pins them.

| fork | Jev chose | p |
| --- | --- | --- |
| ambient heat | a **heat ribbon row**: one shared row of five labelled segments, each a hairline bar whose length is that table's heat, coloured along the dusk ramp | 0.95 |

> First pass returned **no winner above 0.5** (badge tint 0.50, rail 0.27, row wash 0.14,
> none 0.06) and a full-row background wash was contradicted on contrast. Re-put with a
> shared row in evidence: `heat_ribbon_row` **0.95**, all three requirements supported,
> no contradiction. It reuses the dusk ramp the design already owns, changes no
> background behind prose, and makes all five tables comparable without moving the eye.

### 7. Instruments and markers

| fork | Jev chose | p |
| --- | --- | --- |
| the gauges, bars and sparkline | **hairline instruments**: a single `─` line with a subtle value marker and the two pole words for the gauges; short single-block runs for the quality dimensions; a **four-level** sparkline instead of eight | 0.85 |

`keep_sparklines_restyle_bars` 0.15 · `keep_blocks_dim` 0.00 (two contradicted) ·
`numbers_only` 0.00. `jev_noul`: a sparkline communicates a trend faster than a current
number alone (**0.82**), which is why the sparkline survives in reduced form.

| fork | Jev chose | p |
| --- | --- | --- |
| per-agent state markers | keep the **`▶`-style chosen marker** in the roster and move the quality `★` **out of the roster and ticker rows** into the focused transcript only | 0.33 |

Jev ranked `star_only` first (0.50) but with its own requirement **contradicted** — it
loses the roster's "who the judge picked" signal; `caret_only_quality_in_detail` passes
all three. `both_glyphs_muted` 0.12 · `one_live_marker` 0.01.

### 8. Glyph families

`jev_classify`, twelve items in one batch, one shared catalog:

| glyph | Jev | p |
| --- | --- | --- |
| discourse-move glyphs | **keep** | 0.95 |
| wire lattice glyphs | **keep** | 0.83 |
| sparkline `▁▂▃▄▅▆▇█` | restyle (fewer levels) | 0.79 |
| alternating horizon `━`/`▔` | restyle (single hairline) | 0.80 |
| block bar `▏▎▍▌▋▊▉█` | restyle | 0.73 |
| writing flare `✦✧✦⋆` | restyle | 0.66 |
| two-ended track `├───•────┤` | restyle | 0.64 |
| dotted table rule `····` | restyle (or drop: drop 0.36) | 0.63 |
| figure mark `⛭` | keep | 0.66 |
| heard marker | restyle 0.42 / **drop 0.40** — split, unresolved | 0.42 |
| composite `★` | restyle 0.41 / drop 0.22 — split, unresolved | 0.41 |
| chosen `▶` | keep 0.36 / drop 0.32 — split, unresolved | 0.36 |

The three split items are the ones §7 resolved by composition rather than by a single
classification.

---

## When a whole candidate pack lands under 0.5

Two forks came back with **every option below 0.5** on the first pass (how colour
expresses the twenty agents, and how heat is visible in the overview). In both cases the
catalog was missing the option that resolves the tension the requirements were pointing
at — a hue that carries a *meaning* rather than an identity, and a heat signal that is
*comparable across tables* rather than five separate badges. Adding that option, rather
than re-asking the same question, produced a decisive distribution (0.81 and 0.95, every
requirement supported, no contradiction). Re-putting the same wording would have earned
a `review`, not an answer.

---

### 9. Reconciling the research (after the fact)

Four cited research docs landed after the forks above. Three of their findings
**changed decisions**, so each went back to Jev rather than being absorbed quietly.

| fork | first decision | after the cited research | p |
| --- | --- | --- | --- |
| panel chrome | dim full box on every panel (0.84) | **one-sided hairlines** — each panel draws only the edge it shares with a neighbour; the focused panel draws its full box | 0.60 |
| table separation in the overview | dim the dotted rule (0.63) | **a bare `Layout::spacing` gutter, no glyph at all** — the only option passing all three requirements | 0.12 |
| the dissolve | remove it entirely (0.97) | **removed** — re-put against the research's `coalesce`-on-new-lines recommendation, which Jev contradicted and a rerank placed **last of 8 (0.38)** | 0.27 vs 0.71 |
| colour depth | (unasked) | **three palettes chosen at runtime** — truecolor, a pinned 256-colour map, a 16-colour role map — plus **`NO_COLOR`** wired to the existing flat mode | 0.97 |

Sources: `docs/research/tui-craft-survey.md` (one-sided hairlines, spacing, compactness
tiering, bottom-edge subtitles), `docs/research/tachyonfx-catalogue.md` (coalesce,
`with_rng`, clock-free determinism), `docs/research/ratatui-capabilities.md`
(`Layout::spacing`, `Flex`, `Block::title_bottom`, `CellDiffOption`,
`crossterm::style::available_color_count`), `docs/research/readability-and-colour.md`
(contrast thresholds).

**The dissolve, in full.** Jev ranked `coalesce_new_lines` 0.71 against
`remove_entirely` 0.27 — but its own requirement check **contradicted the coalesce
candidate** on "at most one moving element the reader would track", and a `jev_rerank`
over the eight surveyed techniques placed `scoped_coalesce` **8th of 8, relevance
0.38** (above it: one-sided hairlines 0.88, compactness tiering 0.88, bottom-edge
titles 0.87, focus-by-de-emphasis 0.82). So this is a case where the *distribution* and
the *cross-check* disagree with the winner, and the dissent is recorded rather than
resolved: **the frame ships with one animated element, the wire pulse**, and the scoped
coalesce stays on the shelf as a measured alternative to try if the built frame feels
too still. That is exactly the proposition Jev scored 0.55 on — the weakest in the set.

**The palette, with measured numbers.** `readability-and-colour.md` scores every hex
against the ground, and I recomputed the ratios independently (they match to 2 d.p.):

| role | current | measured | verdict | proposed fix |
| --- | --- | --- | --- | --- |
| body | `#ffffff` | 18.5:1 / Lc −107 | over the Lc 90 halation cap | **`#e8e6f2`** (15.0:1, Lc −92) |
| labels (`MUTED`) | `#8a86ad` | 5.37:1 / **Lc −39** | passes WCAG, **fails the Lc 60 chrome target** | **`#b8b5d8`** (9.4:1, Lc −64) |
| belief label (`INDIGO`) | `#5b6ee1` | **4.20:1 / 3.77:1** | **fails WCAG 4.5:1 wherever it carries text** | **`#aab6ff`** as text; the original stays for borders and ramp stops (3:1 applies) |

Two further constraints from the same source: **`NO_COLOR` must be honoured** (wire it
to the existing `f` flat mode), and the CVD analysis found four hue pairs that collapse
under simulated colour-vision deficiency (`AMBER`/`WARN` and `AMBER`/`EMBER` under
deuteranopia, `GOLD`/`GOOD` under protanopia, `EMBER`/`BAD` and `GOLD`/`WARN` under
tritanopia) — each must carry a glyph or a word, which the move glyphs and literal
tokens already do. **`AMBER`/`INDIGO` and `GOLD`/`HEARD` are safe under all three.**

> **Note on `Modifier::DIM`.** The craft survey recommends `Modifier::DIM` for
> de-emphasis; the readability research recommends a low-contrast colour. The earlier
> `jev_noul` (**0.82**) already decided this: dimming by *attribute* is unreliable
> because many terminals do not render faint distinctly. **De-emphasis is by lightness;
> `DIM` is not used for anything the reader must be able to read.**

---

### 10. What the render caught (the review loop)

The design above shipped, and then the built frame was critiqued against its own
screenshots. Two defects the design had *intended* to prevent were in the shipped
output, both found by measuring every glyph's colour against its background in the
rendered SVG rather than by reading the code:

| defect | measured | cause | fix |
| --- | --- | --- | --- |
| one grey carried **both** chrome and text | `#6f6b93` = **3.70:1**, across **584 word cells** | `FAINT` was documented as "decoration only, never text" and then used for every label, gauge pole, metadata field, hint and subtitle | split into `CHROME` (decoration, 3.7:1, non-text floor) and `DIM` (**6.81:1**, text floor) |
| the heat ribbon drew **invisible bars** | dusk stop `#2a2b52` = **1.38:1** | the ribbon walked the raw dusk ramp, whose cold half is under the 3:1 non-text floor | clamp the ribbon's low end to `#9d535c` (**3.37:1**) — `RIBBON_RAMP` |
| the header rule vanished for its first third | same ramp, drawn as an inline rule | the ramp as *decoration* has no floor | the header carries a **neutral hairline**; the ramp now appears only where it encodes heat |

Result, measured on the committed `corral.svg`: **584 → 0** word cells under 4.5:1,
worst contrast now **6.81:1**. Three unit tests were added so the class cannot return:
every text colour asserted at ≥ 4.5:1, every ribbon stop at ≥ 3:1 *and* the ramp it
replaced asserted to fail that floor, and the three neutral tiers asserted ordered
(`CHROME` < `DIM` < `LABEL` < `BODY`).

> The lesson, in the same shape as the earlier ones: **the code asserted an intent the
> frame did not honour.** A palette constant's doc comment is not a check; measuring
> the rendered cells is. The three tests that now guard this were all written *after*
> the defect shipped.

---

## What this adds up to

**Subtracted:** four of five bright-border outlines in the detail view · sixteen of
twenty saturated table/agent hues from the body (replaced by six family hues on one
glyph) · the alternating two-glyph horizon texture · one header content row · four of
six footer hints · the whole-frame dissolve · half the level-count of every instrument ·
the blank separators between transcript messages · the double-marking of overheard lines
· the quality star from the overview.

**Kept, deliberately:** the ember-dusk identity and the dusk ramp (as a hairline, and as
the heat ribbon) · the sigil + name + move-glyph identity that never rests on colour ·
the block-glyph wire lattice · one solid anchor in the title badge.

**Added:** two things. A bright braille dot that travels the wire when a line escapes
one room into another — the app's premise drawn as a moving dot, in the one panel where
it belongs — and a **heat ribbon**: one shared row of five hairline bars that makes the
five tables' judged heat comparable at a glance, painted from the dusk ramp the design
already owns.

---

## Still to settle (not judgements — engineering)

The research resolved the five open items above; what remains is implementation:

1. **Build the three palettes** (`readability-and-colour.md`): truecolor, the pinned
   256-colour index map, and the 16-colour role map, with the colour count read at
   runtime and `NO_COLOR` wired to `f` flat mode. Every text colour must clear 4.5:1 and
   |Lc| ≥ 60 **against its own 256-colour index**, which must be pinned and tested — a
   hex snapped to a nearby index drifts (`#5b6ee1` falls to 3.62:1).
2. **Adopt the ranked techniques** (`tui-craft-survey.md` + a `jev_rerank`): one-sided
   hairlines **0.88**, compactness tiering **0.88**, bottom-edge subtitles **0.87**,
   focus-by-de-emphasis **0.82**; zebra/tint 0.68, `Block::shadow` on the help overlay
   0.60 as optional polish. Compactness tiering is the one that most needs tests: as
   height falls, drop borders → gutters → beliefs → the wire, chrome dying before data.
3. **Draw the wire** (`ratatui-capabilities.md`): `Canvas` + `Marker`, with the pulse
   computed from the frame counter alone, and the resting lattice still block glyphs so
   the static frame stands on its own. The one-sided hairlines use `Layout::spacing` /
   `Flex`; bottom-edge titles use `Block::title_bottom`.
4. **Write the animation once** (`tachyonfx-catalogue.md`): effects are clock-free and
   advance only by the passed delta, so a fixed-delta, constant-seed run is reproducible
   — no `with_rng` needed while the pulse is hand-drawn. If the scoped coalesce is ever
   tried, it needs `with_rng(SimpleRng::new(seed))` and `for_each_cell`-style targeting,
   never `CellFilter::All` on the whole buffer.
5. **Re-measure after building.** The one proposition Jev left weak (0.55) is whether a
   single subtle animation reads as more polished without harming readability — that is
   a claim about the finished frame, not about the plan. Verify it with `--once` at
   five sizes and re-ask Jev if it does not hold.

---

### 11. Second creative round — five more layers, every fork to Jev (10 decides)

Brief changed: *way more creative*, with the no-clutter readability bar held. Two
layers of Jev per fork — a direction pick, then a detail pick — plus a verify and a
creativity read at the end.

| # | fork | Jev chose | p |
| --- | --- | --- | --- |
| 1 | speaker-head composition | **two-weight head**: bold WHO, dim WHAT after a separator | 1.00 |
| 1b | the separator | **middle dot** `·` (thin bar 0.43, runner-up, all checks pass) | 0.49 |
| 2 | wire over time | **age-faded timeline**, newest brightest | 0.99 |
| 2b | fade tiers | **three tiers**: reserved hue → label → chrome | 0.73 |
| 3 | band-header composition | **identity card**, fixed status order | 0.79 |
| 3b | status order | **lines → heard → open** | 0.99 |
| 4 | judge-band composition | **instrument strip**, one row per gauge set | 0.96 |
| 4b | gauge cell | **name → value → track**, poles named | 0.59 |
| 5 | transcript texture | **tight rail**, continuous gutter | 1.00 |
| 5b | reply context | **inline suffix** when it fits, own row when overlong | 0.52 |

Implementation notes: the instrument strip needed a column rule change (four inline
cells need ~40 cols each with poles named, so four columns only at ≥170; 150-wide
frames use two columns and read two rows of two cells). The reply suffix falls back
to its own dim row when the head row is full, so nothing ever wraps.

Multilayer read at the end: `jev_verify` 5/5 `verified`, all `auto`, against the
rendered frame. `jev_noul` on the creativity claims: reads more designed 0.76,
inline-reply preserves context 0.75, wire recency 0.71, speaker findability 0.66,
gauges read faster 0.58 — and two honest flags: adds-no-clutter only 0.44, and a
first-time reader learning every glyph without help only 0.28. The second flag was
fixed without touching the frame: `h` help now teaches the speaker mark
(`?` open, `⟪heard⟫` overheard), the inline `▸ answering` rule, and the wire's
brightest-is-newest rule.

Contrast re-measured on the committed render: 0 *word* cells under 4.5:1. The 4
remaining sub-4.5 cells are the oldest wire thread's `●▸` endpoints at 3.70:1 —
graphical marks by design, correctly above the 3:1 non-text floor.

---

### 12. Grading loop — 100 parameters per frame, then the fixes (target 0.90)

The frames looked the same, so both were graded on the same 100 absolute visual
parameters (four `jev_noul` batches of 50): OLD overall **0.594**, NEW **0.646**,
delta **+0.052**. Zero params at or above 0.90 on either frame — the instrument
itself tops out near 0.85 on aesthetic absolutes from text dumps, so 0.90 was
declared unreachable *on this meter* rather than chased by rewording (that would
be gaming the gauge, the visual equivalent of hard-coding a test).

The grade did its real job anyway: it ranked the deficits. Three bold fixes went
back to Jev and were built: **verdict headlines its band** (0.74; gauge-headline was
0.35), **newest thread double-tracked** with heavy horizontals (0.87; was 0.47) —
realised as `═`/`╪` rather than a second row, because the pick carried legibility
and 60-col unknowns and a word/marker fallback would have cost a row — and **one
continuous gutter rail** in the transcript (0.90; was 0.31). The quality legend
went to label grey (0.86; was 0.30). Re-grade of the 20 touched params: **0.734**.

Low scores that were evidence errors, not frame defects: three "old had none"
claims the old frame refuted (heat rail, open counts, micro-weather all predated
the round — Jev correctly scored them 0.29/0.26/0.05). The grade stands corrected,
not the frame.

---

### 13. Readability round — 40 parameters, per-parameter rises demanded

Baseline on the current frame (20 readability + 20 creativity): readability **0.433**,
creativity **0.669**, overall **0.551**. Worst scores were all reading mechanics, so
four forks went to Jev and were built: prose capped at a 76 book measure (**cap_76**,
0.68 — transcript prose went 149 → 78 chars wide), star + figure trailing the prose
(**trail_metadata**, 0.95), roster as a 2x2 seats grid (**seats_grid**, 0.95), and a
tiered beliefs band so unique information outlives duplicated information at mid
heights (**beliefs_over_roster**, 0.84, implemented tiered to keep its contradicted
size-monotone and pinned-token requirements green). Plus a no-Jev bug fix:
`shorten()` never splits a word.

Re-grade on the identical 40: overall **0.560** (+0.009). 17 rose, 23 did not — and
the pattern matters more than the mean. Every targeted fix rose where aimed
(seats +0.42, fatigue +0.35, respect +0.35, ellipsis +0.26, densest +0.26, metadata
+0.21, 60-col +0.21). But verdict-first fell 0.81 → 0.52 and rail-continuity 0.85 →
0.50 on features that did not change between the two renders. A meter that swings
±0.3 on identical inputs cannot certify per-parameter rises; the goal as stated is
unmeasurable with this instrument. Kept: the measured wins (prose 149 → 78, word
splits gone, seats grid, beliefs tier). Dropped: the idea that a re-grade number
proves any of it.
