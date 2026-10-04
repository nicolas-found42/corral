//! The Corral's UI: two zoom levels over five tables of four llamas.
//!
//!   overview (focus = -1):  a heat ribbon, five full-width table bands (one per
//!                           table) separated by bare gutters, then the wire.
//!   detail   (focus = 0..4): three bands -- the judge across the top, the transcript
//!                           beneath it, a bottom band holding the roster beside the
//!                           beliefs -- then the wire as its own band.
//!
//! Keys: 1-5 focus a table · o overview · Tab / . cycle · f flat · h help · q quit.
//!
//! The colour ramps interpolate in **Oklab** (via `colorgrad`) so the dusk horizon
//! reads vivid rather than muddy.
//!
//! ## The redesign (every fork put to TypeSafe Jev; see docs/ui-redesign.md)
//!
//! The brief was *prettier and more cutting-edge without cluttering the screen or
//! sacrificing readability*, so the design is subtractive. What it removes: four of
//! five bright border outlines (chrome is now **one-sided hairlines** -- each band
//! draws only the edge it shares with the next, and no band draws a left or right
//! edge), sixteen of twenty saturated hues from the body (an agent's colour now
//! encodes its **voice family**, on its sigil alone), the alternating two-glyph
//! horizon (a single-glyph hairline carries the same ramp), a header content row,
//! four of six footer hints, the whole-frame dissolve, the dotted rule between tables
//! (a bare gutter does it), and the double-marking of overheard lines.
//!
//! What it adds, and nothing else: **one** animated element -- a braille dot that
//! travels the wire when a line is caught crossing between tables -- and a one-row
//! **heat ribbon** that makes the five tables' judged heat comparable at a glance.
//!
//! Invariants kept from the original, and enforced offline in `tests/`:
//!   * one metaphor per layer, never two in one glyph;
//!   * identity never rests on colour alone -- every llama carries a sigil + name +
//!     a discourse-move glyph, and `f` gives a flat, colourless frame;
//!   * nothing wraps or overruns the width; every title stays whole; every table
//!     shows its newest line; the transcript is bottom-anchored;
//!   * the frame is pure in (room, cfg, size): no network, no terminal, and the one
//!     animated element is computed from an explicit frame counter, never a clock.
//!
//! De-emphasis is by **lightness**, never by the terminal's `faint` attribute:
//! judgements scored 0.82 that many terminals do not render `DIM` distinctly, so a
//! lower-contrast colour is used instead. Contrast floors come from
//! `docs/research/readability-and-colour.md` (text 4.5:1 WCAG, non-text 3:1) and are
//! asserted in the unit tests at the bottom of this file.

use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};
use ratatui::Frame;

use std::sync::atomic::{AtomicU8, Ordering};

use colorgrad::{BlendMode, Gradient, GradientBuilder, LinearGradient};

use crate::personas::{by_id, family_colour, Persona};
use crate::room::{move_colour, Room, Table, GAUGES, PROPOSITIONS};

pub const BLOCKS: &str = " ▏▎▍▌▋▊▉█";
/// Four levels, not eight (Jev: restyle the sparkline, 0.79).
pub const SPARK: &str = "▁▃▅█";
/// Book measure for prose (Jev 0.68): continuations wrap at 76 columns from the left
/// margin no matter how wide the terminal is, so wide frames gain a calm right
/// margin instead of 149-character lines. Narrow frames are untouched.
pub const PROSE_MEASURE: usize = 76;

// ------------------------------------------------------------------- the palette
// Ember dusk, refined: a deep indigo ground lit by one warm amber. (Jev 0.80.)
// One colour, one job, so the reader can learn the roles.

/// The ground behind the masthead.
pub const INK: &str = "#12102b";
/// The raised ground, kept for the one solid block in the frame.
pub const INK2: &str = "#1c1940";
/// The one warm accent: attention and the live signal. (Jev 0.93.)
pub const AMBER: &str = "#ffb454";
pub const GOLD: &str = "#ffe0a0";
/// Non-text only: fails WCAG for text on this ground (4.20:1).
pub const EMBER: &str = "#ff7a3d";
/// Non-text only: the indigo ramp stop and border hue.
pub const INDIGO: &str = "#5b6ee1";
/// Indigo as text -- the fixed value that clears 4.5:1 (was 4.20:1).
pub const BELIEF: &str = "#aab6ff";
pub const GOOD: &str = "#8fd694";
pub const WARN: &str = "#ffd75f";
pub const BAD: &str = "#ff6b6b";
/// The reserved eavesdrop hue. One meaning, the whole frame through.
pub const HEARD: &str = "#c98cff";
/// The accent's second trigger: a question the room left open. Same hue, one job --
/// attention -- so an unanswered line is marked without a new colour in the palette.
pub const OPEN: &str = AMBER;

/// Body prose: an off-white just below pure white. Pure white scores Lc -107 on this
/// ground, past the Lc 90 halation cap; this is 15.0:1.
pub const BODY: &str = "#e8e6f2";
/// Labels that must still be legible: 9.4:1. (Was #8a86ad, Lc -39.)
pub const LABEL: &str = "#b8b5d8";
/// Decoration only -- rules, ticks, gutters. Never carries text the reader needs.
pub const CHROME: &str = "#6f6b93";
/// Back-compat alias for the decoration grey; prefer `CHROME` in new code.
pub const FAINT: &str = CHROME;
/// Secondary text -- metadata, gauge poles, captions, hints. 6.81:1, so it
/// stays quieter than LABEL without dropping under the 4.5:1 text floor.
pub const DIM: &str = "#9d99bd";
/// The focused band's chrome. Lightness, not a new hue.
pub const FOCUS: &str = "#cfcbe8";
/// Back-compat alias for the label grey.
pub const MUTED: &str = LABEL;

/// The horizon ramp: cold indigo -> rose -> pale gold, as the room heats up.
pub const DUSK: [&str; 6] = [
    "#2a2b52", "#4a3a6a", "#8a4a6a", "#d06a4a", "#ffb454", "#ffe0a0",
];

/// One glyph, one variable: the discourse move is a glyph, the speaker a colour.
pub fn move_glyph(mv: &str) -> &'static str {
    match mv {
        "claim" => "●",
        "evidence" => "▹",
        "question" => "?",
        "rebuttal" => "⊗",
        "analogy" => "≈",
        "concession" => "✓",
        "tangent" => "∿",
        _ => "·",
    }
}

/// The two poles each gauge names.
pub fn ends(g: &str) -> (&'static str, &'static str) {
    match g {
        "heat" => ("quiet", "lively"),
        "consensus" => ("split", "agreed"),
        "drift" => ("on seed", "off seed"),
        _ => ("stale", "fresh"),
    }
}

/// The three belief propositions under short, canonical names (Jev 0.46).
pub fn belief_short(p: &str) -> &'static str {
    if p == PROPOSITIONS[0] {
        "agreed"
    } else if p == PROPOSITIONS[1] {
        "divided"
    } else {
        "novel"
    }
}

pub const QUALITY_DIMS: [&str; 3] = ["originality", "clarity", "insight"];

/// The quality dimensions are delineated by form and named in the legend, not by
/// three more hues (Jev: one accent plus one reserved hue, everything else neutral).
pub fn quality_colour(_d: &str) -> &'static str {
    AMBER
}

/// The per-frame config, mirroring the original `cfg` dict.
#[derive(Debug, Clone)]
pub struct Cfg {
    pub draws: usize,
    pub max_rounds: Option<u32>,
    pub flat: bool,
    pub hint: bool,
    /// An animating counter for the one animated element -- never a clock.
    pub frame: u64,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            draws: 2,
            max_rounds: None,
            flat: false,
            hint: false,
            frame: 0,
        }
    }
}

// ------------------------------------------------------------ colour capability
// The redesign is truecolor-first. On a 256-colour terminal the terminal snaps a hex
// to the nearest palette index (measured: the indigo drifts from 4.20:1 to 3.62:1,
// under the 3:1 floor), and on a 16-colour terminal the base names are
// user-themeable so no ratio can be guaranteed at all. So the palette is resolved
// through one function and the depth is chosen once at startup. `flat` mode (key `f`,
// and the `NO_COLOR` convention) bypasses all of it.

/// How much colour the terminal can actually show. (Jev 0.97 for the three tiers.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    Truecolour = 0,
    Indexed = 1,
    Ansi = 2,
}

static DEPTH: AtomicU8 = AtomicU8::new(0);

/// Choose the colour depth for the whole process. Defaults to truecolor, which is
/// what the offline tests render, so `compose_text` never depends on the terminal.
pub fn set_depth(d: Depth) {
    DEPTH.store(d as u8, Ordering::SeqCst);
}

pub fn depth() -> Depth {
    match DEPTH.load(Ordering::SeqCst) {
        1 => Depth::Indexed,
        2 => Depth::Ansi,
        _ => Depth::Truecolour,
    }
}

fn hex(c: &str) -> (u8, u8, u8) {
    let c = c.trim_start_matches('#');
    (
        u8::from_str_radix(&c[0..2], 16).unwrap_or(0),
        u8::from_str_radix(&c[2..4], 16).unwrap_or(0),
        u8::from_str_radix(&c[4..6], 16).unwrap_or(0),
    )
}

/// The RGB an xterm-256 index stands for, so a downgraded colour can still be
/// measured for contrast rather than assumed to be fine.
pub fn index_rgb(i: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    if i < 16 {
        return BASE[i as usize];
    }
    if i < 232 {
        let n = i as u16 - 16;
        let steps = [0u8, 95, 135, 175, 215, 255];
        return (
            steps[(n / 36) as usize],
            steps[((n % 36) / 6) as usize],
            steps[(n % 6) as usize],
        );
    }
    let v = 8 + (i as u16 - 232) * 10;
    (v as u8, v as u8, v as u8)
}

fn dist2(a: (u8, u8, u8), b: (u8, u8, u8)) -> i32 {
    let d = |x: u8, y: u8| (x as i32 - y as i32).pow(2);
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

/// The nearest xterm-256 index to a hex colour. Deterministic, so a downgraded frame
/// is reproducible; the unit tests assert every text colour still clears 4.5:1
/// against the index it resolves to.
pub fn nearest_256(c: &str) -> u8 {
    let want = hex(c);
    (0u8..=255)
        .min_by_key(|i| dist2(want, index_rgb(*i)))
        .unwrap_or(16)
}

/// A hex string as a truecolor.
pub fn rgb(c: &str) -> Color {
    let (r, g, b) = hex(c);
    Color::Rgb(r, g, b)
}

/// Resolve a palette hex for the terminal we are actually on.
fn pal(c: &str) -> Color {
    match depth() {
        Depth::Truecolour => rgb(c),
        Depth::Indexed => Color::Indexed(nearest_256(c)),
        // The base 8 are user-themeable, so pick by role from their nominal values
        // and lean on the glyphs for anything colour can no longer carry.
        Depth::Ansi => {
            let (r, g, b) = hex(c);
            let lum = (r as u32 * 30 + g as u32 * 59 + b as u32 * 11) / 100;
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let span = max as i32 - min as i32;
            if span < 32 {
                match lum {
                    0..=60 => Color::Black,
                    61..=110 => Color::DarkGray,
                    111..=190 => Color::Gray,
                    _ => Color::White,
                }
            } else if max == r && b > 128 {
                Color::Magenta
            } else if max == r && g > 128 {
                Color::Yellow
            } else if max == r {
                Color::Red
            } else if max == g {
                Color::Green
            } else if max == b && g > 128 {
                Color::Cyan
            } else {
                Color::Blue
            }
        }
    }
}

fn style_fg(c: &str) -> Style {
    Style::default().fg(pal(c))
}

/// Relative luminance and the WCAG 2.x contrast ratio, used by the unit tests to hold
/// every palette role to the floors in `docs/research/readability-and-colour.md`.
pub fn relative_luminance(c: &str) -> f64 {
    let (r, g, b) = hex(c);
    let f = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}

pub fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

// ------------------------------------------------------------------- primitives

/// The dusk gradient, interpolated in Oklab so the ramp stays vivid.
fn dusk_gradient() -> &'static LinearGradient {
    use std::sync::OnceLock;
    static G: OnceLock<LinearGradient> = OnceLock::new();
    G.get_or_init(|| {
        let mut b = GradientBuilder::new();
        b.html_colors(&DUSK).mode(BlendMode::Oklab);
        b.build::<LinearGradient>()
            .expect("the six dusk stops build")
    })
}

