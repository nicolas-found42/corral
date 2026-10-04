# TUI craft survey — techniques that add polish with less ink

**Target:** The Corral (`src/tui.rs`, 1869 lines): five table panels + an eavesdrop "wire"
lattice + a judge band, rendered almost entirely as `Paragraph` into a ratatui `Buffer`.
Current vocabulary: one `panel()` helper (`tui.rs:280`) that draws a **full** `BorderType::Rounded`
box on every surface, an Oklab "dusk horizon" rule (`ramp()`/`horizon_span()`, `tui.rs:163-307`),
block-char gauges (`bar()`, `tui.rs:183`), a sparkline (`spark()`, `tui.rs:212`), a two-ended
scale (`two_ended`, `tui.rs:204`), and one dissolve effect (`shimmer()`, `tui.rs:1836` —
`fx::dissolve`).

**Method.** Primary sources read directly (shallow `git clone --depth 1`). Versions/dates at the
head of each clone: ratatui `0.30.2` (2026-06-19), lipgloss `v2` (`charm.land/lipgloss/v2`,
2026-09-11), glamour (2026-10-02), bubbles `charm.land/bubbletea/v2` era, tachyonfx `0.25.2`
(2026-09-07), gitui `main`, bottom `main`, yazi `main` (2026-10-03), television `main`
(2026-09-29), lazygit `main`, atuin `main`, textual `main`. No claims below come from memory;
each carries a repo + file (line where useful).

**The through-line the sources agree on:** ink spent on *structure* (borders, boxes, rules)
de-emphasises content; ink spent on *difference* (one dim tone, one glyph, one tint, one moving
thing) survives. Every project below moved chrome toward the panel's edge and toward dimness,
and moved focus onto a single channel.

---

## 1. Focus by de-emphasis, not by emphasis — dim the unfocused chrome

**Looks like:** the focused panel's border/title is a real colour; every other panel's border is a
single grey `DarkGray`. You never add a highlight ring; you subtract light from four panels and
leave one alone.

**Where:**
- gitui `src/ui/style.rs:46` — `block(focus)` returns `Style::default()` when focused, else
  `Style::default().fg(self.disabled_fg)`; `title(focused)` (`:54`) is `block_title_focused` + `BOLD`
  vs `fg(disabled_fg)`. `disabled_fg` defaults to `Color::DarkGray` (Default impl, `src/ui/style.rs`).
- lazygit `pkg/gui/style/basic_styles.go` — `AttrDim = New().SetDim()`; `pkg/gui/style/decoration.go`
  maps `dim` → `color.OpFuzzy`. `pkg/gui/gui.go:1260-1263` binds `FgColor`/`FrameColor` to
  `theme.InactiveBorderColor` and `SelFgColor`/`SelFrameColor` to `ActiveBorderColor`.
- bottom `src/options/config/style/themes/default.rs` — `border_style: colour!(TEXT_COLOUR /*Gray*/)`,
  `highlighted_border_style: colour!(HIGHLIGHT_COLOUR /*LightBlue*/)`,
  `disabled_text_style: colour!(Color::DarkGray)`.
- television `television/screen/colors.rs` — `GeneralColorscheme { border_fg, background,
  dimmed_text_fg }` (three tones total for all chrome).

**Transfer:** give `panel()` a `focused: bool`. Focused → `border_style(fg(border))`; unfocused →
`border_style(fg(MUTED))` at reduced luminance. This alone lets you keep five boxes and still read
as one live panel. Cheaper variant: drop to a hairline (technique 2) for the four unfocused.

---

## 2. Hairline separators and one-sided borders

**Looks like:** a single `▏` column, or a `─` rule facing the neighbouring pane, instead of a
completed box. The separator belongs to the *space between* panels, not to either panel.

**Where:**
- television `television/screen/constants.rs` — `HAIRLINE_BORDER_SET`:
  `vertical_left: "▏", vertical_right: "▕", horizontal_top: "─", horizontal_bottom: "─"`, with the
  comment that eighth-block characters are "an eighth of the cell *height* horizontally, which
  renders visibly thicker than the eighth of the cell *width* used vertically, so horizontal
  separators use the light box-drawing line instead." That is a real, checkable rendering pitfall.
- television `television/screen/layout.rs` — `preview_hairline()`: when
  `preview_panel_border_type == BorderType::None`, it draws a hairline **on the one side facing the
  results list** (`results_facing_side()`), via `block.borders(...).border_set(HAIRLINE_BORDER_SET)`.
