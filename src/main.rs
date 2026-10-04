//! The Corral: five tables of four cheap 2024 Meta Llama 3.1 8B instances, talking
//! at once about one seed, refereed live by TypeSafe Jev -- and occasionally
//! overhearing each other.
//!
//!     cargo run --                       # asks for a seed, then runs
//!     cargo run -- -t "your seed"        # run straight from a seed (any text)
//!     cargo run -- -t "..." -n 6         # six rounds then stop
//!     cargo run -- --headless -t "..."   # no TUI: stream all five chats (scriptable)
//!     cargo run -- --once                # render one frame and exit (no network)
//!
//! Keys: q quit · space pause · 1-5 open a table · o overview · Tab/. cycle · t new
//! seed · s save · f flat · h help.

use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use corral::room::empty_room;
use corral::session::Session;
use corral::tui::{self, Cfg};

#[derive(Debug)]
struct Args {
    seed_text: Option<String>,
    rounds: Option<u32>,
    draws: usize,
    rng: i64,
    headless: bool,
    once: bool,
}

const USAGE: &str = "The Corral: five tables of four llama-3.1-8b agents, one seed.

USAGE: corral [-t SEED] [-n ROUNDS] [--draws 1-4] [--rng N] [--headless] [--once]

  -t, --seed SEED    the shared seed: a question, a topic, a sentence, any text
  -n, --rounds N     stop after this many rounds
      --draws N      independent Jev draws per table per turn (1-4), default 2
      --rng N        random seed, for reproducible leaks, default 7
      --headless     no TUI: stream all five chats to stdout
      --once         render one frame and exit (no network)";

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        seed_text: None,
        rounds: None,
        draws: 2,
        rng: 7,
        headless: false,
        once: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "-t" | "--seed" => a.seed_text = Some(it.next().ok_or("missing value for --seed")?),
            "-n" | "--rounds" => {
                a.rounds = Some(
                    it.next()
                        .ok_or("missing value for --rounds")?
                        .parse()
                        .map_err(|_| "rounds must be an integer")?,
                )
            }
            "--draws" => {
                a.draws = it
                    .next()
                    .ok_or("missing value for --draws")?
                    .parse()
                    .map_err(|_| "draws must be an integer")?
            }
            "--rng" => {
                a.rng = it
                    .next()
                    .ok_or("missing value for --rng")?
                    .parse()
                    .map_err(|_| "rng must be an integer")?
            }
            "--headless" => a.headless = true,
            "--once" => a.once = true,
            other => return Err(format!("unknown argument: {other}\n\n{USAGE}")),
        }
    }
    Ok(a)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;

    if args.once {
        let room = empty_room(
            args.seed_text
                .as_deref()
                .unwrap_or("a seed nobody has typed yet"),
        );
        let cfg = Cfg {
            draws: args.draws,
            max_rounds: args.rounds,
            ..Default::default()
        };
        let (w, h) = terminal_size();
        println!("{}", tui::compose_text(&room, &cfg, w, h).join("\n"));
        return Ok(());
    }

    if std::env::var("OPENROUTER_API_KEY")
        .map(|k| k.trim().is_empty())
        .unwrap_or(true)
    {
        return Err("OPENROUTER_API_KEY is not set. Export it (it lives in ~/.zshenv on this machine) and try again.".to_string());
    }

    let mut seed = args.seed_text.clone().unwrap_or_default();
    if seed.trim().is_empty() {
        print!("what should all five tables talk about? › ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok();
        seed = line.trim().to_string();
    }
    if seed.trim().is_empty() {
        seed = "is a crowd wiser than any one of us, or just louder?".to_string();
    }

    let cfg = Cfg {
        draws: args.draws.clamp(1, 4),
        max_rounds: args.rounds,
        flat: false,
        hint: false,
        frame: 0,
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio: {e}"))?;

    if args.headless {
        rt.block_on(run_headless(seed, cfg, args.rng))
    } else {
        rt.block_on(run_tui(seed, cfg, args.rng))
    }
}

async fn run_headless(seed: String, cfg: Cfg, rng_seed: i64) -> Result<(), String> {
    let room = empty_room(&seed);
    let mut sess = Session::new(room, cfg.draws, rng_seed, cfg.max_rounds)?;
    let (tx, mut rx) = tokio::sync::watch::channel(sess.room.clone());
    sess.watch(tx);

    let streamer = tokio::spawn(async move {
        let mut seen: std::collections::BTreeMap<u8, usize> = Default::default();
        while rx.changed().await.is_ok() {
            let room = rx.borrow().clone();
            for t in &room.tables {
                let n = seen.entry(t.id).or_insert(0);
                while *n < t.messages.len() {
                    let m = &t.messages[*n];
                    let tag = if m.heard() {
                        format!(" (heard from T{})", m.heard_from.unwrap())
                    } else {
                        String::new()
                    };
                    println!("[T{}] {}{tag}: {}", t.id, m.persona().name, m.text);
                    *n += 1;
                }
            }
        }
    });

    sess.run().await;
    streamer.abort();

    let room = &sess.room;
    println!("\n=== five tables on: {} ===", room.seed);
    for t in &room.tables {
        println!(
            "\n{} — on: {}",
            t.name(),
            if t.summary.is_empty() {
                "the seed"
            } else {
                &t.summary
            }
        );
        for m in &t.messages {
            let tag = if m.heard() {
                format!("(heard from T{}) ", m.heard_from.unwrap())
            } else {
                String::new()
            };
            println!("  {tag}{}: {}", m.persona().name, m.text);
        }
    }
    if !room.leaks.is_empty() {
        println!("\n=== the wire ===");
        for lk in &room.leaks {
            let snippet: String = lk.line.chars().take(70).collect();
            println!("  T{} ⟶ T{}  ({})  {snippet}", lk.src, lk.dst, lk.effect);
        }
    }
    println!(
        "\n{} rounds · {} lines · {} eavesdrops · ${:.4} · jev {} · llama {}",
        room.turn,
        room.messages(),
        room.leaks.len(),
        room.cost(),
        room.calls_jev,
        room.calls_llama
    );
    Ok(())
}

async fn run_tui(seed: String, cfg: Cfg, rng_seed: i64) -> Result<(), String> {
    let room = empty_room(&seed);
    let mut sess = Session::new(room, cfg.draws, rng_seed, cfg.max_rounds)?;
    let pause = sess.paused_handle();
    let pending = sess.pending_seed_handle();
    let stop = sess.stop_handle();

    let (tx, mut rx) = tokio::sync::watch::channel(sess.room.clone());
    sess.watch(tx);

    let session_task = tokio::spawn(async move {
        sess.run().await;
        sess
    });

    let mut terminal = setup_terminal()?;
    let mut cfg = cfg;
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    let mut last = Instant::now();
    let mut effect = tui::shimmer();
    let mut commands: Vec<KeyCode> = Vec::new();
    let mut focus = -1i32;

    while !session_task.is_finished() {
        while crossterm::event::poll(Duration::from_millis(0)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = crossterm::event::read() {
                if key.kind == KeyEventKind::Press {
                    commands.push(key.code);
                }
            }
        }
        while let Some(code) = commands.first().copied() {
            commands.remove(0);
            let snap = rx.borrow().clone();
            match code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    stop.store(true, Ordering::SeqCst);
                }
                KeyCode::Char(' ') => {
                    let now = !pause.load(Ordering::SeqCst);
                    pause.store(now, Ordering::SeqCst);
                }
                KeyCode::Char('o')
                | KeyCode::Char('O')
                | KeyCode::Char('0')
                | KeyCode::Char('`') => {
                    focus = -1;
                }
                KeyCode::Tab | KeyCode::Char('.') | KeyCode::Char(']') => {
                    let n = snap.tables.len() as i32;
                    focus = if focus >= n - 1 { -1 } else { focus + 1 };
                }
                KeyCode::Char('[') | KeyCode::Char(',') => {
                    let n = snap.tables.len() as i32;
                    focus = if focus <= 0 { n - 1 } else { focus - 1 };
                }
                KeyCode::Char(c @ '1'..='5') => {
                    let want = c as i32 - '0' as i32 - 1;
                    if want < snap.tables.len() as i32 {
                        focus = want;
                    }
                }
                KeyCode::Char('+') | KeyCode::Char('=') => cfg.draws = (cfg.draws + 1).min(4),
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    cfg.draws = cfg.draws.saturating_sub(1).max(1)
                }
                KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') => {
                    cfg.hint = !cfg.hint
                }
                KeyCode::Char('f') | KeyCode::Char('F') => cfg.flat = !cfg.flat,
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    save(&snap)?;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    let typed = prompt_seed(&mut terminal, &mut rx)?;
                    if !typed.trim().is_empty() {
                        if let Ok(mut g) = pending.lock() {
                            *g = Some(typed);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut snapshot = rx.borrow().clone();
        snapshot.focus = focus;
        let dt = last.elapsed();
        last = Instant::now();
        cfg.frame = cfg.frame.wrapping_add(1);
        let draws = cfg.draws;
        terminal
            .draw(|f| {
                let c = Cfg {
                    draws,
                    ..cfg.clone()
                };
                tui::paint(f, &snapshot, &c);
                if !c.flat {
                    let area = f.area();
                    tui::apply_effect(&mut effect, f.buffer_mut(), area, dt);
                }
            })
            .map_err(|e| format!("draw: {e}"))?;
        ticker.tick().await;
    }

    let sess = session_task
        .await
        .map_err(|e| format!("session join: {e}"))?;
    restore_terminal(&mut terminal)?;
    let room = &sess.room;
    println!(
        "\nThe Corral — {} rounds, {} lines across {} tables, {} eavesdrops, ${:.4}",
        room.turn,
        room.messages(),
        room.tables.len(),
        room.leaks.len(),
        room.cost()
    );
    if !room.outcome.is_empty() {
        println!("{}", room.outcome);
    }
    Ok(())
}

fn save(room: &corral::room::Room) -> Result<(), String> {
    let path = format!("corral-{}.txt", corral::timestamp());
    let mut body = format!(
        "The Corral — five tables on: {}\n{} rounds\n",
        room.seed, room.turn
    );
    for t in &room.tables {
        body.push_str(&format!(
            "\n=== {} (on: {}) ===\n",
            t.name(),
            if t.summary.is_empty() {
                "the seed"
            } else {
                &t.summary
            }
        ));
        for m in &t.messages {
            let tag = if m.heard() {
                format!("  ⟪heard from table {}⟫", m.heard_from.unwrap())
            } else {
                String::new()
            };
            body.push_str(&format!("{}:{tag} {}\n\n", m.persona().name, m.text));
        }
    }
    std::fs::write(&path, body).map_err(|e| format!("could not save: {e}"))
}

/// Suspend the TUI, ask for a new seed in cooked mode, then resume.
fn prompt_seed(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    _rx: &mut tokio::sync::watch::Receiver<corral::room::Room>,
) -> Result<String, String> {
    crossterm::terminal::disable_raw_mode().map_err(|e| format!("prompt: {e}"))?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )
    .map_err(|e| format!("prompt: {e}"))?;
    print!("\nNew seed for all five tables (Enter to cancel) › ");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok();
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen
    )
    .map_err(|e| format!("prompt: {e}"))?;
    crossterm::terminal::enable_raw_mode().map_err(|e| format!("prompt: {e}"))?;
    Ok(line.trim().to_string())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, String> {
    crossterm::terminal::enable_raw_mode().map_err(|e| format!("raw mode: {e}"))?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)
        .map_err(|e| format!("alt screen: {e}"))?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(|e| format!("terminal: {e}"))
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), String> {
    crossterm::terminal::disable_raw_mode().map_err(|e| format!("no raw mode: {e}"))?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )
    .map_err(|e| format!("no alt screen: {e}"))?;
    terminal.show_cursor().map_err(|e| format!("cursor: {e}"))
}

fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((150, 45))
}

// Keep the Arc import meaningful even if a future refactor drops the pause handle.
#[allow(dead_code)]
fn _assert_send(_: Arc<std::sync::atomic::AtomicBool>) {}
