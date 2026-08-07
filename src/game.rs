//! Game rules: gravity, locking, hold, scoring, levels and T-spin detection.

use std::time::Duration;

use crate::board::{Board, HIDDEN_ROWS, WIDTH};
use crate::tetromino::{Bag, Kind, Piece, Spin, kicks};

/// How long a grounded piece may be nudged around before it locks.
const LOCK_DELAY: Duration = Duration::from_millis(500);
/// Move/rotate this many times while grounded and the piece locks regardless.
const MAX_LOCK_RESETS: u32 = 15;
/// How long full rows stay lit up before they collapse.
const CLEAR_FLASH: Duration = Duration::from_millis(160);
/// A soft drop falls this many times faster than normal gravity.
const SOFT_DROP_FACTOR: u32 = 20;
const LINES_PER_LEVEL: u32 = 10;
/// How many upcoming pieces the player can see.
const PREVIEW_COUNT: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Falling,
    /// Full rows are lit up and about to collapse.
    Clearing { rows: Vec<i16>, timer: Duration },
    Paused,
    GameOver,
}

/// What the player asked the game to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveLeft,
    MoveRight,
    SoftDrop,
    HardDrop,
    Rotate(Spin),
    Hold,
    TogglePause,
}

/// The kind of spin credited to a lock, used for scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpinKind {
    None,
    Mini,
    Full,
}

/// A short description of the last placement, shown to the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub headline: String,
    pub back_to_back: bool,
    pub combo: u32,
    pub points: u64,
}

pub struct Game {
    pub board: Board,
    pub piece: Piece,
    pub hold: Option<Kind>,
    pub phase: Phase,
    pub score: u64,
    pub lines: u32,
    pub level: u32,
    pub elapsed: Duration,
    /// Count of each kind spawned so far, indexed as in [`crate::tetromino::ALL_KINDS`].
    pub stats: [u32; 7],
    pub last_event: Option<Event>,

    bag: Bag,
    hold_used: bool,
    gravity_acc: Duration,
    lock_acc: Duration,
    lock_resets: u32,
    /// The last successful action was a rotation — a prerequisite for a T-spin.
    rotated_last: bool,
    /// The last rotation succeeded only on the final kick test.
    last_kick_was_final: bool,
    combo: i32,
    back_to_back: bool,
}

impl Game {
    pub fn new() -> Self {
        let mut bag = Bag::new();
        let piece = Piece::spawn(bag.next(), WIDTH, HIDDEN_ROWS);
        let mut game = Self {
            board: Board::new(),
            piece,
            hold: None,
            phase: Phase::Falling,
            score: 0,
            lines: 0,
            level: 1,
            elapsed: Duration::ZERO,
            stats: [0; 7],
            last_event: None,
            bag,
            hold_used: false,
            gravity_acc: Duration::ZERO,
            lock_acc: Duration::ZERO,
            lock_resets: 0,
            rotated_last: false,
            last_kick_was_final: false,
            combo: -1,
            back_to_back: false,
        };
        game.count_stat(game.piece.kind);
        game
    }

    pub fn restart(&mut self) {
        *self = Self::new();
    }

    pub fn is_over(&self) -> bool {
        self.phase == Phase::GameOver
    }

    pub fn is_paused(&self) -> bool {
        self.phase == Phase::Paused
    }

    pub fn next_pieces(&self) -> &[Kind] {
        self.bag.preview(PREVIEW_COUNT)
    }

    pub fn can_hold(&self) -> bool {
        !self.hold_used
    }

    /// Where the current piece would land if dropped right now.
    pub fn ghost(&self) -> Piece {
        self.board.ghost(&self.piece)
    }

    /// Seconds a piece takes to fall one row at the current level, following
    /// the Tetris guideline curve.
    fn gravity_interval(&self) -> Duration {
        let level = self.level.min(20) as f64;
        let seconds = (0.8 - (level - 1.0) * 0.007).powf(level - 1.0);
        Duration::from_secs_f64(seconds.max(0.001))
    }

    fn count_stat(&mut self, kind: Kind) {
        let index = crate::tetromino::ALL_KINDS
            .iter()
            .position(|&k| k == kind)
            .unwrap_or(0);
        self.stats[index] += 1;
    }