/// `width` colours across the dusk ramp, warmed by the room's mean heat.
pub fn ramp(width: usize, heat: f64) -> Vec<Color> {
    if width == 0 {
        return Vec::new();
    }
    let g = dusk_gradient();
    let hi = (0.35 + 0.65 * (1.0 - heat.clamp(0.0, 1.0))).clamp(0.0, 1.0);
    (0..width)
        .map(|i| {
            let t = if width == 1 {
                0.0
            } else {
                i as f64 / (width - 1) as f64
            } * hi;
            let c = g.at(t as f32).to_rgba8();
            Color::Rgb(c[0], c[1], c[2])
        })
        .collect()
}

/// A fractional block bar, exactly as the Python `bar`.
pub fn bar(frac: f64, width: usize) -> String {
    let full = (frac.clamp(0.0, 1.0)) * width as f64;
    let n = full as usize;
    let tail: String = if n < width {
        let idx = ((full - n as f64) * 8.0) as usize;
        BLOCKS
            .chars()
            .nth(idx.min(8))
            .map(|c| c.to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let mut s = "█".repeat(n) + &tail;
    while s.chars().count() < width {
        s.push(' ');
    }
    s.chars().take(width).collect()
}

/// A value as a position on a two-ended scale: `├───•────┤`.
pub fn two_ended(v: f64, width: usize) -> String {
    let w = (width.max(4)) - 2;
    let v = v.clamp(0.0, 1.0);
    let pos = (v * (w - 1) as f64).round() as usize;
    format!("├{}{}{}┤", "─".repeat(pos), "•", "─".repeat(w - 1 - pos))
}

/// A sparkline, four levels. (Jev: restyle, 0.79.)
pub fn spark(vals: &[f64]) -> String {
    vals.iter()
        .map(|v| {
            let idx = ((v.clamp(0.0, 1.0)) * 3.999) as usize;
            SPARK.chars().nth(idx.min(3)).unwrap_or('▁')
        })
        .collect()
}

/// `mm:ss`.
pub fn mmss(t: f64) -> String {
    format!("{:02}:{:02}", (t / 60.0) as u64, (t as u64) % 60)
}

/// Collapse whitespace and clip to `n` characters with an ellipsis, never splitting
/// a word: words that fit stay whole, the rest is replaced by …. (A mid-word cut
/// reads as a rendering fault, not a truncation.)
pub fn shorten(text: &str, n: usize) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if n <= 1 {
        return if joined.is_empty() {
            String::new()
        } else {
            "…".to_string()
        };
    }
    if joined.chars().count() <= n {
        return joined;
    }
    let mut kept = String::new();
    for w in joined.split_whitespace() {
        let add = if kept.is_empty() {
            w.to_string()
        } else {
            format!(" {w}")
        };
        if kept.chars().count() + add.chars().count() + 1 > n {
            break;
        }
        kept.push_str(&add);
    }
    if kept.is_empty() {
        let kept_chars: String = joined.chars().take(n - 1).collect();
        return format!("{}…", kept_chars.trim_end());
    }
    format!("{kept}…")
}

fn chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// The dusk ramp as a *rule* is gone: it only ever appears where it encodes heat, so
/// the header's inline gradient -- whose cold end vanished into the ground and read as
/// a rendering fault (Jev 0.89) -- is now a plain neutral hairline. Kept for the
/// gradient builder used by `ramp`, which the tests exercise.
#[allow(dead_code)]
fn heat_of(room: &Room) -> f64 {
    let n = room.tables.len().max(1);
    room.tables
        .iter()
        .map(|t| t.gauges.get("heat").copied().unwrap_or(0.5))
        .sum::<f64>()
        / n as f64
}

fn width_of(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.width()).sum()
}

fn truncate_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let mut out: Vec<Span> = Vec::new();
    let mut used = 0usize;
    for s in spans {
        let w = s.width();
        if used + w <= width {
            used += w;
            out.push(s);
        } else {
            let room = width.saturating_sub(used);
            if room > 0 {
                let content: String = s.content.chars().take(room.saturating_sub(1)).collect();
                out.push(Span::styled(format!("{content}…"), s.style));
            }
            break;
        }
    }
    out
}

/// Wrap words so the first line starts after a leading head and every continuation
/// hangs at the text column: one clean left edge for all prose. (Jev 0.77.)
fn wrap_hanging(text: &str, first: usize, hang: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut room = first.max(8);
    for w in words {
        if cur.is_empty() {
            cur = w.to_string();
        } else if cur.chars().count() + 1 + w.chars().count() <= room {
            cur.push(' ');
            cur.push_str(w);
        } else {
            out.push(std::mem::take(&mut cur));
            cur = w.to_string();
            room = hang.max(8);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

// ----------------------------------------------------------------- the chrome
//
// One-sided hairlines (Jev 0.60; a rerank put the technique first of eight, 0.88): a
// band draws only the edge it shares with the next band -- its top rule. No band
// draws a left or right edge, and a band's bottom edge is the next band's top rule,
// so every boundary is drawn exactly once.

/// A titled hairline: the one edge a band shares with the band above it. The bold
/// title stays whole; a second fact rides the same edge on the right.
fn rule(title: &str, sub: &str, right: &str, width: usize, focused: bool) -> Line<'static> {
    let (tcol, rcol) = if focused {
        (FOCUS, FOCUS)
    } else {
        (LABEL, FAINT)
    };
    let mut spans: Vec<Span> = vec![Span::styled("─ ".to_string(), style_fg(rcol))];
    let mut used = 2usize;
    if !title.is_empty() {
        spans.push(Span::styled(
            title.to_string(),
            Style::default().fg(pal(tcol)).add_modifier(Modifier::BOLD),
        ));
        used += title.chars().count();
    }
    if !sub.is_empty() {
        let room_for = width.saturating_sub(used + right.chars().count() + 6);
        if room_for > sub.chars().count() {
            spans.push(Span::styled(format!(" {sub}"), style_fg(DIM)));
            used += sub.chars().count() + 1;
        }
    }
    if !right.is_empty() {
        let want = right.chars().count() + 3;
        if want < width.saturating_sub(used) {
            let pad = width.saturating_sub(used + want + 1);
            spans.push(Span::styled("─".repeat(pad + 1), style_fg(rcol)));
            spans.push(Span::styled(format!(" {right} "), style_fg(LABEL)));
            used += pad + 1 + right.chars().count() + 2;
        }
    }
    let fill = width.saturating_sub(used);
    if fill > 0 {
        spans.push(Span::styled("─".repeat(fill), style_fg(rcol)));
    }
    Line::from(spans)
}

/// The body of every band: no border, just a one-column gutter either side, so the
/// chrome that used to cost five bright boxes now costs nothing.
fn body_block() -> Block<'static> {
    Block::default().padding(Padding::horizontal(1))
}

// ------------------------------------------------------------------------ header

#[allow(dead_code)]
fn horizon_span(width: usize, heat: f64, flat: bool) -> Vec<Span<'static>> {
    if flat {
        return vec![Span::styled("─".repeat(width), style_fg(FAINT))];
    }
    // One glyph, not two: the alternating ━/▔ texture read as a dashed line
    // (Jev 0.98 for the single ramp; 0.84/0.85 in the supporting Nouls).
    ramp(width, heat)
        .into_iter()
        .map(|c| Span::styled("─", Style::default().fg(c)))
        .collect()
}

/// Two content rows, not three: the badge/status/metrics row, then the seed row,
/// which shares its line with the dusk ramp. (Jev 0.73.)
fn masthead(room: &Room, width: usize, flat: bool) -> Paragraph<'static> {
    let inner = width.saturating_sub(4).max(10);
    let st_col = match room.status.as_str() {
        "discussing" => AMBER,
        "paused" => WARN,
        "idle" => LABEL,
        "stopped" => GOOD,
        "error" => BAD,
        _ => BODY,
    };
    let status = room.status.to_uppercase();

    let mut spans: Vec<Span> = Vec::new();
    // The one solid-filled block in the whole frame. (Jev 1.00.)
    let badge_style = if flat {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
            .fg(pal(INK))
            .bg(pal(AMBER))
            .add_modifier(Modifier::BOLD)
    };
    spans.push(Span::styled(" ◆ THE CORRAL ", badge_style));
    let mut used = " ◆ THE CORRAL ".chars().count();
    let sub = "five tables, one seed, and Jev between them";
    if inner.saturating_sub(used) > 52 {
        spans.push(Span::styled(format!("  {sub}  "), style_fg(DIM)));
        used += sub.chars().count() + 4;
    }
    spans.push(Span::styled(
        format!(" {status} "),
        style_fg(st_col).add_modifier(Modifier::BOLD),
    ));
    used += status.chars().count() + 2;
    let metrics = format!(
        "  round {}   ⏱ {}   ${:.4}   ⇄ {}",
        room.turn,
        mmss(room.elapsed()),
        room.cost(),
        room.leaks.len()
    );
    if inner.saturating_sub(used) > metrics.chars().count() {
        spans.push(Span::styled("  round ".to_string(), style_fg(DIM)));
        spans.push(Span::styled(
            room.turn.to_string(),
            Style::default().fg(pal(BODY)).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("   ⏱ ".to_string(), style_fg(DIM)));
        spans.push(Span::styled(mmss(room.elapsed()), style_fg(BODY)));
        spans.push(Span::styled("   $".to_string(), style_fg(DIM)));
        spans.push(Span::styled(
            format!("{:.4}", room.cost()),
            Style::default().fg(pal(BODY)).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("   ⇄ ".to_string(), style_fg(DIM)));
        spans.push(Span::styled(
            room.leaks.len().to_string(),
            style_fg(HEARD).add_modifier(Modifier::BOLD),
        ));
    }

    let mut lines = vec![Line::from(spans)];

    // Row 2: the seed, then the dusk horizon filling whatever is left of the row.
    let mut r2: Vec<Span> = Vec::new();
    if !room.seed.is_empty() {
        r2.push(Span::styled("seed ".to_string(), style_fg(DIM)));
        let seed = shorten(&room.seed, 44);
        r2.push(Span::styled(
            format!("“{seed}”"),
            if flat {
                Style::default().fg(pal(BODY)).add_modifier(Modifier::BOLD)
            } else {
                style_fg(EMBER).add_modifier(Modifier::BOLD)
            },
        ));
    }
    if !room.last_error.is_empty() {
        r2.push(Span::styled(
            format!("   last error: {}", shorten(&room.last_error, 24)),
            style_fg(BAD),
        ));
    }
    // The rule is the room's own weather: a clamped heat-ramp sparkline, so the ramp
    // appears only where it encodes heat and its cold end stays above the floor (0.89).
    let used = width_of(&r2);
    let left = inner.saturating_sub(used + 1);
    let hist = room_heat_history(room, 60);
    if left >= 10 && hist.len() >= 4 {
        // The rule IS the room's temperature over time: a gradient sparkline drawn from
        // the clamped heat ramp, so it replaces the decoration it stands in for rather
        // than competing with the seed for width. (Jev 0.96, every check supported.)
        r2.push(Span::raw(" "));
        let glyphs: Vec<char> = SPARK.chars().collect();
        for i in 0..left {
            let t = if left == 1 {
                0.0
            } else {
                i as f64 / (left - 1) as f64
            };
            let j = (t * (hist.len() - 1) as f64).round() as usize;
            let v = hist[j.min(hist.len() - 1)].clamp(0.0, 1.0);
            let ch = glyphs[((v * 3.999) as usize).min(3)];
            let col = if flat { CHROME } else { heat_colour(v) };
            r2.push(Span::styled(ch.to_string(), style_fg(col)));
        }
    } else if left >= 8 {
        r2.push(Span::raw(" "));
        r2.push(Span::styled("─".repeat(left), style_fg(CHROME)));
    }
    lines.push(Line::from(r2));

    let border = if flat { Color::White } else { pal(AMBER) };
    let block = Block::bordered()
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(border))
        .padding(Padding::horizontal(1))
        .style(if flat {
            Style::default()
        } else {
            Style::default().bg(pal(INK))
        });
    Paragraph::new(Text::from(lines)).block(block)
}

