//! The Corral's UI: two zoom levels over five tables of four llamas.
//!
//!   overview (focus = -1):  five full-width ticker rows, one per table, each with
//!                           its roster, its one-line summary and its newest line;
//!                           below them the wire -- the eavesdrop lattice.
//!   detail   (focus = 0..4): one table, arranged as three bands: the judge band
//!                           across the top, the transcript in the middle, and the
//!                           roster and beliefs along the bottom, with the wire
//!                           keeping its own full-width strip beneath.
//!
//! Keys: 1-5 focus a table · o overview · Tab / . cycle · f flat · h help · q quit.
//!
//! A ratatui port of the original rich `tui.py`, with the same frame. The colour
//! ramps interpolate in **Oklab** (via `colorgrad`) so the dusk horizon reads
//! vivid rather than muddy, and the writing seat's flare rides an explicit frame
//! counter (never a wall clock) so a static frame stays reproducible and testable.
//!
//! Design (every fork went to TypeSafe Jev):
//!   * one metaphor per layer, never two in one glyph -- the *dusk parlour*
//!     supplies the ground and the light, the *loom* supplies only the wire, and
//!     the *observatory* supplies only the speaking flare.
//!   * identity never rests on colour alone: every llama carries a sigil + name,
//!     and every line carries a *move glyph*.
//!   * nothing starves: every band is sized from the terminal with a hard floor,
//!     and the transcript is bottom-anchored so the newest line is always visible.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};
use ratatui::Frame;

use colorgrad::{BlendMode, Gradient, GradientBuilder, LinearGradient};

use crate::personas::{by_id, family_colour};
use crate::room::{move_colour, Room, Table, GAUGES, PROPOSITIONS};

pub const BLOCKS: &str = " ▏▎▍▌▋▊▉█";
pub const SPARK: &str = "▁▂▃▄▅▆▇█";

// ------------------------------------------------------------------- the palette
// Ember dusk: a deep indigo ground lit by one warm amber. (Jev 0.80.)
pub const INK: &str = "#12102b";
pub const INK2: &str = "#1c1940";
pub const AMBER: &str = "#ffb454";
pub const EMBER: &str = "#ff7a3d";
pub const GOLD: &str = "#ffe0a0";
pub const INDIGO: &str = "#5b6ee1";
pub const GOOD: &str = "#8fd694";
pub const WARN: &str = "#ffd75f";
pub const BAD: &str = "#ff6b6b";
pub const MUTED: &str = "#8a86ad";
pub const HEARD: &str = "#c98cff";
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

pub fn quality_colour(d: &str) -> &'static str {
    match d {
        "originality" => AMBER,
        "clarity" => "#6fb7e0",
        _ => "#c98cff",
    }
}

/// The per-frame config, mirroring the original `cfg` dict.
#[derive(Debug, Clone)]
pub struct Cfg {
    pub draws: usize,
    pub max_rounds: Option<u32>,
    pub flat: bool,
    pub hint: bool,
    /// An animating counter for the flare -- never a clock.
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

// ------------------------------------------------------------------- primitives

fn hex(c: &str) -> (u8, u8, u8) {
    let c = c.trim_start_matches('#');
    (
        u8::from_str_radix(&c[0..2], 16).unwrap_or(0),
        u8::from_str_radix(&c[2..4], 16).unwrap_or(0),
        u8::from_str_radix(&c[4..6], 16).unwrap_or(0),
    )
}

/// A hex string as a truecolor.
pub fn rgb(c: &str) -> Color {
    let (r, g, b) = hex(c);
    Color::Rgb(r, g, b)
}

fn style_fg(c: &str) -> Style {
    Style::default().fg(rgb(c))
}

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
/// The Python ramp was a linear sRGB lerp; this one is perceptual (Oklab).
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

/// A sparkline, exactly as the Python `spark`.
pub fn spark(vals: &[f64]) -> String {
    vals.iter()
        .map(|v| {
            let idx = ((v.clamp(0.0, 1.0)) * 7.999) as usize;
            SPARK.chars().nth(idx.min(7)).unwrap_or('▁')
        })
        .collect()
}

/// `mm:ss`.
pub fn mmss(t: f64) -> String {
    format!("{:02}:{:02}", (t / 60.0) as u64, (t as u64) % 60)
}

/// Collapse whitespace and clip to `n` characters with an ellipsis.
pub fn shorten(text: &str, n: usize) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if n <= 1 {
        return if joined.is_empty() {
            String::new()
        } else {
            "…".to_string()
        };
    }
    let count = joined.chars().count();
    if count <= n {
        joined
    } else {
        let kept: String = joined.chars().take(n - 1).collect();
        format!("{}…", kept.trim_end())
    }
}

fn chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

fn heat_of(room: &Room) -> f64 {
    let n = room.tables.len().max(1);
    room.tables
        .iter()
        .map(|t| t.gauges.get("heat").copied().unwrap_or(0.5))
        .sum::<f64>()
        / n as f64
}

/// A panel title that can never be truncated mid-word: the bold part is kept whole
/// and the dim subtitle is dropped, then clipped, to fit the panel width.
fn titled(bold: &str, dim: &str, width: usize) -> Line<'static> {
    let mut spans = vec![Span::styled(
        bold.to_string(),
        Style::default().fg(rgb(GOLD)).add_modifier(Modifier::BOLD),
    )];
    if !dim.is_empty() {
        let room_for = width.saturating_sub(bold.chars().count() + 6);
        if room_for >= dim.chars().count() {
            spans.push(Span::styled(format!(" {dim}"), style_fg(MUTED)));
        } else if room_for >= 4 {
            spans.push(Span::styled(
                format!(" {}", shorten(dim, room_for)),
                style_fg(MUTED),
            ));
        }
    }
    Line::from(spans)
}

