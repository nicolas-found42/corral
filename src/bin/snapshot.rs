//! Run a short session and render both views (overview + one table) to SVG + text.
//!
//!     cargo run --bin snapshot -- -t "..." -n 8 -o corral.svg
//!
//! Produces a still of The Corral from a real discussion: the zoomed-out five-table
//! overview and the zoomed-in detail of the busiest table, one after the other.
//! The SVG is generated directly from the rendered cell buffer, so it carries every
//! glyph and truecolor exactly as the terminal would draw it.

use std::io::Write;

use corral::room::{empty_room, Room};
use corral::session::Session;
use corral::tui::{self, Cfg};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

struct Args {
    seed: String,
    rounds: u32,
    draws: usize,
    out: String,
    width: u16,
    height: u16,
    demo: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        seed: "is a crowd wiser than any one of us, or just louder?".to_string(),
        rounds: 8,
        draws: 2,
        out: "corral.svg".to_string(),
        width: 150,
        height: 45,
        demo: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut next = || it.next().ok_or_else(|| format!("missing value for {arg}"));
        match arg.as_str() {
            "-t" | "--seed" => a.seed = next()?,
            "-n" | "--rounds" => {
                a.rounds = next()?.parse().map_err(|_| "rounds must be an integer")?
            }
            "--draws" => a.draws = next()?.parse().map_err(|_| "draws must be an integer")?,
            "-o" | "--out" => a.out = next()?,
            "--width" => a.width = next()?.parse().map_err(|_| "width must be an integer")?,
            "--height" => a.height = next()?.parse().map_err(|_| "height must be an integer")?,
            "--demo" => a.demo = true,
            "-h" | "--help" => {
                println!("snapshot: render a real run to SVG + text\n  -t SEED  -n ROUNDS  --draws N  -o OUT.svg  --width W  --height H  --demo");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(a)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if !args.demo
        && std::env::var("OPENROUTER_API_KEY")
            .map(|k| k.trim().is_empty())
            .unwrap_or(true)
    {
        return Err("OPENROUTER_API_KEY is not set".to_string());
    }

    let room = if args.demo {
        // Offline: a hand-seeded room (the corpus) so the exporter can be exercised
        // with no network and no spend.
        demo_room(&args.seed)
    } else {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("tokio: {e}"))?;
        rt.block_on(async {
            let room = empty_room(&args.seed);
            let mut sess = Session::new(room, args.draws, 7, Some(args.rounds))?;
            sess.run().await;
            Ok::<Room, String>(sess.room)
        })?
    };

    let busy = room
        .tables
        .iter()
        .max_by_key(|t| t.messages.len())
        .map(|t| t.id as i32 - 1)
        .unwrap_or(0);

    let cfg = Cfg {
        draws: args.draws,
        max_rounds: Some(args.rounds),
        ..Default::default()
    };

    let mut overview = room.clone();
    overview.focus = -1;
    let mut detail = room.clone();
    detail.focus = busy;

    let svg = svg_of(
        &[(&overview, &cfg), (&detail, &cfg)],
        args.width,
        args.height,
    );
    std::fs::write(&args.out, svg).map_err(|e| format!("write {}: {e}", args.out))?;

    let mut txt = String::new();
    txt.push_str(&tui::compose_text(&overview, &cfg, args.width, args.height).join("\n"));
    txt.push('\n');
    txt.push_str(&tui::compose_text(&detail, &cfg, args.width, args.height).join("\n"));
    txt.push('\n');
    let txt_path = args
        .out
        .rsplit_once('.')
        .map(|(b, _)| format!("{b}.txt"))
        .unwrap_or_else(|| format!("{}.txt", args.out));
    let mut fh = std::fs::File::create(&txt_path).map_err(|e| format!("write {txt_path}: {e}"))?;
    fh.write_all(txt.as_bytes())
        .map_err(|e| format!("write {txt_path}: {e}"))?;

    println!(
        "wrote {} · {} rounds · {} lines · {} eavesdrops · ${:.4} · jev {} · llama {}",
        args.out,
        room.turn,
        room.messages(),
        room.leaks.len(),
        room.cost(),
        room.calls_jev,
        room.calls_llama
    );
    Ok(())
}

/// A hex string for a colour we know is RGB, else a fallback.
fn colour(c: ratatui::style::Color, fallback: &str) -> String {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        ratatui::style::Color::Reset => fallback.to_string(),
        other => format!("#{fallback}{other:?}"),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Render one or more frames to a single SVG document, one frame per band.
fn svg_of(frames: &[(&Room, &Cfg)], w: u16, h: u16) -> String {
    let cell_w = 8.4_f64;
    let cell_h = 17.0_f64;
    let pad = 12.0_f64;
    let gap = 20.0_f64;
    let total_w = pad * 2.0 + cell_w * w as f64;
    let total_h = pad * 2.0
        + frames.len() as f64 * (cell_h * h as f64)
        + gap * (frames.len().saturating_sub(1)) as f64;

    let mut s = String::new();
    s.push_str(&format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{total_w:.0}\" height=\"{total_h:.0}\" viewBox=\"0 0 {total_w:.0} {total_h:.0}\" font-family=\"Menlo, ui-monospace, monospace\" font-size=\"13\">\n"
    ));
    s.push_str(&format!(
        "<rect width=\"{total_w:.0}\" height=\"{total_h:.0}\" fill=\"#12102b\"/>\n"
    ));

    for (fi, (room, cfg)) in frames.iter().enumerate() {
        let lines = tui::compose_text(room, cfg, w, h);
        let y0 = pad + fi as f64 * (cell_h * h as f64 + gap);
        // We need the styled cells as well as the glyphs; re-render through the
        // backend to read colours per cell.
        let buf = render_buffer(room, cfg, w, h);
        for (y, _) in lines.iter().enumerate().take(h as usize) {
            for x in 0..w as usize {
                let cell = buf.cell((x as u16, y as u16));
                let (sym, _fg, bg, bold) = match cell {
                    Some(c) => (
                        c.symbol().to_string(),
                        colour(c.fg, "#d7d3ef"),
                        c.bg,
                        c.modifier.contains(ratatui::style::Modifier::BOLD),
                    ),
                    None => (
                        " ".to_string(),
                        "#d7d3ef".to_string(),
                        ratatui::style::Color::Reset,
                        false,
                    ),
                };
                if sym == " " && matches!(bg, ratatui::style::Color::Reset) {
                    continue;
                }
                let cx = pad + x as f64 * cell_w;
                let cy = y0 + (y as f64 + 0.8) * cell_h;
                if !matches!(bg, ratatui::style::Color::Reset) {
                    let bgc = colour(bg, "#12102b");
                    s.push_str(&format!(
                        "<rect x=\"{cx:.1}\" y=\"{:.1}\" width=\"{cell_w:.1}\" height=\"{cell_h:.1}\" fill=\"{bgc}\"/>\n",
                        cy - cell_h * 0.75
                    ));
                }
                let weight = if bold { " font-weight=\"700\"" } else { "" };
                let fgc = colour(fg_color(cell), "#d7d3ef");
                s.push_str(&format!(
                    "<text x=\"{cx:.1}\" y=\"{cy:.1}\" fill=\"{fgc}\"{weight}>{}</text>\n",
                    escape(&sym)
                ));
            }
        }
    }
    s.push_str("</svg>\n");
    s
}

fn fg_color(cell: Option<&ratatui::buffer::Cell>) -> ratatui::style::Color {
    cell.map(|c| c.fg)
        .unwrap_or(ratatui::style::Color::Rgb(0xd7, 0xd3, 0xef))
}

/// A hand-seeded room built from the real corpus, for the offline `--demo` path.
fn demo_room(seed: &str) -> Room {
    use corral::corpus;
    use corral::room::{Leak, Message, PROPOSITIONS};
    let mut room = empty_room(seed);
    let lines: Vec<(u8, &str, &str, Option<u8>)> = corpus::LINES
        .iter()
        .map(|l| (l.table, l.speaker, l.text, l.heard_from))
        .collect();
    for (tid, sp, tx, hf) in lines {
        if let Some(t) = room.table_mut(tid) {
            let turn = t.messages.len() as u32;
            let mut m = Message::new(turn, sp, tx);
            m.heard_from = hf;
            m.move_ = ["claim", "evidence", "question", "rebuttal", "analogy"][(turn as usize) % 5]
                .to_string();
            m.composite = if turn.is_multiple_of(3) { 0.66 } else { 0.41 };
            // Exercise the judged open-question field: a line that asks something and
            // was not answered reads as open; a statement reads as resolved.
            m.answers_open = if tx.contains('?') { 0.21 } else { 0.74 };
            t.messages.push(m);
        }
    }
    for (round, src, dst, sp, line) in corpus::LEAKS {
        room.leaks.push(Leak {
            round: round as u32,
            src,
            dst,
            line: line.to_string(),
            speaker: sp.to_string(),
            why: "carried at 0.52".to_string(),
            effect: "adds a new argument".to_string(),
            landed: true,
        });
        if let Some(t) = room.table_mut(dst) {
            t.leaks_in += 1;
        }
    }
    for t in &mut room.tables {
        let last = t
            .messages
            .last()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        t.summary = last;
        t.summary_from = t
            .messages
            .last()
            .map(|m| m.persona().name.to_string())
            .unwrap_or_default();
        t.gauges = [
            ("heat".to_string(), 0.72),
            ("consensus".to_string(), 0.41),
            ("drift".to_string(), 0.17),
            ("novelty".to_string(), 0.63),
        ]
        .into_iter()
        .collect();
        // Diverge the three trajectories so the fan shows the SHAPE of the table's
        // movement: agreement rising, division falling, novelty wandering around the
        // middle -- which is what a real run produces and what the fan is for.
        let demo_beliefs = [
            vec![0.30, 0.38, 0.47, 0.58, 0.67, 0.74],
            vec![0.72, 0.64, 0.55, 0.46, 0.37, 0.29],
            vec![0.50, 0.53, 0.56, 0.52, 0.58, 0.61],
        ];
        for (i, p) in PROPOSITIONS.iter().enumerate() {
            t.belief_history
                .insert(p.to_string(), demo_beliefs[i].clone());
        }
        // and a short gauge history, so each band's micro-weather has something to draw
        t.history = [
            ("heat".to_string(), vec![0.40, 0.48, 0.55, 0.61, 0.66, 0.72]),
            (
                "drift".to_string(),
                vec![0.30, 0.26, 0.22, 0.20, 0.18, 0.17],
            ),
            (
                "novelty".to_string(),
                vec![0.52, 0.58, 0.61, 0.60, 0.62, 0.63],
            ),
            (
                "consensus".to_string(),
                vec![0.30, 0.33, 0.37, 0.39, 0.40, 0.41],
            ),
        ]
        .into_iter()
        .collect();
        t.judgement.chosen = t.members[1].to_string();
        t.judgement.pick_confidence = 0.71;
        t.judgement.move_ = "evidence".to_string();
        t.judgement.stance = "support".to_string();
        t.judgement.cohesion = 0.78;
        t.judgement.composite = 0.66;
        t.judgement.quality = [
            ("originality".to_string(), 0.7),
            ("clarity".to_string(), 0.6),
            ("insight".to_string(), 0.65),
        ]
        .into_iter()
        .collect();
        t.judgement.verdict = "the room keeps redefining its own question".to_string();
    }
    room.turn = 10;
    room.status = "stopped".to_string();
    room.started = Some(corral::room::now() - 93.0);
    room.finished = Some(corral::room::now());
    room.cost_jev = 0.0091;
    room.cost_llama = 0.0026;
    room.calls_jev = 143;
    room.calls_llama = 58;
    room.log_line("five tables seated", "dim");
    room
}

fn render_buffer(room: &Room, cfg: &Cfg, w: u16, h: u16) -> ratatui::buffer::Buffer {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let mut term = Terminal::new(TestBackend::new(w, h)).expect("the test terminal builds");
    term.draw(|f| tui::paint(f, room, cfg))
        .expect("the frame draws");
    term.backend().buffer().clone()
}