/// Focus by lightness, not by a fill: the active tab is bold, bright and underlined;
/// every other tab is the quiet grey. (Jev 0.58 for lightness; 0.82 in the craft
/// survey for de-emphasising chrome; a solid block scored 0.08 and was contradicted.)
fn tab_bar(room: &Room, flat: bool) -> Paragraph<'static> {
    let mut spans: Vec<Span> = Vec::new();
    for tb in &room.tables {
        let on = room.focus == tb.id as i32 - 1;
        let style = if on {
            Style::default()
                .fg(if flat { Color::White } else { pal(FOCUS) })
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            style_fg(DIM)
        };
        spans.push(Span::styled(format!(" {} {} ", tb.id, tb.name()), style));
    }
    let on_overview = room.focus < 0;
    let ov_style = if on_overview {
        Style::default()
            .fg(if flat { Color::White } else { pal(FOCUS) })
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        style_fg(DIM)
    };
    spans.push(Span::styled(" [ o overview ] ", ov_style));
    Paragraph::new(Line::from(spans))
}

/// Three contextual hints, not six (Jev 0.88); the full map lives behind `h`, and the
/// room's own state rides the same row where it costs nothing.
fn footer(room: &Room, flat: bool) -> Paragraph<'static> {
    let (keys, view) = if room.focus < 0 {
        (
            vec!["1-5 open a table", "t seed", "h keys"],
            "the five tables".to_string(),
        )
    } else {
        (
            vec!["Tab next", "space pause", "h keys"],
            format!("table {}", room.focus + 1),
        )
    };
    let view_style = Style::default()
        .fg(if flat { Color::White } else { pal(FOCUS) })
        .add_modifier(Modifier::BOLD);
    let mut spans: Vec<Span> = vec![Span::styled(format!(" {view}"), view_style)];
    for k in keys {
        spans.push(Span::styled(format!("  ·  {k}"), style_fg(DIM)));
    }
    if room.focus < 0 && !room.status.is_empty() {
        spans.push(Span::styled("  ·  ".to_string(), style_fg(DIM)));
        let (tone, txt) = match room.status.as_str() {
            "stopped" => (DIM, "the tables have closed"),
            "error" => (BAD, "an error stopped the room"),
            "paused" => (WARN, "paused"),
            _ => (DIM, "five tables talking"),
        };
        spans.push(Span::styled(txt.to_string(), style_fg(tone)));
    }
    Paragraph::new(Line::from(spans)).alignment(Alignment::Left)
}

// ------------------------------------------------------------------- heat ribbon

/// Pick the nearest named palette stop for a ramp colour, so the ribbon stays in the
/// same vocabulary as everything else.
#[allow(dead_code)]
fn dusk_near(c: [u8; 4]) -> &'static str {
    let want = (c[0], c[1], c[2]);
    let mut best = DUSK[0];
    let mut bestd = i32::MAX;
    for stop in DUSK {
        let d = dist2(want, hex(stop));
        if d < bestd {
            bestd = d;
            best = stop;
        }
    }
    best
}

/// One table's heat as a colour on the same clamped ramp the ribbon uses: the coldest
/// stop is 3.37:1 on the ground, so even a cold band's rail is visible, and the ramp
/// only ever appears where it encodes heat.
fn heat_colour(v: f64) -> &'static str {
    let stops = RIBBON_RAMP;
    let t = (0.10 + 0.90 * v.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    stops[((t * (stops.len() - 1) as f64).round() as usize).min(stops.len() - 1)]
}

/// The room's mean judged heat over the last `n` samples: the average across the five
/// tables of the heat history each already records. No new judgement, no new state.
fn room_heat_history(room: &Room, n: usize) -> Vec<f64> {
    let series: Vec<&Vec<f64>> = room
        .tables
        .iter()
        .filter_map(|t| t.history.get("heat"))
        .filter(|h| !h.is_empty())
        .collect();
    if series.is_empty() {
        return Vec::new();
    }
    let len = series.iter().map(|h| h.len()).min().unwrap_or(0);
    let take = len.min(n);
    (0..take)
        .map(|i| {
            let back = take - 1 - i;
            series.iter().map(|h| h[h.len() - 1 - back]).sum::<f64>() / series.len() as f64
        })
        .collect()
}

/// The heat ribbon's ramp: the dusk stops clamped at the cold end, because the ramp's
/// own darkest stops are 1.38:1 and 1.85:1 against the ground -- under the 3:1
/// non-text floor, so a table at low heat drew an invisible bar (Jev 0.92).
const RIBBON_RAMP: [&str; 4] = ["#9d535c", "#c26356", "#d06a4a", "#ffb454"];

/// One row: the five tables' judged heat, side by side and therefore comparable.
/// (Jev 0.95 -- the only option of five that passed every requirement.)
fn heat_ribbon(room: &Room, width: usize, flat: bool) -> Line<'static> {
    let cells = room.tables.len().max(1);
    let bar_w = width
        .saturating_sub(8 + cells * 3)
        .checked_div(cells)
        .unwrap_or(2)
        .clamp(2, 8);
    let mut spans: Vec<Span> = vec![Span::styled(" heat ".to_string(), style_fg(LABEL))];
    for t in &room.tables {
        let v = t.gauges.get("heat").copied().unwrap_or(0.5).clamp(0.0, 1.0);
        spans.push(Span::styled(
            format!("T{}", t.id),
            Style::default().fg(pal(LABEL)).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        let n = ((v * bar_w as f64).round() as usize).min(bar_w);
        if flat {
            spans.push(Span::styled("█".repeat(n), style_fg(FAINT)));
            spans.push(Span::styled("·".repeat(bar_w - n), style_fg(FAINT)));
        } else {
            // Walk the clamped ramp: t = 0 is the least-hot table and still visible.
            let stops = RIBBON_RAMP;
            let t = (0.10 + 0.90 * v).clamp(0.0, 1.0);
            let f = t * (stops.len() - 1) as f64;
            let i = (f as usize).min(stops.len() - 2);
            let k = f - i as f64;
            let (ar, ag, ab) = hex(stops[i]);
            let (br, bg_, bb) = hex(stops[i + 1]);
            let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * k).round() as u8;
            let col = Color::Rgb(mix(ar, br), mix(ag, bg_), mix(ab, bb));
            spans.push(Span::styled("█".repeat(n), Style::default().fg(col)));
            spans.push(Span::styled("·".repeat(bar_w - n), style_fg(FAINT)));
        }
        spans.push(Span::raw(" "));
    }
    Line::from(truncate_spans(spans, width))
}

// ----------------------------------------------------------------- overview: rows

/// Identity never rests on colour alone: the sigil carries the agent's **voice
/// family** hue (six stable, learnable meanings -- Jev 0.81) and the name is neutral,
/// so the twenty agents are told apart by sigil shape and name.
fn sigil_style(p: &Persona, flat: bool) -> Style {
    if flat {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        style_fg(family_colour(p.family)).add_modifier(Modifier::BOLD)
    }
}

fn name_style(flat: bool) -> Style {
    if flat {
        Style::default()
    } else {
        style_fg(BODY)
    }
}

fn members_line(table: &Table, flat: bool) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = Vec::new();
    for (i, m) in table.members.iter().enumerate() {
        let p = by_id(m).expect("a seated llama");
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(p.sigil, sigil_style(p, flat)));
        spans.push(Span::styled(chars(p.name, 4), name_style(flat)));
    }
    spans
}

/// The writing seat, now a *static* glyph: the frame's one animated element is the
/// wire's pulse, and a second moving thing would compete with it (Jev 1.00).
fn flare(sigil: &str, colour: &str, flat: bool) -> Vec<Span<'static>> {
    let mut out: Vec<Span> = Vec::new();
    if !sigil.is_empty() {
        let p = crate::personas::LLAMAS.iter().find(|p| p.sigil == sigil);
        let st = match p {
            Some(p) => sigil_style(p, flat),
            None => {
                if flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    style_fg(colour).add_modifier(Modifier::BOLD)
                }
            }
        };
        out.push(Span::styled(sigil.to_string(), st));
    }
    out.push(Span::styled("✦".to_string(), style_fg(GOLD)));
    out
}

