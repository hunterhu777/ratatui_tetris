//! A Tetris implementation for the terminal, built on ratatui.
//!
//! Follows the modern Tetris guideline: 7-bag randomiser, SRS rotation with
//! wall kicks, hold, ghost piece, lock delay, T-spins and back-to-back bonuses.

mod board;
mod game;
mod tetromino;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use game::{Action, Game};
use tetromino::Spin;

/// Redraw budget: 60 frames per second is smooth without burning CPU.
const FRAME: Duration = Duration::from_millis(16);

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    let mut game = Game::new();
    let mut last = Instant::now();

    loop {
        terminal.draw(|frame| ui::render(frame, &game))?;

        // Spend whatever is left of this frame waiting for input, so keys feel
        // instant but an idle game still ticks at a steady rate.
        let deadline = last + FRAME;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            if !event::poll(remaining)? {
                break;
            }
            match event::read()? {
                Event::Key(key) => match to_command(key) {
                    Some(Command::Quit) => return Ok(()),
                    Some(Command::Restart) => game.restart(),
                    Some(Command::Play(action)) => game.handle(action),
                    None => {}
                },
                // Resizing is handled by the next draw; nothing else to do.
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        let now = Instant::now();
        game.update(now.duration_since(last));
        last = now;
    }
}

/// Everything a key press can mean.
enum Command {
    Play(Action),
    Restart,
    Quit,
}

fn to_command(key: KeyEvent) -> Option<Command> {
    // Terminals that support the kitty protocol also report key releases and
    // repeats; only presses and repeats should drive the game.
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Command::Quit),
            _ => None,
        };
    }

    let command = match key.code {
        KeyCode::Left | KeyCode::Char('h') => Command::Play(Action::MoveLeft),
        KeyCode::Right | KeyCode::Char('l') => Command::Play(Action::MoveRight),
        KeyCode::Down | KeyCode::Char('j') => Command::Play(Action::SoftDrop),
        KeyCode::Char(' ') => Command::Play(Action::HardDrop),
        KeyCode::Up | KeyCode::Char('x') | KeyCode::Char('k') => {
            Command::Play(Action::Rotate(Spin::Cw))
        }
        KeyCode::Char('z') => Command::Play(Action::Rotate(Spin::Ccw)),
        KeyCode::Char('a') => Command::Play(Action::Rotate(Spin::Flip)),
        KeyCode::Char('c') | KeyCode::Tab => Command::Play(Action::Hold),
        KeyCode::Char('p') | KeyCode::Esc => Command::Play(Action::TogglePause),
        KeyCode::Char('r') => Command::Restart,
        KeyCode::Char('q') => Command::Quit,
        _ => return None,
    };
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn arrows_and_vim_keys_agree() {
        for (arrow, vim, expected) in [
            (KeyCode::Left, KeyCode::Char('h'), Action::MoveLeft),
            (KeyCode::Right, KeyCode::Char('l'), Action::MoveRight),
            (KeyCode::Down, KeyCode::Char('j'), Action::SoftDrop),
        ] {
            for code in [arrow, vim] {
                match to_command(press(code)) {
                    Some(Command::Play(action)) => assert_eq!(action, expected),
                    _ => panic!("{code:?} should map to {expected:?}"),
                }
            }
        }
    }

    #[test]
    fn rotation_keys_map_to_the_right_direction() {
        assert!(matches!(
            to_command(press(KeyCode::Char('x'))),
            Some(Command::Play(Action::Rotate(Spin::Cw)))
        ));
        assert!(matches!(
            to_command(press(KeyCode::Char('z'))),
            Some(Command::Play(Action::Rotate(Spin::Ccw)))
        ));
        assert!(matches!(
            to_command(press(KeyCode::Char('a'))),
            Some(Command::Play(Action::Rotate(Spin::Flip)))
        ));
    }

    #[test]
    fn quit_and_restart_are_recognised() {
        assert!(matches!(
            to_command(press(KeyCode::Char('q'))),
            Some(Command::Quit)
        ));
        assert!(matches!(
            to_command(press(KeyCode::Char('r'))),
            Some(Command::Restart)
        ));
        assert!(matches!(
            to_command(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Command::Quit)
        ));
    }

    #[test]
    fn ctrl_c_beats_the_plain_hold_binding() {
        // Plain `c` holds, but Ctrl-C must always quit.
        assert!(matches!(
            to_command(press(KeyCode::Char('c'))),
            Some(Command::Play(Action::Hold))
        ));
        assert!(matches!(
            to_command(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Command::Quit)
        ));
    }

    #[test]
    fn key_releases_are_ignored() {
        let mut key = press(KeyCode::Left);
        key.kind = KeyEventKind::Release;
        assert!(to_command(key).is_none());
    }

    #[test]
    fn unbound_keys_do_nothing() {
        assert!(to_command(press(KeyCode::Char('!'))).is_none());
        assert!(to_command(press(KeyCode::F(5))).is_none());
    }
}