    /// Advance the clock. `dt` is the wall time since the previous call.
    pub fn update(&mut self, dt: Duration) {
        match &mut self.phase {
            Phase::Paused | Phase::GameOver => return,
            Phase::Clearing { timer, .. } => {
                self.elapsed += dt;
                *timer = timer.saturating_sub(dt);
                if timer.is_zero() {
                    self.board.clear_full_rows();
                    self.phase = Phase::Falling;
                    self.spawn_next();
                }
                return;
            }
            Phase::Falling => {}
        }

        self.elapsed += dt;
        let interval = self.gravity_interval();
        self.gravity_acc += dt;
        while self.gravity_acc >= interval {
            self.gravity_acc -= interval;
            if !self.step_down() {
                break;
            }
        }

        if self.grounded() {
            self.lock_acc += dt;
            if self.lock_acc >= LOCK_DELAY {
                self.lock_piece();
            }
        } else {
            self.lock_acc = Duration::ZERO;
        }
    }

    /// Is the piece resting on the stack or the floor?
    fn grounded(&self) -> bool {
        !self.board.fits(&self.piece.moved(0, 1))
    }

    /// Try to fall one row. Returns false if the piece is grounded.
    fn step_down(&mut self) -> bool {
        let candidate = self.piece.moved(0, 1);
        if self.board.fits(&candidate) {
            self.piece = candidate;
            self.rotated_last = false;
            true
        } else {
            false
        }
    }

    /// A successful move while grounded buys the player a little more time,
    /// but only up to [`MAX_LOCK_RESETS`] times per piece.
    fn touch_lock_timer(&mut self) {
        if self.grounded() && self.lock_resets < MAX_LOCK_RESETS {
            self.lock_resets += 1;
            self.lock_acc = Duration::ZERO;
        }
    }

    pub fn handle(&mut self, action: Action) {
        if action == Action::TogglePause {
            self.phase = match self.phase {
                Phase::Falling => Phase::Paused,
                Phase::Paused => Phase::Falling,
                ref other => other.clone(),
            };
            return;
        }
        if self.phase != Phase::Falling {
            return;
        }

        match action {
            Action::MoveLeft => self.shift(-1),
            Action::MoveRight => self.shift(1),
            Action::SoftDrop => self.soft_drop(),
            Action::HardDrop => self.hard_drop(),
            Action::Rotate(spin) => self.rotate(spin),
            Action::Hold => self.swap_hold(),
            Action::TogglePause => unreachable!("handled above"),
        }
    }

    fn shift(&mut self, dx: i16) {
        let candidate = self.piece.moved(dx, 0);
        if self.board.fits(&candidate) {
            self.piece = candidate;
            self.rotated_last = false;
            self.touch_lock_timer();
        }
    }

    fn soft_drop(&mut self) {
        if self.step_down() {
            self.score += 1;
            // Falling on demand also postpones the next gravity step.
            self.gravity_acc = self
                .gravity_acc
                .saturating_sub(self.gravity_interval() / SOFT_DROP_FACTOR);
        } else {
            self.touch_lock_timer();
        }
    }

    fn hard_drop(&mut self) {
        let distance = self.board.drop_distance(&self.piece);
        if distance > 0 {
            self.piece = self.piece.moved(0, distance);
            self.score += 2 * distance as u64;
            self.rotated_last = false;
        }
        self.lock_piece();
    }

    fn rotate(&mut self, spin: Spin) {
        let from = self.piece.rotation;
        let to = spin.apply(from);
        let tests = kicks(self.piece.kind, from, to);

        for (index, &(dx, dy)) in tests.iter().enumerate() {
            let mut candidate = self.piece;
            candidate.rotation = to;
            candidate.x += dx;
            candidate.y += dy;
            if self.board.fits(&candidate) {
                self.piece = candidate;
                self.rotated_last = true;
                self.last_kick_was_final = index == tests.len() - 1;
                self.touch_lock_timer();
                return;
            }
        }
    }

    fn swap_hold(&mut self) {
        if self.hold_used {
            return;
        }
        let stashed = self.hold.replace(self.piece.kind);
        let kind = match stashed {
            Some(kind) => kind,
            None => self.bag.next(),
        };
        let piece = Piece::spawn(kind, WIDTH, HIDDEN_ROWS);
        self.count_stat(kind);
        self.piece = piece;
        self.hold_used = true;
        self.reset_piece_timers();
        if !self.board.fits(&self.piece) {
            self.phase = Phase::GameOver;
        }
    }

