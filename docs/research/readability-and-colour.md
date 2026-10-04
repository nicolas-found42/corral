# Readability and colour — cited rules for a dark-ground, truecolor TUI

Research for `corral`'s palette (`src/tui.rs:44-58`): a deep-indigo ground (`INK #12102b`,
`INK2 #1c1940`) lit by one warm accent (`AMBER #ffb454`), with a six-stop `DUSK` horizon ramp and
a `flat` accessibility mode. Every rule below carries a numeric threshold and a source URL.

Scope: `ratatui 0.30.2` + `crossterm 0.29` emitting truecolor `Color::Rgb` (`tui.rs:142`), with
`NO_COLOR` and a `flat` mode as the escape hatches. The three hard constraints from the brief hold
throughout: **identity never rests on colour alone**, the five tables stay legible at **60×18**, and
no clutter is added.

All contrast numbers in this document were computed from the exact hex values with two
implementations: WCAG 2.x relative luminance / contrast ratio, and the APCA-W3 0.0.98G-4g formula
transcribed from `apca-w3.js` (constants `blkThrs 0.022`, `blkClmp 1.414`, `scaleBoW/scaleWoB 1.14`,
`normBG 0.56 / normTXT 0.57 / revBG 0.65 / revTXT 0.62`, `loClip 0.1`, `loBoW/WoBoffset 0.027`).
Method sources:

- WCAG 2.x formula: <https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html>
  (`contrast ratio (L1 + 0.05) / (L2 + 0.05)`, relative luminance L).
- APCA formula + constants: <https://github.com/Myndex/apca-w3/blob/master/src/apca-w3.js>
  (`sRGBtoY`, `APCAcontrast`); polarity note: `"reverse contrast (white text on black)
  intentionally returns a negative number"`.

Throughout, **Lc is signed**: `Lc −N` = light text on dark ground (reverse polarity, "WoB"),
`Lc +N` = dark text on light fill (normal polarity, "BoW"). Magnitude is the readability number.

---

## 1. Minimum contrast for body text and for UI chrome

There are two live standards and they disagree on dark grounds; use both, WCAG as the floor and
APCA as the target.

### 1.1 WCAG 2.2 AA — the normative floor

| Content | Threshold | Source |
|---|---|---|
| Body text (< 18pt, or < 14pt bold) | **4.5:1** | SC 1.4.3 Contrast (Minimum), <https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html> |
| Large text (≥ 18pt / 24px, or ≥ 14pt / 18.5px bold) | **3:1** | same |
| UI components, states, graphical objects, focus indicators | **3:1** | SC 1.4.11 Non-text Contrast, <https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html> |