- television `television/config/ui.rs:237` — `enum BorderType { #[default] None, Plain, Rounded,
  Thick }`: **the default border is no border**, and a hairline only where two panes actually touch.
- lazygit `pkg/gui/views.go:178` — `frameRunes` defaults to `{'─','│','┌','┐','└','┘'}` with a
  `"hidden"` case `{' ',…}`; `views.go:187` picks `rounded`/`bold`/`double`/`hidden` per config.

**Transfer:** in the overview, replace five rounded boxes with: one `▔` rule under the masthead
(already the horizon), a blank row, then the five table rows separated by nothing but their own
dim `│`-prefixed speaker columns. Detail view: keep exactly one box (the transcript) and give the
wire a hairline on its top edge only. Use `Block::borders(Borders::LEFT).border_set(HAIRLINE)`
rather than `Block::bordered()`.

---

## 3. Whitespace as the separator (and a height budget that drops it)

**Looks like:** one blank row between bands instead of a rule; and when the terminal is short, the
blank row is the *first* thing dropped — not the data.

**Where:**
- bottom `src/constants.rs:10` — `TABLE_GAP_HEIGHT_LIMIT: u16 = 7`, with the comment "Limits for
  when we should stop showing table gaps/labels (anything less means not shown)"; the gap is
  `config.table_gap` threaded into every table (`src/widgets/process_table.rs:247`,
  `disk_table.rs:352`, `temperature_table.rs:134`).
- atuin `crates/atuin/src/command/client/search/interactive.rs:101` — `to_compactness(f, settings)`
  returns `Compactness::{Full, Compact, Ultracompact}` from the frame; `show_help` (`:986`) and
  `show_tabs` (`:992`) are gated on it, and the tab strip disappears entirely at `Ultracompact`.

**Transfer:** the corral already sizes every band from the terminal with a hard floor
(`tui.rs` module doc, "nothing starves"). Add a derived `Compactness` from `area.height`: `Full`
keeps the wire + beliefs + gaps; `Compact` drops the blank rows and the beliefs band; `Ultracompact`
keeps only transcript + judge band. Gaps are cheap and reversible; borders are neither.

---

## 4. One glyph, one variable (colour never carries meaning alone)

**Looks like:** a discourse move is a glyph (`● ▹ ? ⊗ ≈ ✓ ∿`), a speaker is a colour, and a *position*
is a marker. Strip colour entirely and the line still parses.

**Where:**
- The Corral itself: `move_glyph()` `tui.rs:61`; `ends()` `tui.rs:75`; module doc line 22
  ("identity never rests on colour alone").
- television `television/screen/constants.rs` — `POINTER_SYMBOL = "> "`, `SELECTED_SYMBOL = "● "`,
  `DESELECTED_SYMBOL = "  "`; consumed in `television/screen/result_item.rs` (a two-cell prefix
  column, styled in `selection_fg` only when selected).
- yazi `yazi-config/preset/theme-dark.toml` — `marker_symbol = "│"`, `border_symbol = "│"`: the
  *marker* and the *border* are the same glyph at different colours, so colour is the only
  difference and the glyph stays honest.

**Transfer:** keep it, and extend it — the eavesdrop wire should get a glyph family too (e.g. `⇢`
for a heard edge vs `↩` for a rebuttal edge) so the lattice reads without its colour coding.

---

## 5. Selection and focus as inverted video / underline on the *text*, not a box

**Looks like:** the focused row is `REVERSED`; a previewed item is `UNDERLINED`; a tab is
`UNDERLINED` + one accent colour. No frame moves.

**Where:**
- ratatui `ratatui-widgets/src/tabs.rs:16` — `const DEFAULT_HIGHLIGHT_STYLE: Style =
  Style::new().reversed();` (default divider `|`, per the widget docs).
- yazi `yazi-config/preset/theme-dark.toml` `[indicator]`: `parent = { reversed = true }`,
  `current = { reversed = true }`, `preview = { underline = true }`.
- gitui `src/ui/style.rs:79` — `tab(selected)` = accent `fg` + `Modifier::UNDERLINED`.
- lazygit `pkg/gui/style/decoration.go` — `OpReverse` alongside `OpFuzzy` (dim), so reverse and dim
  are peers in one vocabulary.

**Transfer:** focus a *table* (detail view) by reversing only its title span — one cell of ink —
and use `REVERSED` on the judge's current verdict word rather than a panel re-colour.

---

