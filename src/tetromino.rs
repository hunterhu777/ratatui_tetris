//! Piece shapes, colours and Super Rotation System (SRS) kick tables.

use rand::seq::SliceRandom;
use ratatui::style::Color;

/// The seven one-sided tetrominoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

pub const ALL_KINDS: [Kind; 7] = [
    Kind::I,
    Kind::O,
    Kind::T,
    Kind::S,
    Kind::Z,
    Kind::J,
    Kind::L,
];

/// Cell offsets `(x, y)` for each kind in each of its four rotation states.
/// `y` grows downwards, matching the board's row indexing.
type Shape = [[(i8, i8); 4]; 4];

const I_SHAPE: Shape = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(2, 0), (2, 1), (2, 2), (2, 3)],
    [(0, 2), (1, 2), (2, 2), (3, 2)],
    [(1, 0), (1, 1), (1, 2), (1, 3)],
];

const O_SHAPE: Shape = [
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
];

const T_SHAPE: Shape = [
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (1, 1), (2, 1), (1, 2)],
    [(0, 1), (1, 1), (2, 1), (1, 2)],
    [(1, 0), (0, 1), (1, 1), (1, 2)],
];

const S_SHAPE: Shape = [
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(1, 0), (1, 1), (2, 1), (2, 2)],
    [(1, 1), (2, 1), (0, 2), (1, 2)],
    [(0, 0), (0, 1), (1, 1), (1, 2)],
];

const Z_SHAPE: Shape = [
    [(0, 0), (1, 0), (1, 1), (2, 1)],
    [(2, 0), (1, 1), (2, 1), (1, 2)],
    [(0, 1), (1, 1), (1, 2), (2, 2)],
    [(1, 0), (0, 1), (1, 1), (0, 2)],
];

const J_SHAPE: Shape = [
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (1, 1), (1, 2)],
    [(0, 1), (1, 1), (2, 1), (2, 2)],
    [(1, 0), (1, 1), (0, 2), (1, 2)],
];

const L_SHAPE: Shape = [
    [(2, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (1, 1), (1, 2), (2, 2)],
    [(0, 1), (1, 1), (2, 1), (0, 2)],
    [(0, 0), (1, 0), (1, 1), (1, 2)],
];

impl Kind {
    fn shape(self) -> &'static Shape {
        match self {
            Kind::I => &I_SHAPE,
            Kind::O => &O_SHAPE,
            Kind::T => &T_SHAPE,
            Kind::S => &S_SHAPE,
            Kind::Z => &Z_SHAPE,
            Kind::J => &J_SHAPE,
            Kind::L => &L_SHAPE,
        }
    }

    /// The four occupied cells for a rotation state, relative to the piece origin.
    pub fn cells(self, rotation: u8) -> [(i8, i8); 4] {
        self.shape()[rotation as usize % 4]
    }

    pub fn color(self) -> Color {
        match self {
            Kind::I => Color::Cyan,
            Kind::O => Color::Yellow,
            Kind::T => Color::Magenta,
            Kind::S => Color::Green,
            Kind::Z => Color::Red,
            Kind::J => Color::Blue,
            Kind::L => Color::LightRed,
        }
    }

    /// Width of the rotation bounding box: 4 for I, 2 for O, 3 for the rest.
    pub fn box_size(self) -> i16 {
        match self {
            Kind::I => 4,
            Kind::O => 4,
            _ => 3,
        }
    }
}

/// Rotation direction requested by the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spin {
    Cw,
    Ccw,
    Flip,
}

impl Spin {
    pub fn apply(self, rotation: u8) -> u8 {
        match self {
            Spin::Cw => (rotation + 1) % 4,
            Spin::Ccw => (rotation + 3) % 4,
            Spin::Flip => (rotation + 2) % 4,
        }
    }
}

/// SRS kick tables. Offsets are stored in screen coordinates (`y` downwards),
/// i.e. the vertical component of the published tables is already negated.
const KICKS_JLSTZ: [[(i16, i16); 5]; 8] = [
    // 0 -> R
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
    // R -> 0
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    // R -> 2
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    // 2 -> R
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
    // 2 -> L
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
    // L -> 2
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
    // L -> 0
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
    // 0 -> L
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
];

const KICKS_I: [[(i16, i16); 5]; 8] = [
    // 0 -> R
    [(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)],
    // R -> 0
    [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)],
    // R -> 2
    [(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)],
    // 2 -> R
    [(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)],
    // 2 -> L
    [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)],
    // L -> 2
    [(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)],
    // L -> 0
    [(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)],
    // 0 -> L
    [(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)],
];