Hard rules from the sources: **do not round** — "4.499:1 would not meet the 4.5:1 threshold"
(1.4.3) and "2.999:1 would not meet the 3:1 threshold" (1.4.11). Contrast is measured against the
specified adjacent colour; hue is deliberately excluded ("contrast is calculated in such a way that
color (hue) is not a key factor", 1.4.3 Intent).

A terminal is the rare case where text and background are *always* both author-specified — the
WCAG failure mode "no background colour is specified" (Note 4) does not apply, because `corral`
paints its own ground.

### 1.2 APCA — the readability target (WCAG 3 candidate method)

WCAG 2.x "overstates contrast for dark colors to the point that 4.5:1 can be functionally unreadable
when one of the colors in a pair is near black. As a result, WCAG 2.x contrast cannot be used for
guidance designing 'dark mode'"
(<https://git.apcacontrast.com/documentation/APCA_in_a_Nutshell>). So on a dark ground, *add* an
APCA gate. APCA reports **Lc** from 0 to ~105+; **Lc 15 is the point of invisibility** and
**Lc 90 is preferred** for body text (same source). The APCA Readability Criterion (ARC) Bronze
conformance values, the plain-language equivalent of WCAG 2 AA:

| Use case | APCA minimum | Source |
|---|---|---|
| Body text (columns of fluent prose) | **Lc 75** (preferred **Lc 90**) | ARC Bronze, <https://readtech.org/ARC/tests/visual-readability-contrast/?tn=criterion> |
| Other content text, font ≥ 16px | **Lc 60** | same |
| Large fluent content (> 32px) | **Lc 45** (max Lc 90 for large/bold) | same |
| Spot-readable: placeholders, disabled text, captions | **Lc 30** | APCA in a Nutshell, <https://git.apcacontrast.com/documentation/APCA_in_a_Nutshell> |
| Solid semantic non-text (icons, rule marks ≥ 5.5px solid) | **Lc 30** | same |
| Non-semantic non-text that must merely be discernible (dividers, ≥ 5px solid) | **Lc 15** (treat below as invisible) | same |

**Rule 1.2a — chrome/annotation text.** `corral`'s labels and captions are not prose; they are
"content text that is not body, column, or block text" at a normal weight in a monospace cell
(~16px equiv). That maps to **Lc 60** on the APCA scale (and comfortably clears the WCAG 4.5:1
floor). Body/prose text (the transcript) targets **Lc 75–90**.

**Rule 1.2b — the flat/AAA-style uplift.** "For the equivalent to AAA, simply increase the contrast
values by Lc 15" (<https://git.apcacontrast.com/documentation/WhyAPCA>). So the AAA-grade targets
are body **Lc 90**, chrome **Lc 75**.

**Rule 1.2c — maximum, not just minimum.** For large/bold elements and large areas of colour, keep
contrast **below Lc 90** — "Maximum: for large fonts in dark mode, keep contrast less than Lc 90"
(<https://git.apcacontrast.com/documentation/WhyAPCA>). This is the halation guard rail (§2).

### 1.3 What the current palette actually scores

Measured against `INK #12102b` (body ground) and `INK2 #1c1940` (panel ground):

| Role | Hex | CR on INK | Lc on INK | CR on INK2 | Lc on INK2 | Verdict |
|---|---|---|---|---|---|---|
| body `#ffffff` | `#ffffff` | 18.51:1 | −107.1 | 16.63:1 | −105.9 | passes, but **over Lc 90 max** (halation) |
| `GOLD` `#ffe0a0` | `#ffe0a0` | 14.47:1 | −89.3 | 13.01:1 | −88.1 | OK (at the large/bold max) |
| `AMBER` `#ffb454` | `#ffb454` | 10.49:1 | −69.8 | 9.43:1 | −68.6 | OK for chrome (≥ Lc 60) |
| `WARN` `#ffd75f` | `#ffd75f` | 13.34:1 | −84.0 | 11.99:1 | −82.8 | OK |
| `GOOD` `#8fd694` | `#8fd694` | 10.76:1 | −71.1 | 9.67:1 | −69.9 | OK |
| `HEARD` `#c98cff` | `#c98cff` | 7.65:1 | −53.8 | 6.87:1 | −52.6 | CR passes, **Lc < 60** for small text |
| `EMBER` `#ff7a3d` | `#ff7a3d` | 7.14:1 | −51.2 | 6.42:1 | −50.0 | CR passes, **Lc < 60** for small text |
| `BAD` `#ff6b6b` | `#ff6b6b` | 6.67:1 | −48.3 | 5.99:1 | −47.1 | CR passes, **Lc < 60** for small text |
| **`MUTED` `#8a86ad`** | `#8a86ad` | **5.37:1** | **−38.9** | **4.83:1** | **−37.7** | CR passes, **Lc ≈ 39 ≪ 60 — fails the chrome target** |
| **`INDIGO` `#5b6ee1`** | `#5b6ee1` | **4.20:1** | **−30.6** | **3.77:1** | **−29.4** | **fails WCAG 4.5:1 on both grounds** |
| `CLARITY` `#6fb7e0` | `#6fb7e0` | 8.39:1 | −58.1 | 7.55:1 | −56.9 | borderline (Lc just under 60) |

**Findings.**
- `MUTED #8a86ad` is the workhorse "every label/caption" colour and it passes WCAG 2 AA (5.37:1)
  but scores **Lc −39**, far under the Lc 60 chrome target. Under APCA it reads as low-contrast
  caption text — the exact thing the APCA dark-mode critique warns about. On the `INK2` panel it
  drops to 4.83:1, with little headroom over 4.5:1.
- `INDIGO #5b6ee1` **fails WCAG 2 AA** for text at 4.20:1 on INK and 3.77:1 on INK2 (must be
  ≥ 4.5:1 for text < 18pt). As a non-text mark it is fine (≥ 3:1).
- `EMBER #ff7a3d`, `BAD #ff6b6b`, `HEARD #c98cff` pass 4.5:1 but sit at Lc ~48–54: fine as
  non-text marks / large glyphs, marginal for small caption text.
- `#ffffff` body at **Lc −107** is over the dark-mode ceiling (§2).

**Recommended fixes (measured; satisfy WCAG 4.5:1 *and* Lc ≥ 60 on *both* grounds):**

| Role | Current | Proposed | CR on INK | Lc on INK | CR on INK2 | Lc on INK2 |
|---|---|---|---|---|---|---|
| Caption/label (`MUTED`) | `#8a86ad` | **`#b8b5d8`** | 9.38:1 | −63.5 | 8.43:1 | −62.3 |
| Accent-as-text (`INDIGO`) | `#5b6ee1` | **`#aab6ff`** | 9.56:1 | −64.6 | 8.59:1 | −63.4 |
| Body text | `#ffffff` | **`#e8e6f2`** (off-white) | 15.01:1 | −91.7 | 13.49:1 | −90.5 |
| Ember-as-text (optional) | `#ff7a3d` | **`#ff9d70`** | 9.08:1 | −62.2 | 8.17:1 | −61.0 |
| Danger-as-text (optional) | `#ff6b6b` | **`#ffa0a0`** | 9.51:1 | −64.5 | 8.55:1 | −63.3 |

`INDIGO`/`EMBER`/`BAD` may stay exactly as they are **where they are non-text** (table borders,
ramp stops, sparkline bars, single large glyphs) because SC 1.4.11 only asks 3:1 there — they pass.
The fix is only needed where those hues *carry text*.

---

## 2. Dark backgrounds specifically: why pure white halates, and what to target

**Halation.** On a dark ground the eye adapts to the darker field, so the brightest elements
bloom and bleed into their surround; large or bold elements especially. The primary source states it
plainly: "Some users find **too much contrast in large/bold elements** may be distracting or
overwhelming due to **glare** or neurological issues"
(<https://readtech.org/ARC/guides/designing-with-visual-contrast/?tn=get-started>). APCA therefore
caps dark-mode contrast: "**Maximum: for large fonts in dark mode, keep contrast less than Lc 90**"
(<https://git.apcacontrast.com/documentation/WhyAPCA>; also ARC: "Maximum Contrast: Lc 90 (Large &
Bold Only)").

**Pure `#ffffff` on `#12102b` scores Lc −107**, i.e. above the Lc 90 dark-mode ceiling — the exact
over-bright condition the cap exists to prevent. It is not a WCAG failure (18.5:1), but on an OLED
or a low-ambient screen it is the classic dark-mode bloom source.

**Rule 2a — body text luminance.** Target body text in the **Lc 90–96** band, not 105+. Concretely
on this ground: use an **off-white around `#e8e6f2`–`#eeeeee`** (CR 15.0–16.0:1, Lc −92 to −96)
rather than `#ffffff`. Measured candidates:

| Hex | CR on INK | Lc on INK |
|---|---|---|
| `#ffffff` | 18.51:1 | −107.1 (too bright) |
| `#f5f5f7` | 17.00:1 | −100.6 |
| `#eeeeee` | 15.95:1 | −96.0 |
| **`#e8e6f2`** | 15.01:1 | **−91.7** (recommended: near-neutral, faint indigo cast) |
| `#e2e0ee` | 14.22:1 | −88.0 |

**Rule 2b — don't invert the ground.** Keep a *dark* ground and *light* text. APCA is polarity-aware
and computes reverse-polarity (light-on-dark) pairs with different constants; swapping to a light
ground changes every number in §1.3 and would invalidate the ramp design. Do not "fix" contrast by
inverting.

**Rule 2c — the ground itself must be a real, flat colour.** `INK #12102b` is near-black but tinted;
both `INK` and `INK2 #1c1940` are well under the APCA black-clamp threshold region and behave as
ground. Keep text-vs-ground luminance gaps large; do not lighten `INK` to chase contrast (that
reduces the gap for every foreground at once). `INK2` is only 1.11:1 against `INK` — that is a
*pane tint*, not a legible boundary, so it must **not** be the only cue that separates the panel
from the page; give panels a border or a glyph (see §4).

**Rule 2d — the low end too.** Anything below **Lc 15 is invisible** for many users (APCA in a
Nutshell). `DUSK0 #2a2b52` (CR 1.38:1, Lc 0) and `DUSK1 #4a3a6a` (1.85:1, Lc −8.7) are decoration
only — never let the horizon ramp's cold end *encode* something (a gauge reading, a status). Only
`DUSK3 #d06a4a` (5.15:1) and beyond carry enough luminance to read.

---

## 3. Colour-vision deficiency (CVD): hue pairs to avoid and keep

### 3.1 The failure mode to design against

"Do not rely on colour contrasts of hue and saturation" — readability must come from
**luminance** contrast, because "losing the ability to distinguish certain shades of colour does not
negatively affect light-dark contrast perception"
(<https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html>, Intent). Colourblind people
"rely more on the information from the blue cone cells … they tend to be more sensitive to
distinguish **bluish** hues than the non-colourblind. For example, colourblind people think 'red and
green' or 'yellow and yellow-green' are almost the same colour" — and the recommendation is to use
**magenta (equal red+blue) instead of red**, because the blue component survives
(<https://jfly.uni-koeln.de/color/> — Masataka Okabe & Kei Ito, "Color Universal Design").

Prevalence for calibration: "Per NIH, color insensitivity impacts approximately **0.5% of adult
women and 8% of adult men** (4.5% of the total population)"
(<https://designsystem.digital.gov/design-tokens/color/overview/>). Red–green is the most common.

### 3.2 Which of *corral's* hues collapse

Simulated with the Machado, Oliveira & Fernandes (2009) physiologically-based dichromat matrices
(<https://www.inf.ufrgs.br/~oliveira/pubs_files/CVD_Simulation/CVD_Simulation.html>), then compared
in CIE76 ΔE (≈10 ΔE or less ⇒ effectively indistinguishable). **This is the pair list to fix:**

| CVD type | Collapsing pair (corral hues) | ΔE | Why |
|---|---|---|---|
| **Deuteranopia** (~6% of men) | `AMBER #ffb454` / `WARN #ffd75f` | **8.5** | both collapse to the same yellow |
| Deuteranopia | `AMBER` / `EMBER #ff7a3d` | **10.9** | warm-yellow vs warm-orange merge |
| **Protanopia** (~2% of men) | `GOLD #ffe0a0` / `GOOD #8fd694` | **8.8** | pale gold ↔ green both go grey-green |
| **Tritanopia** (rare, ~0.01%) | `EMBER #ff7a3d` / `BAD #ff6b6b` | **5.6** | red vs orange-vs-red merge |
| Tritanopia | `GOLD #ffe0a0` / `WARN #ffd75f` | **7.4** | yellow pair merge |
| Deuteranopia | `BAD #ff6b6b` / `GOOD #8fd694` | 15.8 | classic red↔green, borderline |
| All three | `MUTED #8a86ad` / `CLARITY #6fb7e0` | 15.7–16.3 | desaturated grey-blue ↔ blue |

**Rule 3a — the AMBER/WARN and EMBER/BAD collisions are the dangerous ones** because they pair a
*status* meaning (`GOOD`/`WARN`/`BAD` are semantic) with a *decorative* hue (`AMBER`/`EMBER`).
A deuteranope cannot tell `AMBER` from `WARN`; a tritanope cannot tell `EMBER` from `BAD`. Never let
a warning and an accent differ only by these hues — a glyph or a word must separate them (§4).

**Rule 3b — safe pairs.** Under all three dichromacies the following survive *and* differ in
luminance by ≥ 3:1 (so they are WCAG-1.4.1-compliant even as a colour cue):
`AMBER #ffb454` / `INDIGO #5b6ee1` (10.49 vs 4.20, ΔE ≥ 24 under every simulation — warm vs cool),
`GOLD #ffe0a0` / `HEARD #c98cff` (ΔE ≥ 26), `AMBER` / `GOOD` (only ΔE 27.4 under protan, still
separable). General rule from Okabe–Ito: **use warm and cool colours alternately**; when forced to
two warm or two cool colours, "put distinct differences in brightness or saturation".

**Rule 3c — prefer pairs separated by luminance, not just hue.** Because `AMBER` is Lc −70 and
`INDIGO` is Lc −31, that pairing is *also* a lightness contrast. Even where hue collapses, the
lightness gap (≥ 3:1) keeps them distinguishable — this is the WCAG-1.4.1-approved escape: "if the
difference in relative luminance between the colors leads to a contrast ratio of 3:1 or greater"
this "counts as an additional visual distinction"
(<https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html>, Note).

**Rule 3d — keep the `flat` mode; do not rely on a CVD-simulated check.** Okabe–Ito caution that a
simulator is "not the true simulation of colourblind vision"; validate with both a simulation pass
*and* the redundant encodings in §4, and keep `flat` as the user's override.

---

## 4. Colour must never be the sole carrier of meaning

**WCAG 2.2 SC 1.4.1 Use of Color (Level A):** "Color is not used as the only visual means of
conveying information, indicating an action, prompting a response, or distinguishing a visual
element" (<https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html>). The Intent is explicit:
colour coding is fine "if it is complemented by other visual indication"; and where content relies
on "perceive or differentiate **a particular colour**, an additional visual indicator will be
required **regardless of the contrast ratio** between those colors." So even a 21:1 hue difference
does **not** satisfy 1.4.1 if colour is the only cue.

`corral`'s existing design already follows the rule (doc comment, `src/tui.rs:22-23`: "identity
never rests on colour alone: every llama carries a sigil + name, and every line carries a *move
glyph*"). The rules below make the invariant checkable.

**Rule 4a — one glyph per meaning, always present.** Every discourse move already has a distinct
non-colour glyph (`move_glyph`, `src/tui.rs:61-72`): `●` claim, `▹` evidence, `?` question,
`⊗` rebuttal, `≈` analogy, `✓` concession, `∿` tangent, `·` other. Keep this as the *primary*
encoding; colour is the secondary channel. The same applies to semantic status: `GOOD/WARN/BAD` must
each pair a hue with a word or a shape, never hue alone.

**Rule 4b — redundant encodings available in a monospace TUI** (all measured to keep glyphs
non-text-contrast ≥ 3:1): (i) **glyph/icon** — a distinct character (§4a); (ii) **shape of the rule
or border** — `━` vs `┄` vs `┈` for table boundaries; (iii) **weight/attribute** — bold `Modifier::BOLD`
for emphasis (works in `flat` mode too); (iv) **position** — the transcript is bottom-anchored and
the judge band is always on top, so *location* alone already distinguishes them
(`src/tui.rs:6-9`, `:24-25`); (v) **a literal label** — the word `warn` / `ok` / `bad` next to the
mark; (vi) **bar-fill density** — `BLOCKS " ▏▎▍▌▋▊▉█"` (`src/tui.rs:39`) and `SPARK "▁▂▃▄▅▆▇█"`
(`:40`) already encode magnitude by *fill*, independent of hue; keep them that way.

**Rule 4c — the `flat` mode must carry all meaning.** `Cfg::flat` (`src/tui.rs:110`) "strips all
colour for accessibility". Make it a *test invariant*: with `flat = true`, every distinction that
exists in colour mode must still be recoverable from glyph + text + position + fill. If any meaning
disappears in `flat`, that meaning was colour-only and violates 1.4.1.

**Rule 4d — `NO_COLOR`.** Honour the informal standard: an environment variable `NO_COLOR`, when
present and non-empty, "prevents the addition of ANSI color" regardless of value
(<https://no-color.org/>). Wire `flat` to `NO_COLOR` presence so a user with a colour-impaired
setup or a pipe gets monochrome automatically. This is a UX obligation, not a WCAG one, but it is
the cheapest 1.4.1 insurance.

**Rule 4e — never encode by hue where a simulation collapses it.** For each collapsing pair in §3.2,
require an independent non-colour cue. Concretely: `WARN` and `AMBER` must differ by more than hue —
e.g. `WARN` carries a `!` or `▲` and `AMBER` does not.

---

## 5. Terminal colour depth: what `#RRGGBB` does, and a graceful downgrade

`ratatui` emits `Color::Rgb(r,g,b)` (`src/tui.rs:142`) as a 24-bit SGR sequence. What the terminal
does with it depends entirely on the terminal's colour depth.

### 5.1 Truecolor (24-bit)

A truecolor terminal renders any sRGB `#RRGGBB` exactly. The escape is **ISO-8613-6** direct colour,
adopted by xterm. xterm's `ctlseqs` documents both the standard and legacy forms:

```
Ps = 3 8 : 2 : Pi : Pr : Pg : Pb   ⇒ Set foreground colour using RGB values.
Ps = 4 8 : 2 : Pi : Pr : Pg : Pb   ⇒ Set background colour using RGB values.
      (standard: colon-separated sub-parameters; colour-space id Pi ignored)
Ps = 3 8 ; 2 ; Pr ; Pg ; Pb        ⇒ legacy semicolon form, "for compatibility with KDE konsole"
```

Sources: <https://invisible-island.net/xterm/ctlseqs/ctlseqs.html> (SGR section). Crucially: "xterm
allows either colons (standard) or semicolons (legacy) to separate the subparameters (but after the
first colon, colons must be used)." Both forms set the same colour, and if xterm is *not* compiled
with direct-colour support it "uses the closest match in its palette for the given RGB Pr/Pg/Pb".
Detection convention: **`COLORTERM=truecolor`** (or `24bit`), advertised by VTE, Konsole, iTerm2;
terminfo exposes the capability as `RGB` since ncurses 6.0
(<https://github.com/termstandard/colors>).

Modern emulators accept `#RRGGBB` in config as sRGB. WezTerm: "you can use `#RRGGBB` to specify a
color code using the usual hex notation; eg: `#000000` is equivalent to `black`"
(<https://wezterm.org/config/appearance.html>). kitty and Ghostty likewise take hex RGB for their
palette entries. So `INK #12102b` / `AMBER #ffb454` are honoured literally on
kitty/Alacritty/WezTerm/Ghostty/iTerm2.

### 5.2 256-colour (8-bit)

The 256-colour palette is standardised as **16 base + a 6×6×6 colour cube + 24 grey steps**
(<https://github.com/termstandard/colors>: "256-color palette: 16 colors, plus a 6×6×6 color-cube
and a 24-level gray scale"). The cube's construction (xterm's `256colres.pl`) maps index
`n` (16–231) to `R,G,B ∈ {0,95,135,175,215,255}`. The 24 greys (232–255) run
`8 + 10·(n−232)`.

A `#RRGGBB` on a 256-colour terminal is **snapped to the nearest palette index** — xterm's own
wording: "uses the closest match in its palette for the given RGB" (ctlseqs, above). Nearest-index
results for this palette (measured):

| Colour | Hex | Nearest 256 index | Resulting RGB | CR on INK |
|---|---|---|---|---|
| INK | `#12102b` | 234 | (28,28,28) | (ground) |
| MUTED | `#8a86ad` | 103 | (135,135,175) | 5.39:1 |
| AMBER | `#ffb454` | 215 | (255,175,95) | 10.18:1 |
| GOLD | `#ffe0a0` | 223 | (255,215,175) | 13.74:1 |
| GOOD | `#8fd694` | 114 | (135,215,135) | 10.66:1 |
| BAD | `#ff6b6b` | 203 | (255,95,95) | 6.22:1 |
| INDIGO | `#5b6ee1` | 62 | (95,95,215) | 3.62:1 |

The important consequence: **256-colour snapping *moves* the colours** — `INDIGO` drops from 4.20:1
to **3.62:1** (now failing even for non-text 3:1). The warm hues survive; the cool blue and the
muted grey are the ones that drift. So a 256-colour downgrade needs its *own* contrast check, not
the truecolor one.

### 5.3 16-colour (4-bit ANSI)

The base 16 are the ECMA-48 ANSI colours plus the aixterm brights, addressed by SGR codes 30–37
/ 40–47 (standard) and 90–97 / 100–107 (bright). xterm's ctlseqs:

```
Ps = 3 0 … 3 7  ⇒ Set foreground colour to Black/Red/Green/Yellow/Blue/Magenta/Cyan/White
Ps = 9 0 … 9 7  ⇒ Same, bright versions (aixterm)
Ps = 4 0 … 4 7 / 1 0 0 … 1 0 7 ⇒ background
```

"While 8-color support is described in ECMA-48 2nd edition, the VT500 series (introduced in 1993)
were the first DEC terminals implementing 'ANSI' colour." The critical fact for a *palette*: the
**16 base colour values are user-settable** — a terminal theme redefines exactly what "red" is. So
on a 16-colour terminal you get *the user's* idea of the 8/16 names, not `corral`'s hexes. Two
users see different colours from the same `SGR 31`. There is no way to force a specific luminance;
you cannot guarantee any contrast ratio for a colour-*name* mapping.

### 5.4 Concrete graceful-downgrade strategy

Detect depth in this order (each step is a capability probe, not a guess):

1. **`flat` / `NO_COLOR`** → emit zero SGR colour (attributes only). Highest priority.
2. **`COLORTERM` contains `truecolor` or `24bit`** (or terminfo reports `RGB`) → use truecolor
   `Color::Rgb`; run the §1.3 checks against the *truecolor* numbers.
3. **`TERM` matches `*-256color`** (or `$TERM` contains `256color`) → map each palette entry to its
   **fixed chosen 256 index** (table in §5.2), not the library's runtime nearest-match, so the
   result is deterministic and testable; verify each chosen index clears WCAG 4.5:1 (text) / 3:1
   (non-text) against the chosen ground index.
4. **Otherwise (16-colour / dumb)** → do not attempt a hex palette. Collapse to the **base 8**
   ANSI colours mapped by *role*, and rely on the redundant encodings (§4) for everything the
   colour would have carried. Set a "reduced-contrast" flag and, where a colour carried a warning,
   surface the word/glyph instead.
5. **Pipe / not a TTY** → no colour at all.

Two extra guards, both cheap:

- **Never let a colour-only meaning survive a downgrade.** §4c's `flat` invariant, extended: the
  16-colour path must be as meaningful as `flat`.
- **The 256-index table is a fixture.** Because the nearest-index result is deterministic for a
  fixed palette, the chosen indices and their contrast ratios belong in a unit test (see §7), so a
  palette tweak that silently drops a below-3:1 index fails CI.

Ratatui/crossterm support all of this directly: `Color::Rgb`, `Color::Indexed(u8)`,
`Color::AnsiValue`, and `Color::Reset`. The `flat` path is `Color::Reset` / no style.

---

## 6. Text on a coloured background (the inverse badge)

An "inverse badge" swaps fg/bg: e.g. `AMBER #ffb454` as the *fill*, with dark ink text on top.
WCAG treats this identically (it is just a foreground/background pair), so the **same 4.5:1 text /
3:1 non-text thresholds apply**, and 1.4.11 additionally says the badge's boundary needs 3:1 against
its surround only if the badge has no other visible content to identify it.

**When it is safe:** safe when the fill's luminance is far enough from the ink's *and* the fill is
far enough from the page ground that the badge reads as a distinct chip. `GOOD`'s own note in 1.4.1
— "if content is distinguished by inverting an element's foreground and background colors, this
would pass (again, assuming that the foreground and background colors have a sufficient contrast)"
(<https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html>) — makes inversion an explicitly
blessed redundant encoding, *provided* the fill/ink pair clears contrast.

Measured: dark ink on each fill (text on badge), and white on fill for comparison:

| Fill | `#12102b` ink on fill: CR / Lc | `#ffffff` on fill: CR / Lc | Verdict |
|---|---|---|---|
| `GOLD #ffe0a0` | **14.47:1 / +88.8** | 1.28:1 / −15.9 | ink safe; white **fails** |
| `WARN #ffd75f` | **13.34:1 / +84.0** | 1.39:1 / −21.4 | ink safe; white **fails** |
| `GOOD #8fd694` | **10.76:1 / +71.9** | 1.72:1 / −34.7 | ink safe; white **fails** |
| `AMBER #ffb454` | **10.49:1 / +70.7** | 1.76:1 / −36.1 | ink safe (≥ Lc 75 for label text); white **fails** |
| `CLARITY #6fb7e0` | 8.39:1 / +59.7 | 2.20:1 / −48.0 | ink passes 4.5:1, Lc just under 60 |
| `HEARD #c98cff` | 7.65:1 / +55.5 | 2.42:1 / −52.4 | ink 4.5:1, Lc < 60 |
| `EMBER #ff7a3d` | 7.14:1 / +53.0 | 2.59:1 / −55.1 | ink 4.5:1, Lc < 60 |
| `BAD #ff6b6b` | 6.67:1 / +50.3 | 2.78:1 / −58.0 | ink 4.5:1, Lc < 60 |
| `MUTED #8a86ad` | 5.37:1 / +41.1 | 3.45:1 / −67.5 | ink 4.5:1 but Lc very low |
| `INDIGO #5b6ee1` | **4.20:1 / +32.8** | 4.41:1 / −75.8 | **ink fails 4.5:1**; white also fails |

**Rules.**
- **Rule 6a — dark ink, not white, for every warm fill.** `AMBER/GOLD/WARN/GOOD` all pass with the
  dark ink and **all fail** with white (≤ 3.45:1, some near 1.3:1). Use the ground colour
  (`#12102b`) as the badge text on any of these fills.
- **Rule 6b — badge text is text.** It needs **4.5:1** (WCAG 1.4.3); if it is small label text,
  also **Lc ≥ 60** (APCA chrome target). That rules out `INDIGO` (4.20:1) and leaves `MUTED`
  (Lc +41) only for large/bold badge text.
- **Rule 6c — the badge must not be the *only* cue.** 1.4.1 still applies: a coloured chip that
  means "warning" needs a word or glyph inside it, not just the fill colour. `WARN`/`BAD` chips must
  carry their glyph/word, exactly as the flat mode would render them.
- **Rule 6d — badge boundary.** If the badge has no visible content other than its fill, its edge
  needs **3:1** against the surrounding ground (1.4.11). A filled chip whose fill contrasts ≥ 3:1
  with the ground satisfies this automatically — `AMBER` on `INK` is 10.49:1, fine; a *near-ground*
  fill such as `INK2 #1c1940` (1.11:1) would not be.

---

## 7. Verification recipe (make it a test, not a vibe)

The numbers above are only useful if they stay true. Suggested fixtures (Rust unit tests, no
network):

1. **Implement both formulas in-repo** — WCAG 2.x relative luminance + contrast ratio, and APCA
   `sRGBtoY`/`APCAcontrast` with the constants listed at the top. Both are short and pure; ~40 lines
   total. Assert against the published reference pairs from
   <https://github.com/Myndex/SAPC-APCA/blob/master/README.md> (e.g. `#888 vs #fff → 63.056…`,
   `#fff vs #888 → −68.54…`, `#123 vs #def → 91.67…`) to prove the transcription is correct.
2. **Assert per palette entry:** for every foreground used *as text*, `CR(fg, INK) ≥ 4.5` and
   `CR(fg, INK2) ≥ 4.5`, and `|Lc| ≥ 60`. For every *non-text* mark (borders, ramp stops that are
   above Lc 15, glyphs), `CR ≥ 3.0`.
3. **Assert the dark-mode ceiling:** body text `|Lc| ≤ ~100` (the `#ffffff` → off-white rule, §2a).
4. **Assert the `flat` invariant** (§4c): with `flat = true`, a rendered frame contains no two cells
   that differ *only* in fg colour while carrying the same glyph and text.
5. **Assert the CVD pairs** (§3.2): the specific collapsing pairs above must each be accompanied by
   a non-colour cue. A cheap proxy: assert that `move_glyph` and the status glyph are never empty
   for any semantic value.
6. **Assert the 256-index table** (§5.2): each fixed index's simulated RGB clears the 4.5:1/3:1
   floors against the chosen ground index, so a palette edit cannot silently drop it.
7. **Manual probe:** `printf '\e[38;2;255;180;84mAMBER\e[0m\n'` on the target terminal to confirm
   truecolor; and run once with `NO_COLOR=1` to confirm `flat`.

---

## Sources

- WCAG 2.2 SC 1.4.3 Contrast (Minimum) — <https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html>
- WCAG 2.2 SC 1.4.11 Non-text Contrast — <https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html>
- WCAG 2.2 SC 1.4.1 Use of Color — <https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html>
- WCAG 2.2 Recommendation — <https://www.w3.org/TR/WCAG22/>
- WCAG 3.0 Working Draft — <https://www.w3.org/TR/wcag-3.0/>
- APCA / SAPC main documentation — <https://github.com/Myndex/SAPC-APCA>
- APCA in a Nutshell (Lc use-case levels) — <https://git.apcacontrast.com/documentation/APCA_in_a_Nutshell>
- Why APCA (dark-mode critique, AAA = +Lc 15, max Lc 90) — <https://git.apcacontrast.com/documentation/WhyAPCA>
- APCA Easy Intro — <https://git.apcacontrast.com/documentation/APCAeasyIntro>
- APCA Readability Criterion (ARC) Bronze/Silver/Gold — <https://readtech.org/ARC/tests/visual-readability-contrast/?tn=criterion>
- ARC design guide, "Designing with Visual Contrast" (glare / too-much-contrast) — <https://readtech.org/ARC/guides/designing-with-visual-contrast/?tn=get-started>
- APCA reference implementation / constants — <https://github.com/Myndex/apca-w3/blob/master/src/apca-w3.js>
- Okabe & Ito, "Color Universal Design" (CVD palette, magenta-not-red, redundant coding) — <https://jfly.uni-koeln.de/color/>
- Machado, Oliveira & Fernandes (2009), CVD simulation model — <https://www.inf.ufrgs.br/~oliveira/pubs_files/CVD_Simulation/CVD_Simulation.html>
- US Web Design System, colour and accessibility — <https://designsystem.digital.gov/design-tokens/color/overview/>
- no-color.org — <https://no-color.org/>
- xterm Control Sequences (`ctlseqs`), SGR 38/48 direct colour, ANSI/aixterm colours — <https://invisible-island.net/xterm/ctlseqs/ctlseqs.html>
- xterm FAQ, "Can I set a color by its number?" / 256-colour palette — <https://invisible-island.net/xterm/xterm.faq.html>
- xterm 256-colour palette construction — <https://github.com/ThomasDickey/xterm-snapshots/blob/master/256colres.pl>
- ECMA-48 (5th edition, 1991), SGR / colour — <https://www.ecma-international.org/wp-content/uploads/ECMA-48_5th_edition_june_1991.pdf>
- Terminal Colors (truecolor detection, `COLORTERM`, 256 layout) — <https://github.com/termstandard/colors>
- WezTerm appearance / `#RRGGBB` — <https://wezterm.org/config/appearance.html>
- kitty colour control / OSC stacking — <https://sw.kovidgoyal.net/kitty/color-stack.rst> (rendered: <https://sw.kovidgoyal.net/kitty/color-stack/>)