## 6. Border-only gradient — the gradient lives in the frame, not in a filled row

**Looks like:** the perimeter of one panel walks a colour ramp (top → right → bottom → left), and a
`BorderForegroundBlendOffset` makes that ramp march around the border over time.

**Where:**
- lipgloss `set.go:612-656` — `BorderForegroundBlend(colors...)` / `BorderForegroundBlendOffset(v)`;
  reads back in `get.go:301-308`.
- lipgloss `borders.go:288` — `borderBlend(w,h,colors)` computes
  `Blend1D((height+width+2)*2, colors...)`, splits it into top/right/bottom/left runs, and reverses
  the bottom/left runs "because they are drawn in reverse order"; the offset path rotates the array
  with three `slices.Reverse` calls (`borders.go:294-303`).
- lipgloss `blending.go` — `Blend1D` and `Blend2D(width,height,angle,stops…)`; both interpolate in
  **CIELAB** via `from.BlendLab(to, t)` (go-colorful), and `Blend2D`'s doc gives the exact
  cell-by-cell `for y { for x { Background(gradient[y*width+x]).Render(" ") } }` recipe.

**Transfer:** the corral already has `ramp(width, heat)` (`tui.rs:163`) returning per-cell `Color`s.
Point it at the *border cells* of the focused panel via `Block::border_style` per segment, or (cheaper
and Paragraph-native) keep the current single horizon row but make it the **only** full-width rule in
the app. An animated offset is a one-line upgrade to `horizon_span()` — the frame counter already
exists (`Cfg.frame`, `tui.rs:113`).

---

## 7. Perceptual gradients (Oklab / Okhsv) — and never a linear sRGB lerp

**Looks like:** ramps stay vivid through the muddy middle instead of going grey.

**Where:**
- ratatui `examples/apps/demo2/src/colors.rs` — `color_from_oklab(hue, sat, value)` builds colours
  through `palette::Okhsv`; the swatch widget paints each cell `set_char('▀').set_fg(fg).set_bg(bg)`
  with `fg` and `bg` sampled at two different `value`s — **two data values per cell**.
- lipgloss `blending.go` — Lab (`BlendLab`) as the interpolation space.
- The Corral: `dusk_gradient()` `tui.rs:150` uses `colorgrad` `BlendMode::Oklab`.

**Transfer:** keep Oklab; spend it on *thin* things. Move the ramp off any full-band fill and onto
(a) the one horizon rule, (b) the gauge fill (`bar()`), (c) the judge band's single confidence
track. Same perceived richness, a fraction of the lit cells.

---

## 8. Two values per cell: half-block pairing

**Looks like:** `▀` with the *foreground* showing one series and the *background* the other — twice
the vertical resolution for the same row budget.

**Where:**
- ratatui `examples/apps/demo2/src/colors.rs` — the `▀` fg/bg trick described above.
- ratatui `ratatui-core/src/symbols/marker.rs` — `Marker::{Dot, Block, Bar, Braille, HalfBlock,
  Sextant, Octant, Custom(char)}`, with the doc note that Octants "have the same 2x4 resolution as
  Braille characters but display densely packed and regularly spaced pseudo-pixels, without visible
  bands between cells."
- The Corral already owns the family: `BLOCKS`/`SPARK` `tui.rs:39-40`.

**Transfer:** the wire lattice is the natural home. A heard edge whose intensity spans a cell height
becomes one row of `▀` with `fg = edge colour` and `bg = edge colour darkened`, so the lattice gains
a vertical axis without gaining rows.

---

## 9. Focus as a barely-visible background tint (not a border)

**Looks like:** the focused surface is a few percent lighter than its neighbours. You notice the
grouping, not the highlight.

