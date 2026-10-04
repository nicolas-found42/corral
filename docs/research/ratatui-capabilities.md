# Ratatui 0.30.2 capabilities — a working map

Verified against the exact pin in `Cargo.toml` (`ratatui = "0.30.2"`) and its transitive
workspace crates. All file paths below are rooted at the release tag
`ratatui-v0.30.2` (`git ls-remote --tags` → `8d20b4f` / commit `e665c36`).
Source URL prefix used throughout:

- Tag tree: `https://github.com/ratatui/ratatui/tree/ratatui-v0.30.2/`
- Tag blob: `https://github.com/ratatui/ratatui/blob/ratatui-v0.30.2/<path>`

## 0. Crate split (this is the 0.30 change)

0.30 modularized the workspace. `ratatui 0.30.2` depends on `ratatui-core ^0.1.2` and
`ratatui-widgets ^0.3.2` (`ratatui/Cargo.toml`). Apps keep depending on `ratatui`; the split only
matters if you want the widget crate alone or a stable core for a widget library
(`ARCHITECTURE.md`; `ratatui-core/src/lib.rs`, `ratatui-widgets/src/lib.rs`).
`ratatui/src/lib.rs:479-519` re-exports everything: `pub use ratatui_core::{buffer, layout}`,
`style`, `symbols`, `text`, `terminal::{Frame, Terminal,…}`, plus `pub mod widgets`.

Default features of `ratatui`: `["all-widgets", "crossterm", "layout-cache", "macros", "underline-color"]`
(`ratatui/Cargo.toml:30`). Note `underline-color` and `layout-cache` are on by default — the doc
for this repo already gets them for free.

---

## 1. Buffer / Cell API — what is safe to mutate directly

Types live in `ratatui-core/src/buffer/` (`buffer.rs`, `cell.rs`, `cell_width.rs`, `diff.rs`).

`Cell` (`ratatui-core/src/buffer/cell.rs:37-56`) is a plain public struct you may mutate:
public fields `fg: Color`, `bg: Color`, `underline_color: Color` (the last is gated behind the
`underline-color` feature), plus a private `symbol`/`modifier`. Constructors and setters:

- `Cell::EMPTY` const (`cell.rs:77`), `Cell::new(&'static str)` (`cell.rs:94`).
- `set_symbol(&mut self, &str) -> &mut Self` (`cell.rs:156`), `set_char(char)` (`cell.rs:170`),
  `set_fg`/`set_bg` const (`cell.rs:177`, `:182`), `set_style(impl Into<Style>)` (`cell.rs:192`),
  `reset()` (`cell.rs:248`).
- `set_skip` is **deprecated since 0.30.1** — use `set_diff_option(CellDiffOption::Skip)`
  (`cell.rs:216-232`).

`Buffer` (`ratatui-core/src/buffer/buffer.rs`) public API:

- Reads: `get(x,y)` / `cell(P: Into<Position>) -> Option<&Cell>` (`buffer.rs:131,179`).
- Safe writes: `cell_mut((x,y)) -> Option<&mut Cell>` (`buffer.rs:210`) — returns `None` outside
  the area; `index_mut`/`Buffer[(x,y)]` panics instead. `get_mut` is **deprecated** in favour of
  `Buffer[(x,y)]` or `cell_mut` (`buffer.rs:151-156`).
- Bulk: `set_string`, `set_stringn`, `set_line`, `set_span`, `set_style(area, style)`,
  `merge(&other)`, `diff(&other)`, `diff_iter` (`buffer.rs:324,336,373,395,405,435,471,506`).
- `Buffer::empty(area)` / `Buffer::filled(area, Cell)` / `Buffer::with_lines` (`buffer.rs:78,84,92`).
- `Frame::buffer_mut() -> &mut Buffer` is the supported way to hand-write cells
  (`ratatui-core/src/terminal/frame.rs:207`, doc example writes
  `frame.buffer_mut()[(0,0)].set_symbol("h")`).