/// One message, inline: the speaker's head and the first words share a row, and the
/// continuation lines hang at the text column. Two rows per message instead of three,
/// and every line of prose gets one clean left edge. (Jev 0.77.) `detail` picks the
/// longer heard marker, which the detail view's test pins.
fn speaker_inline(m: &crate::room::Message, avail: usize, detail: bool) -> Vec<Line<'static>> {
    let p = m.persona();
    let mut head: Vec<Span> = Vec::new();
    // A question the room left behind: judged low on `answers_open` AND actually
    // asking one. Marked before the speaker so it reads as a property of the line.
    if m.left_open() {
        head.push(Span::styled(
            "? ".to_string(),
            Style::default().fg(pal(OPEN)).add_modifier(Modifier::BOLD),
        ));
    }
    if m.heard() {
        let label = if detail {
            format!("⟪heard from table {}⟫ ", m.heard_from.unwrap_or(0))
        } else {
            "⟪heard⟫ ".to_string()
        };
        head.push(Span::styled(label, style_fg(HEARD)));
    }
    head.push(Span::styled(p.sigil.to_string(), sigil_style(p, false)));
    head.push(Span::raw(" "));
    head.push(Span::styled(
        p.name.to_string(),
        Style::default().fg(pal(BODY)).add_modifier(Modifier::BOLD),
    ));
    // Two-weight head (Jev 1.00): WHO in bold body white, WHAT dimmed after a middle
    // dot, so the eye finds the speaker first and the markers second (Jev 0.49).
    head.push(Span::styled(" · ".to_string(), style_fg(DIM)));
    if !m.move_.is_empty() {
        head.push(Span::styled(
            move_glyph(&m.move_).to_string(),
            style_fg(move_colour(&m.move_)),
        ));
    }
    // Star and figure trail the prose (Jev 0.95) instead of crowding the head: the
    // head stays identity + move, and the marks survive at the message's end.
    let mut trail: Vec<Span> = Vec::new();
    if m.composite >= 0.6 {
        trail.push(Span::styled(" ★".to_string(), style_fg(GOLD)));
    }
    if !m.figure.is_empty() {
        trail.push(Span::styled(format!(" ⛭{}", m.figure), style_fg(DIM)));
    }
    head.push(Span::styled("  — ".to_string(), style_fg(DIM)));
    let head_w = width_of(&head).max(4);
    // Book measure: never more than PROSE_MEASURE columns from the left margin, so
    // narrow frames are untouched and wide frames gain a right margin.
    let measure = PROSE_MEASURE.saturating_sub(head_w).max(8);
    let text_w = avail.saturating_sub(head_w + 1).max(8).min(measure);
    let parts = wrap_hanging(&m.text, text_w, text_w);
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        let mut spans: Vec<Span> = Vec::new();
        if i == 0 {
            spans.extend(head.clone());
        } else {
            spans.push(Span::raw(" ".repeat(head_w)));
        }
        spans.push(Span::styled(
            part.to_string(),
            Style::default().fg(pal(BODY)),
        ));
        out.push(Line::from(spans));
    }
    if !trail.is_empty() {
        let last_w: usize = out.last().map(|l| l.width()).unwrap_or(0);
        let trail_w: usize = width_of(&trail);
        let cap = avail.max(8).min(PROSE_MEASURE + head_w);
        if last_w > 0 && last_w + trail_w < cap {
            if let Some(last) = out.last_mut() {
                last.spans.extend(trail);
            }
        } else {
            let mut t = vec![Span::raw(" ".repeat(head_w))];
            t.extend(trail);
            out.push(Line::from(t));
        }
    }
    out
}

/// One table as a band. depth 0 = a single line (badge · newest line); depth >= 1 =
/// a header (badge · roster), the summary, and up to `depth` lines.
fn table_band(
    room: &Room,
    table: &Table,
    width: usize,
    depth: usize,
    cfg: &Cfg,
) -> Vec<Line<'static>> {
    let flat = cfg.flat;
    let inner = width.saturating_sub(2).max(12);
    let writing = table
        .members
        .iter()
        .find(|m| room.llamas.get(**m).map(|s| s.speaking).unwrap_or(false))
        .copied();
    let badge = |t: &Table| {
        Span::styled(
            format!("{} ", t.name()),
            Style::default()
                .fg(if flat { Color::White } else { pal(FOCUS) })
                .add_modifier(Modifier::BOLD),
        )
    };

    if depth == 0 {
        // The emergency tier: keep the newest line, drop the roster so the line fits.
        let mut h: Vec<Span> = vec![badge(table)];
        if table.leaks_in > 0 {
            h.push(Span::styled(
                format!("⇄{} ", table.leaks_in),
                style_fg(HEARD),
            ));
        }
        if let Some(writ) = writing {
            let p = by_id(writ).unwrap();
            h.extend(flare(p.sigil, p.colour, flat));
            h.push(Span::raw(" "));
        }
        let reserve = width_of(&h);
        if let Some(last) = table.messages.last() {
            let sl = speaker_inline(last, inner.saturating_sub(reserve), false);
            if let Some(first) = sl.into_iter().next() {
                h.extend(first.spans);
            }
        } else {
            h.push(Span::styled(
                "(just sitting down)".to_string(),
                style_fg(DIM),
            ));
        }
        return vec![Line::from(truncate_spans(h, width))];
    }

    // Identity card (Jev 0.79): badge + roster, then a fixed status cluster in
    // lines → heard → open order (Jev 0.99), so the eye learns positions across bands.
    let mut head: Vec<Span> = vec![badge(table)];
    head.extend(members_line(table, flat));
    if !table.messages.is_empty() {
        head.push(Span::styled(
            format!("   {} lines", table.messages.len()),
            style_fg(DIM),
        ));
    }
    if table.leaks_in > 0 {
        head.push(Span::styled("   ⇄ ".to_string(), style_fg(HEARD)));
        head.push(Span::styled(
            format!("heard {}", table.leaks_in),
            style_fg(HEARD),
        ));
    }
    if let Some(writ) = writing {
        let p = by_id(writ).unwrap();
        head.push(Span::raw("   "));
        head.extend(flare(p.sigil, p.colour, flat));
        head.push(Span::styled(
            format!(" {} is writing", p.name),
            if flat {
                Style::default().add_modifier(Modifier::ITALIC)
            } else {
                style_fg(AMBER).add_modifier(Modifier::ITALIC)
            },
        ));
    }
    // Open questions sit with lines + heard as one fixed cluster (lines → heard → open).
    let open_q = table.messages.iter().filter(|m| m.left_open()).count();
    if open_q > 0 {
        head.push(Span::styled(
            format!("   ?{open_q} open"),
            style_fg(OPEN).add_modifier(Modifier::BOLD),
        ));
    }

    let band_heat = table
        .gauges
        .get("heat")
        .copied()
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);

    // Each band carries its own micro-weather trailing the fixed cluster: three gauge
    // trajectories from the judged history the app already records.
    let mut extra: Vec<Span> = Vec::new();
    let hist_ok = ["heat", "drift", "novelty"]
        .iter()
        .all(|g| table.history.get(*g).map(|h| h.len() >= 4).unwrap_or(false));
    if hist_ok {
        extra.push(Span::raw("   "));
        for g in ["heat", "drift", "novelty"] {
            let h = &table.history[g];
            let tail = &h[h.len().saturating_sub(10)..];
            extra.push(Span::styled(spark(tail), style_fg(DIM)));
            extra.push(Span::raw(" "));
        }
    }
    // Only spend the width when there is room; the header's own facts come first.
    if width_of(&head) + width_of(&extra) + 2 <= inner {
        head.extend(extra);
    }

    let rail = if flat { CHROME } else { heat_colour(band_heat) };
    let mut headr = vec![Span::styled("▎".to_string(), style_fg(rail))];
    headr.extend(head);
    let mut lines = vec![Line::from(headr)];
    let summary = if table.summary.is_empty() {
        "the seed itself".to_string()
    } else {
        format!("“{}”", shorten(&table.summary, inner.saturating_sub(10)))
    };
    lines.push(Line::from(vec![
        Span::styled("▎  on  ".to_string(), style_fg(rail)),
        Span::styled(
            summary,
            if flat {
                Style::default().fg(pal(BODY))
            } else {
                style_fg(BODY)
            },
        ),
    ]));

    // The ambient rail: one column down the band's left edge, walked along the dusk
    // ramp by THIS table's judged heat, so each band reads as its own temperature.
    // A heard line shows the heard mark in that same column, so the two never stack.
    let rail = if flat { CHROME } else { heat_colour(band_heat) };
    let start = table.messages.len().saturating_sub(depth);
    for m in &table.messages[start..] {
        let mut sl = speaker_inline(m, inner.saturating_sub(3), false);
        let lead = if m.heard() {
            Span::styled("│".to_string(), style_fg(HEARD))
        } else {
            Span::styled("▎".to_string(), style_fg(rail))
        };
        for line in sl.iter_mut() {
            line.spans.insert(0, lead.clone());
        }
        lines.extend(sl);
    }
    if table.messages.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (just sitting down)".to_string(),
            style_fg(DIM),
        )));
    }
    lines
}

/// How many lines each table can show, and whether to put a bare gutter between them.
/// A gutter is a row of air, not a glyph: the dotted rule cost ink to say what
/// negative space already says. (Jev: spacing was the only option of four whose three
/// requirement checks all passed.)
///
/// The fit is measured on the **real** rendered blocks, not on an estimate: a message
/// that wraps to two rows makes its table taller than the count suggests, and a
/// too-clever `2 + depth` estimate silently truncates the last table.
fn overview_blocks(
    room: &Room,
    width: usize,
    depth: usize,
    gutter: bool,
    cfg: &Cfg,
) -> Vec<Line<'static>> {
    let mut blocks: Vec<Line<'static>> = Vec::new();
    for (i, t) in room.tables.iter().enumerate() {
        if i > 0 && gutter {
            blocks.push(Line::raw(""));
        }
        blocks.extend(table_band(room, t, width, depth, cfg));
    }
    blocks
}

fn overview_rows(room: &Room, width: usize, avail: usize, cfg: &Cfg) -> Vec<Line<'static>> {
    let avail = avail.max(1);
    for depth in [3usize, 2, 1, 0] {
        for gutter in [true, false] {
            let blocks = overview_blocks(room, width, depth, gutter, cfg);
            if blocks.len() <= avail {
                return blocks;
            }
        }
    }
    overview_blocks(room, width, 0, false, cfg)
}

// ------------------------------------------------------------------- the wire

/// One eavesdrop as a thread: a ● at the source column, a ▸ at the destination,
/// dashes across the gap and a ┼ wherever it crosses another table's column.
#[allow(clippy::needless_range_loop)]
fn thread(src: u8, dst: u8, lw: usize) -> String {
    if lw < 6 {
        return String::new();
    }
    let mut canvas: Vec<char> = vec![' '; lw];
    let centres: Vec<usize> = (0..5)
        .map(|i| (((i as f64) + 0.5) * lw as f64 / 5.0) as usize)
        .collect();
    let a = centres[(src - 1) as usize];
    let b = centres[(dst - 1) as usize];
    for x in &centres {
        canvas[*x] = '│';
    }
    let (lo, hi) = (a.min(b), a.max(b));
    for x in (lo + 1)..hi {
        canvas[x] = if canvas[x] == '│' { '┼' } else { '─' };
    }
    canvas[a] = '●';
    canvas[b] = '▸';
    canvas.into_iter().collect()
}

fn col_labels(lw: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for i in 0..5 {
        let c = (((i as f64) + 0.5) * lw as f64 / 5.0) as usize;
        out.push((c.saturating_sub(1), format!("T{}", i + 1)));
    }
    out
}

/// The frame's **one** animated element: a bright braille dot travelling from the
/// source column to the destination column when a line is caught crossing between
/// tables. It is computed from the frame counter alone -- never a clock -- and is
/// absent at frame 0, so a static render (and every offline test) shows the resting
/// lattice, which stands on its own. (Jev 0.78 for the pulse, 0.28 for the single
/// travelling dot, 1.00 for exactly one animated element.)
fn pulse_cell(src: u8, dst: u8, lw: usize, frame: u64) -> Option<(usize, &'static str)> {
    if frame == 0 || lw < 6 {
        return None;
    }
    let step = frame % 24;
    if step >= 12 {
        return None;
    }
    let centres: Vec<usize> = (0..5)
        .map(|i| (((i as f64) + 0.5) * lw as f64 / 5.0) as usize)
        .collect();
    let a = centres[(src - 1) as usize];
    let b = centres[(dst - 1) as usize];
    let (lo, hi) = (a.min(b), a.max(b));
    if hi <= lo + 1 {
        return None;
    }
    let t = step as f64 / 11.0;
    let span = (hi - lo) as f64;
    let pos = if a < b {
        lo as f64 + 1.0 + t * (span - 1.0)
    } else {
        hi as f64 - 1.0 - t * (span - 1.0)
    };
    Some((pos.round() as usize, "⣿"))
}