**Where:**
- textual `src/textual/color.py:432` — `Color.blend(destination, factor)` ("Generate a new color
  between two colors … 0 is the original, 1 is the destination"); `:473 tint()`, `:640 darken()`,
  `:654 lighten()` — all first-class, all computed.
- ratatui `examples/apps/custom-widget/src/main.rs:105` — `buf.set_style(area, Style::new().bg(background).fg(text))` paints a whole rect in one call (the same full-bleed technique bottom's tables use).

**Transfer:** `buf.set_style(panel_rect, Style::default().bg(rgb(INK2)))` for the focused panel
(`INK` `#12102b` vs `INK2` `#1c1940` — already in the palette, `tui.rs:44-45`) before drawing its
content. Zero border ink; the panel separates by tone alone.

---

## 10. `text-opacity` — dim by blending toward the background, not by SGR Faint

**Looks like:** de-emphasised text that stays legible on light terminals, where `Modifier::DIM`/`Faint`
often collapses to nothing.

**Where:**
- textual `docs/styles/text_opacity.md`, wired at `src/textual/css/_styles_builder.py:430`
  (`process_text_opacity`) and `:446`; `blend()` (`src/textual/color.py:432`) does the arithmetic.
- glamour exposes the SGR equivalent per style: `ansi/style.go:53` `Faint *bool`, merged down a
  style hierarchy at `:168/:207`.
- lazygit's dim is an SGR attribute: `pkg/gui/style/decoration.go` → `color.OpFuzzy`.

**Transfer:** add a helper `fn fade(fg: Color, bg: Color, t: f32) -> Color` (lerp in Oklab, reusing
`dusk_gradient`'s space) and use it for the "quoted / superseded" transcript lines and for helper
hints, instead of `Modifier::ITALIC` at full brightness (`tui.rs:684`) or a flat `MUTED`. The corral's
ink is already explicit hex, so the blend target is known at every call site.

---

## 11. Border-title and border-subtitle rendered *in* the frame

**Looks like:** `╭ Transcript ──────── 42 lines ╮` — the title occupies the top border, the count the
bottom, and no content row is spent on a header.

**Where:**
- ratatui `ratatui-widgets/src/block.rs:366-445` — `title`, `title_top`, `title_bottom`,
  `title_style`; `:520` `border_style`; plus `Padding` (see `ratatui-widgets/src/block/padding.rs`).
- textual `src/textual/_border.py:23` — `BORDER_CHARS` keyed by `EdgeType`, three rows of
  `(left, center, right)` glyphs per border style (`round`, `solid`, `double`, `dashed` `┏╍┓╏`,
  `inner` `▗▄▖`, `outer` `▛▀▜`, `thick` `█▀█`); `:328-457` renders the label with
  `is_title`, `label_alignment` (`left`/`center`/`right`) and `BORDER_LABEL_LOCATIONS`;
  `src/textual/widget.py:384` exposes `border_title` / `border_subtitle` as distinct attributes.

**Transfer:** the corral's `titled(bold, dim, width)` (`tui.rs:260`) already does the bold-then-dim
title correctly (keeps the bold whole, clips the dim). Move it into `Block::title()` +
`Block::title_bottom()` (count on the bottom edge) so panels recover one content row each — five
rows back across the overview. Textual's `dashed` set is a ready-made vocabulary for an
"ephemeral" border (the wire) vs a solid one (the transcript).

---

## 12. Markdown / ASCII / half-block border sets as deliberate registers

**Looks like:** the border glyph set is a *tone of voice*: `╭─╮` is calm, `┏━┓` is urgent,
`┏╍┓`/`╏` is provisional, `▛▀▜`/`▌` is "this is a surface", `.`/`|`/`-` is "this is data".

**Where:**
- lipgloss `borders.go` — `NormalBorder`, `RoundedBorder`, `BlockBorder`, `OuterHalfBlockBorder`
  (`▛▜▙▟▀▄▌▐`), `InnerHalfBlockBorder`, `ThickBorder`, `DoubleBorder`, `HiddenBorder`,
  `MarkdownBorder` (`-`/`|`), `ASCIIBorder`; plus per-side toggles `BorderTop/Right/Bottom/Left`.
- textual `src/textual/_border.py:23` — the same set as data, including `hidden`/`blank` which render
  as spaces but keep layout.

**Transfer:** keep `Rounded` for the transcript only. Use the half-block set (`▌`/`▐` verticals) for
the wire — it reads as a *field*, not a box — and `HiddenBorder` (or `BorderType::None` + manual
padding) for the five overview rows.

---

## 13. Title that cannot truncate mid-word

**Looks like:** the bold part survives at any width; the dim part is dropped whole, then ellipsised,
then dropped — never clipped mid-glyph.

**Where:** the corral's own `titled()` `tui.rs:260-277` (the room calculation
`width - (bold.len() + 6)`, then `shorten()`); the same priority appears in gitui
(`theme.title(focused)` is optional decoration in `src/ui/scrolllist.rs:70`) and in television's
status bar, where the `faint` separator span `" · "` is unconditional but the `dimmed` detail span is
conditional (`television/screen/status_bar.rs`, `HINT_SEP`).

**Transfer:** keep verbatim; generalise it into a `fit(bold, dim, tail, width)` used by the judge
band and the wire title too, so no band can ever show a broken word.

---

## 14. Spinners that cost one cell and repaint only on change

**Looks like:** a single braille cell in the masthead while work is in flight; nothing when idle.

**Where:**
- gitui `src/spinner.rs` — `SPINNER_CHARS = ['⣷','⣯','⣟','⡿','⢿','⣻','⣽','⣾']`; `update()` advances
  `idx`; `draw()` **only writes a cell when `last_char != char_to_draw`** (tracked in a `Cell<char>`),
  otherwise it is a no-op. It writes directly through `Backend::draw` + `flush`.
- bubbles `spinner/spinner.go` — `Spinner { Frames []string; FPS time.Duration }` with presets
  `Line`, `Dot`, `MiniDot`, `Jump`, `Pulse` (`█▓▒░`), `Points`, `Meter`, `Ellipsis`; FPS is per
  spinner (`time.Second/10`, `/12`, `/8`…).

**Transfer:** the corral's `Cfg.frame` is already an explicit counter, never a wall clock
(`tui.rs:113`, doc line 16) — which is exactly the reproducibility the demo needs. Add one
`spinner_char(frame)` in the masthead, gated on `room.status == "discussing"`, and adopt gitui's
"only repaint when the glyph changes" discipline so a static frame stays byte-identical for snapshots.

---

## 15. Breathing / ping-pong — a reversible effect with no clock drift

**Looks like:** the speaking seat's flare eases up and back down; the loop is defined by the effect,
not by `Instant::now()`.

**Where:**
- tachyonfx `src/fx/ping_pong.rs` — `PingPong::process` runs `self.fx` forward; on overflow it sets
  `is_reversing`, calls `self.fx.reset(); self.fx.reverse()`, and returns `None` to "consume any
  overflow when reversing, to reset the area"; `done()` only when reversed and finished.
- tachyonfx `src/fx/lighten.rs` — `Lighten::new(fg: Option<f32>, bg: Option<f32>, timer)`:
  lightness deltas in `-1.0..=1.0`, `is_darken()` derived from sign.
- tachyonfx `src/wave.rs` — a `SignalSampler` (`sample(x, y, t) -> -1..1`) with `WaveFn::{Sin, Cos,
  Triangle, Sawtooth}`, built on `parabolic_sin`/`parabolic_cos` (cheap, no float `sin`).

**Transfer:** the corral's flare is the obvious seat: `fx::ping_pong(fx::lighten(Some(0.12), None,
EffectTimer::from_ms(1400, Interpolation::SineInOut)))` applied to the speaking row's `Rect` only.
Because the effect is bounded and self-reversing, `Cfg.frame` stays the sole time source.

---

## 16. Easing as a library, not hand-rolled lerps

**Looks like:** a 12-frame transition that decelerates into place instead of stopping dead.

**Where:**
- textual `src/textual/_easing.py` — a full set ported from easings.net:
  `_in_out_expo`, `_in_out_circ`, `_in_out_back`, `_in_elastic`, `_out_elastic`, `_out_bounce`, …
- tachyonfx `src/interpolation.rs` — `mod easing { back_in, back_out, back_in_out, bounce_out,
  bounce_in, bounce_in_out, … }` (same constants: `c1 = 1.70158`, `n1 = 7.5625, d1 = 2.75`).
- tachyonfx `src/fx/sweep_in.rs` imports `Interpolation::CircOut` by name.

**Transfer:** `tachyonfx::Interpolation` is already a dependency (`Cargo.toml`). Use `CircOut` for
the seat flare decay, `SineInOut` for the horizon's heat response, and `ExpoOut` if a new panel ever
expands — no new code, and the interpolation is a named, testable value rather than a magic curve.

---

## 17. Coalesce / sweep: a reveal that is *randomised*, not uniform

**Looks like:** a new line doesn't fade in as a block — its cells activate at staggered random
thresholds, so it "coalesces" like settling pixels; an edge sweeps in with a gradient tail.

**Where:**
- tachyonfx `src/pattern/coalesce.rs` — `CoalescePattern` assigns **each cell a random threshold**
  from a `SimpleRng` reset per frame; `map_alpha` returns `0.0` until `global_alpha > threshold`, then
  a smooth `(alpha - threshold) / (1.0 - threshold)`. Doc: "creates truly random distributions that
  feel natural and unpredictable."
- tachyonfx `src/fx/sweep_in.rs` — `SweepIn { gradient_length, randomness_extent, faded_color,
  direction, rng }`, with `direction.flips_timer()` mirroring the lifetime.
- tachyonfx `src/pattern/` also ships `checkerboard`, `radial`, `spiral`, `diamond`, `wave`,
  `sweep`, `dissolve`, `inverted`, `blend`, `combined` — a whole family behind one `Pattern` trait.
- tachyonfx `src/lib.rs:44` re-exports `pattern::*`.

**Transfer:** the corral's single effect is `fx::dissolve(EffectTimer::from_ms(900, Linear))`
(`tui.rs:1836`, applied to the whole frame at `main.rs:329`). Two cheap upgrades: (a) attach
`CoalescePattern` to that dissolve so cells stagger; (b) scope the effect to the transcript `Rect'
and to newly-arrived lines only, rather than the whole buffer — a dissolve over the entire screen is
the single biggest source of visual noise in a dense frame.