/// Candidate offsets to try, in order, when rotating `from` -> `to`.
pub fn kicks(kind: Kind, from: u8, to: u8) -> &'static [(i16, i16)] {
    // The O piece is rotationally symmetric, so it never needs to kick.
    if kind == Kind::O {
        return &[(0, 0)];
    }
    let row = match (from % 4, to % 4) {
        (0, 1) => 0,
        (1, 0) => 1,
        (1, 2) => 2,
        (2, 1) => 3,
        (2, 3) => 4,
        (3, 2) => 5,
        (3, 0) => 6,
        (0, 3) => 7,
        // 180° spins are not part of the original SRS; allow the simple case
        // plus a nudge up so a flip against the floor can still succeed.
        _ => return &[(0, 0), (0, -1), (1, 0), (-1, 0)],
    };
    if kind == Kind::I {
        &KICKS_I[row]
    } else {
        &KICKS_JLSTZ[row]
    }
}

/// A piece in play: a kind, a rotation state and a board position.
#[derive(Debug, Clone, Copy)]
pub struct Piece {
    pub kind: Kind,
    pub rotation: u8,
    /// Board column of the bounding box's left edge.
    pub x: i16,
    /// Board row of the bounding box's top edge.
    pub y: i16,
}

impl Piece {
    /// A piece at its spawn position: horizontally centred and straddling the
    /// ceiling, so its lower half is visible the moment it appears.
    pub fn spawn(kind: Kind, board_width: i16, hidden_rows: i16) -> Self {
        let x = (board_width - kind.box_size()) / 2;
        Self {
            kind,
            rotation: 0,
            x,
            y: hidden_rows - 1,
        }
    }

    /// Absolute board coordinates of the piece's four cells.
    pub fn cells(&self) -> [(i16, i16); 4] {
        let mut out = [(0i16, 0i16); 4];
        for (slot, (dx, dy)) in out.iter_mut().zip(self.kind.cells(self.rotation)) {
            *slot = (self.x + dx as i16, self.y + dy as i16);
        }
        out
    }

    pub fn moved(&self, dx: i16, dy: i16) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
}

/// The "7-bag" randomiser: every permutation of the seven kinds is dealt
/// before any kind repeats, which is what modern Tetris guidelines specify.
pub struct Bag {
    queue: Vec<Kind>,
    rng: rand::rngs::ThreadRng,
}

impl Bag {
    pub fn new() -> Self {
        let mut bag = Self {
            queue: Vec::with_capacity(14),
            rng: rand::rng(),
        };
        bag.refill();
        bag.refill();
        bag
    }

    fn refill(&mut self) {
        let mut batch = ALL_KINDS;
        batch.shuffle(&mut self.rng);
        self.queue.extend_from_slice(&batch);
    }

    pub fn next(&mut self) -> Kind {
        if self.queue.len() <= 7 {
            self.refill();
        }
        self.queue.remove(0)
    }

    /// Peek at the upcoming pieces without consuming them.
    pub fn preview(&self, count: usize) -> &[Kind] {
        &self.queue[..count.min(self.queue.len())]
    }
}

impl Default for Bag {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rotation_has_four_distinct_cells() {
        for kind in ALL_KINDS {
            for rotation in 0..4u8 {
                let cells = kind.cells(rotation);
                let mut seen = cells.to_vec();
                seen.sort_unstable();
                seen.dedup();
                assert_eq!(seen.len(), 4, "{kind:?} r{rotation} has duplicate cells");
            }
        }
    }

    #[test]
    fn rotations_stay_inside_the_bounding_box() {
        for kind in ALL_KINDS {
            let size = kind.box_size() as i8;
            for rotation in 0..4u8 {
                for (x, y) in kind.cells(rotation) {
                    assert!(
                        (0..size).contains(&x) && (0..size).contains(&y),
                        "{kind:?} r{rotation} cell ({x},{y}) escapes its {size}x{size} box"
                    );
                }
            }
        }
    }

    #[test]
    fn rotating_four_times_returns_to_start() {
        for kind in ALL_KINDS {
            let mut r = 0u8;
            for _ in 0..4 {
                r = Spin::Cw.apply(r);
            }
            assert_eq!(r, 0, "{kind:?} did not cycle back to spawn rotation");
        }
    }

    #[test]
    fn bag_deals_each_kind_once_per_seven() {
        let mut bag = Bag::new();
        for _ in 0..10 {
            let mut batch: Vec<Kind> = (0..7).map(|_| bag.next()).collect();
            batch.sort_unstable_by_key(|k| format!("{k:?}"));
            batch.dedup();
            assert_eq!(batch.len(), 7, "a bag of seven repeated a kind");
        }
    }

    #[test]
    fn preview_matches_the_pieces_that_are_dealt() {
        let mut bag = Bag::new();
        let expected: Vec<Kind> = bag.preview(5).to_vec();
        let actual: Vec<Kind> = (0..5).map(|_| bag.next()).collect();
        assert_eq!(expected, actual);
    }
}