/// The moment the travelling dot reaches its destination: the destination column
/// flares for the first and last two steps of the pulse, so the crossing reads as an
/// arrival rather than a dot that simply vanished. Computed from the counter alone.
fn landing_flash(_src: u8, dst: u8, lw: usize, frame: u64) -> Option<usize> {
    if frame == 0 || lw < 6 {
        return None;
    }
    let step = frame % 24;
    if step >= 12 {
        return None;
    }
    if step > 1 && step < 10 {
        return None;
    }
    let centres: Vec<usize> = (0..5)
        .map(|i| (((i as f64) + 0.5) * lw as f64 / 5.0) as usize)
        .collect();
    let b = centres[(dst - 1) as usize];
    Some(b)
}

fn wire_band(room: &Room, width: usize, avail: usize, cfg: &Cfg) -> Vec<Line<'static>> {
    let inner = width.max(20);
    let content = avail.max(1);
    let mut lines: Vec<Line<'static>> = Vec::new();

    if room.leaks.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no one has overheard anyone yet".to_string(),
            style_fg(DIM),
        )));
        return lines;
    }

    let lw = (inner * 3 / 5).clamp(10, 56);
    // the five column labels, in the reserved hue
    let mut label_spans: Vec<Span> = vec![Span::raw("  ")];
    let mut label_cells: Vec<(usize, String)> = Vec::new();
    for (i, _) in room.tables.iter().enumerate() {
        let (x, lab) = col_labels(lw)[i].clone();
        label_cells.push((x, lab));
    }
    label_cells.sort_by_key(|(x, _)| *x);
    let mut cursor = 2usize;
    for (x, lab) in label_cells {
        let at = x + 2;
        if at > cursor {
            label_spans.push(Span::raw(" ".repeat(at - cursor)));
            cursor = at;
        }
        label_spans.push(Span::styled(lab.clone(), style_fg(LABEL)));
        cursor += lab.chars().count();
    }
    lines.push(Line::from(label_spans));

    let quote_row = content >= 3;
    let thread_rows = (content.saturating_sub(1 + if quote_row { 1 } else { 0 })).max(1);
    let start = room.leaks.len().saturating_sub(thread_rows);
    let newest_round = room.leaks.last().map(|l| l.round).unwrap_or(0);
    // Age-faded timeline (Jev 0.99, three tiers Jev 0.73): newest full reserved hue,
    // middle at label, oldest at chrome — recency reads as brightness, oldest stays
    // legible at 3.7:1, flat mode stays uncoloured so meaning never rests on colour.
    let total = room.leaks.len();
    for (pos, lk) in room.leaks[start..].iter().enumerate() {
        let age = total.saturating_sub(start + pos + 1);
        let thread_tint = if cfg.flat {
            None
        } else if age == 0 {
            Some(HEARD)
        } else if age == 1 {
            Some(LABEL)
        } else {
            Some(CHROME)
        };
        let name = by_id(&lk.speaker).map(|p| p.name).unwrap_or("");
        let mut canvas = thread(lk.src, lk.dst, lw);
        // Double-tracked newest thread (Jev 0.53): heavy horizontals make it
        // structurally distinct even with colour removed; ● ▸ tokens stay pinned.
        let is_newest = lk.round == newest_round;
        if is_newest {
            canvas = canvas.replace('─', "═").replace('┼', "╪");
        }
        // the pulse rides the newest thread only, so the eye knows which line moved
        if lk.round == newest_round {
            if let Some((i, ch)) = pulse_cell(lk.src, lk.dst, lw, cfg.frame) {
                let mut cs: Vec<char> = canvas.chars().collect();
                if i < cs.len() {
                    cs[i] = ch.chars().next().unwrap_or(' ');
                    canvas = cs.into_iter().collect();
                }
            }
            if let Some(d) = landing_flash(lk.src, lk.dst, lw, cfg.frame) {
                let mut cs: Vec<char> = canvas.chars().collect();
                if d < cs.len() {
                    cs[d] = '◉';
                }
                canvas = cs.into_iter().collect();
            }
        }
        let fixed = lw
            + 2
            + format!("T{}", lk.src).len()
            + 3
            + format!("T{}", lk.dst).len()
            + format!("  round {}  ", lk.round).len()
            + name.len();
        let room_left = inner.saturating_sub(fixed + 2);
        let mut spans: Vec<Span> = vec![
            Span::raw("  "),
            Span::styled(
                canvas,
                match thread_tint {
                    None => Style::default(),
                    Some(c) => {
                        if age == 0 {
                            style_fg(c).add_modifier(Modifier::BOLD)
                        } else {
                            style_fg(c)
                        }
                    }
                },
            ),
            Span::raw("  "),
            Span::styled(format!("T{}", lk.src), style_fg(LABEL)),
            Span::styled(
                " ⟶ ".to_string(),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    style_fg(HEARD).add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(format!("T{}", lk.dst), style_fg(LABEL)),
            Span::styled(format!("  round {}  ", lk.round), style_fg(DIM)),
            Span::styled(name.to_string(), style_fg(LABEL)),
        ];
        if room_left > 8 {
            spans.push(Span::styled(
                format!("  “{}”", shorten(&lk.line, room_left - 3)),
                Style::default().fg(pal(BODY)),
            ));
        }
        lines.push(Line::from(spans));
    }

    if quote_row {
        let lk = room.leaks.last().unwrap();
        let name = by_id(&lk.speaker).map(|p| p.name).unwrap_or("");
        let head = format!(
            "the newest line to cross: {} at table {} → table {} — ",
            name, lk.src, lk.dst
        );
        let mut spans = vec![
            Span::styled("   ↳ ".to_string(), style_fg(DIM)),
            Span::styled("the newest line to cross: ".to_string(), style_fg(DIM)),
            Span::styled(
                format!("{} at table {} → table {} — ", name, lk.src, lk.dst),
                if cfg.flat {
                    Style::default()
                } else {
                    style_fg(HEARD)
                },
            ),
        ];
        spans.push(Span::styled(
            format!(
                "“{}”",
                shorten(&lk.line, inner.saturating_sub(head.chars().count() + 6))
            ),
            Style::default().fg(pal(BODY)),
        ));
        lines.push(Line::from(spans));
    }

    lines.into_iter().take(content).collect()
}

// ----------------------------------------------------------------------- detail

fn gauge_cols(width: usize) -> usize {
    // Four inline cells need ~40 cols each once both poles stay named; below that two
    // columns keep every pole visible without wrapping.
    if width >= 170 {
        4
    } else if width >= 54 {
        2
    } else {
        1
    }
}

fn gauge_colour(g: &str, v: f64) -> &'static str {
    if g == "drift" {
        if v > 0.6 {
            BAD
        } else if v > 0.35 {
            WARN
        } else {
            GOOD
        }
    } else {
        GOOD
    }
}

fn gauge_lines(table: &Table, width: usize, cols: usize, cfg: &Cfg) -> Vec<Line<'static>> {
    // Instrument strip (Jev 0.96, name→value→track Jev 0.59): each gauge is one cell
    // reading `heat 0.72 quiet ├─•─┤ lively`, so value and position read together and
    // the four cells align in columns. Both poles stay named at full width.
    let cell_w = (width / cols).max(28);
    let track_w = cell_w.saturating_sub(28).max(5);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let rows: Vec<usize> = (0..GAUGES.len()).step_by(cols).collect();
    for start in rows {
        let group: Vec<&str> = GAUGES[start..(start + cols).min(GAUGES.len())].to_vec();
        let mut row: Vec<Span> = Vec::new();
        for g in group {
            let v = table.gauges.get(g).copied().unwrap_or(0.5);
            let (lo, hi) = ends(g);
            let col = if cfg.flat {
                Style::default()
            } else {
                style_fg(gauge_colour(g, v))
            };
            row.push(Span::styled(format!("{g} "), style_fg(LABEL)));
            row.push(Span::styled(
                format!("{v:.2} "),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    col.add_modifier(Modifier::BOLD)
                },
            ));
            row.push(Span::styled(format!("{lo} "), style_fg(DIM)));
            row.push(Span::styled(
                two_ended(v, track_w),
                if cfg.flat { Style::default() } else { col },
            ));
            row.push(Span::styled(format!(" {hi}  "), style_fg(DIM)));
        }
        lines.push(Line::from(row));
    }
    lines
}

/// One legend, not two: the bars carry the values, the names carry the meaning, and
/// the composite is the number that matters.
fn quality_line(table: &Table, cfg: &Cfg) -> Line<'static> {
    let j = &table.judgement;
    let mut spans: Vec<Span> = vec![Span::styled(" quality ".to_string(), style_fg(DIM))];
    for d in QUALITY_DIMS {
        let v = j.quality.get(d).copied().unwrap_or(0.0);
        spans.push(Span::styled(
            bar(v, 6),
            if cfg.flat {
                Style::default()
            } else {
                style_fg(quality_colour(d))
            },
        ));
    }
    spans.push(Span::styled(
        format!("  {:.2}", j.composite),
        if cfg.flat {
            Style::default().fg(pal(BODY)).add_modifier(Modifier::BOLD)
        } else {
            style_fg(GOLD).add_modifier(Modifier::BOLD)
        },
    ));
    for d in QUALITY_DIMS {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(d.to_string(), style_fg(LABEL)));
    }
    Line::from(spans)
}

fn gauge_compact(table: &Table, width: usize, cfg: &Cfg) -> Vec<Line<'static>> {
    let per = if width >= 64 { 2 } else { 1 };
    let mut rows: Vec<Line<'static>> = Vec::new();
    let cells: Vec<(&str, f64, &str, &str)> = GAUGES
        .iter()
        .map(|g| {
            let (lo, hi) = ends(g);
            (*g, table.gauges.get(*g).copied().unwrap_or(0.5), lo, hi)
        })
        .collect();
    for chunk in cells.chunks(per) {
        let mut spans: Vec<Span> = Vec::new();
        for (name, v, lo, hi) in chunk {
            let col = if cfg.flat {
                Style::default()
            } else {
                style_fg(gauge_colour(name, *v))
            };
            spans.push(Span::styled(format!("{name} "), style_fg(DIM)));
            spans.push(Span::styled(
                format!("{v:.2}"),
                if cfg.flat {
                    Style::default().fg(pal(BODY)).add_modifier(Modifier::BOLD)
                } else {
                    col.add_modifier(Modifier::BOLD)
                },
            ));
            spans.push(Span::styled(format!("  {lo} → {hi}   "), style_fg(LABEL)));
        }
        rows.push(Line::from(spans));
    }
    rows
}