---

## 18. HSL shift for temperature, not recolouring

**Looks like:** the room "heats up" by a hue/lightness delta applied to existing colours, clamped;
no palette swap, no new styles.

**Where:** tachyonfx `src/fx/hsl_shift.rs` — `hsl_mod_fg`/`hsl_mod_bg: [f32; 3]`; the lerp
`(h + 0.0.lerp(hsl[0], alpha)) % 360`, `s` and `l` clamped to `0..100`. Companion ops:
`src/fx/saturate.rs`, `src/fx/fade.rs` (`FadeColors { fg, bg, color_space, pattern }`), and
`src/color_space.rs` `ColorSpace::{Rgb, Hsl, Hsv}` with `#[default] Hsl`.

**Transfer:** replace `heat`-conditioned recolouring of many spans (`ramp()`'s `hi` clamp,
`tui.rs:168`) with one `hsl_shift` over the wire rect, so heat is *one* moving property rather than a
recompute of every gauge.

---

## 19. Shadow behind a floating layer (ratatui 0.30.2, new API)

**Looks like:**
```
┌Popup─────┐▒
│content   │▒
└──────────┘▒
  ▒▒▒▒▒▒▒▒▒▒▒
```
a one-cell offset cast to the right and bottom in `▒`, giving an overlay depth with zero extra
chrome anywhere else.

