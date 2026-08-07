//! All drawing. The playfield is rendered two terminal columns per cell so
//! that blocks come out roughly square.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Widget};
use ratatui::Frame;

use crate::board::{HIDDEN_ROWS, VISIBLE_ROWS, WIDTH};
use crate::game::{Game, Phase};
use crate::tetromino::{ALL_KINDS, Kind};

/// Terminal columns used per board cell.
const CELL_W: u16 = 2;
const PLAYFIELD_W: u16 = WIDTH as u16 * CELL_W + 2;
const PLAYFIELD_H: u16 = VISIBLE_ROWS as u16 + 2;
const SIDE_W: u16 = 15;
const FOOTER_H: u16 = 2;
const MIN_W: u16 = SIDE_W * 2 + PLAYFIELD_W;
const MIN_H: u16 = PLAYFIELD_H + FOOTER_H;

const GRID_FG: Color = Color::DarkGray;
const FRAME_FG: Color = Color::Gray;

/// A board cell spans two terminal columns, so a glyph supplies both halves.
type Glyph = [&'static str; CELL_W as usize];

const BLOCK: Glyph = ["[", "]"];
const GHOST: Glyph = [":", ":"];
const FLASH: Glyph = ["#", "#"];
const GRID: Glyph = ["·", " "];

pub fn render(frame: &mut Frame, game: &Game) {
    let area = frame.area();
    if area.width < MIN_W || area.height < MIN_H {
        render_too_small(frame, area);
        return;
    }

    let root = center(area, MIN_W, MIN_H);
    let [top, footer] =
        Layout::vertical([Constraint::Length(PLAYFIELD_H), Constraint::Length(FOOTER_H)])
            .areas(root);
    let [left, middle, right] = Layout::horizontal([
        Constraint::Length(SIDE_W),
        Constraint::Length(PLAYFIELD_W),
        Constraint::Length(SIDE_W),
    ])
    .areas(top);

    render_left_panel(frame, left, game);
    frame.render_widget(Playfield { game }, middle);
    render_right_panel(frame, right, game);
    render_footer(frame, footer);

    if game.is_paused() {
        render_overlay(
            frame,
            middle,
            "PAUSED",
            &["p to resume".into(), "q to quit".into()],
            Color::Yellow,
        );
    } else if game.is_over() {
        render_overlay(
            frame,
            middle,
            "GAME OVER",
            &[
                format!("Score  {}", group_digits(game.score)),
                format!("Lines  {}", game.lines),
                format!("Level  {}", game.level),
                String::new(),
                "r to play again".into(),
                "q to quit".into(),
            ],
            Color::Red,
        );
    }
}

fn render_too_small(frame: &mut Frame, area: Rect) {
    let message = Paragraph::new(vec![
        Line::from("Terminal too small".bold().red()),
        Line::from(format!(
            "need {MIN_W}x{MIN_H}, have {}x{}",
            area.width, area.height
        )),
    ])
    .centered();
    frame.render_widget(message, center(area, area.width, 2));
}

/// Carve a `w` x `h` rectangle out of the middle of `area`, clamped to fit.
fn center(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn panel(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(FRAME_FG))
        .title(Line::from(format!(" {title} ")).bold())
}

// ---------------------------------------------------------------- playfield

/// The 10x20 well, its locked stack, the falling piece and its ghost.
struct Playfield<'a> {
    game: &'a Game,
}

impl Widget for Playfield<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .border_type(BorderType::Thick)
            .border_style(Style::new().fg(FRAME_FG));
        let inner = block.inner(area);
        block.render(area, buf);

        let game = self.game;
        let flashing: &[i16] = match &game.phase {
            Phase::Clearing { rows, .. } => rows,
            _ => &[],
        };

        for row in 0..VISIBLE_ROWS {
            let board_y = row + HIDDEN_ROWS;
            for col in 0..WIDTH {
                if flashing.contains(&board_y) {
                    paint_cell(buf, inner, col, row, Color::White, FLASH);
                } else if let Some(kind) = game.board.get(col, board_y) {
                    paint_cell(buf, inner, col, row, kind.color(), BLOCK);
                } else {
                    paint_cell(buf, inner, col, row, GRID_FG, GRID);
                }
            }
        }

        if matches!(game.phase, Phase::Falling | Phase::Paused) {
            // Ghost first, so the real piece wins wherever the two overlap.
            let ghost = game.ghost();
            for (x, y) in ghost.cells() {
                paint_board_cell(buf, inner, x, y, ghost.kind.color(), GHOST);
            }
            for (x, y) in game.piece.cells() {
                paint_board_cell(buf, inner, x, y, game.piece.kind.color(), BLOCK);
            }
        }
    }
}

/// Paint a cell addressed in board coordinates, skipping the hidden rows.
fn paint_board_cell(buf: &mut Buffer, inner: Rect, x: i16, y: i16, color: Color, glyph: Glyph) {
    let row = y - HIDDEN_ROWS;
    if (0..VISIBLE_ROWS).contains(&row) && (0..WIDTH).contains(&x) {
        paint_cell(buf, inner, x, row, color, glyph);
    }
}