/// A rounded panel with a left-aligned title, matching the Python `box.ROUNDED`.
fn panel(title: Line<'static>, border: Color, flat: bool) -> Block<'static> {
    let b = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .title_alignment(Alignment::Left)
        .padding(Padding::horizontal(1));
    if flat {
        b.border_style(Style::default().fg(Color::White))
    } else {
        b.border_style(Style::default().fg(border))
    }
}

// ------------------------------------------------------------------------ header

fn horizon_span(width: usize, heat: f64, flat: bool) -> Vec<Span<'static>> {
    if flat {
        return vec![Span::styled("─".repeat(width), style_fg(MUTED))];
    }
    ramp(width, heat)
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let ch = if i % 2 == 0 { "━" } else { "▔" };
            Span::styled(ch, Style::default().fg(c))
        })
        .collect()
}

fn masthead(room: &Room, width: usize, flat: bool) -> Paragraph<'static> {
    let inner = width.saturating_sub(4).max(10);
    let st_col = match room.status.as_str() {
        "discussing" => AMBER,
        "paused" => WARN,
        "idle" => MUTED,
        "stopped" => GOOD,
        "error" => BAD,
        _ => "#ffffff",
    };
    let status = room.status.to_uppercase();

    let mut spans: Vec<Span> = Vec::new();
    let badge_style = if flat {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(rgb(AMBER))
            .add_modifier(Modifier::BOLD)
    };
    spans.push(Span::styled(" ◆ THE CORRAL ", badge_style));
    let mut used = " ◆ THE CORRAL ".chars().count();
    let sub = "five tables, one seed, and Jev between them";
    if inner.saturating_sub(used) > 52 {
        spans.push(Span::styled(format!("  {sub}  "), style_fg(MUTED)));
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
        spans.push(Span::styled("  round ".to_string(), style_fg(MUTED)));
        spans.push(Span::styled(
            format!("{}", room.turn),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("   ⏱ ".to_string(), style_fg(MUTED)));
        spans.push(Span::styled(
            mmss(room.elapsed()),
            Style::default().fg(Color::White),
        ));
        spans.push(Span::styled("   $".to_string(), style_fg(MUTED)));
        spans.push(Span::styled(
            format!("{:.4}", room.cost()),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("   ⇄ ".to_string(), style_fg(MUTED)));
        spans.push(Span::styled(
            format!("{}", room.leaks.len()),
            style_fg(HEARD).add_modifier(Modifier::BOLD),
        ));
    }

    let mut lines = vec![Line::from(spans)];

    // row 2 of content: a short seed/phase/error summary, then the dusk horizon.
    let mut r2: Vec<Span> = Vec::new();
    if !room.seed.is_empty() {
        r2.push(Span::styled("seed ".to_string(), style_fg(MUTED)));
        let seed = shorten(&room.seed, inner.saturating_sub(20).max(18));
        r2.push(Span::styled(
            format!("“{seed}”"),
            if flat {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                style_fg(EMBER).add_modifier(Modifier::BOLD)
            },
        ));
    }
    if !room.phase.is_empty() && room.seed.chars().count() + room.phase.chars().count() + 14 < inner
    {
        r2.push(Span::styled("   · ".to_string(), style_fg(MUTED)));
        r2.push(Span::styled(room.phase.clone(), style_fg(MUTED)));
    }
    if !room.last_error.is_empty() {
        r2.push(Span::styled("   · ".to_string(), style_fg(MUTED)));
        r2.push(Span::styled(
            format!(
                "last error: {}",
                shorten(&room.last_error, (inner / 3).max(12))
            ),
            style_fg(BAD),
        ));
    }
    lines.push(Line::from(r2));

    // the horizon rule rides its own content row, so the dusk gradient is always visible
    lines.push(Line::from(horizon_span(inner, heat_of(room), flat)));

    let border = if flat { Color::White } else { rgb(AMBER) };
    let block = Block::bordered()
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(border))
        .padding(Padding::horizontal(1))
        .style(if flat {
            Style::default()
        } else {
            Style::default().bg(rgb(INK))
        });
    Paragraph::new(Text::from(lines)).block(block)
}

fn tab_bar(room: &Room, flat: bool) -> Paragraph<'static> {
    let mut spans: Vec<Span> = Vec::new();
    for tb in &room.tables {
        let on = room.focus == tb.id as i32 - 1;
        let style = if on {
            if flat {
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
                    .fg(Color::Black)
                    .bg(rgb(&tb.colour))
                    .add_modifier(Modifier::BOLD)
            }
        } else if flat {
            Style::default()
        } else {
            style_fg(MUTED)
        };
        spans.push(Span::styled(format!(" {} {} ", tb.id, tb.name()), style));
        spans.push(Span::raw(" "));
    }
    let on_overview = room.focus < 0;
    let ov_style = if on_overview {
        if flat {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default()
                .fg(Color::Black)
                .bg(rgb(AMBER))
                .add_modifier(Modifier::BOLD)
        }
    } else {
        style_fg(MUTED)
    };
    spans.push(Span::styled(" [ o overview ] ", ov_style));
    Paragraph::new(Line::from(spans))
}

fn footer(room: &Room, width: usize, flat: bool) -> Paragraph<'static> {
    let keys: Vec<&str> = if room.focus < 0 {
        vec![
            "1-5 open a table",
            "o overview",
            "t seed",
            "s save",
            "h help",
            "q quit",
        ]
    } else {
        vec![
            "o overview",
            "Tab next",
            "1-5 switch",
            "space pause",
            "s save",
            "h help",
            "q quit",
        ]
    };
    let view = if room.focus < 0 {
        "the five tables".to_string()
    } else {
        format!("table {}", room.focus + 1)
    };
    let mut spans: Vec<Span> = Vec::new();
    let view_style = if flat {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(rgb(AMBER))
            .add_modifier(Modifier::BOLD)
    };
    spans.push(Span::styled(format!(" {view} "), view_style));
    let mut budget = width.saturating_sub(view.chars().count() + 4);
    for k in keys {
        let seg = format!("  ·  {k}");
        if seg.chars().count() > budget {
            break;
        }
        spans.push(Span::styled(seg.clone(), style_fg(MUTED)));
        budget = budget.saturating_sub(seg.chars().count());
    }
    if matches!(room.status.as_str(), "stopped" | "error") {
        let tail = "   ·  the tables have closed";
        if tail.chars().count() <= budget {
            spans.push(Span::styled(
                tail,
                style_fg(if room.status == "stopped" { GOOD } else { BAD }),
            ));
        }
    }
    Paragraph::new(Line::from(spans))
}