**Where:** ratatui `ratatui-widgets/src/block/shadow.rs` — `Shadow { effect, style, offset }` with
presets `overlay()` (style only), `block()`, `light_shade()`, `medium_shade()`, `dark_shade()`,
`symbol(&'static str)`, `custom(effect)`; default `offset: Offset::new(1,1)`. The `CellEffect` trait
(`fn apply(shadow_area, base_area, buf)`) is `Debug + Send + Sync + UnwindSafe + RefUnwindSafe` so a
shadow can live inside a `Block`. Wiring: `ratatui-widgets/src/block.rs:731` `Block::shadow()`,
`:1071` `render_shadow()`, re-exported at `block.rs:21`
(`pub use self::shadow::{CellEffect, Dimmed, Shadow, dimmed}`). Note `Dimmed`/`dimmed` — a built-in
effect that dims the cells under a shadow.

**Transfer:** the corral's `h` help overlay and any future confirm popup should be
`Block::bordered().shadow(Shadow::light_shade().style(Style::new().fg(rgb(INK2))))`. This is the
correct way to buy depth: one overlay is raised, the five panels stay flat.

---

## 20. Layers and z-order as an explicit model

**Looks like:** an overlay is drawn *on top of* a cell buffer at a z-index, without reflowing the
panels beneath it.

**Where:**
- lipgloss `layer.go` — `Layer { content, x, y, z, layers }`, `Z(z)` / `MaxZ()`, `AddLayers()` which
  computes its own `Width()/Height()` from children.
- lipgloss `canvas.go` — `Canvas` wrapping a `uv.ScreenBuffer` (a real cell buffer) with
  `Compose(drawable)` ("later drawables will appear 'on top' of earlier ones") and
  `Render()` that trims trailing space.
- yazi `yazi-fm/src/spot/spot.rs` — a `Spot` widget that iterates locked overlay widgets, transforms
  their area and renders only `if rect.intersection(win) == rect` (fully inside the window) — a
  clipping rule that keeps overlays from bleeding.
- ratatui's own `examples/apps/popup` and docs recipe "Popups (overwrite regions)".

**Transfer:** corral already has `frame.buffer_mut()` (`main.rs:329`); make the wire/help a layer
drawn after the five panels into the same buffer, and adopt yazi's
`intersection == rect` guard so an overlay never half-draws at a small terminal size.