Direct `Cell` mutation is the intended low-level escape hatch — widgets do exactly this in their
`render` (e.g. `examples/apps/custom-widget/src/main.rs` writes cells directly). Prefer
`cell_mut` over indexing so out-of-range renders become no-ops instead of panics.

0.30.2 buffer fix worth knowing: when a wide char is replaced by a normal cell, trailing cells'
background is now correctly cleared (`CHANGELOG.md` 0.30.2, PR #2587).

---

## 2. Custom `Widget` / `StatefulWidget`, rendering without `Paragraph`

Traits: `ratatui-core/src/widgets/widget.rs` and `ratatui-core/src/widgets/stateful_widget.rs`
(module `ratatui-core/src/widgets.rs`).

```rust
pub trait Widget { fn render(self, area: Rect, buf: &mut Buffer) where Self: Sized; }
pub trait StatefulWidget {
    type State: ?Sized;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
}
```
(`widget.rs`; `stateful_widget.rs`). `State::State` may be unsized (`type State = [u8]` is a
documented example).

Render calls on a frame: `Frame::render_widget(widget, area)` and
`Frame::render_stateful_widget(widget, area, &mut state)` (`frame.rs:106,147`).

You do **not** need `Paragraph`. A widget is just "write into a `Rect` of the buffer", and nesting
is normal — the trait doc example renders a `Line` from inside a custom `Widget`
(`widget.rs`: `impl Widget for MyWidget { fn render(self, area, buf) { Line::raw("Hello").render(area, buf); } }`).
`Widget` is implemented for `&str`, `String`, and `Option<W>` (`widget.rs`), so plain strings are
already widgets.

Patterns the docs recommend (`widget.rs` header): implement `Widget` for `&MyWidget` when the data
is immutable; implement `StatefulWidget` when you need state across draws; implement `Widget` for
`&mut MyWidget` for the rare mutable case. All built-in widgets implement both `Widget` and
`WidgetRef` (render references, store boxed widgets). 0.30.1 added `AsRef` impls for the built-ins
(`ratatui.rs/highlights/v0301/`).

Full worked example (button with theme + hover/active state, mouse hit-testing via `Rect`):
`examples/apps/custom-widget/src/main.rs`.

---

## 3. `Canvas` + `Marker` types

`Canvas` is `ratatui-widgets/src/canvas.rs`; shapes are `ratatui-widgets/src/canvas/*.rs`
(`circle.rs`, `line.rs`, `map.rs`, `points.rs`, `rectangle.rs`). Import as
`ratatui::widgets::canvas::{Canvas, Circle, Line, Points, Rectangle, Map, FilledLine, Context, Painter}`
(`examples/apps/canvas/src/main.rs:24-25`).

`Canvas` builder (`canvas.rs:758-845`): `Canvas::default()` / `new(...)`, `.block(Block)`,
`.x_bounds([f64;2])`, `.y_bounds([f64;2])`, `.marker(Marker)`, `.paint(|ctx| …)`,
`.background_color(Color)`.

`Marker` — `ratatui-core/src/symbols/marker.rs`, re-exported as `ratatui::symbols::Marker`. It is
`#[non_exhaustive]`. Variants and resolution:

| Marker | Glyph | Grid per cell | Good for |
|---|---|---|---|
| `Dot` (default) | `•` | 1×1 | sparse scatter, maximum font support |
| `Block` | `█` | 1×1 | solid fills, stacked lines |
| `Bar` | `▄` | 1×1 | bar-ish strokes |
| `Braille` | `⠓⣇⣿` | 2×4 | dense curves/plots; **default for Chart/Canvas**, broadly supported |
| `HalfBlock` | `█▄▀` | 1×2 | square pixels (cell is ~2× tall) |
| `Quadrant` | quadrant chars | 2×2 | dense, regularly spaced pseudo-pixels |
| `Sextant` | Legacy Computing | 2×3 | dense; newer Unicode, less widely supported |
| `Octant` | Legacy Computing | 2×4 | Braille resolution without visible bands; newer Unicode |
| `Custom(char)` | any char | 1×1 | custom dot, added 0.30.1 |

(`marker.rs`; `CHANGELOG.md` 0.30.0 PR #2235 added Quadrant/Sextant/Octant; 0.30.1 added
`Custom(char)` per `ratatui.rs/highlights/v0301/`.) Braille/Sextant/Octant docs warn that
unsupported terminals show `�`.

`Painter`/`Context` drawing API (`canvas.rs`): `ctx.draw(&shape)`, `ctx.print(x, y, line)`,
`ctx.layer()`, `get_point(x,y) -> Option<(usize,usize)>`, `paint(x,y,color)` (`canvas.rs:449-791`).
0.30.1 added filled areas via `FilledLine` (`ratatui.rs/highlights/v0301/`).
Caveat: `Canvas` resolution is `u16::MAX`-bounded per the 0.30.0 fix "Allow canvas area to exceed
u16::MAX" (`CHANGELOG.md` 0.30.0). Full demo: `examples/apps/canvas/src/main.rs`; a rich 3D/braille
use is `examples/apps/volatility-surface/` (Braille projection,
`ratatui.rs/highlights/v0301/`).

---

## 4. `Layout`: constraints, `Flex`, spacing, nested split/areas

`ratatui-core/src/layout/` (`constraint.rs`, `flex.rs`, `layout.rs`, `margin.rs`, `offset.rs`,
`direction.rs`, `rect.rs`).

`Constraint` (`constraint.rs:72-218`), all variants present:

- `Length(u16)` (`:140`) — fixed cells.
- `Min(u16)` (`:94`) — at least n; grows.
- `Max(u16)` (`:117`) — at most n.
- `Percentage(u16)` (`:168`) — percent of the split area.
- `Ratio(u32, u32)` (`:192`) — exact proportion, e.g. `Ratio(1,3)`.
- `Fill(u16)` (`:218`) — fills leftover space, weighted; **lower priority than spacers**, only gets
  width under `Flex::Stretch`/`Flex::Legacy` (`examples/apps/flex/src/main.rs:101`).

`Flex` (`flex.rs:26-209`): `Legacy` (excess to last element; the pre-0.30 behaviour), `Start`
(default), `End`, `Center`, `SpaceBetween`, `SpaceEvenly`, `SpaceAround`. `Layout` defaults:
`margin 0`, `flex Flex::Start`, `spacing 0` (`layout.rs` docs near `:230`). 0.30.0 split the old
`SpaceAround` into `SpaceEvenly` + a CSS-aligned `SpaceAround` (breaking, `CHANGELOG.md` 0.30.0
PR #1952) — pick the name that matches your spacing distribution.

`Layout` builder + results (`layout.rs`): `Layout::vertical(..)` / `horizontal(..)` / `new(dir, ..)`,
`.constraints(..)`, `.spacing(u16)`, `.flex(Flex)`, then:

- `.areas::<N>(area) -> [Rect; N]` (`:578`) — const-size split, and `.try_areas` (`:614`).
- `.split(area) -> Rects` (`:688`) and `.split_with_spacers` (`:738`) — dynamic-size split;
  `.spacers::<N>()` (`:646`) returns the gaps (useful to draw in the gutters).
- Nest by splitting a returned `Rect` again (`Layout::vertical(...).areas()` then split a band).

`Flex::Center` + `Rect::centered(..)` / `centered_horizontally/vertically(constraint)`
(`rect.rs:513,531,551`) is the clean way to popup/modal without manual math; `Rect::outer` and
`Rect::resize` area helpers were added in 0.30.0 (`CHANGELOG.md` PRs #1929, #2240). Full worked demo with every constraint × every flex:
`examples/apps/flex/src/main.rs`; interactive: `examples/apps/constraint-explorer/`.
`Layout::init_cache()` / `DEFAULT_CACHE_SIZE = 500` (feature `layout-cache`, on by default)
(`layout.rs`).

---

## 5. `Block` / `BorderType`, titles, padding

`Block` is `ratatui-widgets/src/block.rs`; borders in `ratatui-widgets/src/borders.rs`.

Constructors: `Block::default()` / `Block::new()` / `Block::bordered()` (`block.rs:293`).
Builder: `borders(Borders)`, `border_style(Style)` (`:520`), `style(Style)` (`:562`),
`inner(area) -> Rect` (`:762`) to get the usable content rect (use this instead of hand-computing).

`BorderType` (`borders.rs:36-…`), 12 variants with glyph examples in-source: `Plain` (default),
`Rounded`, `Double`, `Thick`, `LightDoubleDashed`, `HeavyDoubleDashed`, `LightTripleDashed`,
`HeavyTripleDashed`, `LightQuadrupleDashed`, `HeavyQuadrupleDashed`, `QuadrantInside`,
`QuadrantOutside`. Dashed variants added 0.30.0 (`CHANGELOG.md`, PR #1573). `to_border_set()` gives
the raw symbol `Set` if you want to draw matching separators.

Titles: `title(impl Into<Line>)` (`:366`, defaults to top), `title_top` (`:397`),
`title_bottom` (`:426`), `title_style` (`:445`), `title_alignment(Alignment)` (`:471`),
`title_position(TitlePosition)` (`:494`). Multiple titles are supported; 0.30.0 fixed left-aligned
title truncation and preserved titles when merging borders (`CHANGELOG.md` 0.30.0 PRs #1931, #1977).

Padding: `Padding` (`ratatui-widgets/src/block/padding.rs`) with `Block::padding(..)`:
`Padding::uniform/horizontal/vertical/proportional/symmetric/left/right/top/bottom(u16)`
(`padding.rs:50-152`). Doc note: cells are ~2× tall, so double horizontal padding to look equal.

Cross-cutting polish added in 0.30.x, all usable on any `Block`-backed widget:

- **Shadows**: `Block::shadow(Shadow)` (`block.rs:731`) with presets `Shadow::overlay/block/
  light_shade/medium_shade/dark_shade/symbol/custom(CellEffect)` (`block/shadow.rs:162-268`),
  `.style(..)`, `.offset(Offset)` (`shadow.rs:268,278`). Added 0.30.1
  (`ratatui.rs/highlights/v0301/`); custom effects must be thread-safe/Send (fixed 0.30.2).
- **Border merging**: `Block::merge_borders(MergeStrategy)` (`block.rs:707`).

---

## 6. `Style` / `Modifier`, colours, and a capability-aware downgrade

`Style` is `ratatui-core/src/style.rs` (type) with `color.rs`, `stylize.rs`, `anstyle.rs`.
Fields are public: `fg: Option<Color>`, `bg: Option<Color>`, `underline_color: Option<Color>`
(`style.rs:239-249`). Builders: `Style::new()` (`:300`), `reset()` (`:312`), `.fg(..)` (`:335`),
`.bg(..)` (`:352`), `.underline_color(..)` (`:387` — only rendered when the `underline-color`
feature is on, default), `.add_modifier`/`.remove_modifier` (`:408,430`), `.has_modifier`
(`:448`, added 0.30.0), `.patch(other)` (`:471`).

`Color` (`color.rs:69-126`): `Reset`, named `Black Red Green Yellow Blue Magenta Cyan Gray DarkGray
LightRed … White`, `Rgb(u8,u8,u8)` (truecolor), `Indexed(u8)` (0-255 ANSI).
Construction helpers: `Color::from_u32(u32)` (`:133`), `from_hsl`/`from_hsluv` (feature `palette`,
`:416,469`), plus 0.30.0 array/tuple RGB conversions (`CHANGELOG.md`, PR #1703).

`Modifier` (`style.rs:104-114`) is a `bitflags` over `u16`: `BOLD`, `DIM`, `ITALIC`, `UNDERLINED`,
`SLOW_BLINK`, `RAPID_BLINK`, `REVERSED`, `HIDDEN`, `CROSSED_OUT`. Combine with `|`; `Debug` prints
`NONE`/bits. `DIM` and `ITALIC` are exactly the low-cost "de-emphasise a whole band without a new
colour" levers. 0.30.0 fixed "Terminal should keep Bold when removing Dim" (`CHANGELOG.md`) and
0.30.1 supported `Modifier::HIDDEN` in the crossterm backend (`ratatui.rs/highlights/v0301/`).

**Underline colour**: `Style::underline_color(Color)` / `cell.underline_color` — requires the
`underline-color` feature (default on), **Crossterm backend only, not Windows 7**
(`ratatui-core/src/lib.rs` feature docs: `underline-color` "enables the backend code that sets the
underline color"). Good for a coloured rule under a focused row without recolouring the text.

**Capability-aware colour downgrade.** Ratatui does **not** ship a truecolor→Indexed quantiser —
`anstyle::TryFrom<Color> for Ansi256Color` **errors** for `Rgb` ("cannot convert Ratatui Color to an
Ansi256Color as it is not an indexed color", `style/anstyle.rs:10-30`). So downgrade yourself.
The signal lives in the backend crate, not ratatui:

- `crossterm::style::available_color_count() -> u16` (`crossterm 0.29.0`,
  `docs.rs/crossterm/0.29.0/crossterm/style/fn.available_color_count.html`; source
  `crossterm/src/style.rs:163-181`). Notes field: "This does not always provide a good result."
- Practical tiers: `TERM`/`NO_COLOR` env + `available_color_count()` → pick `Rgb` when ≥ 16_777_216,
  `Indexed` when ≥ 256, else the 16 named colours. Keep a `HashMap<Rgb, Indexed>` memo and a simple
  nearest-ANSI-256 table (no dependency needed; `colorgrad` can hand you the interpolation stops).

This matters here: the repo's Oklab ramps come out as `Color::Rgb`, and `identity never rests on
colour alone` is already an invariant — a downgrade that snaps to `Indexed` (or adds a sigil/glyph
distinction) keeps that invariant on 256-colour terminals. Reference palette code for the
`Indexed` path: `examples/apps/color-explorer/src/main.rs:117-219`. `Style`/`Color` are `serde`-able
with the `serde` feature (save a theme to disk) (`ratatui/Cargo.toml` feature docs).

---

## 7. Built-in widgets in 0.30 (`ratatui-widgets 0.3.2`)

All re-exported under `ratatui::widgets` (`ratatui-widgets/src/lib.rs`). What each renders:

| Widget | Renders | Key builder (source) |
|---|---|---|
| `Block` | borders / titles / padding / shadow | `block.rs` |
| `Paragraph` | wrapped, styled, scrollable text | `paragraph.rs` — `.scroll((y,x))` (`:233`), `.line_count(w)` (`:332`), inherits `Text` alignment (0.30.1) |
| `Table` | rows × columns grid, selection | `table.rs` — `.header/.footer` (`:386,409`), `.widths` (`:439`), `.column_spacing` (`:465`), `.row_highlight_style` (`:586`), `.highlight_symbol` (`:656`), `.highlight_spacing` (`:690`), `.flex(Flex)` (`:719`) |
| `List` | selectable item list | `list.rs` |
| `Tabs` | horizontal tab bar w/ one selected | `tabs.rs` — `.select` (`:222`), `.divider` (`:277`), `.padding(l,r)` (`:304`), `.padding_left/right` |
| `Scrollbar` | scrollbar thumb+track+arrows | `scrollbar.rs` — `ScrollbarOrientation::{VerticalLeft,VerticalRight,HorizontalTop,HorizontalBottom}` (`:106-113`); `.thumb_style/.track_style/.begin_style/.end_style` (`:267-348`) |
| `Sparkline` | one-line dataset as bars | `sparkline.rs` — `.data` (`:209`), `.max` (`:223`), `.bar_set` (`:233`), `.direction` (`:242`), `.absent_value_style/symbol` (`:132,143`) |
| `BarChart` | grouped/vertical bars | `barchart/` — `BarChart::grouped` (0.30.0, PR #1513) |
| `Chart` | line/scatter/area datasets | `chart.rs` — `GraphType::Area` + `Dataset::fill_to_y(f64)` (0.30.1) |
| `Gauge` | progress bar w/ label | `gauge.rs` — `.percent` (`:75`), `.ratio` (`:97`), `.use_unicode` (`:148`) |
| `LineGauge` | one-line progress | `gauge.rs:298-412` — `.line_set` (`:333`), `.filled_symbol/.unfilled_symbol` (`:341,348`), `.filled_style/.unfilled_style` (`:402,412`) |
| `Calendar::Monthly` | a month grid w/ events | `calendar.rs` — `Monthly::new(Date, events)` (`:38`), `.show_month_header` (`:83`), `.show_weekdays_header` (`:71`), `.width()/.height()` (`:109,124`, added 0.30.0) |
| `Clear` | blanks its area (paint-over) | `clear.rs` |
| `Fill` | repeats one symbol+style over an area | `fill.rs:51-87` — `Fill::new("X").blue()`; added 0.30.1 |
| `RatatuiLogo` / `RatatuiMascot` | decorative logo/mascot | `logo.rs`, `mascot.rs` |

Stateful widgets and their state types (drive with `render_stateful_widget`):

- `TableState` (`table/state.rs`): `offset()`, `offset_mut()`, `selected()`, `selected_column()`,
  `selected_cell()`, `with_offset/with_selected/with_selected_cell`, `select*`, `scroll_up_by`,
  `scroll_down_by`, `scroll_right_by`, `scroll_left_by`. 0.30.0 added multi-column cells
  (`Cell::column_span(n)`, `ratatui.rs/highlights/v0301/`).
- `ListState` (`list/state.rs`) — same natural-scroll contract described in
  `stateful_widget.rs`.
- `ScrollbarState` (`scrollbar.rs:145`): `new(content_length)`, `.position(usize)`,
  `.content_length(usize)`, `.viewport_content_length(usize)`, `prev/next/first/last/scroll`,
  `get_position()` (added 0.30.0).

`Calendar` needs the `calendar` feature (default-on, pulls `time`) (`ratatui-widgets/src/lib.rs`
feature docs). Everything else is `all-widgets`, default-on.

---

## 8. Unicode width handling and the `unicode-width` dependency

Deps: `ratatui-core` and `ratatui-widgets` both depend on `unicode-width` plus `unicode-segmentation`
and `unicode-truncate` (`ratatui-core/Cargo.toml:83-85`, `ratatui-widgets/Cargo.toml:63-64`).
Workspace pin: `unicode-width = ">=0.2.0"` (deliberately permissive; see
`github.com/ratatui/ratatui/issues/1271`), `unicode-truncate = "2"`, `unicode-segmentation = "1"`
(`Cargo.toml:78-81`).

Where it is used: `Line`/`Span`/`Text` implement `UnicodeWidthStr` (`text/line.rs:568`,
`text/span.rs:379`, `text/text.rs:565`); `Span::styled_graphemes()` and `Buffer::set_stringn`
truncate on width, and `CellWidth`/`cell_width()` (`buffer/cell_width.rs`) is the 0.30.1
Ratatui-native width API. `cell_width.rs` also compensates for halfwidth katakana dakuten/handakuten
`U+FF9E/FF9F` that `unicode-width` reports as zero-width but terminals render as one cell each
(`cell_width.rs` doc + tests). Use `Span::width()` / `Line::width()` / `str::cell_width()` for your
own truncation instead of `.len()`; the repo's "nothing wraps or overruns the width" invariant is
exactly what these enforce.

---

## 9. Image / graphics-protocol support

Ratatui itself ships **no** image widget. It provides only the plumbing to not fight a graphics
protocol: `Cell::set_diff_option(CellDiffOption::Skip)` to stop the buffer overwriting cells covered
by Sixel/iTerm/Kitty output (`buffer/cell.rs:216-232`), `CellDiffOption::ForcedWidth(NonZeroU16)`
for cells holding ANSI/OSC/kitty placeholder sequences, and `CellDiffOption::AlwaysUpdate`
(`ratatui.rs/highlights/v0301/`; `buffer/buffer.rs` kitty-placeholder tests at `:1257-1290`).
`Terminal::insert_before` + the `scrolling-regions` feature reduce flicker for inline drawing
(`ratatui-core/src/terminal/inline.rs:109`; `ratatui-core/Cargo.toml:62`).

The real integration is **`ratatui-image`** (separate crate, `docs.rs/ratatui-image`; 11.1.0
targets `ratatui ^0.30.1`). It queries the terminal for a graphics protocol and font size via
`Picker::from_query_stdio()` (env-var guess, then control-sequence query), falling back to unicode
**halfblocks** when no protocol exists. Widgets: `Image` (fixed size, stateless) and `StatefulImage`
(resize at render; offload encode to `thread::ThreadProtocol`, see the ratatui-image repo's
`examples/thread.rs` and `examples/tokio.rs`). Protocols:
Sixel, Kitty, iTerm2, halfblock. Terminal requirement: a terminal speaking one of those protocols
(Kitty/Ghostty/WezTerm/Foot for Kitty/Sixel; iTerm2/Terminal.app for iTerm; halfblock works
everywhere with truecolor). For this repo — a dense five-panel text grid rendered to text for
tests — the value is halfblocks only, if ever; it is not needed for the polish goal.

---

## 10. Scroll / viewport patterns for a bottom-anchored transcript

Bottom-anchoring = render the last `H` lines of a `Vec<Line>` into the band. Two viable shapes:

1. **`Paragraph` + `.scroll`** — `Paragraph::new(&lines).scroll((offset, 0))`, where `offset =
   total_lines.saturating_sub(viewport_height)`, so offset 0 is the newest. `scroll((Vertical,
   Horizontal))` is `(y, x)` (`paragraph.rs:86,233`); `.line_count(width)` (`:332`) tells you how
   many wrapped lines exist so you can anchor to the end after wrapping. This is the direct
   translation of the current `tui.py`/`tui.rs` bottom-anchored transcript.

2. **Stateful `Table`/`List`** — keep newest last and set the offset to the max so the newest row is
   the last visible one: `TableState::with_offset(n_rows.saturating_sub(h))` or
   `*state.offset_mut() = …` (`table/state.rs`; `ScrollbarState` `offset`/`position`). `List` uses
   the same natural-scroll offset contract (`stateful_widget.rs`).

`Viewport` (`ratatui::Viewport`, `ratatui-core/src/terminal/viewport.rs`): `Fullscreen`, `Inline`,
`Fixed`. For a live transcript that should not take the whole terminal, `Inline` +
`Terminal::insert_before(height, |buf| …)` (`inline.rs:109`) appends lines above the viewport —
needs `scrolling-regions` to avoid flicker (`ratatui-core/Cargo.toml:62`).
`Terminal::apply_buffer()` (0.30.1, `terminal/render.rs:239`) lets a component commit a buffer
outside the single `draw` closure — handy if a transcript buffer is assembled separately.

---

## Quick "not-used-yet, worth it" checklist for this repo

`src/tui.rs` currently uses only `Block`/`BorderType`/`Padding`/`Paragraph`/`Modifier`, with
**zero** occurrences of `Marker`, `Canvas`, `Indexed`, `Flex`, `spacing`, `Buffer::cell_mut`,
`buffer_mut`, `Table::`, `List::`, `title_bottom`, `column_spacing`, `underline_color`,
`insert_before`, `Shadow`, `Fill::` (verified by grep over the 1869-line file). Highest-leverage
0.30 additions not yet touched: `Layout::spacing` + `Flex::SpaceBetween` (gutters between the five
bands), `Rect::centered`/`Layout` `Flex::Center` (popups/help), `title_bottom` + `Block::inner`
(second title without a second block), `Style::underline_color` (focused row rule),
`Modifier::DIM` (de-emphasise non-focused tables), `Fill`/per-cell `bg` (subtle band tint),
`CellDiffOption`/`buffer_mut` (hand-painted sparkline/bar glyphs), and a truecolor→Indexed
downgrade via `crossterm::style::available_color_count`.