// ----------------------------------------------------------------- overview: rows

fn members_line(table: &Table, flat: bool) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = Vec::new();
    for (i, m) in table.members.iter().enumerate() {
        let p = by_id(m).expect("a seated llama");
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let st = if flat {
            Style::default()
        } else {
            style_fg(p.colour).add_modifier(Modifier::BOLD)
        };
        let st2 = if flat {
            Style::default()
        } else {
            style_fg(p.colour)
        };
        spans.push(Span::styled(p.sigil, st));
        spans.push(Span::styled(chars(p.name, 4), st2));
    }
    spans
}

/// The writing seat: a small star that pulses on the frame counter.
fn flare(sigil: &str, colour: &str, frame: u64, flat: bool) -> Vec<Span<'static>> {
    let mut out: Vec<Span> = Vec::new();
    if !sigil.is_empty() {
        let st = if flat {
            Style::default()
        } else {
            style_fg(colour).add_modifier(Modifier::BOLD)
        };
        out.push(Span::styled(sigil.to_string(), st));
    }
    if flat {
        out.push(Span::raw("✦"));
        return out;
    }
    let glyph = ["✦", "✧", "✦", "⋆"][(frame % 4) as usize];
    let shade = [AMBER, GOLD, AMBER, EMBER][(frame % 4) as usize];
    out.push(Span::styled(
        glyph,
        style_fg(shade).add_modifier(Modifier::BOLD),
    ));
    out
}

fn speaker_line(
    room: &Room,
    m: &crate::room::Message,
    width: usize,
    prefix: &str,
    reserve: usize,
) -> Line<'static> {
    let _ = room;
    let flat = false; // colour is decided by the caller through cfg; spans carry their palette
    let p = m.persona();
    let mut spans: Vec<Span> = Vec::new();
    if !prefix.is_empty() {
        spans.push(Span::raw(prefix.to_string()));
    }
    if m.heard() {
        spans.push(Span::styled("⟪heard⟫ ".to_string(), style_fg(HEARD)));
    }
    spans.push(Span::styled(
        p.sigil.to_string(),
        style_fg(p.colour).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        p.name.to_string(),
        style_fg(p.colour).add_modifier(Modifier::BOLD),
    ));
    if !m.move_.is_empty() {
        spans.push(Span::styled(
            format!(" {}", move_glyph(&m.move_)),
            style_fg(move_colour(&m.move_)),
        ));
    }
    if m.composite >= 0.6 {
        spans.push(Span::styled(" ★".to_string(), style_fg(GOLD)));
    }
    if !m.figure.is_empty() {
        spans.push(Span::styled(format!(" ⛭{}", m.figure), style_fg(MUTED)));
    }
    let head_w: usize = spans.iter().map(|s| s.width()).sum();
    spans.push(Span::raw("  "));
    let _ = flat;
    spans.push(Span::styled(
        shorten(&m.text, width.saturating_sub(reserve + head_w + 2).max(8)),
        Style::default().fg(Color::White),
    ));
    Line::from(spans)
}

/// One table as a ticker row. depth 0 = a single line (name · newest line);
/// depth >= 1 = a header (name · roster), the summary, and up to `depth` lines.
fn table_row(
    room: &Room,
    table: &Table,
    width: usize,
    depth: usize,
    cfg: &Cfg,
) -> Vec<Line<'static>> {
    let flat = cfg.flat;
    let inner = width.saturating_sub(4).max(12);
    let badge = if flat {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::Black)
            .bg(rgb(&table.colour))
            .add_modifier(Modifier::BOLD)
    };
    let mut head: Vec<Span> = vec![Span::styled(format!(" ◆ {} ", table.name()), badge)];

    if depth == 0 {
        if table.leaks_in > 0 {
            head.push(Span::styled(
                format!(" ⇄{}", table.leaks_in),
                style_fg(HEARD),
            ));
        }
        if let Some(writ) = table
            .members
            .iter()
            .find(|m| room.llamas.get(**m).map(|s| s.speaking).unwrap_or(false))
        {
            let p = by_id(writ).unwrap();
            head.push(Span::raw(" "));
            head.extend(flare(p.sigil, p.colour, cfg.frame, flat));
        }
        head.push(Span::raw("  "));
        let reserve: usize = head.iter().map(|s| s.width()).sum();
        if let Some(last) = table.messages.last() {
            let sl = speaker_line(room, last, inner, "", reserve);
            head.extend(sl.spans);
        } else {
            head.push(Span::styled(
                "(just sitting down)".to_string(),
                style_fg(MUTED),
            ));
        }
        return vec![Line::from(head)];
    }

    head.push(Span::raw("  "));
    head.extend(members_line(table, flat));
    if table.leaks_in > 0 {
        head.push(Span::styled("   ⇄ ".to_string(), style_fg(HEARD)));
        head.push(Span::styled(
            format!("heard {}", table.leaks_in),
            style_fg(HEARD),
        ));
    }
    if let Some(writ) = table
        .members
        .iter()
        .find(|m| room.llamas.get(**m).map(|s| s.speaking).unwrap_or(false))
    {
        let p = by_id(writ).unwrap();
        head.push(Span::raw("   "));
        head.extend(flare(p.sigil, p.colour, cfg.frame, flat));
        head.push(Span::styled(
            format!(" {} is writing", p.name),
            if flat {
                Style::default().add_modifier(Modifier::ITALIC)
            } else {
                style_fg(AMBER).add_modifier(Modifier::ITALIC)
            },
        ));
    } else if !table.messages.is_empty() {
        head.push(Span::styled(
            format!("   {} lines", table.messages.len()),
            style_fg(MUTED),
        ));
    }

    let mut lines = vec![Line::from(head)];
    let mut s: Vec<Span> = vec![Span::styled("   on  ".to_string(), style_fg(MUTED))];
    if table.summary.is_empty() {
        s.push(Span::styled(
            "the seed itself".to_string(),
            if flat {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                style_fg(EMBER).add_modifier(Modifier::BOLD)
            },
        ));
    } else {
        s.push(Span::styled(
            format!("“{}”", shorten(&table.summary, inner.saturating_sub(10))),
            if flat {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                style_fg(EMBER).add_modifier(Modifier::BOLD)
            },
        ));
    }
    lines.push(Line::from(s));
    let start = table.messages.len().saturating_sub(depth);
    for m in &table.messages[start..] {
        lines.push(speaker_line(room, m, inner, "   ", 0));
    }
    if table.messages.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (just sitting down)".to_string(),
            style_fg(MUTED),
        )));
    }
    lines
}

