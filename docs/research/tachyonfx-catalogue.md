# tachyonfx 0.25.2 — Effect Catalogue & Integration Notes

Pin verified against the project: `Cargo.toml` declares
`tachyonfx = { version = "0.25.2", features = ["std-duration"] }`, and `Cargo.lock`
resolves `tachyonfx 0.25.2` (checksum `7e74555389c0a1c1c20e302994dc6eb478ab8754472b944b79a6558d44f33be3`).
The crate repo has moved to **`github.com/ratatui/tachyonfx`** (tag `tachyonfx-v0.25.2`);
the old `junkdog/tachyonfx` redirects there.

## Sources (primary)

All claims below are traced to one of:

- **Published crate source** (the exact code that compiles for this project):
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tachyonfx-0.25.2/`
  — referred to below as `tachyonfx-0.25.2/…`. This is the definitive 0.25.2 artifact.
- **docs.rs/tachyonfx/0.25.2** for the rendered API (`fx`, `EffectTimer`, `Interpolation`,
  `CellFilter`, `Shader`).
- **GitHub tag `ratatui/tachyonfx@tachyonfx-v0.25.2`** for `README.md`, `CHANGELOG.md`,
  and `examples/` (`minimal`, `basic-effects`, `effect-registry`, `effect-showcase`, `tweens`).

Notes on the pin: 0.25.2 ships `README.md`, `CHANGELOG.md`, `src/`, `benches/` inside the
registry tarball, so the catalogue is self-contained. The changelog's newest release entry is
`tachyonfx-0.25.1` (the 0.25.2 tag carries no separate changelog section; behaviour is
0.25.1's plus the `ratatui 0.30.2` alignment already present at the tag).

---

## 1. Built-in effects, grouped by what they are good for

Constructors live in the `fx` module: `tachyonfx-0.25.2/src/fx/mod.rs` (2 685 lines,
`Summary` doc at `fx/mod.rs:1-2685`; rendered at
<https://docs.rs/tachyonfx/0.25.2/tachyonfx/fx/index.html>). Every constructor returns an
`Effect`. Signatures are quoted from source.

### 1a. Colour / highlight (fg/bg transitions — the safe, low-clutter family)

| Constructor | Signature (`fx/mod.rs`) | What it does | Source |
|---|---|---|---|
| `fade_to_fg` | `(fg: C, timer: T) -> Effect` | fades foreground **to** a colour | `fx/mod.rs:1585` |
| `fade_from_fg` | `(fg: C, timer: T) -> Effect` | fades foreground **from** a colour | `fx/mod.rs:1611` |
| `fade_to` | `(fg: C, bg: C, timer: T) -> Effect` | fades fg+bg to colours | `fx/mod.rs:1903` |
| `fade_from` | `(fg: C, bg: C, timer: T) -> Effect` | fades fg+bg from colours | `fx/mod.rs:1928` |
| `paint_fg` / `paint` / `paint_bg` | `(fg[, bg], timer: T)` | paints fg / fg+bg / bg | `fx/mod.rs:1639,1665,1693` |
| `hsl_shift` / `hsl_shift_fg` | `([f32;3] fg, [f32;3] bg, timer)` / `([f32;3] fg, timer)` | animates hue/sat/lightness | `fx/mod.rs:445,485` |
| `saturate` / `saturate_fg` | `(Option<f32> fg, Option<f32> bg, timer)` | adjust saturation | `fx/mod.rs:1733,1757` |
| `lighten` / `lighten_fg` | `(Option<f32> fg, Option<f32> bg, timer)` | increase lightness toward white | `fx/mod.rs:1794,1817` |
| `darken` / `darken_fg` | `(Option<f32> fg, Option<f32> bg, timer)` | decrease lightness toward black | `fx/mod.rs:1855,1878` |

`fade_to_fg`/`fade_from_fg` are the canonical "attention" / "new content" transitions
(docstring examples at `fx/mod.rs:1585-1638`). `fade_from`/`fade_to` internally call the
private `fade(...)`; `fade_from` builds a **mirrored** timer so the motion runs the other way
(`fx/mod.rs:2308-2320`). Colour effects accept `.with_color_space(ColorSpace::Rgb|Hsl|Hsv)`
(`src/effect.rs:110-151`); HSL is the default.

### 1b. Text / character reveal (materialise & dissolve)

| Constructor | Signature | Notes | Source |
|---|---|---|---|
| `dissolve` | `(timer: T) -> Effect` | dissolves fg text into the new text; **uses randomness** | `fx/mod.rs:1476` |
| `dissolve_to` | `(style: Style, timer: T) -> Effect` | dissolve fg+bg to a target style | `fx/mod.rs:1501` |
| `coalesce` | `(timer: T) -> Effect` | reverse of `dissolve` (`Dissolve::new(timer.mirrored())`) | `fx/mod.rs:1523` |
| `coalesce_from` | `(style: Style, timer: T) -> Effect` | reverse of `dissolve_to` | `fx/mod.rs:1555` |
| `evolve` | `(symbols: Into<EvolveSymbolConfig>, timer: T)` | scrambles through symbol sets | `fx/mod.rs:1048` |
| `evolve_into` / `evolve_from` | `(symbols, timer)` | reveal to / from underlying text | `fx/mod.rs:1088,1129` |
| `sweep_in` | `(direction: Motion, gradient_length: u16, randomness: u16, faded_color: C, timer)` | sweeps text in with a colour gradient; randomness for jitter | `fx/mod.rs:837` |
| `sweep_out` | same shape | `sweep_in(direction.flipped(), …).reversed()` | `fx/mod.rs:741` |
| `slide_in` | `(direction: Motion, gradient_length: u16, randomness: u16, color_behind_cells: C, timer)` | slides cells in with a fade gradient | `fx/mod.rs:891` |
| `slide_out` | same shape | slides cells out (`SlideCell` builder) | `fx/mod.rs:943` |
| `stretch` | `(direction: Motion, style: Style, timer)` | stretch/shrink using block chars | `fx/mod.rs:1006` |
| `expand` | `(direction: ExpandDirection, style: Style, timer)` | bidirectional expand from centre | `fx/mod.rs:1180` |
| `explode` | `(force: f32, force_rng_factor: f32, timer)` | particles dispersing outward | `fx/mod.rs:545` |

Docs warn that `dissolve` (and the other randomized effects) "uses randomness … Use
`Effect::with_rng()` to control the random pattern for reproducible animations"
(`fx/mod.rs:1447-1475`).

### 1c. Timing / control wrappers (compose around other effects)

| Constructor | Signature | Purpose | Source |
|---|---|---|---|
| `parallel` | `(&[Effect]) -> Effect` | run simultaneously; completes when all done | `fx/mod.rs:1439` |
| `sequence` | `(&[Effect]) -> Effect` | run one after another; completes with the last | `fx/mod.rs:1404` |
| `sleep` | `(timer: T) -> Effect` | pure pause | `fx/mod.rs:1954` |
| `delay` | `(timer: T, effect: Effect) -> Effect` | `sequence(&[sleep(d), effect])` | `fx/mod.rs:1990` |
| `prolong_start` | `(timer: T, effect: Effect) -> Effect` | hold effect at 0 for a while, then run | `fx/mod.rs:2030` |
| `prolong_end` | `(timer: T, effect: Effect) -> Effect` | run, then hold the final state | `fx/mod.rs:2068` |
| `repeat` | `(effect, RepeatMode) -> Effect` | `RepeatMode::{Forever,Times(n),Duration(d)}` | `fx/mod.rs:658` |
| `repeating` | `(effect) -> Effect` | repeat forever | `fx/mod.rs:716` |
| `ping_pong` | `(effect) -> Effect` | forward then backward; **total duration doubles** | `fx/mod.rs:687` |
| `remap_alpha` | `(start: f32, end: f32, effect) -> Effect` | clamp alpha progress into a sub-range | `fx/mod.rs:624` |
| `freeze_at` | `(alpha: f32, set_raw_alpha: bool, effect) -> Effect` | freeze at a specific transition point | `fx/mod.rs:579` |
| `run_once` | `(effect) -> Effect` | force exactly one tick (keeps zero-duration effects alive in a sequence) | `fx/mod.rs:2129` |
| `never_complete` | `(effect) -> Effect` | hold final state forever, never report done | `fx/mod.rs:2166` |
| `with_duration` | `(duration: Duration, effect) -> Effect` | hard cap the wrapped effect | `fx/mod.rs:2185` |
| `timed_never_complete` | `(duration: Duration, effect) -> Effect` | run forever but auto-complete after a limit | `fx/mod.rs:2204` |
| `consume_tick` | `() -> Effect` | one-tick no-op; the standard "cancel" effect | `fx/mod.rs:2091` |
| `dispatch_event` | `(Sender<T>, T) -> Effect` | fire an app event when it starts | `fx/mod.rs:2297` |

### 1d. Geometry

`translate(fx, Offset, timer)` (`fx/mod.rs:1240`), `translate_buf(...)` (`fx/mod.rs:1263`),
`resize_area(...)` (`fx/mod.rs:1313`, **deprecated**), `dynamic_area(RefRect, effect)`
(`fx/mod.rs:2242`).

### 1e. Custom-effect constructors

`effect_fn(state, timer, |state, ctx, cell_iter| …)` (`fx/mod.rs:334`),
`effect_fn_buf(state, timer, |state, ctx, buf| …)` (`fx/mod.rs:402`),
`offscreen_buffer(fx, RefCount<Buffer>)` (`fx/mod.rs:1360`). Also the `fx::Glitch` struct
(`fx/mod.rs` struct list; `Glitch::builder()…`, see the `minimal` example).

### 1f. Spatial patterns (not effects — alpha modifiers)

`tachyonfx::pattern` module: `RadialPattern`, `DiamondPattern`, `SpiralPattern`,
`DiagonalPattern`, `CheckerboardPattern`, `SweepPattern`, `WavePattern`, `CoalescePattern`,
`DissolvePattern`, `CombinedPattern`, `BlendPattern`, `InvertedPattern`
(README §Spatial Patterns; `src/pattern/`). Applied with `Effect::with_pattern(p)`
(`src/effect.rs:403`), e.g.
`fx::dissolve(2000).with_pattern(SweepPattern::left_to_right(25))` (`fx/index.html`).

---

## 2. Timing: `EffectTimer`, `Interpolation`, and the `duration` delta

### `EffectTimer` (`tachyonfx-0.25.2/src/effect_timer.rs`)

Fields: `remaining`, `total`, `interpolation`, `reverse` (`effect_timer.rs:25-31`).
Constructors (`effect_timer.rs:19-24`):

```rust
let a = EffectTimer::new(Duration::from_millis(500), Interpolation::Linear);
let b = EffectTimer::from_ms(500, Interpolation::Linear);
let c: EffectTimer = 500.into();                    // u32 ms, Linear
let d: EffectTimer = (500, Interpolation::Linear).into();
```

Methods: `reversed()` (`:89`), `mirrored()` (`:122`, reverses direction **and** flips the
easing so the visual curve is preserved), `started()` (`:140`), `reset()` (`:152`),
`alpha()` (`:169`), `remaining()` (`:184`), `duration()` (`:190`), `done()` (`:233`).

**How timing works with a `duration` delta** — the critical contract
(`effect_timer.rs:210-219`):

```rust
pub fn process(&mut self, duration: Duration) -> Option<Duration> {
    if self.remaining >= duration { self.remaining -= duration; None }
    else { let overflow = duration - self.remaining; self.remaining = ZERO; Some(overflow) }
}
```

So every `process()` call **advances the timer by the delta you pass**, and returns `Some(overflow)`
once the timer is exhausted. There is no internal clock. `alpha()` is computed from
`remaining / total` then run through the easing (`effect_timer.rs:169-180`); a zero-duration
timer returns `1.0` (or `0.0` when reversed). Interpolation is applied once per `process`.

### `Interpolation` (`src/interpolation.rs:238-320`)

34 variants. `Linear` is the `Default`. Full list, straight from the enum:

`BackIn/BackOut/BackInOut`, `BounceIn/Out/InOut`, `CircIn/Out/InOut`,
`CubicIn/Out/InOut`, `ElasticIn/Out/InOut`, `ExpoIn/Out/InOut`, `Linear`,
`QuadIn/Out/InOut`, `QuartIn/Out/InOut`, `QuintIn/Out/InOut`, `Reverse`,
`SmoothStep`, `Spring`, `SineIn/Out/InOut`.

`Interpolation::alpha(f32)` applies the curve; `flipped()` swaps In↔Out
(`interpolation.rs:322-429`). Doc semantics per variant at
<https://docs.rs/tachyonfx/0.25.2/tachyonfx/enum.Interpolation.html>. Shorthand timing
accepts `u32` ms, `(ms, Interpolation)`, `EffectTimer`, or `Duration` — e.g.
`fx::fade_to_fg(Color::Red, 1000)` / `(1000, Interpolation::BounceOut)` /
`EffectTimer::from_ms(1000, Interpolation::Linear)` / `Duration::from_secs(2)`
(`fx/index.html` §Interpolation & Timing).

### `Duration`

Feature `std-duration` (which this project enables) aliases
`pub type Duration = core::time::Duration` (`src/duration.rs:5-11`). Without it, the crate
uses a 32-bit millisecond `Duration` (`duration.rs:15-60`). The project's `dt` in
`tui::apply_effect` is a `std::time::Duration` and is passed straight through — valid because
`process` takes the aliased `Duration`.

---

## 3. Composition, sequencing, and targeting

### Composition

In **0.25.2 there is no `then`, no `with`, and no `+`/`|` operator** on `Effect`. I checked:
`src/effect.rs` exposes only builder/inspection methods (`effect.rs:28-445`), and there is no
`impl Add/BitOr` or `fn then` anywhere in `src/`. All composition is done with the free
functions:

```rust
let fx = fx::sequence(&[
    fx::fade_from_fg(Color::Black, 300),
    fx::coalesce(500),
]);
let fx = fx::parallel(&[
    fx::fade_from_fg(Color::Red, 500),
    fx::sweep_in(Motion::LeftToRight, 10, 0, Color::Black, 800),
]);
```

(`README.md` §Combining Effects; `src/fx/mod.rs:1404,1439`). `delay`, `prolong_start`,
`prolong_end` are sugar over `sequence` (`fx/mod.rs:1990-2070`). `effect_fn`/`effect_fn_buf`
plus `.into_effect()` (or `Effect::new`) turn a `Shader` impl into an `Effect`
(`src/effect.rs:28-33,449-461`).

### Effect builder methods (`src/effect.rs`)

`with_area(Rect)` (`:52`), `with_filter(CellFilter)` (`:80`), `with_color_space(ColorSpace)`
(`:148`), `with_rng(SimpleRng)` (`:188`), `with_pattern(P: Into<AnyPattern>)` (`:403`),
`reversed()` (`:198`); plus runtime `process`/`done`/`running`/`area`/`set_area`/`filter`/
`reverse`/`timer`/`timer_mut`/`reset`.

Important nuance: `with_filter`/`Effect::filter` **only sets the filter if one is not already
set** — "preserving any existing filters during effect composition"
(`effect.rs:57-83`, `ShaderExt::propagate_filter` at `effect.rs:464-486`).

### Area vs CellFilter targeting

- **Area**: `fx.process(dt, buf, area)` restricts the effect to `area`, but "If the effect has
  its own area set, that takes precedence" (`effect.rs:220-258`). `Fx::with_area` /
  `Effect::set_area` set the shader's own area.
- **`CellFilter`** (`src/cell_filter/filter.rs:83-359`) selects *cells* by predicate:
  `All`, `Area(Rect)`, `RefArea(RefRect)`, `FgColor(Color)`, `BgColor(Color)`,
  `Inner(Margin)`, `Outer(Margin)`, `Text`, `NonEmpty`, `AllOf(vec)`, `AnyOf(vec)`,
  `NoneOf(vec)`, `Not(Box)`, `Layout(Layout,u16)`, `PositionFn(..)`, `EvalCell(..)`,
  `Static(Box)`.
  Doc example: apply a fade only to text cells with
  `fx::fade_to_fg(Color::Yellow, 1000).with_filter(CellFilter::Text)`
  (`fx/index.html` §Cell Filtering).
- **Performance categorisation** (documented on `CellFilter`): *static* filters
  (`All`, `Area`, `Inner`, `Layout`) are pre-computed as bitmasks for O(1) lookup; *dynamic*
  filters (`FgColor`, `Text`, `EvalCell`) are re-evaluated every frame
  (`cell_filter/filter.rs` doc; CHANGELOG notes `~30-50%` reduction in filter overhead).
  Prefer static filters on the hot path.

---

## 4. The `Shader` trait — writing a custom effect

Source: `src/shader.rs:18-285`.

```rust
pub trait Shader: ThreadSafetyMarker + Debug {
    fn name(&self) -> &'static str;                                     // :20
    fn process(&mut self, duration: Duration, buf: &mut Buffer, area: Rect)
        -> Option<Duration>;                                            // :51
    fn execute(&mut self, duration: Duration, area: Rect, buf: &mut Buffer) {} // :71, default no-op
    fn done(&self) -> bool;                                             // :91 (required)
    fn clone_box(&self) -> Box<dyn Shader>;                             // :105 (required)
    fn area(&self) -> Option<Rect>;                                     // :111 (required)
    fn set_area(&mut self, area: Rect);                                 // :117 (required)
    fn filter(&mut self, filter: CellFilter);                           // :133 (required)
    // provided: running, cell_iter, reverse, timer_mut, timer(),
    //           cell_filter, filter_processor(_mut), set_color_space,
    //           color_space, set_pattern, set_rng, reset
}
```

**The `process` contract** (`shader.rs:22-57`): the *default* implementation:

1. updates the timer (`timer_mut().and_then(|t| t.process(duration))`),
2. calls `execute(duration, area, buf)`,
3. returns any overflow duration.

Returns `Some(overflow)` **when done**, `None` **while still running** (`shader.rs:36-38`).
The doc is explicit: "When implementing this trait, you typically only need to override
`execute()`. … Only override `process()` if you need custom timer handling"
(`shader.rs:15-28`).

For most custom effects, prefer the closure form rather than impl'ing the trait:
`fx::effect_fn(state, timer, |state, ctx, cell_iter| …)` where `ctx: ShaderFnContext`
exposes `ctx.alpha()` and `ctx.area`, and the closure returns/handles per-cell updates
(`src/fx/shader_fn.rs:65-98`; `README.md` §Custom Effects). `ShaderFn::reset` restores the
original state (`shader_fn.rs:179-182`).

**Boilerplate macro** `default_shader_impl!` (`shader.rs:310-381`) generates `area`/`timer`/
`filter`/`color_space`/`clone` groups for a struct that has fields `area: Option<Rect>`,
`timer: EffectTimer`, `cell_filter: Option<FilterProcessor>`, `color_space: ColorSpace` and
`Clone`. Minimal required methods remain `name`, `done`, `clone_box`, `area`, `set_area`,
`filter` (see the `CounterShader` test at `src/effect_manager.rs:303-341`).

---

## 5. Determinism & testability

**Yes — effects are clock-free.** Every effect advances only by the `Duration` handed to
`process`/`process_effects`; the only time source is the caller. There is no
`Instant::now()`/`SystemTime` in the effect code paths (the crate's own examples pass
`elapsed.into()` / a fixed `Duration::from_millis(33)` — `README.md` Quick Start,
`examples/minimal/src/main.rs`). Consequently, feeding an explicit, fixed `dt` per frame
produces a reproducible frame sequence: the timer arithmetic (`effect_timer.rs:210-219`) is
deterministic.

Caveats, documented:

- **Randomized effects are not deterministic unless seeded.** `dissolve`, `dissolve_to`,
  `coalesce`, `coalesce_from`, `explode`, `glitch`, `slide_in/out`, `sweep_in/out` draw from
  `SimpleRng`. CHANGELOG 0.21.0: "`Effect::with_rng()` now properly supported by randomized
  effects for reproducible animations" (`CHANGELOG.md:85-90`; doc `src/effect.rs:153-191`).
  RNG is SplitMix32 (`CHANGELOG.md:42`). Fix the seed for a stable cell-selection pattern:
  `fx::dissolve(1000).with_rng(SimpleRng::new(42))`.
- **Ordering caveat for zero-duration effects**: a zero-duration effect is *skipped* inside
  `sequence`/`parallel` unless wrapped in `fx::run_once(...)` (`fx/mod.rs:2095-2131`;
  manager tests at `src/effect_manager.rs:347-465`).
- `EffectManager::is_running()` lets a loop skip redraws entirely when nothing is animating
  (`src/effect_manager.rs:96-115`) — useful to keep a mostly-static UI from paying any effect
  cost.
- The crate ships no wall-clock and no test backend; it operates on any `ratatui-core`
  `Buffer`, so ratatui's `TestBackend` (as the project already uses) can drive effects
  frame-by-frame for golden-frame tests.

---

## 6. Restraint & performance guidance (from the crate itself)

- **Iterate cells with `for_each_cell`, not the `Iterator` impl.** `cell_iter.rs:19-27`:
  "prefer `for_each_cell` over iterator-based iteration … avoids division and modulo
  operations, making it significantly faster." Use `Iterator` combinators only when needed.
- **Filter cost is classified.** Static filters are pre-computed bitmasks (O(1)); dynamic ones
  are per-frame. Prefer `All`/`Area`/`Inner`/`Layout`; avoid `EvalCell`/`Text`/`FgColor` on
  large hot areas unless necessary (`cell_filter/filter.rs` doc; `CHANGELOG.md:241-256`).
- **Run effects on the smallest area that changes**, not the whole buffer. `process` takes an
  explicit `area`, and `with_area`/`set_area` let an effect carry its own rect
  (`effect.rs:52,220-293`); the crate's own `offscreen_buffer` exists so "performance-intensive
  effects [can] be computed separately" (`fx/mod.rs:1359`).
- **Overflow return is the signal to stop drawing.** `process` returns `Some(overflow)` when
  done (`shader.rs:36-38`); `EffectManager::process_effects` drops finished effects and
  `is_running()` gates redraws (`effect_manager.rs:112-137`). Don't keep processing a
  completed effect.
- **Never-complete / repeating effects cost every frame.** `never_complete` "continue[s]
  consuming processing ticks" by design (`fx/mod.rs:2133-2141`) — fine for one or two, ruinous
  if applied to every cell of a dense screen.
- The crate maintains micro-benchmarks (`benches/`: `cell_filter`, `cell_iteration`,
  `color_conversion`, `color_interpolation`, `hsl_hsv_conversion`, `lightness_adjustment`,
  `math_functions`, `parabolic_sin`, `saturation_adjustment`) and has repeatedly optimised the
  colour pipeline — evidence it treats per-frame colour work as the thing to keep cheap
  (`CHANGELOG.md:44-47`).

---

## 7. Recommendations for this project (corral)

Context: `src/tui.rs:1834-1849` builds exactly one effect,
`fx::dissolve(EffectTimer::from_ms(900, Interpolation::Linear))`, applied via
`fx.process(dt, buf, area)` in the live loop (`src/main.rs:248`). The room renders ~20
concurrent speakers' text every frame; identity must not rest on colour alone; static frames
must stay reproducible.

### Use (subtle, low-clutter)

1. **Current-speaker breathing highlight** — `fx::fade_to_fg` / `fade_from_fg` wrapped in
   `fx::ping_pong` (or `fx::repeat(…, RepeatMode::Forever)`), targeted with
   `with_filter(CellFilter::Text)` and confined to that speaker's rect via `with_area`.
   A slow `Interpolation::SineInOut` in the 1.5–3 s range reads as "breathing", not blinking.
   Pair with a glyph/border change so identity is not colour-only.
2. **Soft new-line appear** — `fx::coalesce` (the reverse of dissolve) on the *new* line only,
   short (150–300 ms), `Interpolation::SineOut`, so text materialises rather than pops. Same
   for the current effect's spirit but gentler than dissolve-into-void.
3. **Gentle incoming-message sweep** — `fx::sweep_in(Motion::LeftToRight, small_gradient,
   0, faded_color, ~250ms)` with `randomness = 0`, scoped to the message's rect. Keep
   `gradient_length` small (≤ 6) so it's a soft wipe, not a marquee.
4. **Transient "new content" tint** — `fx::fade_from_fg(accent, (300, Interpolation::CubicOut))`
   that decays to normal, driven by the existing frame counter; add `fx::run_once` if it lives
   in a sequence.
5. **Reveal/hide panels** — `fx::coalesce` / `fx::dissolve` on panel open/close only, never
   over the whole screen.
6. Optionally seed any randomized pick with `SimpleRng::new(constant)` to keep golden frames
   reproducible.

Implementation note: keep one `EffectManager<()>` and feed it the same explicit `dt` already
passed to `apply_effect`; use `add_unique_effect(key, fx)` so a new speaker's highlight cancels
the previous one (`effect_manager.rs:43-94`).

### Avoid for a text-dense UI

- **Per-cell randomness over large areas**: `dissolve` (the current shimmer), `evolve`,
  `explode`, `glitch` — they scramble glyphs/cells and destroy readability. `glitch`
  especially ("applies a glitch effect to random parts of the screen").
- **Character-motion geometry**: `slide_in/out`, `stretch`, `expand`, `translate`,
  `translate_buf` — moving/resizing glyphs across a 20-speaker wall is motion sickness.
- **Colour-cycle / hue rotation**: repeated `hsl_shift`/`hsl_shift_fg`, `color_cycle`-style
  loops — flicker, and they break the "identity never rests on colour alone" rule.
- **Full-buffer heavy effects**: applying any of the above with `CellFilter::All` on the whole
  screen, or running many `never_complete`/`repeating` effects at once.
- **`never_complete` on many concurrent effects** — keeps every one alive every frame
  (`fx/mod.rs:2133-2141`).
- **`resize_area`** — deprecated in 0.25.2 (`fx/mod.rs:1313`).

### Composition/targeting rules of thumb

- One animation at a time per region; `sequence` rather than overlapping `parallel` unless the
  two are complementary.
- Restrict every effect with `with_area` (or the `area` passed to `process`) to the exact
  speaker/message rect, and prefer static `CellFilter`s (`Text`, `Area`, `Inner`).
- Drive everything from the existing explicit frame delta — no wall clock — so static frames
  remain reproducible and seeded randomness stays stable.
