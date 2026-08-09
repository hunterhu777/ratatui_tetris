//! An optional Unix-socket control channel, enabled with `--control <path>`.
//!
//! The game still runs normally in the terminal and still responds to the
//! keyboard; this just adds a second way to drive it. A connected client
//! sends one command per connection and gets the resulting state back as
//! JSON, which saves it from having to scrape the rendered screen.
//!
//! Commands:
//!   `state`                    -> current state, changing nothing
//!   `play <action> [action…]`  -> apply actions in order, then report state
//!   `pause` / `resume`         -> set the paused flag explicitly
//!
//! Action names: left right soft drop cw ccw flip hold

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use crate::board::{HIDDEN_ROWS, VISIBLE_ROWS, WIDTH};
use crate::game::{Action, Game, Phase};
use crate::tetromino::Spin;

/// What a client asked for, plus the channel to answer on.
pub struct Request {
    pub kind: Kind,
    pub reply: Sender<String>,
}

pub enum Kind {
    State,
    Play(Vec<Action>),
    SetPaused(bool),
}

/// Bind the socket and start accepting clients on a background thread.
/// The returned receiver is drained by the main loop, so all game mutation
/// still happens on one thread.
pub fn listen(path: &Path) -> std::io::Result<Receiver<Request>> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    let (tx, rx) = channel();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            // Errors here are a client's problem, not ours; keep serving.
            let _ = serve(stream, &tx);
        }
    });

    Ok(rx)
}

/// Best-effort cleanup so a stale socket file doesn't linger.
pub fn cleanup(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}

fn serve(stream: UnixStream, tx: &Sender<Request>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let Some(kind) = parse(line.trim()) else {
        writeln!(writer, "{{\"error\":\"bad command\"}}")?;
        return Ok(());
    };

    let (reply_tx, reply_rx) = channel();
    if tx.send(Request { kind, reply: reply_tx }).is_err() {
        writeln!(writer, "{{\"error\":\"game gone\"}}")?;
        return Ok(());
    }

    match reply_rx.recv() {
        Ok(json) => writeln!(writer, "{json}")?,
        Err(_) => writeln!(writer, "{{\"error\":\"no reply\"}}")?,
    }
    Ok(())
}

fn parse(line: &str) -> Option<Kind> {
    let mut words = line.split_whitespace();
    match words.next()? {
        "state" => Some(Kind::State),
        "pause" => Some(Kind::SetPaused(true)),
        "resume" => Some(Kind::SetPaused(false)),
        "play" => {
            let actions = words.map(action).collect::<Option<Vec<_>>>()?;
            Some(Kind::Play(actions))
        }
        _ => None,
    }
}

fn action(name: &str) -> Option<Action> {
    Some(match name {
        "left" => Action::MoveLeft,
        "right" => Action::MoveRight,
        "soft" => Action::SoftDrop,
        "drop" => Action::HardDrop,
        "cw" => Action::Rotate(Spin::Cw),
        "ccw" => Action::Rotate(Spin::Ccw),
        "flip" => Action::Rotate(Spin::Flip),
        "hold" => Action::Hold,
        _ => return None,
    })
}

/// Everything a driver needs, so it never has to read the screen.
pub fn state_json(game: &Game) -> String {
    let mut out = String::with_capacity(1024);
    out.push('{');

    out.push_str(&format!("\"score\":{},", game.score));
    out.push_str(&format!("\"lines\":{},", game.lines));
    out.push_str(&format!("\"level\":{},", game.level));
    out.push_str(&format!("\"phase\":\"{}\",", phase_name(&game.phase)));
    out.push_str(&format!("\"can_hold\":{},", game.can_hold()));

    match game.hold {
        Some(kind) => out.push_str(&format!("\"hold\":\"{kind:?}\",")),
        None => out.push_str("\"hold\":null,"),
    }

    let next = game
        .next_pieces()
        .iter()
        .map(|k| format!("\"{k:?}\""))
        .collect::<Vec<_>>()
        .join(",");
    out.push_str(&format!("\"next\":[{next}],"));

    out.push_str(&format!("\"current\":\"{:?}\",", game.piece.kind));
    out.push_str(&format!("\"rotation\":{},", game.piece.rotation));
    out.push_str(&format!("\"piece\":{},", cells_json(&game.piece.cells())));
    out.push_str(&format!("\"ghost\":{},", cells_json(&game.ghost().cells())));

    // The locked stack only: '.' for empty, the piece letter otherwise.
    // The falling piece is reported separately so a driver never confuses
    // the two, which is the mistake screen-scraping kept inviting.
    out.push_str("\"grid\":[");
    for row in 0..VISIBLE_ROWS {
        if row > 0 {
            out.push(',');
        }
        out.push('"');
        for col in 0..WIDTH {
            match game.board.get(col, row + HIDDEN_ROWS) {
                Some(kind) => out.push_str(&format!("{kind:?}")),
                None => out.push('.'),
            }
        }
        out.push('"');
    }
    out.push_str("],");

    // Surface height per column, and every buried empty cell. Computed here
    // from the real board rather than inferred from pixels.
    let heights = (0..WIDTH).map(column_height(game)).collect::<Vec<_>>();
    out.push_str(&format!(
        "\"heights\":[{}],",
        heights
            .iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));

    let mut holes = Vec::new();
    for col in 0..WIDTH {
        let height = heights[col as usize];
        if height == 0 {
            continue;
        }
        let top = VISIBLE_ROWS - height;
        for row in top..VISIBLE_ROWS {
            if game.board.get(col, row + HIDDEN_ROWS).is_none() {
                holes.push(format!("[{col},{row}]"));
            }
        }
    }
    out.push_str(&format!("\"holes\":[{}]", holes.join(",")));

    out.push('}');
    out
}

/// Rows from the highest filled cell in a column down to the floor.
fn column_height(game: &Game) -> impl Fn(i16) -> i16 + '_ {
    move |col| {
        (0..VISIBLE_ROWS)
            .find(|&row| game.board.get(col, row + HIDDEN_ROWS).is_some())
            .map_or(0, |row| VISIBLE_ROWS - row)
    }
}

fn cells_json(cells: &[(i16, i16); 4]) -> String {
    let inner = cells
        .iter()
        // Report in visible-row coordinates; negative means still above the well.
        .map(|(x, y)| format!("[{},{}]", x, y - HIDDEN_ROWS))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{inner}]")
}

fn phase_name(phase: &Phase) -> &'static str {
    match phase {
        Phase::Falling => "falling",
        Phase::Clearing { .. } => "clearing",
        Phase::Paused => "paused",
        Phase::GameOver => "gameover",
    }
}