/// How many message lines each table can show, and whether to rule between them.
fn fit_depth(avail: usize) -> (usize, bool) {
    if avail < 16 {
        return (0, false);
    }
    let sep = avail >= 14;
    for depth in [4, 3, 2, 1] {
        let total = 5 * (2 + depth) + if sep && depth >= 2 { 4 } else { 0 };
        if total <= avail {
            return (depth, sep && depth >= 2);
        }
    }
    (1, false)
}

fn overview_rows(room: &Room, width: usize, avail: usize, cfg: &Cfg) -> Paragraph<'static> {
    let (depth, sep) = fit_depth(avail);
    let mut blocks: Vec<Line<'static>> = Vec::new();
    for (i, t) in room.tables.iter().enumerate() {
        if i > 0 && sep {
            blocks.push(Line::from(Span::styled(
                format!(" {}", "·".repeat(width.saturating_sub(6))),
                if cfg.flat {
                    Style::default()
                } else {
                    style_fg(&t.colour)
                },
            )));
        }
        blocks.extend(table_row(room, t, width, depth, cfg));
    }
    let cap = avail.saturating_sub(2).max(1);
    let mut out: Vec<Line> = blocks.iter().take(cap).cloned().collect();
    let dropped = blocks.len().saturating_sub(out.len());
    if dropped > 0 {
        let mut v = vec![Line::from(Span::styled(
            format!("   ▲ {dropped} rows above"),
            style_fg(MUTED),
        ))];
        let keep = avail.saturating_sub(3);
        v.extend(blocks.into_iter().take(keep));
        out = v;
    }
    let title = titled(
        "the five tables",
        "· press 1-5 to open one",
        width.saturating_sub(4),
    );
    let border = if cfg.flat {
        Color::White
    } else {
        rgb("#8a6a4a")
    };
    Paragraph::new(Text::from(out)).block(panel(title, border, cfg.flat))
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

fn wire_title(room: &Room, width: usize) -> Line<'static> {
    if room.leaks.is_empty() {
        titled("the wire", "· eavesdrops between tables", width)
    } else {
        titled(
            "the wire",
            &format!("· {} eavesdrops, one-way", room.leaks.len()),
            width,
        )
    }
}

fn wire_panel(room: &Room, width: usize, avail: usize, cfg: &Cfg) -> Paragraph<'static> {
    let content = avail.saturating_sub(2).max(1);
    if room.leaks.is_empty() {
        let border = if cfg.flat { Color::White } else { rgb(HEARD) };
        return Paragraph::new(Text::from(Line::from(Span::styled(
            "no one has overheard anyone yet".to_string(),
            style_fg(MUTED),
        ))))
        .block(panel(
            wire_title(room, width.saturating_sub(4)),
            border,
            cfg.flat,
        ));
    }

    let cap = width.saturating_sub(4).max(20);
    let lw = (cap * 3 / 5).clamp(10, 56);

    let mut lines: Vec<Line<'static>> = Vec::new();
    // the column labels, tinted per table
    let mut label_spans: Vec<Span> = Vec::new();
    let mut label_cells: Vec<(usize, String, String)> = Vec::new();
    for (i, tb) in room.tables.iter().enumerate() {
        let (x, lab) = col_labels(lw)[i].clone();
        label_cells.push((x, lab, tb.colour.clone()));
    }
    label_cells.sort_by_key(|(x, _, _)| *x);
    let mut cursor = 0usize;
    for (x, lab, colour) in label_cells {
        if x > cursor {
            label_spans.push(Span::raw(" ".repeat(x - cursor)));
            cursor = x;
        }
        label_spans.push(Span::styled(
            lab.clone(),
            if cfg.flat {
                Style::default()
            } else {
                style_fg(&colour).add_modifier(Modifier::BOLD)
            },
        ));
        cursor += lab.chars().count();
    }
    lines.push(Line::from(label_spans));

    let quote_row = content >= 3;
    let thread_rows = (content.saturating_sub(1 + if quote_row { 1 } else { 0 })).max(1);
    let start = room.leaks.len().saturating_sub(thread_rows);
    for lk in &room.leaks[start..] {
        let src_t = room.table(lk.src);
        let dst_t = room.table(lk.dst);
        let name = by_id(&lk.speaker).map(|p| p.name).unwrap_or("");
        let fixed = lw
            + 2
            + format!("T{}", lk.src).len()
            + 3
            + format!("T{}", lk.dst).len()
            + format!("  round {}  ", lk.round).len()
            + name.len();
        let room_left = cap.saturating_sub(fixed + 2);
        let mut spans: Vec<Span> = vec![Span::styled(
            thread(lk.src, lk.dst, lw),
            if cfg.flat {
                Style::default()
            } else {
                style_fg(HEARD).add_modifier(Modifier::BOLD)
            },
        )];
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("T{}", lk.src),
            if cfg.flat {
                Style::default()
            } else {
                style_fg(src_t.map(|t| t.colour.as_str()).unwrap_or("#ffffff"))
            },
        ));
        spans.push(Span::styled(
            " ⟶ ".to_string(),
            if cfg.flat {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                style_fg(HEARD).add_modifier(Modifier::BOLD)
            },
        ));
        spans.push(Span::styled(
            format!("T{}", lk.dst),
            if cfg.flat {
                Style::default()
            } else {
                style_fg(dst_t.map(|t| t.colour.as_str()).unwrap_or("#ffffff"))
            },
        ));
        spans.push(Span::styled(
            format!("  round {}  ", lk.round),
            style_fg(MUTED),
        ));
        spans.push(Span::styled(name.to_string(), style_fg(MUTED)));
        if room_left > 8 {
            spans.push(Span::styled(
                format!("  “{}”", shorten(&lk.line, room_left - 3)),
                Style::default().fg(Color::White),
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
            Span::styled("   ↳ ".to_string(), style_fg(MUTED)),
            Span::styled("the newest line to cross: ".to_string(), style_fg(MUTED)),
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
                shorten(&lk.line, cap.saturating_sub(head.chars().count() + 6))
            ),
            Style::default().fg(Color::White),
        ));
        lines.push(Line::from(spans));
    }

    let body: Vec<Line> = lines.into_iter().take(content).collect();
    let border = if cfg.flat { Color::White } else { rgb(HEARD) };
    Paragraph::new(Text::from(body)).block(panel(
        wire_title(room, width.saturating_sub(4)),
        border,
        cfg.flat,
    ))
}

