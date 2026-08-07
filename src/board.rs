//! The playfield: a fixed grid of locked cells plus collision and line-clear logic.

use crate::tetromino::{Kind, Piece};

pub const WIDTH: i16 = 10;
/// Rows above the visible playfield, where pieces spawn and rotate.
pub const HIDDEN_ROWS: i16 = 4;
pub const VISIBLE_ROWS: i16 = 20;
pub const HEIGHT: i16 = HIDDEN_ROWS + VISIBLE_ROWS;

/// A locked-in playfield. `None` is empty; `Some(kind)` keeps the colour of
/// whichever piece came to rest there.
#[derive(Debug, Clone)]
pub struct Board {
    cells: Vec<Option<Kind>>,
}

impl Board {
    pub fn new() -> Self {
        Self {
            cells: vec![None; (WIDTH * HEIGHT) as usize],
        }
    }

    fn index(x: i16, y: i16) -> usize {
        (y * WIDTH + x) as usize
    }

    pub fn get(&self, x: i16, y: i16) -> Option<Kind> {
        if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
            self.cells[Self::index(x, y)]
        } else {
            None
        }
    }

    pub(crate) fn set(&mut self, x: i16, y: i16, kind: Option<Kind>) {
        if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
            self.cells[Self::index(x, y)] = kind;
        }
    }

    /// Is this cell blocked, either by a wall/floor or by a locked cell?
    pub fn is_blocked(&self, x: i16, y: i16) -> bool {
        if !(0..WIDTH).contains(&x) || y >= HEIGHT {
            return true;
        }
        // Above the field is open air: pieces may rotate and move up there.
        if y < 0 {
            return false;
        }
        self.cells[Self::index(x, y)].is_some()
    }

    /// Can this piece occupy its current position without overlapping anything?
    pub fn fits(&self, piece: &Piece) -> bool {
        piece.cells().iter().all(|&(x, y)| !self.is_blocked(x, y))
    }

    /// Write the piece's cells into the grid.
    pub fn lock(&mut self, piece: &Piece) {
        for (x, y) in piece.cells() {
            self.set(x, y, Some(piece.kind));
        }
    }

    /// Remove every full row, shifting the rows above down. Returns how many
    /// rows were cleared.
    pub fn clear_full_rows(&mut self) -> usize {
        let mut write = HEIGHT - 1;
        let mut cleared = 0;

        for read in (0..HEIGHT).rev() {
            let full = (0..WIDTH).all(|x| self.get(x, read).is_some());
            if full {
                cleared += 1;
                continue;
            }
            if write != read {
                for x in 0..WIDTH {
                    let cell = self.get(x, read);
                    self.set(x, write, cell);
                }
            }
            write -= 1;
        }

        // Everything at or above `write` is now vacated.
        for y in 0..=write {
            for x in 0..WIDTH {
                self.set(x, y, None);
            }
        }

        cleared
    }

    /// Indices of every currently full row, top to bottom.
    pub fn full_rows(&self) -> Vec<i16> {
        (0..HEIGHT)
            .filter(|&y| (0..WIDTH).all(|x| self.get(x, y).is_some()))
            .collect()
    }

    /// True when no cell anywhere is filled — used for perfect-clear bonuses.
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(Option::is_none)
    }

    /// How far this piece can fall before it lands.
    pub fn drop_distance(&self, piece: &Piece) -> i16 {
        let mut distance = 0;
        while self.fits(&piece.moved(0, distance + 1)) {
            distance += 1;
        }
        distance
    }

    /// Where the piece would come to rest if hard-dropped from here.
    pub fn ghost(&self, piece: &Piece) -> Piece {
        piece.moved(0, self.drop_distance(piece))
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tetromino::Kind;

    fn fill_row(board: &mut Board, y: i16, gap: Option<i16>) {
        for x in 0..WIDTH {
            if Some(x) != gap {
                board.set(x, y, Some(Kind::I));
            }
        }
    }

    #[test]
    fn walls_and_floor_block_but_open_sky_does_not() {
        let board = Board::new();
        assert!(board.is_blocked(-1, 5));
        assert!(board.is_blocked(WIDTH, 5));
        assert!(board.is_blocked(0, HEIGHT));
        assert!(!board.is_blocked(0, -3));
        assert!(!board.is_blocked(0, 0));
    }

    #[test]
    fn clearing_one_row_shifts_the_stack_down() {
        let mut board = Board::new();
        fill_row(&mut board, HEIGHT - 1, None);
        board.set(3, HEIGHT - 2, Some(Kind::T));

        assert_eq!(board.clear_full_rows(), 1);
        assert_eq!(board.get(3, HEIGHT - 1), Some(Kind::T));
        assert_eq!(board.get(3, HEIGHT - 2), None);
    }

    #[test]
    fn a_tetris_clears_four_rows_at_once() {
        let mut board = Board::new();
        for y in (HEIGHT - 4)..HEIGHT {
            fill_row(&mut board, y, None);
        }
        assert_eq!(board.clear_full_rows(), 4);
        assert!(board.is_empty());
    }

    #[test]
    fn non_adjacent_rows_clear_and_close_the_gaps() {
        let mut board = Board::new();
        fill_row(&mut board, HEIGHT - 1, None);
        fill_row(&mut board, HEIGHT - 3, None);
        // A lone marker on the row between the two full ones.
        board.set(0, HEIGHT - 2, Some(Kind::T));

        assert_eq!(board.clear_full_rows(), 2);
        assert_eq!(board.get(0, HEIGHT - 1), Some(Kind::T));
        for y in 0..(HEIGHT - 1) {
            for x in 0..WIDTH {
                assert_eq!(board.get(x, y), None, "cell ({x},{y}) should be empty");
            }
        }
    }

    #[test]
    fn a_row_with_a_gap_is_not_cleared() {
        let mut board = Board::new();
        fill_row(&mut board, HEIGHT - 1, Some(4));
        assert_eq!(board.clear_full_rows(), 0);
        assert!(!board.is_empty());
    }

    #[test]
    fn ghost_lands_on_the_floor_of_an_empty_board() {
        let board = Board::new();
        let piece = Piece::spawn(Kind::O, WIDTH, HIDDEN_ROWS);
        let ghost = board.ghost(&piece);
        let lowest = ghost.cells().iter().map(|c| c.1).max().unwrap();
        assert_eq!(lowest, HEIGHT - 1);
    }

    #[test]
    fn ghost_rests_on_top_of_the_stack() {
        let mut board = Board::new();
        fill_row(&mut board, HEIGHT - 1, None);
        let piece = Piece::spawn(Kind::O, WIDTH, HIDDEN_ROWS);
        let ghost = board.ghost(&piece);
        let lowest = ghost.cells().iter().map(|c| c.1).max().unwrap();
        assert_eq!(lowest, HEIGHT - 2);
    }
}