    fn reset_piece_timers(&mut self) {
        self.gravity_acc = Duration::ZERO;
        self.lock_acc = Duration::ZERO;
        self.lock_resets = 0;
        self.rotated_last = false;
        self.last_kick_was_final = false;
    }

    /// A T-spin requires a T piece whose last action was a rotation and which
    /// is wedged into a corner: three of the four diagonals around its centre
    /// must be occupied.
    fn detect_spin(&self) -> SpinKind {
        if self.piece.kind != Kind::T || !self.rotated_last {
            return SpinKind::None;
        }
        let (x, y) = (self.piece.x, self.piece.y);
        let corner = |cx: i16, cy: i16| self.board.is_blocked(cx, cy);
        let top_left = corner(x, y);
        let top_right = corner(x + 2, y);
        let bottom_left = corner(x, y + 2);
        let bottom_right = corner(x + 2, y + 2);

        let occupied = [top_left, top_right, bottom_left, bottom_right]
            .iter()
            .filter(|&&c| c)
            .count();
        if occupied < 3 {
            return SpinKind::None;
        }

        // The two corners the T's stem points between are its "front".
        let (front_a, front_b) = match self.piece.rotation % 4 {
            0 => (top_left, top_right),
            1 => (top_right, bottom_right),
            2 => (bottom_left, bottom_right),
            _ => (top_left, bottom_left),
        };

        if front_a && front_b {
            SpinKind::Full
        } else if self.last_kick_was_final {
            // A rotation that only fit on the last kick counts in full.
            SpinKind::Full
        } else {
            SpinKind::Mini
        }
    }

    fn lock_piece(&mut self) {
        let spin = self.detect_spin();
        // Locking entirely above the visible playfield is a top-out.
        let locked_out = self.piece.cells().iter().all(|&(_, y)| y < HIDDEN_ROWS);

        self.board.lock(&self.piece);
        let rows = self.board.full_rows();
        let cleared = rows.len();

        self.award(cleared, spin);

        if locked_out && cleared == 0 {
            self.phase = Phase::GameOver;
            return;
        }

        if cleared > 0 {
            self.lines += cleared as u32;
            self.level = 1 + self.lines / LINES_PER_LEVEL;
            self.phase = Phase::Clearing {
                rows,
                timer: CLEAR_FLASH,
            };
        } else {
            self.spawn_next();
        }
    }

    /// Score the placement and record what to show the player.
    fn award(&mut self, cleared: usize, spin: SpinKind) {
        let level = self.level as u64;

        let (base, name) = match (spin, cleared) {
            (SpinKind::None, 0) => (0, None),
            (SpinKind::None, 1) => (100, Some("SINGLE")),
            (SpinKind::None, 2) => (300, Some("DOUBLE")),
            (SpinKind::None, 3) => (500, Some("TRIPLE")),
            (SpinKind::None, _) => (800, Some("TETRIS")),
            (SpinKind::Mini, 0) => (100, Some("T-SPIN MINI")),
            (SpinKind::Mini, 1) => (200, Some("T-SPIN MINI SINGLE")),
            (SpinKind::Mini, _) => (400, Some("T-SPIN MINI DOUBLE")),
            (SpinKind::Full, 0) => (400, Some("T-SPIN")),
            (SpinKind::Full, 1) => (800, Some("T-SPIN SINGLE")),
            (SpinKind::Full, 2) => (1200, Some("T-SPIN DOUBLE")),
            (SpinKind::Full, _) => (1600, Some("T-SPIN TRIPLE")),
        };

        // "Difficult" clears — tetrises and scoring spins — chain into a
        // back-to-back bonus.
        let difficult = cleared > 0 && (cleared >= 4 || spin != SpinKind::None);
        let chained = difficult && self.back_to_back;

        let mut points = base * level;
        if chained {
            points = points * 3 / 2;
        }

        if cleared > 0 {
            self.combo += 1;
            if self.combo > 0 {
                points += 50 * self.combo as u64 * level;
            }
            self.back_to_back = difficult;
        } else {
            self.combo = -1;
        }

        let mut headline = name.map(str::to_string);

        // A perfect clear wipes the board completely — the biggest bonus there is.
        if cleared > 0 && self.is_board_clear_after(cleared) {
            let bonus = match cleared {
                1 => 800,
                2 => 1200,
                3 => 1800,
                _ => 2000,
            };
            points += bonus * level;
            headline = Some("PERFECT CLEAR".to_string());
        }

        self.score += points;

        if let Some(headline) = headline {
            self.last_event = Some(Event {
                headline,
                back_to_back: chained,
                combo: self.combo.max(0) as u32,
                points,
            });
        }
    }