// ----------------------------------------------------------------------- detail

fn detail_blocks(room: &Room, table: &Table, width: usize, cfg: &Cfg) -> Vec<Vec<Line<'static>>> {
    let _ = room;
    let mut blocks: Vec<Vec<Line<'static>>> = Vec::new();
    for (i, m) in table.messages.iter().enumerate() {
        let p = m.persona();
        let mut head: Vec<Span> = Vec::new();
        if m.heard() {
            head.push(Span::styled(
                format!("⟪heard from table {}⟫ ", m.heard_from.unwrap()),
                style_fg(HEARD),
            ));
        }
        head.push(Span::styled(
            p.sigil.to_string(),
            style_fg(p.colour).add_modifier(Modifier::BOLD),
        ));
        head.push(Span::raw(" "));
        head.push(Span::styled(
            p.name.to_string(),
            style_fg(p.colour).add_modifier(Modifier::BOLD),
        ));
        if !m.move_.is_empty() {
            head.push(Span::styled(
                format!("  {} {}", move_glyph(&m.move_), m.move_),
                style_fg(move_colour(&m.move_)),
            ));
        }
        if m.composite >= 0.6 {
            head.push(Span::styled("  ★".to_string(), style_fg(GOLD)));
        }
        if !m.figure.is_empty() {
            head.push(Span::styled(format!("  ⛭{}", m.figure), style_fg(MUTED)));
        }
        if let Some(rt) = m.reply_to {
            if rt < table.messages.len() && rt != i {
                let src = &table.messages[rt];
                let tag = if src.heard() {
                    format!("table {}", src.heard_from.unwrap())
                } else {
                    src.persona().name.to_string()
                };
                head.push(Span::styled(
                    format!(
                        "   ▸ answering {tag}: “{}”",
                        shorten(&src.text, (width / 4).max(10))
                    ),
                    if src.heard() {
                        style_fg(HEARD)
                    } else {
                        style_fg(MUTED)
                    },
                ));
            }
        }
        let mut lines = vec![Line::from(
            truncate_spans(head, width.saturating_sub(6))
                .into_iter()
                .collect::<Vec<_>>(),
        )];
        for ln in wrap_text(&m.text, width.saturating_sub(7).max(18)) {
            lines.push(Line::from(Span::styled(
                format!("   {ln}"),
                Style::default().fg(Color::White),
            )));
        }
        lines.push(Line::raw(""));
        blocks.push(lines);
    }
    let _ = cfg;
    blocks
}

/// Word-wrap to `width` characters, never splitting a word.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.chars().count() + 1 + word.chars().count() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            out.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        vec![String::new()]
    } else {
        out
    }
}

fn detail_chat(
    room: &Room,
    table: &Table,
    width: usize,
    avail: usize,
    cfg: &Cfg,
) -> Paragraph<'static> {
    let blocks = detail_blocks(room, table, width, cfg);
    let writ = table
        .members
        .iter()
        .find(|m| room.llamas.get(**m).map(|s| s.speaking).unwrap_or(false))
        .copied();
    let limit = avail
        .saturating_sub(if writ.is_some() { 1 } else { 0 })
        .saturating_sub(1)
        .max(1);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut used = 0usize;
    let mut dropped = 0usize;
    for idx in (0..blocks.len()).rev() {
        if !out.is_empty() && used + blocks[idx].len() > limit {
            dropped = idx + 1;
            break;
        }
        let mut merged = blocks[idx].clone();
        merged.extend(out);
        out = merged;
        used += blocks[idx].len();
    }
    if out.len() > limit && !table.messages.is_empty() {
        let m = &table.messages[table.messages.len() - 1];
        out = vec![speaker_line(
            room,
            m,
            width.saturating_sub(1).max(18),
            "   ",
            0,
        )];
        dropped = table.messages.len() - 1;
    }
    let mut lines: Vec<Line<'static>> = Vec::new();
    if dropped > 0 {
        lines.push(Line::from(Span::styled(
            format!("   ▲ {dropped} older lines above"),
            style_fg(MUTED),
        )));
    }
    lines.extend(out);
    if let Some(w) = writ {
        let p = by_id(w).unwrap();
        let mut spans: Vec<Span> = vec![Span::raw("   ")];
        spans.extend(flare(p.sigil, p.colour, cfg.frame, cfg.flat));
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
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (nothing said yet)".to_string(),
            style_fg(MUTED),
        )));
    }
    let border = if cfg.flat {
        Color::White
    } else {
        rgb(&table.colour)
    };
    Paragraph::new(Text::from(lines)).block(panel(
        titled(
            &table.name(),
            "· newest at the bottom",
            width.saturating_sub(4),
        ),
        border,
        cfg.flat,
    ))
}