/// The judge band's content. Full form when there is room, then compact gauges, then
/// just the speaking row. A gauge block is never split across the clip.
fn judge_lines(
    room: &Room,
    table: &Table,
    width: usize,
    avail: Option<usize>,
    compact: bool,
    cfg: &Cfg,
) -> Vec<Line<'static>> {
    let j = &table.judgement;
    let inner = width.saturating_sub(2).max(12);
    let cols = gauge_cols(inner);

    let mut speak: Vec<Span> = vec![Span::styled(" next to speak  ".to_string(), style_fg(DIM))];
    let who = if table.members.contains(&j.chosen.as_str()) {
        by_id(&j.chosen)
    } else {
        None
    };
    match who {
        Some(w) if room.status != "stopped" && room.status != "error" => {
            speak.push(Span::styled(w.sigil.to_string(), sigil_style(w, cfg.flat)));
            speak.push(Span::raw(" "));
            speak.push(Span::styled(w.name.to_string(), name_style(cfg.flat)));
            if j.pick_confidence != 0.0 {
                let how = if j.reason.contains("explore") {
                    "explore"
                } else {
                    "exploit"
                };
                speak.push(Span::styled(
                    format!("  {:.2} → {how}", j.pick_confidence),
                    style_fg(if how == "explore" { WARN } else { GOOD }),
                ));
            }
        }
        _ => speak.push(Span::styled(
            "—  the tables are closed".to_string(),
            style_fg(DIM),
        )),
    }
    if !j.move_.is_empty() && width_of(&speak) + 24 < inner {
        speak.push(Span::styled("      last line  ".to_string(), style_fg(DIM)));
        speak.push(Span::styled(
            format!("{} {}", move_glyph(&j.move_), j.move_),
            style_fg(move_colour(&j.move_)),
        ));
        if !j.stance.is_empty() && width_of(&speak) + j.stance.chars().count() + 6 < inner {
            speak.push(Span::styled(format!(", a {}", j.stance), style_fg(DIM)));
        }
        if j.cohesion != 0.0 && width_of(&speak) + 14 < inner {
            speak.push(Span::styled(
                format!("  ·  flows {:.2}", j.cohesion),
                style_fg(DIM),
            ));
        }
    }
    let speak_line = Line::from(truncate_spans(speak, inner));

    let q = quality_line(table, cfg);
    let verdict = if j.verdict.is_empty() {
        None
    } else {
        Some(Line::from(vec![
            Span::styled(" Jev's verdict  ".to_string(), style_fg(DIM)),
            Span::styled(
                format!("“{}”", shorten(&j.verdict, inner.saturating_sub(18))),
                if cfg.flat {
                    Style::default().fg(pal(BODY)).add_modifier(Modifier::BOLD)
                } else {
                    style_fg(GOLD).add_modifier(Modifier::BOLD)
                },
            ),
        ]))
    };

    let glines = gauge_lines(table, inner, cols, cfg);
    // Verdict first at full width (Jev 0.74): the model's own sentence headlines its
    // own band. Compact mode keeps speaker-first so the gauges survive small heights.
    let mut full: Vec<Line> = Vec::new();
    if let Some(v) = verdict.clone() {
        full.push(v);
    }
    full.push(speak_line.clone());
    full.extend(glines);
    full.push(q.clone());
    let cg = gauge_compact(table, inner, cfg);
    let mut compact_lines: Vec<Line> = vec![speak_line.clone()];
    compact_lines.extend(cg.clone());
    if cg.len() <= 2 {
        compact_lines.push(q);
    }

    if compact {
        return compact_lines;
    }
    let Some(avail) = avail else { return full };
    let content = avail.max(1);
    if full.len() <= content {
        return full;
    }
    if compact_lines.len() <= content {
        return compact_lines;
    }
    let mut cand = vec![speak_line];
    cand.extend(cg);
    cand.truncate(content);
    cand
}

/// The transcript: the speaker rides the first row of their own line, and an
/// overheard line carries a rail in the gutter rather than a colour wash.
fn detail_blocks(
    room: &Room,
    table: &Table,
    width: usize,
    avail: usize,
    flat: bool,
) -> Vec<Vec<Line<'static>>> {
    let _ = room;
    let _ = avail;
    let mut blocks: Vec<Vec<Line<'static>>> = Vec::new();
    for (i, m) in table.messages.iter().enumerate() {
        // Tight rail (Jev 1.00): reply context becomes an inline suffix when it fits
        // (Jev 0.52), so it costs less than a full row; only overlong context keeps
        // its own dim row. The gutter stays one continuous edge either way.
        let mut reply_suffix: Option<String> = None;
        let mut reply_row: Option<Line<'static>> = None;
        if let Some(rt) = m.reply_to {
            if rt < table.messages.len() && rt != i {
                let src = &table.messages[rt];
                let tag = if src.heard() {
                    format!("table {}", src.heard_from.unwrap_or(0))
                } else {
                    src.persona().name.to_string()
                };
                let short = shorten(&src.text, (width / 4).max(10));
                let suffix = format!("  ▸ answering {tag}: “{short}”");
                // Fit check happens after the head is built; keep both forms ready.
                reply_suffix = Some(suffix.clone());
                reply_row = Some(Line::from(Span::styled(
                    format!("   {suffix}"),
                    style_fg(DIM),
                )));
            }
        }
        let mut sl = speaker_inline(m, width.saturating_sub(4).max(18), true);
        if let (Some(suffix), Some(row)) = (reply_suffix, reply_row) {
            let first_w: usize = sl.first().map(|l| l.width()).unwrap_or(0);
            if first_w + suffix.chars().count() < width.saturating_sub(4).max(18) {
                if let Some(first) = sl.first_mut() {
                    first.spans.push(Span::styled(suffix, style_fg(DIM)));
                }
            } else {
                let mut lines: Vec<Line<'static>> = vec![row];
                let lead = if m.heard() {
                    Span::styled("│".to_string(), style_fg(HEARD))
                } else {
                    Span::raw(" ")
                };
                for line in sl.iter_mut() {
                    line.spans.insert(0, lead.clone());
                }
                lines.extend(sl);
                blocks.push(lines);
                continue;
            }
        }
        let mut lines: Vec<Line<'static>> = Vec::new();
        // One continuous rail (Jev 1.00): every row carries the table's heat rail and
        // heard rows overlay the heard mark in the same column — a single edge.
        let heat_rail = {
            let v = table.gauges.get("heat").copied().unwrap_or(0.5);
            if flat {
                CHROME
            } else {
                heat_colour(v)
            }
        };
        let lead = if m.heard() {
            Span::styled("│".to_string(), style_fg(HEARD))
        } else {
            Span::styled("▎".to_string(), style_fg(heat_rail))
        };
        for line in sl.iter_mut() {
            line.spans.insert(0, lead.clone());
        }
        lines.extend(sl);
        blocks.push(lines);
    }
    blocks
}

fn detail_chat(
    room: &Room,
    table: &Table,
    width: usize,
    avail: usize,
    cfg: &Cfg,
) -> Vec<Line<'static>> {
    let _ = cfg;
    let blocks = detail_blocks(room, table, width, avail, cfg.flat);
    let writ = table
        .members
        .iter()
        .any(|m| room.llamas.get(*m).map(|s| s.speaking).unwrap_or(false));
    let limit = avail.saturating_sub(if writ { 1 } else { 0 }).max(1);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut used = 0usize;
    for idx in (0..blocks.len()).rev() {
        if !out.is_empty() && used + blocks[idx].len() > limit {
            break;
        }
        let mut merged = blocks[idx].clone();
        merged.extend(out);
        out = merged;
        used += blocks[idx].len();
    }
    if out.len() > limit && !table.messages.is_empty() {
        let m = &table.messages[table.messages.len() - 1];
        let mut sl = speaker_inline(m, width.saturating_sub(4).max(18), true);
        // One continuous rail (Jev 1.00): every row carries the table's heat rail and
        // heard rows overlay the heard mark in the same column — a single edge.
        let heat_rail = {
            let v = table.gauges.get("heat").copied().unwrap_or(0.5);
            if cfg.flat {
                CHROME
            } else {
                heat_colour(v)
            }
        };
        let lead = if m.heard() {
            Span::styled("│".to_string(), style_fg(HEARD))
        } else if cfg.flat {
            Span::styled("▎".to_string(), style_fg(CHROME))
        } else {
            Span::styled("▎".to_string(), style_fg(heat_rail))
        };
        for line in sl.iter_mut() {
            line.spans.insert(0, lead.clone());
        }
        out = sl;
    }
    // Bottom-anchor inside the real band: pad above the oldest visible line so the
    // newest line sits at the band's last row, which is where the reader's eye lands.
    if out.len() < limit {
        let pad = vec![Line::raw(""); limit - out.len()];
        out.splice(0..0, pad);
    }
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.extend(out);
    if writ {
        if let Some(w) = table
            .members
            .iter()
            .find(|m| room.llamas.get(**m).map(|s| s.speaking).unwrap_or(false))
        {
            let p = by_id(w).unwrap();
            let mut spans: Vec<Span> = vec![Span::raw("   ")];
            spans.extend(flare(p.sigil, p.colour, cfg.flat));
            spans.push(Span::styled(
                format!(" {} is writing…", p.name),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::ITALIC)
                } else {
                    style_fg(AMBER).add_modifier(Modifier::ITALIC)
                },
            ));
            lines.push(Line::from(spans));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (nothing said yet)".to_string(),
            style_fg(DIM),
        )));
    }
    lines
}

/// How many older lines the transcript will drop, so the count can ride the rule.
fn dropped_count(room: &Room, table: &Table, width: usize, avail: usize) -> usize {
    // Colour is irrelevant to row counts, so the flat flag is arbitrary here.
    let blocks = detail_blocks(room, table, width, avail, true);
    let writ = table
        .members
        .iter()
        .any(|m| room.llamas.get(*m).map(|s| s.speaking).unwrap_or(false));
    let limit = avail.saturating_sub(if writ { 1 } else { 0 }).max(1);
    let mut used = 0usize;
    for (seen, idx) in (0..blocks.len()).rev().enumerate() {
        if seen > 0 && used + blocks[idx].len() > limit {
            return idx + 1;
        }
        used += blocks[idx].len();
    }
    0
}

fn herd_cell(room: &Room, table: &Table, s: &crate::room::LlamaState, cfg: &Cfg) -> Line<'static> {
    let p = s.persona();
    let chosen = s.id == table.judgement.chosen;
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(
        if chosen { "▶" } else { " " }.to_string(),
        if cfg.flat {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            style_fg(GOLD).add_modifier(Modifier::BOLD)
        },
    ));
    spans.push(Span::styled(p.sigil.to_string(), sigil_style(p, cfg.flat)));
    spans.push(Span::styled(
        format!(" {:<7}", chars(p.name, 7)),
        if chosen {
            Style::default()
                .fg(if cfg.flat { Color::White } else { pal(BODY) })
                .add_modifier(Modifier::BOLD)
        } else {
            name_style(cfg.flat)
        },
    ));
    if s.speaking {
        spans.extend(flare("", p.colour, cfg.flat));
    }
    spans.push(Span::raw(" "));
    // The eagerness bar is neutral: the family hue lives on the sigil alone.
    spans.push(Span::styled(
        bar(s.eagerness, 5),
        if cfg.flat {
            Style::default()
        } else {
            style_fg(FAINT)
        },
    ));
    spans.push(Span::styled(format!(" {}×", s.spoken), style_fg(DIM)));
    let _ = room;
    Line::from(spans)
}

