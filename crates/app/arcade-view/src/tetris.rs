//! Tetris: the seven pieces fall into a well ten wide and twenty deep; a
//! full row clears. As the NES counts it: a row cleared at level L is 40 x
//! (L + 1), two 100, three 300 and four 1,200 times the same; a cell of soft
//! drop is 1 and a cell of hard drop 2. Ten rows clear a level, and the
//! level sets how many ticks a row of fall takes, 48 at level 0 down to
//! one from level 29. The tick is 16 ms, a frame: the pieces come from a
//! shuffled bag of the seven, each once a bag. A key is read on the next
//! tick; a turn that hits a wall or a block tries one and two cells either
//! side before it gives up.
use std::collections::VecDeque;

use ducktape_view_guest::Canvas;
use ducktape_view_guest::prelude::*;

use crate::games::{Fact, Rng, square};

pub(crate) const COLS: i32 = 10;
pub(crate) const ROWS: i32 = 20;
pub(crate) const KEYS: &str = "← → move, ↑ or X turns, Z turns back, ↓ drops a row, Space drops";
pub(crate) const PERIOD: i64 = 16;

/// The ticks a row of fall takes at each level; one from level 29 on.
const GRAVITY: [u32; 29] = [
    48, 43, 38, 33, 28, 23, 18, 13, 8, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];