fn gauge_cols(width: usize) -> usize {
    if width >= 96 {
        4
    } else if width >= 54 {
        2
    } else {
        1
    }
}

fn gauge_lines(table: &Table, width: usize, cols: usize, cfg: &Cfg) -> Vec<Line<'static>> {
    let cell_w = (width / cols).max(22);
    let track_w = cell_w.saturating_sub(18).max(5);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let rows: Vec<usize> = (0..GAUGES.len()).step_by(cols).collect();
    for start in rows {
        let group: Vec<&str> = GAUGES[start..(start + cols).min(GAUGES.len())].to_vec();
        let mut name_row: Vec<Span> = Vec::new();
        let mut track_row: Vec<Span> = Vec::new();
        for g in group {
            let v = table.gauges.get(g).copied().unwrap_or(0.5);
            let (lo, hi) = ends(g);
            let col = if cfg.flat {
                Style::default()
            } else {
                style_fg(gauge_colour(g, v))
            };
            name_row.push(Span::styled(format!("{g:<10}"), style_fg(MUTED)));
            name_row.push(Span::styled(format!("{v:.2}"), style_fg(MUTED)));
            name_row.push(Span::raw(" ".repeat(cell_w.saturating_sub(14).max(1))));
            track_row.push(Span::styled(format!("{lo:<8}"), style_fg(MUTED)));
            track_row.push(Span::styled(
                two_ended(v, track_w),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    col.add_modifier(Modifier::BOLD)
                },
            ));
            track_row.push(Span::styled(format!("{hi:<8}"), style_fg(MUTED)));
            track_row.push(Span::raw(
                " ".repeat(cell_w.saturating_sub(16 + track_w).max(1)),
            ));
        }
        lines.push(Line::from(name_row));
        lines.push(Line::from(track_row));
    }
    lines
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

fn quality_line(table: &Table, cfg: &Cfg) -> Line<'static> {
    let j = &table.judgement;
    let mut spans: Vec<Span> = vec![Span::styled("quality ".to_string(), style_fg(MUTED))];
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
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            style_fg(AMBER).add_modifier(Modifier::BOLD)
        },
    ));
    for d in QUALITY_DIMS {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            "▍",
            if cfg.flat {
                Style::default()
            } else {
                style_fg(quality_colour(d))
            },
        ));
        spans.push(Span::styled(
            format!(" {d} {:.2}", j.quality.get(d).copied().unwrap_or(0.0)),
            style_fg(MUTED),
        ));
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
            spans.push(Span::styled(format!("{name} "), style_fg(MUTED)));
            spans.push(Span::styled(
                format!("{v:.2}"),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    col.add_modifier(Modifier::BOLD)
                },
            ));
            spans.push(Span::styled(format!("  {lo} → {hi}   "), style_fg(MUTED)));
        }
        rows.push(Line::from(spans));
    }
    rows
}

/// The judge band's content. Full form when there is room, then compact gauges,
/// then just the speaking row. A gauge block is never split across the clip.
fn judge_band_lines(
    room: &Room,
    table: &Table,
    width: usize,
    avail: Option<usize>,
    compact: bool,
    cfg: &Cfg,
) -> Vec<Line<'static>> {
    let j = &table.judgement;
    let inner = width.saturating_sub(4).max(12);
    let cols = gauge_cols(inner);

    let mut speak: Vec<Span> = vec![Span::styled("next to speak  ".to_string(), style_fg(MUTED))];
    let who = if table.members.contains(&j.chosen.as_str()) {
        by_id(&j.chosen)
    } else {
        None
    };
    if let Some(w) = who {
        if room.status != "stopped" && room.status != "error" {
            speak.push(Span::styled(
                w.sigil.to_string(),
                style_fg(w.colour).add_modifier(Modifier::BOLD),
            ));
            speak.push(Span::raw(" "));
            speak.push(Span::styled(
                w.name.to_string(),
                style_fg(w.colour).add_modifier(Modifier::BOLD),
            ));
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
        } else {
            speak.push(Span::styled(
                "—  the tables are closed".to_string(),
                style_fg(MUTED),
            ));
        }
    } else {
        speak.push(Span::styled(
            "—  the tables are closed".to_string(),
            style_fg(MUTED),
        ));
    }
    let speak_w: usize = speak.iter().map(|s| s.width()).sum();
    if !j.move_.is_empty() && speak_w + 24 < inner {
        speak.push(Span::styled(
            "      last line  ".to_string(),
            style_fg(MUTED),
        ));
        speak.push(Span::styled(
            format!("{} {}", move_glyph(&j.move_), j.move_),
            style_fg(move_colour(&j.move_)),
        ));
        let w2: usize = speak.iter().map(|s| s.width()).sum();
        if !j.stance.is_empty() && w2 + j.stance.chars().count() + 6 < inner {
            speak.push(Span::styled(format!(", a {}", j.stance), style_fg(MUTED)));
        }
        let w3: usize = speak.iter().map(|s| s.width()).sum();
        if j.cohesion != 0.0 && w3 + 14 < inner {
            speak.push(Span::styled(
                format!("  ·  flows {:.2}", j.cohesion),
                style_fg(MUTED),
            ));
        }
    }
    let speak_line = Line::from(truncate_spans(speak, inner));

    let q = quality_line(table, cfg);
    let mut verdict: Option<Line<'static>> = None;
    if !j.verdict.is_empty() {
        verdict = Some(Line::from(vec![
            Span::styled("Jev's verdict  ".to_string(), style_fg(MUTED)),
            Span::styled(
                format!("“{}”", shorten(&j.verdict, inner.saturating_sub(18))),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    style_fg(GOLD).add_modifier(Modifier::BOLD)
                },
            ),
        ]));
    }

    let glines = gauge_lines(table, inner, cols, cfg);
    let mut full: Vec<Line> = vec![speak_line.clone()];
    full.extend(glines);
    full.push(q.clone());
    if let Some(v) = verdict.clone() {
        full.push(v);
    }
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
    let content = avail.saturating_sub(2).max(1);
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