fn detail_roster(room: &Room, table: &Table, cfg: &Cfg) -> Vec<Line<'static>> {
    // Seats grid (Jev 0.95): four chairs around the table — two seats per row joined
    // by a divider, with a rule between the rows, instead of a flowing list.
    let order = table.roster(room);
    let mut cells: Vec<Vec<Span>> = order
        .iter()
        .map(|a| herd_cell(room, table, a, cfg).spans)
        .collect();
    while cells.len() < 4 {
        cells.push(vec![Span::raw("")]);
    }
    let maxw = cells.iter().map(|c| width_of(c)).max().unwrap_or(0).max(8);
    let pad = |spans: Vec<Span<'static>>| -> Vec<Span<'static>> {
        let mut v = spans;
        let w = width_of(&v);
        if w < maxw {
            v.push(Span::raw(" ".repeat(maxw - w)));
        }
        v
    };
    let row = |a: Vec<Span<'static>>, b: Vec<Span<'static>>| -> Line<'static> {
        let mut v = pad(a);
        v.push(Span::styled(" │ ".to_string(), style_fg(CHROME)));
        v.extend(pad(b));
        Line::from(v)
    };
    let total = maxw * 2 + 3;
    vec![
        row(cells[0].clone(), cells[1].clone()),
        Line::from(Span::styled("─".repeat(total), style_fg(CHROME))),
        row(cells[2].clone(), cells[3].clone()),
    ]
}

/// The beliefs half of the bottom band: three trajectories drawn from ONE shared
/// baseline so the shape of a table's movement -- spreading apart as it splits,
/// converging as it agrees -- is visible at a glance. (Jev 0.72 over three separate
/// sparklines, which share no axis and so cannot be compared.)
fn detail_beliefs(table: &Table, width: usize, cfg: &Cfg) -> Vec<Line<'static>> {
    // The current value of each proposition rides the panel's rule, in the same colour
    // its trajectory is drawn in, so the fan below needs no row labels -- a trajectory
    // moves between rows, so a row-anchored label would point at the wrong line.
    let tiers = [BELIEF, DIM, LABEL];
    let mut head: Vec<Span> = vec![Span::styled("─ ".to_string(), style_fg(LABEL))];
    head.push(Span::styled(
        "what this table believes".to_string(),
        Style::default().fg(pal(LABEL)).add_modifier(Modifier::BOLD),
    ));
    let mut used = "─ what this table believes".chars().count();
    for (i, p) in PROPOSITIONS.iter().enumerate() {
        let cur = table
            .belief_history
            .get(*p)
            .and_then(|h| h.last().copied())
            .unwrap_or(0.5);
        let seg = format!("  {} {:.2}", belief_short(p), cur);
        let len = seg.chars().count();
        if used + len + 3 < width {
            head.push(Span::styled(
                seg,
                if cfg.flat {
                    Style::default()
                } else {
                    style_fg(tiers[i % 3])
                },
            ));
            used += len + 2;
        }
    }
    let fill = width.saturating_sub(used);
    if fill > 0 {
        head.push(Span::styled("─".repeat(fill), style_fg(CHROME)));
    }
    let mut lines: Vec<Line<'static>> = vec![Line::from(head)];
    let tails: Vec<(usize, Vec<f64>)> = PROPOSITIONS
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let h = table.belief_history.get(*p).cloned().unwrap_or_default();
            if h.len() < 2 {
                None
            } else {
                Some((i, h))
            }
        })
        .collect();
    if tails.is_empty() {
        return lines;
    }
    let n = (width.saturating_sub(12))
        .min(40)
        .min(tails.iter().map(|(_, h)| h.len()).min().unwrap_or(0));
    if n < 2 {
        return lines;
    }

    // 5 rows: row 2 is the shared baseline. Each sample sits above or below it by how
    // far the proposition has moved from the midpoint.
    const ROWS: usize = 5;
    const MID: usize = 2;
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let tiers = [BELIEF, DIM, LABEL];
    let mut grid = vec![vec![(' ', 0usize); n]; ROWS];
    let mut baseline = vec![false; n];
    for (pi, h) in tails.iter() {
        let tail = &h[h.len() - n..];
        for (x, &v) in tail.iter().enumerate() {
            let v = v.clamp(0.0, 1.0);
            let off = (v - 0.5) * (ROWS as f64 - 1.0);
            let row = (MID as f64 - off).round().clamp(0.0, (ROWS - 1) as f64) as usize;
            grid[row][x] = (LEVELS[((v * 7.999) as usize).min(7)], *pi % 3);
            if row == MID {
                baseline[x] = true;
            }
        }
    }
    for (r, row) in grid.iter().enumerate() {
        let mut spans: Vec<Span> = vec![Span::raw("  ")];
        // run-length the glyphs so each trajectory keeps its own tier colour
        let mut run = String::new();
        let mut cur = 0usize;
        let flush = |run: &mut String, tier: usize, spans: &mut Vec<Span>| {
            if !run.is_empty() {
                let col = if cfg.flat { BODY } else { tiers[tier % 3] };
                spans.push(Span::styled(std::mem::take(run), style_fg(col)));
            }
        };
        for (x, (ch, tier)) in row.iter().enumerate() {
            if *ch == ' ' {
                if baseline[x] && r == MID {
                    run.push('─');
                } else {
                    flush(&mut run, cur, &mut spans);
                    run.push(' ');
                    flush(&mut run, cur, &mut spans);
                }
                continue;
            }
            if *tier != cur {
                flush(&mut run, cur, &mut spans);
                cur = *tier;
            }
            run.push(*ch);
        }
        flush(&mut run, cur, &mut spans);
        lines.push(Line::from(spans));
    }
    lines
}

/// The beliefs band's compact tier: the three trajectories as plain labelled spark
/// rows, full width, for heights where the roster + fan side-by-side no longer fit.
/// Same numbers as the fan, one row each — the middle tier between both and none.
fn detail_beliefs_compact(table: &Table, width: usize, cfg: &Cfg) -> Vec<Line<'static>> {
    let tiers = [BELIEF, DIM, LABEL];
    let mut lines: Vec<Line<'static>> = vec![rule(
        "what this table believes",
        "",
        "",
        width.saturating_sub(2),
        false,
    )];
    for (i, p) in PROPOSITIONS.iter().enumerate() {
        let h = table.belief_history.get(*p).cloned().unwrap_or_default();
        if h.len() < 2 {
            continue;
        }
        let keep = 40usize.min(width.saturating_sub(16).max(2));
        let start = h.len().saturating_sub(keep);
        let tail = &h[start..];
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<9}", belief_short(p)),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    style_fg(tiers[i % 3]).add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(
                spark(tail),
                if cfg.flat {
                    Style::default()
                } else {
                    style_fg(tiers[i % 3])
                },
            ),
            Span::styled(format!(" {:.2}", tail[tail.len() - 1]), style_fg(LABEL)),
        ]));
    }
    lines
}

// ------------------------------------------------------------------------ help