/// What one to four rows cleared at once score at level 0.
const CLEARED: [u64; 5] = [0, 40, 100, 300, 1200];
/// The keys that may wait for a tick.
const WAITING: usize = 8;
/// Where a turn that does not fit tries it, sideways.
const KICKS: [i32; 5] = [0, -1, 1, -2, 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

const PIECES: [Piece; 7] = [
    Piece::I,
    Piece::O,
    Piece::T,
    Piece::S,
    Piece::Z,
    Piece::J,
    Piece::L,
];

impl Piece {
    /// The piece unturned in its box, and the box's side: SRS's shapes.
    fn shape(self) -> ([(i32, i32); 4], i32) {
        match self {
            Piece::I => ([(0, 1), (1, 1), (2, 1), (3, 1)], 4),
            Piece::O => ([(0, 0), (1, 0), (0, 1), (1, 1)], 2),
            Piece::T => ([(1, 0), (0, 1), (1, 1), (2, 1)], 3),
            Piece::S => ([(1, 0), (2, 0), (0, 1), (1, 1)], 3),
            Piece::Z => ([(0, 0), (1, 0), (1, 1), (2, 1)], 3),
            Piece::J => ([(0, 0), (0, 1), (1, 1), (2, 1)], 3),
            Piece::L => ([(2, 0), (0, 1), (1, 1), (2, 1)], 3),
        }
    }

    /// The piece's cells turned clockwise `turn` times, in its box. The O
    /// sits in the middle of a box of four.
    fn cells(self, turn: u8) -> [(i32, i32); 4] {
        let (mut cells, side) = self.shape();
        for _ in 0..turn % 4 {
            cells = cells.map(|(x, y)| (side - 1 - y, x));
        }
        let shift = i32::from(self == Piece::O);
        cells.map(|(x, y)| (x + shift, y))
    }

    pub(crate) fn color(self, theme: &Theme) -> Hsla {
        match self {
            Piece::I => theme.foreground,
            Piece::O => theme.warning,
            Piece::T => theme.agent,
            Piece::S => theme.success,
            Piece::Z => theme.danger,
            Piece::J => theme.muted,
            Piece::L => theme.warning.opacity(0.55),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Move {
    Left,
    Right,
    Turn,
    TurnBack,
    Down,
    Drop,
}

pub(crate) struct Tetris {
    /// The settled blocks, rows top first.
    pub(crate) well: [[Option<Piece>; COLS as usize]; ROWS as usize],
    pub(crate) piece: Piece,
    pub(crate) turn: u8,
    /// The top left corner of the falling piece's box.
    pub(crate) at: (i32, i32),
    pub(crate) next: Piece,
    bag: Vec<Piece>,
    rng: Rng,
    moves: VecDeque<Move>,
    /// Ticks since the piece last fell a row.
    fell: u32,
    pub(crate) lines: u64,
    pub(crate) level: u64,
    score: u64,
    over: bool,
}

impl Tetris {
    pub(crate) fn new(seed: u64) -> Tetris {
        let mut tetris = Tetris {
            well: [[None; COLS as usize]; ROWS as usize],
            piece: Piece::I,
            turn: 0,
            at: (0, 0),
            next: Piece::I,
            bag: Vec::new(),
            rng: Rng::new(seed),
            moves: VecDeque::new(),
            fell: 0,
            lines: 0,
            level: 0,
            score: 0,
            over: false,
        };
        tetris.next = tetris.draw_bag();
        tetris.spawn();
        tetris
    }

    /// The next piece from the bag, which is filled again, shuffled, when
    /// it runs out.
    fn draw_bag(&mut self) -> Piece {
        if self.bag.is_empty() {
            self.bag = PIECES.to_vec();
            for at in (1..self.bag.len()).rev() {
                let other = self.rng.below(at as u64 + 1) as usize;
                self.bag.swap(at, other);
            }
        }
        self.bag.pop().expect("a filled bag")
    }

    /// The next piece at the top of the well; one that does not fit there
    /// ends the game.
    fn spawn(&mut self) {
        self.piece = self.next;
        self.next = self.draw_bag();
        self.turn = 0;
        self.at = (3, 0);
        self.fell = 0;
        self.over = !self.fits(self.piece, self.turn, self.at);
    }

    fn fits(&self, piece: Piece, turn: u8, (x, y): (i32, i32)) -> bool {
        piece.cells(turn).iter().all(|(dx, dy)| {
            let (x, y) = (x + dx, y + dy);
            (0..COLS).contains(&x)
                && (0..ROWS).contains(&y)
                && self.well[y as usize][x as usize].is_none()
        })
    }

    /// The falling piece's cells in the well.
    pub(crate) fn falling(&self) -> [(i32, i32); 4] {
        let (x, y) = self.at;
        self.piece.cells(self.turn).map(|(dx, dy)| (x + dx, y + dy))
    }

    /// Where the falling piece would land, dropped now.
    pub(crate) fn landing(&self) -> i32 {
        let (x, mut y) = self.at;
        while self.fits(self.piece, self.turn, (x, y + 1)) {
            y += 1;
        }
        y
    }

    /// Moves the piece by `(dx, dy)` if it fits there.
    fn shift(&mut self, dx: i32, dy: i32) -> bool {
        let to = (self.at.0 + dx, self.at.1 + dy);
        let fits = self.fits(self.piece, self.turn, to);
        if fits {
            self.at = to;
        }
        fits
    }

    /// Turns the piece `by` quarters clockwise, sideways as far as two
    /// cells where it does not fit in place.
    fn rotate(&mut self, by: u8) -> bool {
        let turn = (self.turn + by) % 4;
        let (x, y) = self.at;
        let Some(kick) = KICKS
            .into_iter()
            .find(|kick| self.fits(self.piece, turn, (x + kick, y)))
        else {
            return false;
        };
        self.turn = turn;
        self.at = (x + kick, y);
        true
    }

    /// The piece settles where it is: full rows clear, and the next piece
    /// comes.
    fn lock(&mut self) {
        for (x, y) in self.falling() {
            self.well[y as usize][x as usize] = Some(self.piece);
        }
        let kept: Vec<_> = self
            .well
            .iter()
            .filter(|row| row.iter().any(Option::is_none))
            .copied()
            .collect();
        let cleared = ROWS as usize - kept.len();
        let mut well = [[None; COLS as usize]; ROWS as usize];
        well[cleared..].copy_from_slice(&kept);
        self.well = well;
        self.score += CLEARED[cleared] * (self.level + 1);
        self.lines += cleared as u64;
        self.level = self.lines / 10;
        self.spawn();
    }

    fn apply(&mut self, held: Move) -> bool {
        match held {
            Move::Left => self.shift(-1, 0),
            Move::Right => self.shift(1, 0),
            Move::Turn => self.rotate(1),
            Move::TurnBack => self.rotate(3),
            Move::Down => {
                if self.shift(0, 1) {
                    self.score += 1;
                    self.fell = 0;
                } else {
                    self.lock();
                }
                true
            }
            Move::Drop => {
                let rows = self.landing() - self.at.1;
                self.at.1 += rows;
                self.score += 2 * rows as u64;
                self.lock();
                true
            }
        }
    }

    /// A key waits for the next tick.
    pub(crate) fn key(&mut self, key: &str) -> bool {
        let held = match key {
            "left" | "a" => Move::Left,
            "right" | "d" => Move::Right,
            "up" | "x" | "w" => Move::Turn,
            "z" => Move::TurnBack,
            "down" | "s" => Move::Down,
            "space" => Move::Drop,
            _ => return false,
        };
        if self.moves.len() < WAITING {
            self.moves.push_back(held);
        }
        false
    }

    /// One frame: the keys that waited, then gravity's count.
    pub(crate) fn tick(&mut self) -> bool {
        let mut moved = false;
        while let Some(held) = self.moves.pop_front() {
            if self.over {
                return moved;
            }
            moved |= self.apply(held);
        }
        if self.over {
            return moved;
        }
        self.fell += 1;
        if self.fell >= self.gravity() {
            self.fell = 0;
            if !self.shift(0, 1) {
                self.lock();
            }
            moved = true;
        }
        moved
    }

    fn gravity(&self) -> u32 {
        GRAVITY.get(self.level as usize).copied().unwrap_or(1)
    }

    pub(crate) fn score(&self) -> u64 {
        self.score
    }

    pub(crate) fn over(&self) -> bool {
        self.over
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        vec![
            ("Level", self.level.to_string()),
            ("Lines", self.lines.to_string()),
        ]
    }

    /// The well, `cell` px a cell: the settled blocks, where the piece
    /// would land, and the piece.
    pub(crate) fn draw(&self, cell: f32, theme: &Theme) -> Canvas {
        let (width, height) = (COLS as f32 * cell, ROWS as f32 * cell);
        let block = |x: i32, y: i32| {
            square(
                x as f32 * cell + 1.,
                y as f32 * cell + 1.,
                cell - 2.,
                cell - 2.,
            )
        };
        let mut drawn = canvas()
            .w(px(width))
            .h(px(height))
            .rect(square(0., 0., width, height), theme.surface);
        for (y, row) in (0..).zip(&self.well) {
            for (x, settled) in (0..).zip(row) {
                if let Some(piece) = settled {
                    drawn = drawn.rect(block(x, y), piece.color(theme));
                }
            }
        }
        if self.over {
            return drawn;
        }
        let fall = self.landing() - self.at.1;
        for (x, y) in self.falling() {
            drawn = drawn.rect(block(x, y + fall), theme.border_strong);
        }
        let color = self.piece.color(theme);
        for (x, y) in self.falling() {
            drawn = drawn.rect(block(x, y), color);
        }
        drawn
    }

    /// The next piece, in a box of four by two cells of `cell` px.
    pub(crate) fn draw_next(&self, cell: f32, theme: &Theme) -> Canvas {
        piece(self.next, cell, theme)
            .w(px(4. * cell))
            .h(px(2. * cell))
    }
}

/// `shown` unturned, `cell` px a cell, from the top left corner.
fn piece(shown: Piece, cell: f32, theme: &Theme) -> Canvas {
    let mut drawn = canvas();
    // the I lies in its box's second row: drawn a row up
    let up = i32::from(shown == Piece::I);
    for (x, y) in shown.cells(0) {
        let at = square(
            x as f32 * cell + 1.,
            (y - up) as f32 * cell + 1.,
            cell - 2.,
            cell - 2.,
        );
        drawn = drawn.rect(at, shown.color(theme));
    }
    drawn
}

/// The hub's picture of the game: a well's last rows and a piece falling,
/// a letter a piece, in the middle of its 208 x 120 px.
pub(crate) fn preview(theme: &Theme) -> Canvas {
    const PICTURE: [&str; 9] = [
        "...TTT....",
        "....T.....",
        "..........",
        "........OO",
        "I.......OO",
        "I.L..SS.ZZ",
        "I.L.SSJZZ.",
        "ILL.JJJ.SS",
        "IZZ..LJSS.",
    ];
    let (cell, left, top) = (11., 49., 11.);
    let mut drawn = canvas().size_full();
    for (y, row) in PICTURE.iter().enumerate() {
        for (x, letter) in row.chars().enumerate() {
            let shown = match letter {
                'I' => Piece::I,
                'O' => Piece::O,
                'T' => Piece::T,
                'S' => Piece::S,
                'Z' => Piece::Z,
                'J' => Piece::J,
                'L' => Piece::L,
                _ => continue,
            };
            let at = square(
                left + x as f32 * cell + 1.,
                top + y as f32 * cell + 1.,
                cell - 2.,
                cell - 2.,
            );
            drawn = drawn.rect(at, shown.color(theme));
        }
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game with the well cleared and `piece` at the top.
    fn with(piece: Piece) -> Tetris {
        let mut tetris = Tetris::new(1);
        tetris.piece = piece;
        tetris.turn = 0;
        tetris.at = (3, 0);
        tetris
    }

    #[test]
    fn a_turn_against_a_wall_moves_the_piece_off_it() {
        let mut tetris = with(Piece::I);
        // upright, on the right wall: its cells are its box's third column
        tetris.turn = 1;
        tetris.at = (COLS - 3, 5);
        assert_eq!(tetris.falling().map(|(x, _)| x), [COLS - 1; 4]);
        tetris.key("up");
        assert!(tetris.tick());
        assert_eq!(tetris.turn, 2);
        let mut columns = tetris.falling().map(|(x, _)| x);
        columns.sort();
        assert_eq!(
            columns,
            [COLS - 4, COLS - 3, COLS - 2, COLS - 1],
            "kicked one left"
        );
    }

    #[test]
    fn a_turn_with_no_room_on_either_side_stays_put() {
        let mut tetris = with(Piece::I);
        tetris.turn = 1;
        tetris.at = (2, 5);
        // walls of blocks two cells either side of the upright I's column
        for y in 0..ROWS as usize {
            tetris.well[y][3] = Some(Piece::O);
            tetris.well[y][5] = Some(Piece::O);
        }
        tetris.key("up");
        tetris.tick();
        assert_eq!(tetris.turn, 1);
        assert_eq!(tetris.at, (2, 5));
    }

    #[test]
    fn every_turn_of_a_piece_is_four_cells_in_its_box() {
        for shown in PIECES {
            for turn in 0..4 {
                let cells = shown.cells(turn);
                assert!(
                    cells
                        .iter()
                        .all(|(x, y)| (0..4).contains(x) && (0..4).contains(y))
                );
                let mut unique = cells.to_vec();
                unique.sort();
                unique.dedup();
                assert_eq!(unique.len(), 4, "{shown:?} turn {turn}");
            }
            assert_eq!(shown.cells(4), shown.cells(0));
        }
    }

    #[test]
    fn a_row_falls_after_the_levels_count_of_ticks() {
        let mut tetris = with(Piece::T);
        for _ in 0..47 {
            assert!(!tetris.tick(), "nothing moves before the 48th tick");
        }
        assert_eq!(tetris.at, (3, 0));
        assert!(tetris.tick());
        assert_eq!(tetris.at, (3, 1));
        tetris.level = 29;
        assert!(tetris.tick(), "a row a tick from level 29");
    }

    #[test]
    fn a_full_row_clears_and_scores_by_the_level() {
        let mut tetris = with(Piece::I);
        // the bottom row full but for the four cells the I lies in
        for x in 4..COLS as usize {
            tetris.well[ROWS as usize - 1][x] = Some(Piece::O);
        }
        tetris.well[ROWS as usize - 2][9] = Some(Piece::Z);
        tetris.at = (0, 3);
        tetris.key("space");
        tetris.tick();
        // the drop's 15 rows at 2 and one row at level 0
        assert_eq!(tetris.score(), 2 * 15 + 40);
        assert_eq!(tetris.lines, 1);
        // the row above came down
        assert_eq!(tetris.well[ROWS as usize - 1][9], Some(Piece::Z));
        assert!(
            tetris.well[ROWS as usize - 1][..9]
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    fn four_rows_at_once_score_1200_a_level_and_ten_rows_raise_it() {
        let mut tetris = with(Piece::I);
        tetris.lines = 8;
        tetris.level = 0;
        for y in ROWS as usize - 4..ROWS as usize {
            for x in 1..COLS as usize {
                tetris.well[y][x] = Some(Piece::O);
            }
        }
        // upright in the first column
        tetris.turn = 1;
        tetris.at = (-2, 0);
        tetris.key("space");
        tetris.tick();
        assert_eq!(tetris.score(), 2 * 16 + 1200);
        assert_eq!((tetris.lines, tetris.level), (12, 1));
        assert!(tetris.well.iter().flatten().all(Option::is_none));
    }

    #[test]
    fn a_soft_drop_scores_a_cell_and_locks_on_the_floor() {
        let mut tetris = with(Piece::O);
        tetris.at = (3, ROWS - 3);
        tetris.key("down");
        tetris.tick();
        assert_eq!((tetris.at.1, tetris.score()), (ROWS - 2, 1));
        tetris.key("down");
        tetris.tick();
        assert!(tetris.well[ROWS as usize - 1][4].is_some(), "locked");
        assert_eq!(tetris.at, (3, 0), "the next piece at the top");
    }

    #[test]
    fn a_piece_that_cannot_come_in_ends_the_game() {
        let mut tetris = with(Piece::O);
        // blocks where every piece comes in
        for y in 0..2 {
            for x in 3..7 {
                tetris.well[y][x] = Some(Piece::Z);
            }
        }
        tetris.at = (7, 0);
        tetris.key("space");
        tetris.tick();
        assert!(tetris.well[ROWS as usize - 1][8].is_some(), "the O settled");
        assert!(tetris.over());
        assert!(!tetris.tick(), "an ended game does not move");
    }

    #[test]
    fn the_bag_deals_each_piece_once_in_seven() {
        let mut tetris = Tetris::new(42);
        tetris.bag.clear();
        let mut dealt: Vec<Piece> = (0..7).map(|_| tetris.draw_bag()).collect();
        dealt.sort_by_key(|shown| *shown as u8);
        assert_eq!(dealt, PIECES);
        let deal = |seed| {
            let mut tetris = Tetris::new(seed);
            (0..14).map(|_| tetris.draw_bag()).collect::<Vec<_>>()
        };
        assert_eq!(deal(7), deal(7), "one seed, one deal");
        assert_ne!(deal(7), deal(8));
    }
}