/// Paint one board cell across its two terminal columns.
fn paint_cell(buf: &mut Buffer, inner: Rect, col: i16, row: i16, color: Color, glyph: Glyph) {
    let y = inner.y + row as u16;
    if y >= inner.y + inner.height {
        return;
    }
    for (offset, half) in glyph.iter().enumerate() {
        let x = inner.x + col as u16 * CELL_W + offset as u16;
        if x >= inner.x + inner.width {
            return;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(half).set_fg(color);
        }
    }
}

// ------------------------------------------------------------- side panels

fn render_left_panel(frame: &mut Frame, area: Rect, game: &Game) {
    let [hold_area, stats_area] =
        Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(area);

    let hold_block = panel("HOLD");
    let hold_inner = hold_block.inner(hold_area);
    frame.render_widget(hold_block, hold_area);
    if let Some(kind) = game.hold {
        // Faded while the hold is spent for the current piece.
        let mini = if game.can_hold() {
            MiniPiece::new(kind)
        } else {
            MiniPiece::faded(kind)
        };
        frame.render_widget(mini, hold_inner);
    }

    let stats_block = panel("STATS");
    let stats_inner = stats_block.inner(stats_area);
    frame.render_widget(stats_block, stats_area);

    let mut lines = vec![
        stat("SCORE", &group_digits(game.score), Color::White),
        stat("LEVEL", &game.level.to_string(), Color::Cyan),
        stat("LINES", &game.lines.to_string(), Color::Green),
        stat("TIME", &format_time(game.elapsed), Color::Blue),
        Line::from(""),
    ];

    // Per-kind counts, so the player can see how the bag has treated them.
    for (index, kind) in ALL_KINDS.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled(format!("{kind:?}     "), Style::new().fg(kind.color()).bold()),
            Span::styled(
                format!("{:>6}", game.stats[index]),
                Style::new().fg(Color::Gray),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), stats_inner);
}

fn stat(label: &str, value: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<6}"), Style::new().fg(Color::DarkGray)),
        Span::styled(
            format!("{value:>6}"),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn render_right_panel(frame: &mut Frame, area: Rect, game: &Game) {
    let previews = game.next_pieces();
    let next_h = (previews.len() as u16 * 2 + 2).min(area.height);
    let [next_area, event_area] =
        Layout::vertical([Constraint::Length(next_h), Constraint::Min(0)]).areas(area);

    let block = panel("NEXT");
    let inner = block.inner(next_area);
    frame.render_widget(block, next_area);

    for (slot, &kind) in previews.iter().enumerate() {
        let y = inner.y + slot as u16 * 2;
        let bottom = inner.y + inner.height;
        if y >= bottom {
            break;
        }
        // The first preview is the piece you actually get next: show it brightest.
        let mini = if slot == 0 {
            MiniPiece::new(kind)
        } else {
            MiniPiece::faded(kind)
        };
        let slot_area = Rect {
            y,
            height: (bottom - y).min(2),
            ..inner
        };
        frame.render_widget(mini, slot_area);
    }

    render_event(frame, event_area, game);
}

/// The banner describing the last scoring placement.
fn render_event(frame: &mut Frame, area: Rect, game: &Game) {
    let block = panel("LAST");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(event) = &game.last_event else {
        return;
    };

    let color = match event.headline.as_str() {
        "PERFECT CLEAR" => Color::Magenta,
        "TETRIS" => Color::Cyan,
        headline if headline.starts_with("T-SPIN") => Color::LightMagenta,
        _ => Color::White,
    };

    let mut lines = vec![Line::from(Span::styled(
        event.headline.clone(),
        Style::new().fg(color).bold(),
    ))];
    if event.back_to_back {
        lines.push(Line::from("BACK-TO-BACK".yellow()));
    }
    if event.combo > 0 {
        lines.push(Line::from(format!("{} COMBO", event.combo).green()));
    }
    lines.push(Line::from(
        Span::styled(
            format!("+{}", group_digits(event.points)),
            Style::new().fg(Color::DarkGray),
        ),
    ));

    frame.render_widget(Paragraph::new(lines), inner);
}

/// A piece drawn small, centred in a preview box.
struct MiniPiece {
    kind: Kind,
    style: Style,
}

impl MiniPiece {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            style: Style::new().fg(kind.color()),
        }
    }

    /// Muted, for a spent hold or the further-out previews.
    fn faded(kind: Kind) -> Self {
        Self {
            kind,
            style: Style::new()
                .fg(kind.color())
                .add_modifier(Modifier::DIM),
        }
    }
}

impl Widget for MiniPiece {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let cells = self.kind.cells(0);
        let min_x = cells.iter().map(|c| c.0).min().unwrap_or(0);
        let min_y = cells.iter().map(|c| c.1).min().unwrap_or(0);
        let width = cells.iter().map(|c| c.0 - min_x).max().unwrap_or(0) as u16 + 1;
        let pad = area.width.saturating_sub(width * CELL_W) / 2;