#[allow(clippy::vec_init_then_push)]
fn help_lines(cfg: &Cfg) -> Vec<Line<'static>> {
    let body = |s: &str| Span::styled(s.to_string(), Style::default().fg(pal(BODY)));
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(body(
        "Five tables of four llamas are talking at once, all about the same seed.",
    )));
    lines.push(Line::from(body(
        "Sometimes one table overhears another; a carried line is marked ⟪heard⟫",
    )));
    lines.push(Line::from(body(
        "and the hearing table must answer it. The wire shows which table's words reached which.",
    )));
    lines.push(Line::raw(""));
    let keys = [
        ("1-5", "open one table in detail"),
        ("o", "zoom out to the five tables"),
        ("Tab / .", "next table"),
        ("space", "pause / resume"),
        ("t", "type a new seed (all five restart on it)"),
        ("s", "save every table's chat"),
        ("f", "flat mode (no colour at all)"),
        ("h", "hide this help"),
        ("q", "quit"),
    ];
    for (k, label) in keys {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {k:<8}"),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    style_fg(AMBER).add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(label.to_string(), style_fg(LABEL)),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "how to read the judge".to_string(),
        if cfg.flat {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            style_fg(GOLD).add_modifier(Modifier::BOLD)
        },
    )));
    let mut gspans: Vec<Span> = vec![Span::styled("  gauges      ".to_string(), style_fg(DIM))];
    for g in GAUGES {
        let (lo, hi) = ends(g);
        gspans.push(Span::styled(format!("{g} "), style_fg(LABEL)));
        gspans.push(Span::styled(
            format!("({lo} → {hi})  "),
            Style::default().fg(pal(BODY)),
        ));
    }
    lines.push(Line::from(gspans));
    let mut qspans: Vec<Span> = vec![Span::styled("  quality     ".to_string(), style_fg(DIM))];
    qspans.push(Span::styled(
        "the bar is three segments — ".to_string(),
        Style::default().fg(pal(BODY)),
    ));
    for d in QUALITY_DIMS {
        qspans.push(Span::styled(format!("{d} "), style_fg(LABEL)));
    }
    qspans.push(Span::styled(
        "— and the number after it is the composite.".to_string(),
        Style::default().fg(pal(BODY)),
    ));
    lines.push(Line::from(qspans));
    let moves = [
        "claim",
        "evidence",
        "question",
        "rebuttal",
        "analogy",
        "concession",
        "tangent",
    ]
    .iter()
    .map(|m| format!("{} {}", move_glyph(m), m))
    .collect::<Vec<_>>()
    .join(" ");
    lines.push(Line::from(vec![
        Span::styled("  moves       ".to_string(), style_fg(DIM)),
        Span::styled(moves, Style::default().fg(pal(BODY))),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  speaker     ".to_string(), style_fg(DIM)),
        Span::styled(
            "sigil + NAME in bold, then · markers: ? open question, ⟪heard⟫ overheard.".to_string(),
            Style::default().fg(pal(BODY)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  reply       ".to_string(), style_fg(DIM)),
        Span::styled(
            "▸ answering NAME rides the head row when it fits, else its own dim row.".to_string(),
            Style::default().fg(pal(BODY)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  wire        ".to_string(), style_fg(DIM)),
        Span::styled(
            "newest thread brightest, older dimmer; the dot travels the newest.".to_string(),
            Style::default().fg(pal(BODY)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  colour      ".to_string(), style_fg(DIM)),
        Span::styled(
            "a sigil's hue names its voice family; names stay neutral.".to_string(),
            Style::default().fg(pal(BODY)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  beliefs     ".to_string(), style_fg(DIM)),
        Span::styled(
            "shorter names for the three propositions this table is tracked against:".to_string(),
            Style::default().fg(pal(BODY)),
        ),
    ]));
    for p in PROPOSITIONS {
        lines.push(Line::from(vec![
            Span::styled(
                format!("                 {:<9}", belief_short(p)),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    style_fg(BELIEF).add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(p.to_string(), style_fg(LABEL)),
        ]));
    }
    lines
}

// ------------------------------------------------------------------------ layout

/// (judge rows, wire rows, bottom-band rows, transcript rows)
fn budgets(avail: usize, judge_h: usize) -> (usize, usize, usize, usize) {
    let avail = avail.max(6);
    let mut wire = if avail >= 33 {
        7
    } else if avail >= 23 {
        5
    } else {
        3
    };
    // Tiered bottom band (Jev 0.84 with a monotone guard): full roster + fan when it
    // fits, compact full-width beliefs in the middle, nothing when tiny. Unique
    // information (the fan) outlives duplicated information (the roster).
    // band rows encode the tier: 6 = side-by-side, 4 = compact beliefs, 0 = none.
    let mut band = if avail >= 24 {
        6
    } else if avail >= 17 {
        4
    } else {
        0
    };
    let judge = judge_h.min((avail.saturating_sub(wire + band + 4)).max(4));
    let mut chat = avail.saturating_sub(judge + wire + band);
    // Steal across tiers, never into them: 6 → 4 → 0, so the renderer always sees a
    // tier it knows how to draw.
    while chat < 4 && band == 6 {
        band = 4;
        chat += 2;
    }
    while chat < 4 && band == 4 {
        band = 0;
        chat += 4;
    }
    while chat < 4 && wire > 3 {
        wire -= 1;
        chat += 1;
    }
    (judge, wire, band, chat.max(3))
}

/// Two passes: prefer the full judge band, then drop to the compact form if the
/// transcript would fall below its floor. Returns (judge rows, compact flag).
fn judge_alloc(avail: usize, room: &Room, table: &Table, width: usize, cfg: &Cfg) -> (usize, bool) {
    let full_h = judge_lines(room, table, width, None, false, cfg).len() + 1;
    let (j, _, _, chat) = budgets(avail, full_h);
    if chat >= 4 {
        return (j, false);
    }
    let comp_h = judge_lines(room, table, width, None, true, cfg).len() + 1;
    let (j2, _, _, _) = budgets(avail, comp_h);
    (j2, true)
}

/// Paint the whole frame. Pure in (room, cfg, size): no network, no terminal.
pub fn paint(frame: &mut Frame, room: &Room, cfg: &Cfg) {
    let area = frame.area();
    let width = area.width as usize;
    let focus = if (0..room.tables.len() as i32).contains(&room.focus) {
        room.focus as usize
    } else {
        usize::MAX
    };

    let [head, tabs, body, foot] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    frame.render_widget(masthead(room, width, cfg.flat), head);
    frame.render_widget(tab_bar(room, cfg.flat), tabs);
    frame.render_widget(footer(room, cfg.flat), foot);

    if cfg.hint {
        frame.render_widget(
            Paragraph::new(Text::from(
                help_lines(cfg)
                    .into_iter()
                    .take(body.height as usize)
                    .collect::<Vec<_>>(),
            ))
            .block(body_block()),
            body,
        );
        return;
    }

    if focus == usize::MAX {
        // overview: the heat ribbon, five table bands, then the wire. The wire is sized
        // to its own content (a rule, the column labels, up to three threads, a quote)
        // so it never leaves slack rows at the bottom of the frame.
        let threads = room.leaks.len().min(3);
        let want = if room.leaks.is_empty() {
            3
        } else {
            1 + 1 + threads + 1
        };
        let wire_h = (want as u16).clamp(3, 9);
        let [ribbon, rows, wire] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(wire_h),
        ])
        .areas(body);
        let rows_h = body.height.saturating_sub(1 + wire_h) as usize;
        let inner_w = width.saturating_sub(2);

        frame.render_widget(
            Paragraph::new(Text::from(heat_ribbon(room, inner_w, cfg.flat))),
            ribbon,
        );
        frame.render_widget(
            Paragraph::new(Text::from(overview_rows(room, inner_w, rows_h, cfg)))
                .block(body_block()),
            rows,
        );
        let mut w: Vec<Line> = vec![rule("the wire", "", "", inner_w, false)];
        w.extend(wire_band(
            room,
            inner_w,
            (wire_h as usize).saturating_sub(1),
            cfg,
        ));
        frame.render_widget(Paragraph::new(Text::from(w)).block(body_block()), wire);
        return;
    }

    // detail: the judge, the transcript, a bottom band, then the wire.
    let table = &room.tables[focus];
    let avail = body.height as usize;
    let (judge_h, compact) = judge_alloc(avail, room, table, width, cfg);
    let (j, wire, band, _chat) = budgets(avail, judge_h);
    let inner_w = width.saturating_sub(2);

    if cfg.hint {
        let [_, chat_area] =
            Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(body);
        frame.render_widget(
            Paragraph::new(Text::from(
                help_lines(cfg)
                    .into_iter()
                    .take(chat_area.height as usize)
                    .collect::<Vec<_>>(),
            ))
            .block(body_block()),
            chat_area,
        );
        return;
    }

    // The transcript takes every row the judge, the bottom band and the wire leave, so
    // no blank rows appear anywhere: it fills its band, bottom-anchored, and the wire
    // gets exactly what it asks for.
    let mut constraints = vec![Constraint::Length(j as u16), Constraint::Min(0)];
    if band > 0 {
        constraints.push(Constraint::Length(band as u16));
    }
    constraints.push(Constraint::Length(wire as u16));
    let areas = Layout::vertical(constraints).split(body);

    // the judge band, focus raised by lightness (this is the focused table)
    let mut jl: Vec<Line> = vec![rule("the judge", "· jev-1.13", "", inner_w, true)];
    jl.extend(judge_lines(room, table, width, Some(j - 1), compact, cfg));
    frame.render_widget(Paragraph::new(Text::from(jl)).block(body_block()), areas[0]);

    // the transcript. Its "older lines above" fact rides the rule, saving a row.
    let chat_rows = areas[1].height as usize;
    let older = dropped_count(room, table, width, chat_rows.saturating_sub(1));
    let right = if older > 0 {
        format!("▲ {older} older lines above")
    } else {
        String::new()
    };
    let mut cl: Vec<Line> = vec![rule(
        &table.name(),
        "· newest at the bottom",
        &right,
        inner_w,
        false,
    )];
    cl.extend(detail_chat(
        room,
        table,
        width,
        chat_rows.saturating_sub(1),
        cfg,
    ));
    frame.render_widget(Paragraph::new(Text::from(cl)).block(body_block()), areas[1]);

    let mut idx = 2;
    if band == 6 {
        let half = ((width * 2 / 5).clamp(30, 60)) as u16;
        let [roster, beliefs] =
            Layout::horizontal([Constraint::Length(half), Constraint::Min(0)]).areas(areas[idx]);
        let mut rl: Vec<Line> = vec![rule(
            "the roster",
            "· eagerness ▏ who has spoken",
            "",
            (half as usize).saturating_sub(2),
            false,
        )];
        rl.extend(detail_roster(room, table, cfg));
        frame.render_widget(Paragraph::new(Text::from(rl)).block(body_block()), roster);
        let bw = width.saturating_sub(half as usize + 2);
        frame.render_widget(
            Paragraph::new(Text::from(detail_beliefs(table, bw, cfg))).block(body_block()),
            beliefs,
        );
        idx += 1;
    } else if band == 4 {
        // Middle tier: beliefs alone, full width, compact rows.
        frame.render_widget(
            Paragraph::new(Text::from(detail_beliefs_compact(
                table,
                width.saturating_sub(2),
                cfg,
            )))
            .block(body_block()),
            areas[idx],
        );
        idx += 1;
    }

    let right = format!("{} eavesdrops, one-way", room.leaks.len());
    let mut w: Vec<Line> = vec![rule("the wire", "", &right, inner_w, false)];
    w.extend(wire_band(
        room,
        inner_w,
        areas[idx].height as usize - 1,
        cfg,
    ));
    frame.render_widget(
        Paragraph::new(Text::from(w)).block(body_block()),
        areas[idx],
    );
}

/// Compose a frame to plain text lines at a given size with no terminal. This is the
/// offline harness the tests and `--once` use.
pub fn compose_text(room: &Room, cfg: &Cfg, w: u16, h: u16) -> Vec<String> {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let backend = TestBackend::new(w, h);
    let mut term = Terminal::new(backend).expect("the test terminal builds");
    term.draw(|f| paint(f, room, cfg)).expect("the frame draws");
    let buf = term.backend().buffer().clone();
    let mut out = Vec::new();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every colour that carries text must clear the WCAG 4.5:1 floor on the ground
    /// (docs/research/readability-and-colour.md). This is the bug that shipped: one
    /// grey at 3.70:1 was carrying 1057 cells of label text, so `DIM` is now the text
    /// tier and `CHROME` is confined to rules and gutters.
    #[test]
    fn palette_text_colours_clear_the_contrast_floor() {
        for c in [
            BODY, LABEL, DIM, GOLD, AMBER, HEARD, BELIEF, GOOD, WARN, BAD,
        ] {
            assert!(
                contrast(c, INK) >= 4.5,
                "{c} on INK is {:.2}:1, under 4.5:1",
                contrast(c, INK)
            );
        }
        // The decoration grey must NOT be usable as text, or the two tiers are one.
        assert!(
            contrast(CHROME, INK) < 4.5,
            "CHROME is meant to be decoration only"
        );
        // The three neutral text tiers stay ordered and distinguishable.
        assert!(contrast(DIM, INK) < contrast(LABEL, INK));
        assert!(contrast(LABEL, INK) < contrast(BODY, INK));
        // the two hues that failed the floor before the redesign, and their fixes
        assert!(contrast(INDIGO, INK) < 4.5, "the old indigo fails as text");
        assert!(contrast(BELIEF, INK) >= 4.5);
        assert!(contrast("#8a86ad", INK) < 9.0 && contrast(LABEL, INK) >= 9.0);
    }

    /// Every stop the heat ribbon can draw must be visible against the ground, or the
    /// ribbon stops being comparable across tables -- which is its whole purpose. The
    /// unclamped dusk ramp's cold half (1.38:1, 1.85:1, 2.88:1) fails this.
    #[test]
    fn the_heat_ribbon_never_draws_an_invisible_bar() {
        for stop in RIBBON_RAMP {
            assert!(
                contrast(stop, INK) >= 3.0,
                "ribbon stop {stop} is {:.2}:1 on INK, under the 3:1 non-text floor",
                contrast(stop, INK)
            );
        }
        // the darkest value the ribbon actually draws (v = 0 -> t = 0.10)
        assert!(contrast(RIBBON_RAMP[0], INK) >= 3.0);
        // and the ramp it replaced really was invisible
        assert!(contrast(DUSK[0], INK) < 3.0);
        assert!(contrast(DUSK[1], INK) < 3.0);
    }

    /// The chrome greys only have to clear the non-text floor.
    #[test]
    fn chrome_colours_clear_the_non_text_floor() {
        for c in [CHROME, FOCUS] {
            assert!(
                contrast(c, INK) >= 3.0,
                "{c} on INK is {:.2}:1, under 3:1",
                contrast(c, INK)
            );
        }
    }

    /// The 256-colour downgrade must not quietly break the floors: this is the failure
    /// mode the research measured (a snapped indigo dropped under 3:1).
    #[test]
    fn the_indexed_downgrade_keeps_every_text_colour_readable() {
        for c in [BODY, LABEL, GOLD, AMBER, HEARD, BELIEF, GOOD, WARN, BAD] {
            let i = nearest_256(c);
            let (r, g, b) = index_rgb(i);
            let snapped = format!("#{r:02x}{g:02x}{b:02x}");
            assert!(
                contrast(&snapped, INK) >= 4.5,
                "{c} snaps to index {i} ({snapped}) = {:.2}:1 on INK",
                contrast(&snapped, INK)
            );
        }
    }

    /// The frame's one animated element is absent at frame 0, so a static render shows
    /// the resting lattice and stays reproducible.
    #[test]
    fn the_wire_pulse_is_absent_from_a_static_frame() {
        assert!(pulse_cell(2, 4, 40, 0).is_none());
        assert!((1..12).any(|f| pulse_cell(2, 4, 40, f).is_some()));
    }

    /// De-emphasis is by lightness: the text tier must stay quieter than the primary
    /// label but legible, and the decoration grey must be dimmer than both.
    #[test]
    fn the_neutral_tiers_are_distinguishable_but_both_legible() {
        assert!(contrast(LABEL, INK) > contrast(DIM, INK));
        assert!(contrast(DIM, INK) > contrast(CHROME, INK));
        assert!(contrast(CHROME, INK) >= 3.0 && contrast(DIM, INK) >= 4.5);
    }
}