fn herd_cell(room: &Room, table: &Table, s: &crate::room::LlamaState, cfg: &Cfg) -> Line<'static> {
    let p = s.persona();
    let chosen = s.id == table.judgement.chosen;
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(
        if chosen { "▶" } else { " " }.to_string(),
        if cfg.flat {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            style_fg(AMBER).add_modifier(Modifier::BOLD)
        },
    ));
    spans.push(Span::styled(
        p.sigil.to_string(),
        if cfg.flat {
            Style::default()
        } else {
            style_fg(p.colour).add_modifier(Modifier::BOLD)
        },
    ));
    spans.push(Span::styled(
        format!(" {:<7}", chars(p.name, 7)),
        if chosen || cfg.flat {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            style_fg(p.colour)
        },
    ));
    if s.speaking {
        spans.extend(flare("", p.colour, cfg.frame, cfg.flat));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        bar(s.eagerness, 5),
        if cfg.flat {
            Style::default()
        } else {
            style_fg(family_colour(p.family))
        },
    ));
    spans.push(Span::styled(format!(" {}×", s.spoken), style_fg(MUTED)));
    let _ = room;
    Line::from(spans)
}

fn detail_herd(room: &Room, table: &Table, width: usize, cfg: &Cfg) -> Paragraph<'static> {
    let order = table.roster(room);
    let half = order.len().div_ceil(2);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for i in 0..half {
        let a = &order[i];
        let mut spans = herd_cell(room, table, a, cfg).spans;
        if let Some(b) = order.get(i + half) {
            spans.push(Span::raw("  "));
            spans.extend(herd_cell(room, table, b, cfg).spans);
        }
        lines.push(Line::from(spans));
    }
    let border = if cfg.flat {
        Color::White
    } else {
        rgb(&table.colour)
    };
    Paragraph::new(Text::from(lines)).block(panel(
        titled("the roster", "· eagerness ▏ who has spoken", width),
        border,
        cfg.flat,
    ))
}

fn detail_beliefs(table: &Table, width: usize, cfg: &Cfg) -> Paragraph<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for p in PROPOSITIONS {
        let h = table.belief_history.get(p).cloned().unwrap_or_default();
        if h.len() < 2 {
            continue;
        }
        let name = belief_short(p);
        let start = h.len().saturating_sub(40);
        let tail = &h[start..];
        let mut spans: Vec<Span> = vec![Span::styled(
            format!("{name:<9}"),
            if cfg.flat {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                style_fg(INDIGO).add_modifier(Modifier::BOLD)
            },
        )];
        spans.push(Span::styled(
            spark(tail),
            if cfg.flat {
                Style::default()
            } else {
                style_fg(INDIGO)
            },
        ));
        spans.push(Span::styled(
            format!(" {:.2}", h[h.len() - 1]),
            style_fg(MUTED),
        ));
        lines.push(Line::from(spans));
    }
    let body = if lines.is_empty() {
        Text::from(Line::from(Span::styled(
            "(no beliefs yet)".to_string(),
            style_fg(MUTED),
        )))
    } else {
        Text::from(lines)
    };
    let border = if cfg.flat {
        Color::White
    } else {
        rgb("#5a5f7a")
    };
    Paragraph::new(body).block(panel(
        titled(
            "what this table believes",
            "· h for the full sentences",
            width.saturating_sub(4),
        ),
        border,
        cfg.flat,
    ))
}

// ------------------------------------------------------------------------ help