        for (dx, dy) in cells {
            let y = area.y + (dy - min_y) as u16;
            if y >= area.y + area.height {
                continue;
            }
            for (offset, half) in BLOCK.iter().enumerate() {
                let x = area.x + pad + (dx - min_x) as u16 * CELL_W + offset as u16;
                if x >= area.x + area.width {
                    continue;
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(half).set_style(self.style);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ chrome

/// Two rows of key hints, sized to fit the minimum layout width.
fn render_footer(frame: &mut Frame, area: Rect) {
    const ROWS: [&[(&str, &str)]; 2] = [
        &[("←→", "move"), ("↓", "soft"), ("space", "drop"), ("↑/x", "spin")],
        &[
            ("z", "ccw"),
            ("a", "180"),
            ("c", "hold"),
            ("p", "pause"),
            ("r", "restart"),
            ("q", "quit"),
        ],
    ];

    let lines = ROWS.iter().map(|keys| {
        let mut spans = Vec::with_capacity(keys.len() * 3);
        for &(key, label) in *keys {
            if !spans.is_empty() {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(key, Style::new().fg(Color::Cyan).bold()));
            spans.push(Span::styled(
                format!(" {label}"),
                Style::new().fg(Color::DarkGray),
            ));
        }
        Line::from(spans)
    });
    frame.render_widget(Paragraph::new(lines.collect::<Vec<_>>()).centered(), area);
}

fn render_overlay(frame: &mut Frame, area: Rect, title: &str, body: &[String], color: Color) {
    let height = body.len() as u16 + 4;
    let popup = center(area, area.width.saturating_sub(2), height);

    frame.render_widget(Clear, popup);
    let block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(color));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines = vec![
        Line::from(Span::styled(title.to_string(), Style::new().fg(color).bold())),
        Line::from(""),
    ];
    lines.extend(body.iter().map(|text| {
        Line::from(Span::styled(text.clone(), Style::new().fg(Color::Gray)))
    }));
    frame.render_widget(Paragraph::new(lines).centered(), inner);
}

// ----------------------------------------------------------------- helpers

/// `1234567` -> `1,234,567`
fn group_digits(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn format_time(elapsed: std::time::Duration) -> String {
    let total = elapsed.as_secs();
    format!("{}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn digits_are_grouped_in_threes() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(42), "42");
        assert_eq!(group_digits(1234), "1,234");
        assert_eq!(group_digits(1234567), "1,234,567");
    }

    #[test]
    fn time_is_minutes_and_padded_seconds() {
        use std::time::Duration;
        assert_eq!(format_time(Duration::from_secs(0)), "0:00");
        assert_eq!(format_time(Duration::from_secs(65)), "1:05");
        assert_eq!(format_time(Duration::from_secs(3600)), "60:00");
    }

    #[test]
    fn centering_keeps_the_rect_inside_its_parent() {
        let parent = Rect::new(0, 0, 80, 24);
        let child = center(parent, 50, 23);
        assert_eq!((child.width, child.height), (50, 23));
        assert!(child.x + child.width <= parent.width);
        assert!(child.y + child.height <= parent.height);
    }

    #[test]
    fn centering_clamps_to_a_small_parent() {
        let parent = Rect::new(0, 0, 10, 4);
        let child = center(parent, 50, 23);
        assert_eq!((child.width, child.height), (10, 4));
    }

    /// Rendering must not panic or overflow at any terminal size, including
    /// degenerate ones.
    #[test]
    fn renders_at_many_terminal_sizes() {
        let game = Game::new();
        for (w, h) in [(1, 1), (20, 10), (MIN_W, MIN_H), (80, 24), (200, 60)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| render(frame, &game)).unwrap();
        }
    }

    #[test]
    fn renders_the_pause_and_game_over_overlays() {
        use crate::game::Action;

        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();

        let mut game = Game::new();
        game.handle(Action::TogglePause);
        terminal.draw(|frame| render(frame, &game)).unwrap();

        let mut over = Game::new();
        over.phase = Phase::GameOver;
        terminal.draw(|frame| render(frame, &over)).unwrap();
    }

    /// Not an assertion — run with `--ignored --nocapture` to eyeball the layout.
    #[test]
    #[ignore]
    fn dump_layout() {
        use crate::game::Action;
        let mut game = Game::new();
        for _ in 0..6 {
            game.handle(Action::HardDrop);
            game.update(std::time::Duration::from_millis(200));
        }
        game.handle(Action::Hold);
        let mut terminal = Terminal::new(TestBackend::new(MIN_W, MIN_H)).unwrap();
        terminal.draw(|frame| render(frame, &game)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        for y in 0..buffer.area.height {
            let row: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            println!("{row}");
        }
    }

    #[test]
    fn the_playfield_shows_the_falling_piece() {
        let game = Game::new();
        let mut terminal = Terminal::new(TestBackend::new(MIN_W, MIN_H)).unwrap();
        terminal.draw(|frame| render(frame, &game)).unwrap();

        let buffer = terminal.backend().buffer().clone();
        let painted = buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == BLOCK[0])
            .count();
        assert!(painted > 0, "the falling piece and preview should be drawn");
    }
}