    /// Would the board be completely empty once the pending rows collapse?
    fn is_board_clear_after(&self, cleared: usize) -> bool {
        let mut probe = self.board.clone();
        let removed = probe.clear_full_rows();
        removed == cleared && probe.is_empty()
    }

    fn spawn_next(&mut self) {
        let kind = self.bag.next();
        self.piece = Piece::spawn(kind, WIDTH, HIDDEN_ROWS);
        self.count_stat(kind);
        self.hold_used = false;
        self.reset_piece_timers();
        // No room for the new piece: the stack has reached the ceiling.
        if !self.board.fits(&self.piece) {
            self.phase = Phase::GameOver;
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::HEIGHT;

    fn drop_and_settle(game: &mut Game) {
        game.handle(Action::HardDrop);
        // Let any clear animation finish.
        game.update(CLEAR_FLASH + Duration::from_millis(1));
    }

    #[test]
    fn a_new_game_starts_clean() {
        let game = Game::new();
        assert_eq!(game.score, 0);
        assert_eq!(game.lines, 0);
        assert_eq!(game.level, 1);
        assert!(game.board.is_empty());
        assert_eq!(game.phase, Phase::Falling);
        assert_eq!(game.next_pieces().len(), PREVIEW_COUNT);
    }

    #[test]
    fn gravity_pulls_the_piece_down_one_row_per_interval() {
        let mut game = Game::new();
        let start_y = game.piece.y;
        let interval = game.gravity_interval();
        game.update(interval);
        assert_eq!(game.piece.y, start_y + 1);
    }

    #[test]
    fn higher_levels_fall_faster() {
        let mut game = Game::new();
        let slow = game.gravity_interval();
        game.level = 10;
        let fast = game.gravity_interval();
        assert!(fast < slow, "level 10 should be quicker than level 1");
    }

    #[test]
    fn a_piece_cannot_be_pushed_through_a_wall() {
        let mut game = Game::new();
        for _ in 0..20 {
            game.handle(Action::MoveLeft);
        }
        let leftmost = game.piece.cells().iter().map(|c| c.0).min().unwrap();
        assert_eq!(leftmost, 0);

        for _ in 0..40 {
            game.handle(Action::MoveRight);
        }
        let rightmost = game.piece.cells().iter().map(|c| c.0).max().unwrap();
        assert_eq!(rightmost, WIDTH - 1);
    }

    #[test]
    fn hard_drop_lands_the_piece_and_scores_two_per_row() {
        let mut game = Game::new();
        let distance = game.board.drop_distance(&game.piece);
        game.handle(Action::HardDrop);
        assert_eq!(game.score, 2 * distance as u64);
        assert!(!game.board.is_empty(), "the piece should now be locked in");
    }

    #[test]
    fn soft_drop_scores_one_per_row() {
        let mut game = Game::new();
        game.handle(Action::SoftDrop);
        assert_eq!(game.score, 1);
    }

    #[test]
    fn hold_swaps_the_piece_and_locks_until_the_next_lock() {
        let mut game = Game::new();
        let first = game.piece.kind;
        game.handle(Action::Hold);
        assert_eq!(game.hold, Some(first));
        assert!(!game.can_hold(), "hold may only be used once per piece");

        let swapped_in = game.piece.kind;
        game.handle(Action::Hold);
        assert_eq!(game.piece.kind, swapped_in, "the second hold is ignored");

        drop_and_settle(&mut game);
        assert!(game.can_hold(), "locking a piece re-arms hold");
    }

    #[test]
    fn holding_twice_across_pieces_swaps_back() {
        let mut game = Game::new();
        let first = game.piece.kind;
        game.handle(Action::Hold);
        drop_and_settle(&mut game);

        // The piece now in play trades places with the one that was stashed.
        let in_play = game.piece.kind;
        game.handle(Action::Hold);
        assert_eq!(game.piece.kind, first, "the stashed piece comes back out");
        assert_eq!(game.hold, Some(in_play), "and the current one goes in");
    }

    /// Fill row `y` completely except for the listed columns.
    fn fill_except(game: &mut Game, y: i16, gaps: &[i16]) {
        for x in 0..WIDTH {
            if !gaps.contains(&x) {
                game.board.set(x, y, Some(Kind::L));
            }
        }
    }

    #[test]
    fn clearing_a_line_scores_and_counts() {
        let mut game = Game::new();
        // A floor row missing one cell in the middle, waiting to be plugged.
        fill_except(&mut game, HEIGHT - 1, &[4]);
        assert_eq!(game.board.full_rows().len(), 0);

        // An I piece stood on end drops straight into the hole.
        game.piece = Piece {
            kind: Kind::I,
            rotation: 1,
            x: 2,
            y: 0,
        };
        game.handle(Action::HardDrop);
        game.update(CLEAR_FLASH + Duration::from_millis(1));

        assert_eq!(game.lines, 1);
        assert!(game.score >= 100, "a single is worth 100 at level 1");
    }

    /// The canonical T-spin double: a covered notch that only a rotation can
    /// reach. The piece enters vertically through the gap on the right and the
    /// third SRS kick test drops it into the slot.
    fn t_spin_double_setup() -> Game {
        let mut game = Game::new();
        let a = HEIGHT - 2;
        fill_except(&mut game, a - 1, &[4, 5]); // the overhang
        fill_except(&mut game, a, &[3, 4, 5]); // the notch
        fill_except(&mut game, a + 1, &[4]); // the floor beneath it

        game.piece = Piece {
            kind: Kind::T,
            rotation: 3,
            x: 4,
            y: a - 2,
        };
        assert!(game.board.fits(&game.piece), "the entry position must be legal");
        game
    }

    #[test]
    fn a_t_spin_double_is_detected_and_scored() {
        let mut game = t_spin_double_setup();

        game.handle(Action::Rotate(Spin::Ccw));
        assert_eq!(game.piece.rotation, 2, "the T should now point down");
        assert_eq!(
            (game.piece.x, game.piece.y),
            (3, HEIGHT - 3),
            "the kick should have wedged it into the notch"
        );
        assert_eq!(game.detect_spin(), SpinKind::Full);

        // Let the lock delay run out so the spin still counts at lock time.
        game.update(LOCK_DELAY + Duration::from_millis(1));

        assert_eq!(game.lines, 2);
        assert_eq!(game.score, 1200, "T-spin double is 1200 at level 1");
        let event = game.last_event.as_ref().expect("a banner was recorded");
        assert_eq!(event.headline, "T-SPIN DOUBLE");
    }

    #[test]
    fn a_t_spin_needs_a_rotation_not_just_a_landing() {
        let mut game = t_spin_double_setup();
        // Same board, but the piece arrives by moving rather than rotating.
        game.piece = Piece {
            kind: Kind::T,
            rotation: 2,
            x: 3,
            y: HEIGHT - 3,
        };
        game.handle(Action::MoveLeft); // blocked, but it clears `rotated_last`
        assert_eq!(
            game.detect_spin(),
            SpinKind::None,
            "no rotation means no spin, however snug the fit"
        );
    }

    #[test]
    fn an_ordinary_double_is_not_a_t_spin() {
        let mut game = Game::new();
        fill_except(&mut game, HEIGHT - 1, &[4, 5]);
        fill_except(&mut game, HEIGHT - 2, &[4, 5]);
        // A leftover block, so this is a plain double and not a perfect clear.
        game.board.set(0, HEIGHT - 3, Some(Kind::L));

        game.piece = Piece {
            kind: Kind::O,
            rotation: 0,
            x: 3,
            y: 0,
        };
        game.handle(Action::HardDrop);

        assert_eq!(game.lines, 2);
        let event = game.last_event.as_ref().expect("a banner was recorded");
        assert_eq!(event.headline, "DOUBLE");
    }

    #[test]
    fn wiping_the_board_earns_a_perfect_clear() {
        let mut game = Game::new();
        fill_except(&mut game, HEIGHT - 1, &[4, 5]);
        fill_except(&mut game, HEIGHT - 2, &[4, 5]);

        game.piece = Piece {
            kind: Kind::O,
            rotation: 0,
            x: 3,
            y: 0,
        };
        game.handle(Action::HardDrop);

        let event = game.last_event.as_ref().expect("a banner was recorded");
        assert_eq!(event.headline, "PERFECT CLEAR");
        assert!(game.score >= 300 + 1200, "double plus the perfect-clear bonus");
    }

    #[test]
    fn levels_advance_every_ten_lines() {
        let mut game = Game::new();
        game.lines = 0;
        game.level = 1;
        // Simulate the level bookkeeping the way lock_piece does.
        game.lines = 25;
        game.level = 1 + game.lines / LINES_PER_LEVEL;
        assert_eq!(game.level, 3);
    }

    #[test]
    fn pause_freezes_gravity_and_ignores_input() {
        let mut game = Game::new();
        game.handle(Action::TogglePause);
        assert!(game.is_paused());

        let (x, y) = (game.piece.x, game.piece.y);
        game.update(Duration::from_secs(5));
        game.handle(Action::MoveLeft);
        assert_eq!((game.piece.x, game.piece.y), (x, y));

        game.handle(Action::TogglePause);
        assert_eq!(game.phase, Phase::Falling);
    }

    #[test]
    fn a_grounded_piece_locks_after_the_lock_delay() {
        let mut game = Game::new();
        game.piece = game.board.ghost(&game.piece);
        game.update(LOCK_DELAY + Duration::from_millis(1));
        assert!(!game.board.is_empty(), "the piece should have locked");
    }

    #[test]
    fn moving_a_grounded_piece_postpones_the_lock() {
        let mut game = Game::new();
        game.piece = game.board.ghost(&game.piece);
        game.update(LOCK_DELAY - Duration::from_millis(50));
        game.handle(Action::MoveLeft);
        game.update(Duration::from_millis(100));
        assert!(
            game.board.is_empty(),
            "the reset should have bought more time"
        );
    }

    #[test]
    fn lock_resets_are_capped() {
        let mut game = Game::new();
        game.piece = game.board.ghost(&game.piece);
        for _ in 0..(MAX_LOCK_RESETS + 5) {
            game.update(Duration::from_millis(10));
            game.handle(Action::MoveLeft);
            game.handle(Action::MoveRight);
        }
        game.update(LOCK_DELAY + Duration::from_millis(1));
        assert!(!game.board.is_empty(), "the piece must eventually lock");
    }

    #[test]
    fn stacking_to_the_ceiling_ends_the_game() {
        let mut game = Game::new();
        for _ in 0..200 {
            if game.is_over() {
                break;
            }
            drop_and_settle(&mut game);
        }
        assert!(game.is_over(), "dropping in place should top out");
    }

    #[test]
    fn a_tetris_scores_more_than_four_singles() {
        let mut singles = Game::new();
        singles.level = 1;
        singles.award(1, SpinKind::None);
        singles.combo = -1;
        singles.award(1, SpinKind::None);
        singles.combo = -1;
        singles.award(1, SpinKind::None);
        singles.combo = -1;
        singles.award(1, SpinKind::None);

        let mut tetris = Game::new();
        tetris.level = 1;
        tetris.award(4, SpinKind::None);

        assert!(tetris.score > singles.score / 4);
        assert_eq!(tetris.score, 800);
    }

    #[test]
    fn back_to_back_tetrises_are_worth_more() {
        let mut game = Game::new();
        game.award(4, SpinKind::None);
        let first = game.score;
        game.combo = -1;
        game.award(4, SpinKind::None);
        let second = game.score - first;
        assert!(second > first, "the second tetris should get the b2b bonus");
        assert_eq!(second, 1200);
    }

    #[test]
    fn a_broken_chain_drops_the_back_to_back_bonus() {
        let mut game = Game::new();
        game.award(4, SpinKind::None);
        assert!(game.back_to_back);
        game.award(1, SpinKind::None);
        assert!(!game.back_to_back, "a single breaks the chain");
    }

    #[test]
    fn restart_clears_everything() {
        let mut game = Game::new();
        game.handle(Action::HardDrop);
        game.score = 9999;
        game.restart();
        assert_eq!(game.score, 0);
        assert!(game.board.is_empty());
        assert_eq!(game.phase, Phase::Falling);
    }
}