#[allow(clippy::vec_init_then_push)]
fn help_panel(cfg: &Cfg) -> Paragraph<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Five tables of four llamas are talking at once, all about the same seed.".to_string(),
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from(Span::styled(
        "Sometimes one table overhears another; a carried line is marked ⟪heard⟫".to_string(),
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from(Span::styled(
        "and the hearing table must answer it. The wire at the bottom shows which table's words reached which.".to_string(),
        Style::default().fg(Color::White),
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
            Span::styled(label.to_string(), style_fg(MUTED)),
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
    let mut gspans: Vec<Span> = vec![Span::styled("  gauges      ".to_string(), style_fg(MUTED))];
    for g in GAUGES {
        let (lo, hi) = ends(g);
        gspans.push(Span::styled(format!("{g} "), style_fg(MUTED)));
        gspans.push(Span::styled(
            format!("({lo} → {hi})  "),
            Style::default().fg(Color::White),
        ));
    }
    lines.push(Line::from(gspans));
    let mut qspans: Vec<Span> = vec![Span::styled("  quality     ".to_string(), style_fg(MUTED))];
    qspans.push(Span::styled(
        "the bar is three segments — ".to_string(),
        Style::default().fg(Color::White),
    ));
    for d in QUALITY_DIMS {
        qspans.push(Span::styled(
            "▍",
            if cfg.flat {
                Style::default()
            } else {
                style_fg(quality_colour(d))
            },
        ));
        qspans.push(Span::styled(format!("{d} "), style_fg(MUTED)));
    }
    qspans.push(Span::styled(
        "— and the number after it is the composite.".to_string(),
        Style::default().fg(Color::White),
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
        Span::styled("  moves       ".to_string(), style_fg(MUTED)),
        Span::styled(moves, Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  beliefs     ".to_string(), style_fg(MUTED)),
        Span::styled(
            "shorter names for the three propositions this table is tracked against:".to_string(),
            Style::default().fg(Color::White),
        ),
    ]));
    for p in PROPOSITIONS {
        lines.push(Line::from(vec![
            Span::styled(
                format!("                 {:<9}", belief_short(p)),
                if cfg.flat {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    style_fg(INDIGO).add_modifier(Modifier::BOLD)
                },
            ),
            Span::styled(p.to_string(), style_fg(MUTED)),
        ]));
    }
    let border = if cfg.flat { Color::White } else { rgb(INDIGO) };
    let title = Line::from(vec![
        Span::styled(
            "how to use this".to_string(),
            style_fg(GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (h to hide)".to_string(), style_fg(MUTED)),
    ]);
    Paragraph::new(Text::from(lines)).block(panel(title, border, cfg.flat))
}

// ------------------------------------------------------------------------ layout

fn budgets(avail: usize, judge_h: usize) -> (usize, usize, usize, usize) {
    let avail = avail.max(6);
    let mut wire = if avail >= 33 {
        7
    } else if avail >= 23 {
        5
    } else {
        3
    };
    let mut band = if avail >= 33 {
        7
    } else if avail >= 23 {
        5
    } else {
        0
    };
    let judge = judge_h.min((avail.saturating_sub(wire + band + 4)).max(3));
    let mut chat = avail.saturating_sub(judge + wire + band);
    while chat < 4 && band > 0 {
        band -= 1;
        chat += 1;
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
    let full_h = judge_band_lines(room, table, width, None, false, cfg).len() + 2;
    let (j, _, _, chat) = budgets(avail, full_h);
    if chat >= 4 {
        return (j, false);
    }
    let comp_h = judge_band_lines(room, table, width, None, true, cfg).len() + 2;
    let (j2, _, _, _) = budgets(avail, comp_h);
    (j2, true)
}

/// Paint the whole frame. Pure in (room, cfg, size): no network, no terminal.
pub fn paint(frame: &mut Frame, room: &Room, cfg: &Cfg) {
    let area = frame.area();
    let width = area.width as usize;
    let height = area.height as usize;
    let focus = if (0..room.tables.len() as i32).contains(&room.focus) {
        room.focus as usize
    } else {
        usize::MAX
    };

    let h_head = if height >= 26 { 5u16 } else { 4u16 };
    let [head, tabs, body, foot] = Layout::vertical([
        Constraint::Length(h_head),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    frame.render_widget(masthead(room, width, cfg.flat), head);
    frame.render_widget(tab_bar(room, cfg.flat), tabs);
    frame.render_widget(footer(room, width, cfg.flat), foot);

    if focus == usize::MAX {
        // overview: five rows + the wire
        let wire_h = (height / 6).clamp(3, 6);
        let [rows, wire] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(wire_h as u16)]).areas(body);
        let rows_h = height.saturating_sub(6 + wire_h);
        let widget = if cfg.hint {
            help_panel(cfg)
        } else {
            overview_rows(room, width, rows_h, cfg)
        };
        frame.render_widget(widget, rows);
        frame.render_widget(wire_panel(room, width, wire_h, cfg), wire);
        return;
    }

    // detail: three bands + the wire. Budgets are computed from the *actual* body
    // height (after the header/tabs/footer), so nothing is double-counted.
    let table = &room.tables[focus];
    let avail = body.height as usize;
    let (judge_h, compact) = judge_alloc(avail, room, table, width, cfg);
    let (j, wire, band, chat) = budgets(avail, judge_h);

    let mut constraints = vec![Constraint::Length(j as u16), Constraint::Min(0)];
    if band > 0 {
        constraints.push(Constraint::Length(band as u16));
    }
    constraints.push(Constraint::Length(wire as u16));
    let areas = Layout::vertical(constraints).split(body);

    let lines = judge_band_lines(room, table, width, Some(j), compact, cfg);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(panel(
            titled("the judge", "· jev-1.13", width.saturating_sub(4)),
            if cfg.flat { Color::White } else { rgb(INDIGO) },
            cfg.flat,
        )),
        areas[0],
    );
    let chat_widget = if cfg.hint {
        help_panel(cfg)
    } else {
        detail_chat(room, table, width, chat.saturating_sub(2).max(3), cfg)
    };
    frame.render_widget(chat_widget, areas[1]);
    let mut idx = 2;
    if band > 0 {
        let half = ((width * 2 / 5).clamp(34, 64)) as u16;
        let [roster, beliefs] =
            Layout::horizontal([Constraint::Length(half), Constraint::Min(0)]).areas(areas[idx]);
        frame.render_widget(
            detail_herd(room, table, (half as usize).saturating_sub(4), cfg),
            roster,
        );
        frame.render_widget(
            detail_beliefs(table, width.saturating_sub(half as usize + 4), cfg),
            beliefs,
        );
        idx += 1;
    }
    frame.render_widget(wire_panel(room, width, wire, cfg), areas[idx]);
}

/// The shimmer effect: a slow dissolve that keeps a frame alive. Applied by the
/// live loop onto a composed frame's buffer; never used for static frames.
pub fn shimmer() -> tachyonfx::Effect {
    use tachyonfx::{fx, EffectTimer, Interpolation};
    fx::dissolve(EffectTimer::from_ms(900, Interpolation::Linear))
}

/// Paint the given effect onto a buffer (used by the live TUI loop).
pub fn apply_effect(
    fx: &mut tachyonfx::Effect,
    buf: &mut Buffer,
    area: Rect,
    dt: std::time::Duration,
) {
    fx.process(dt, buf, area);
}

/// Compose a frame to plain text lines at a given size with no terminal. This is
/// the offline harness the tests and `--once` use.
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