---

## 21. Compactness tiering — chrome is the first casualty of small terminals

**Looks like:** the same app at 3 heights: full (all chrome), compact (no gaps, no tabs), ultracompact
(input + results only).

**Where:** atuin `crates/atuin/src/command/client/search/interactive.rs:101` `to_compactness()`,
`:186` `enum Compactness`, `:971` `border_size` derived from it, `:1121` the indicator string
switches on it; bottom `src/constants.rs:11` `TABLE_GAP_HEIGHT_LIMIT`.
yazi's `theme-dark.toml` `[mgr] linemode = "none"` and `yazi-config/preset/yazi-default.toml:14`
show the same idea at the *data* level: the default density mode is the quiet one.

**Transfer:** this is the corral's most direct win — a dense five-panel view cannot show five
bordered panels, a judge band, a transcript, a roster, beliefs *and* the wire at 24 rows. Define a
3-tier `Density` from `area.height` and let tiers drop borders → gaps → the beliefs band → the wire.

---

## 22. A status ribbon, not a status box

**Looks like:** left-aligned mode dot + name, a dim ` · ` separated key-hint run, right-aligned
version — one row, three zones, separated by whitespace and a dim middot rather than `│`.

**Where:** television `television/screen/status_bar.rs` — a `Layout` of `Fill(1)/Fill(3)/Fill(1)`
with `const HINT_SEP: &str = " · "`, `dimmed = fg(dimmed_text_fg)`, `faint = fg(border_fg)`;
a single `●` in the mode colour anchors the row; the selected-count span is
`dimmed.add_modifier(Modifier::ITALIC)` and only appears when non-zero.
yazi `theme-dark.toml` pairs this with `sep_left`/`sep_right` powerline caps for a ribbon
with rounded ends (`{ open = "", close = "" }`).

**Transfer:** the corral's masthead (`masthead()`, `tui.rs:309`) already has the status badge; move
the key hints onto the *same* row as a ` · `-separated dim run and delete the separate hint line —
one row saved, and the hints de-emphasise themselves.

---

## 23. Title-in-border alignment and the bottom-edge subtitle

**Looks like:** the count/state lives on the bottom border edge; the title can be left/centre/right,
and the label occupies border cells rather than a content row.

**Where:** textual `src/textual/_border.py:328-457` (`is_title`, `label_alignment`,
`BORDER_LABEL_LOCATIONS`, `flip_top`/`flip_bottom`), `src/textual/widget.py:384`.
ratatui `ratatui-widgets/src/block.rs:397-445` (`title_top`, `title_bottom`, `title_style`),
`Block::padding` (`ratatui-widgets/src/block/padding.rs`) and `Padding::horizontal(1)` — which the
corral already sets (`panel()`, `tui.rs:285`).

**Transfer:** five panels × one recovered header row = five rows back in the overview; in detail view,
put the transcript's line count on the bottom edge with `title_bottom`.

---

## 24. Scroll-affordance as a hairline, not a border

**Looks like:** a 1-column `▐`/`│` thumb the height of the visible window, in a dim track — the only
right-edge cue. No full right border.

**Where:** ratatui `ratatui-widgets/src/scrollbar.rs` — `Scrollbar` with `thumb_symbol`/`thumb_style`,
`track_symbol(Option<&str>)`/`track_style`, `begin_symbol`/`end_symbol` + their styles (docs example
uses `↑`/`↓`). gitui's equivalent is a single style: `scroll_bar_pos()` returns
`Style::default().fg(self.selection_bg)` (`src/ui/style.rs:42`).

**Transfer:** give the transcript a `Scrollbar` on its right column (`track_symbol(Some("▏"))`,
`thumb_symbol("▐")`) and drop the panel's right border. The bar is *information*; the border was
decoration.

---

## 25. Full-bleed background + subtle striping instead of rules

**Looks like:** alternating rows are a few percent apart in background lightness; the eye tracks the
row across five columns without any vertical rules.

**Where:** ratatui `examples/apps/custom-widget/src/main.rs:105` `buf.set_style(area, …)`;
bottom `src/options/config/style/themes/default.rs` — `table_header_style:
colour!(HIGHLIGHT).add_modifier(Modifier::BOLD)` and a `table_gap` between table blocks, with
`selected_text_style: colour!(Color::Black).bg(HIGHLIGHT_COLOUR)` as the *only* strong fill in a
table. gitui is the opposite lesson from the same file: `tags()` sets `bg(selection_bg)` only when
selected and `Color::Reset` otherwise (`src/ui/style.rs:89`).

**Transfer:** the corral's transcript is a list of `Line`s; a zebra tint of
`bg(blend(INK, INK2, 0.35))` on alternate lines replaces any separator rule and improves horizontal
tracking across the speaker/glyph/text columns.

---

## 26. Theme as a semantic vocabulary, not a list of colours

**Looks like:** every colour is named for its *role* (`Muted`, `Guidance`, `Annotation`, `Title`),
so a new surface can reuse the vocabulary instead of inventing a shade.

**Where:**
- atuin `crates/atuin-client/src/theme.rs` — `enum Meaning { AlertInfo, AlertWarn, AlertError,
  Annotation, Base, Guidance, Important, Title, Muted, Syntax* }`, `Meaning -> ContentStyle` map,
  `MEANING_FALLBACKS` for forward compatibility ("add a fallback … so that themes which do not have
  it get a sensible fallback").
- yazi `yazi-config/src/theme/theme.rs` — `Style` per role (`Indicator { parent, current, preview,
  padding }`, `Status { prog_* }`, `Which { mask, cand, rest, desc }`), with `ArcSwap`/`Overlay` so
  theme files compose by merge.
- textual's entire model: CSS-like styles resolved per widget (`src/textual/css/_style_properties.py`,
  `_styles_builder.py`), with `HATCHES` (`src/textual/css/constants.py:127`: `{"left": "╲",
  "right": "╱", "cross": "╳", "horizontal": "─", "vertical": "│"}`) as a *role-indexed* glyph table.

**Transfer:** the corral's palette is already 11 named consts (`tui.rs:44-54`). Promote it to
`enum Meaning` with exactly one `Style` per meaning and one `blend` helper, so a new band cannot
introduce a 12th shade by accident — the discipline that keeps a dense UI from going noisy.

---

## 27. Novelty flags (2024–2026), for the record

- **ratatui 0.30 `Block::shadow` + `CellEffect`** — `ratatui-widgets/src/block/shadow.rs`
  (shipped 2026-06-19, per `CHANGELOG.md` 0.30.2). Depth for overlays that costs one cell of `▒`.
- **lipgloss v2 `BorderForegroundBlend` + half-block/inner/outer border presets** —
  `borders.go`, `set.go:628`; a marching perimeter gradient and a half-block frame vocabulary.
- **lipgloss v2 `Layer`/`Canvas` over `ultraviolet`** — `layer.go`, `canvas.go`: string-styling
  library grew a real z-ordered cell-buffer compositor.
- **television's `HAIRLINE_BORDER_SET` with an explicit eighth-block-asymmetry rationale** —
  `television/screen/constants.rs`; the "one-sided hairline only where panes touch" rule
  (`layout.rs preview_hairline`).
- **tachyonfx `CoalescePattern` / `SweepIn` / `PingPong` / `WaveFn`** —
  `src/pattern/coalesce.rs`, `src/fx/sweep_in.rs`, `src/fx/ping_pong.rs`, `src/wave.rs`: randomised
  per-cell thresholds and parabolic-sin signals for organic, clock-free motion.
- **textual `text-opacity`** — `docs/styles/text_opacity.md`, `src/textual/css/_styles_builder.py:430`:
  opacity as a first-class style rather than terminal `Faint`.
- **ratatui `Marker::Octant`/`Sextant`** — `ratatui-core/src/symbols/marker.rs`: the new
  Symbols-for-Legacy-Computing Supplement glyphs for "no visible bands between cells".

---

## The eight to adopt, in order

1. **Focus by de-emphasis (#1)** — one dim tone on four panels. Biggest gain, least code.
2. **Hairline / one-sided borders (#2, #23)** — stop completing boxes; five panels lose ten edges.
3. **Compactness tiering (#21)** — chrome drops before data as height shrinks.
4. **Scope and randomise the dissolve (#17)** — effect the new lines, not the whole buffer; add
   `CoalescePattern`.
5. **Spinners that repaint only on change (#14)** — one braille cell, deterministic frame counter.
6. **Border titles and bottom-edge subtitles (#23)** — five rows recovered, no new ink.
7. **Background tint for focus + zebra transcript (#9, #25)** — separate panels and rows by tone,
   not rules.
8. **`Block::shadow` for the help overlay (#19)** — one raised surface; everything else stays flat.
