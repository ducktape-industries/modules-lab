//! Tetris: the seven pieces fall into a well ten wide and twenty deep; a
//! full row clears. As the NES counts it: a row cleared at level L is 40 x
//! (L + 1), two 100, three 300 and four 1,200 times the same; a cell of soft
//! drop is 1 and a cell of hard drop 2. Ten rows clear a level, and the
//! level sets how many ticks a row of fall takes, 48 at level 0 down to
//! one from level 29. The tick is 16 ms, a frame: the pieces come from a
//! shuffled bag of the seven, each once a bag, and the panel shows the
//! next three. A key is read on the next tick. The rest is the Tetris Guideline's: a turn that does not fit
//! tries SRS's kicks, the I its own; a piece that lands locks after half a
//! second, which a move or a turn on the ground starts again, 15 times a
//! piece at most (the 16th locks it); a hard drop locks at once; C or
//! Shift puts the piece in hold and takes the one there, or the next, once
//! a piece.
//! A held key repeats on the game's own count, never the keyboard's (half
//! a second before its first repeat): a held arrow slides the piece again
//! after 160 ms and then a cell every 32 ms, a held ↓ drops a row every
//! other tick, and a turn, a drop or a hold does not repeat.
use std::collections::VecDeque;

use view::Canvas;
use view::prelude::*;

use crate::games::{Fact, Rng, square};

pub(crate) const COLS: i32 = 10;
pub(crate) const ROWS: i32 = 20;
pub(crate) const KEYS: &str =
    "← → move, ↑ or X turns, Z turns back, ↓ drops a row, Space drops, C or Shift holds";
pub(crate) const PERIOD: i64 = 16;

/// The ticks a row of fall takes at each level; one from level 29 on.
const GRAVITY: [u32; 29] = [
    48, 43, 38, 33, 28, 23, 18, 13, 8, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];
/// What one to four rows cleared at once score at level 0.
const CLEARED: [u64; 5] = [0, 40, 100, 300, 1200];
/// The keys that may wait for a tick.
const WAITING: usize = 8;
/// The pieces the panel shows coming.
const NEXT: usize = 3;
/// SRS's kicks for a clockwise turn out of each of the four states, as the
/// Guideline writes them (x right, y up); a turn back out of state s + 1
/// tries the same, negated. The first that fits is the turn.
const KICKS: [[(i32, i32); 5]; 4] = [
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
];
/// The same for the I.
const I_KICKS: [[(i32, i32); 5]; 4] = [
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
];
/// How long a landed piece lies before it locks: half a second of ticks.
const LOCK_DELAY: u32 = (500 / PERIOD) as u32;
/// How many times a move or a turn on the ground starts the delay again.
const RESETS: u32 = 15;
/// The ticks an arrow is held before the piece slides on its own, 160 ms,
/// and the ticks a cell after that, 32 ms.
const DAS: u32 = 10;
const ARR: u32 = 2;
/// The ticks a row of a held ↓: NES's soft drop.
const SOFT: u32 = 2;

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
    Hold,
}

pub(crate) struct Tetris {
    /// The settled blocks, rows top first.
    pub(crate) well: [[Option<Piece>; COLS as usize]; ROWS as usize],
    pub(crate) piece: Piece,
    pub(crate) turn: u8,
    /// The top left corner of the falling piece's box.
    pub(crate) at: (i32, i32),
    /// The pieces coming, the first next.
    pub(crate) next: [Piece; NEXT],
    /// The piece put aside with C or Shift.
    pub(crate) hold: Option<Piece>,
    /// Hold was used for the piece falling: no hold again until it locks.
    held: bool,
    bag: Vec<Piece>,
    rng: Rng,
    moves: VecDeque<Move>,
    /// The arrows held to a side, the newest last: the piece slides to it.
    sides: Vec<Move>,
    /// Ticks since the newest of `sides` went down, or took over from a
    /// newer one let go.
    charge: u32,
    /// Ticks ↓ has been held.
    soft: Option<u32>,
    /// Ticks since the piece last fell a row.
    fell: u32,
    /// Ticks the piece has lain on the floor or a block.
    rest: u32,
    /// How often a move or a turn on the ground started `rest` again.
    resets: u32,
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
            next: [Piece::I; NEXT],
            hold: None,
            held: false,
            bag: Vec::new(),
            rng: Rng::new(seed),
            moves: VecDeque::new(),
            sides: Vec::new(),
            charge: 0,
            soft: None,
            fell: 0,
            rest: 0,
            resets: 0,
            lines: 0,
            level: 0,
            score: 0,
            over: false,
        };
        tetris.next = std::array::from_fn(|_| tetris.draw_bag());
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

    /// The next piece at the top of the well, and one more from the bag
    /// at the end of those coming.
    fn spawn(&mut self) {
        let next = self.next[0];
        self.next.rotate_left(1);
        self.next[NEXT - 1] = self.draw_bag();
        self.held = false;
        self.enter(next);
    }

    /// `piece` at the top of the well, unturned; one that does not fit
    /// there ends the game.
    fn enter(&mut self, piece: Piece) {
        self.piece = piece;
        self.turn = 0;
        self.at = (3, 0);
        self.fell = 0;
        self.rest = 0;
        self.resets = 0;
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

    /// Whether the piece lies on the floor or a block.
    fn landed(&self) -> bool {
        !self.fits(self.piece, self.turn, (self.at.0, self.at.1 + 1))
    }

    /// Turns the piece a quarter clockwise, or back, at the first of its
    /// kicks where it fits.
    fn rotate(&mut self, back: bool) -> bool {
        let table = match self.piece {
            Piece::I => I_KICKS,
            _ => KICKS,
        };
        let (turn, kicks) = match back {
            false => ((self.turn + 1) % 4, table[self.turn as usize]),
            true => {
                let turn = (self.turn + 3) % 4;
                (turn, table[turn as usize].map(|(x, y)| (-x, -y)))
            }
        };
        let (x, y) = self.at;
        let Some(at) = kicks
            .into_iter()
            .map(|(dx, dy)| (x + dx, y - dy))
            .find(|at| self.fits(self.piece, turn, *at))
        else {
            return false;
        };
        self.turn = turn;
        self.at = at;
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

    /// Puts the piece in hold and brings in the one there, or the next.
    fn swap_hold(&mut self) -> bool {
        if self.held {
            return false;
        }
        match self.hold.replace(self.piece) {
            Some(held) => self.enter(held),
            None => self.spawn(),
        }
        self.held = true;
        true
    }

    fn apply(&mut self, held: Move) -> bool {
        let landed = self.landed();
        let moved = match held {
            Move::Left => self.shift(-1, 0),
            Move::Right => self.shift(1, 0),
            Move::Turn => self.rotate(false),
            Move::TurnBack => self.rotate(true),
            Move::Down => {
                let fell = self.shift(0, 1);
                if fell {
                    self.score += 1;
                    self.fell = 0;
                }
                return fell;
            }
            Move::Drop => {
                let rows = self.landing() - self.at.1;
                self.at.1 += rows;
                self.score += 2 * rows as u64;
                self.lock();
                return true;
            }
            Move::Hold => return self.swap_hold(),
        };
        // a move on the ground starts the lock delay again, 15 times a
        // piece; the 16th locks it where it lies
        if moved && landed {
            if self.resets < RESETS {
                self.resets += 1;
                self.rest = 0;
            } else if self.landed() {
                self.lock();
            }
        }
        moved
    }

    /// A key waits for the next tick; an arrow is held until it comes up.
    /// The keyboard's own repeat of a held key is not read.
    pub(crate) fn key_down(&mut self, key: &str, repeat: bool) -> bool {
        let Some(held) = read(key) else {
            return false;
        };
        if repeat {
            return false;
        }
        match held {
            Move::Left | Move::Right => {
                self.sides.retain(|side| *side != held);
                self.sides.push(held);
                self.charge = 0;
            }
            Move::Down => self.soft = Some(0),
            _ => {}
        }
        if self.moves.len() < WAITING {
            self.moves.push_back(held);
        }
        false
    }

    /// An arrow came up: the other side's, if it is still held, slides
    /// the piece after its own count.
    pub(crate) fn key_up(&mut self, key: &str) {
        match read(key) {
            Some(Move::Down) => self.soft = None,
            Some(up @ (Move::Left | Move::Right)) => {
                let newest = self.sides.last().copied();
                self.sides.retain(|side| *side != up);
                if self.sides.last().copied() != newest {
                    self.charge = 0;
                }
            }
            _ => {}
        }
    }

    /// No key is held: a pause or a game off screen hears no key come up.
    pub(crate) fn let_go(&mut self) {
        self.sides.clear();
        self.soft = None;
    }

    /// The keys held: the newest side arrow slides the piece once DAS
    /// ticks have passed, then every ARR; ↓ drops a row every SOFT.
    fn repeat(&mut self) -> bool {
        let mut moved = false;
        if let Some(&side) = self.sides.last() {
            self.charge += 1;
            if self.charge >= DAS && (self.charge - DAS).is_multiple_of(ARR) {
                moved |= self.apply(side);
            }
        }
        if let Some(held) = self.soft.map(|held| held + 1) {
            self.soft = Some(held);
            if held.is_multiple_of(SOFT) && !self.over {
                moved |= self.apply(Move::Down);
            }
        }
        moved
    }

    /// One frame: the keys that waited, then gravity's count, or the lock
    /// delay's on the ground.
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
        moved |= self.repeat();
        if self.over {
            return moved;
        }
        if self.landed() {
            self.rest += 1;
            if self.rest >= LOCK_DELAY {
                self.lock();
                moved = true;
            }
        } else {
            self.fell += 1;
            if self.fell >= self.gravity() {
                self.fell = 0;
                self.at.1 += 1;
                moved = true;
            }
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

    /// The pieces coming, the first on top, each in a box of four by two
    /// cells of `cell` px and half a cell apart.
    pub(crate) fn draw_next(&self, cell: f32, theme: &Theme) -> Canvas {
        let drawn = canvas()
            .w(px(4. * cell))
            .h(px((2.5 * NEXT as f32 - 0.5) * cell));
        (0..).zip(self.next).fold(drawn, |drawn, (at, coming)| {
            piece(
                drawn,
                coming,
                cell,
                2.5 * at as f32 * cell,
                coming.color(theme),
            )
        })
    }

    /// The piece in hold, as one coming is drawn; faint once hold is used
    /// for the piece falling.
    pub(crate) fn draw_hold(&self, cell: f32, theme: &Theme) -> Canvas {
        let drawn = canvas().w(px(4. * cell)).h(px(2. * cell));
        let Some(held) = self.hold else {
            return drawn;
        };
        let color = match self.held {
            true => theme.border_strong,
            false => held.color(theme),
        };
        piece(drawn, held, cell, 0., color)
    }
}

/// The key's move, if it is one.
fn read(key: &str) -> Option<Move> {
    Some(match key {
        "left" | "a" => Move::Left,
        "right" | "d" => Move::Right,
        "up" | "x" | "w" => Move::Turn,
        "z" => Move::TurnBack,
        "down" | "s" => Move::Down,
        "space" => Move::Drop,
        "c" | "shift" => Move::Hold,
        _ => return None,
    })
}

/// `shown` unturned in `color` on `drawn`, `cell` px a cell, in a box of
/// four by two cells `top` px down.
fn piece(mut drawn: Canvas, shown: Piece, cell: f32, top: f32, color: Hsla) -> Canvas {
    // the I lies in its box's second row: drawn a row up
    let up = i32::from(shown == Piece::I);
    for (x, y) in shown.cells(0) {
        let at = square(
            x as f32 * cell + 1.,
            top + (y - up) as f32 * cell + 1.,
            cell - 2.,
            cell - 2.,
        );
        drawn = drawn.rect(at, color);
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

    impl Tetris {
        /// `key` goes down and up before the next tick.
        fn tap(&mut self, key: &str) {
            self.key_down(key, false);
            self.key_up(key);
        }
    }

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
        tetris.tap("up");
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
    fn a_piece_lying_flat_on_the_floor_turns_upright_either_way() {
        let mut stuck = Vec::new();
        for shown in [Piece::T, Piece::S, Piece::Z, Piece::J, Piece::L] {
            for key in ["up", "z"] {
                let mut tetris = with(shown);
                // flat, its lower row on the floor
                tetris.at = (3, ROWS - 2);
                tetris.tap(key);
                tetris.tick();
                let rows = tetris.falling().map(|(_, y)| y);
                let upright = tetris.turn == if key == "up" { 1 } else { 3 }
                    && rows.iter().min() == Some(&(ROWS - 3))
                    && rows.iter().max() == Some(&(ROWS - 1));
                if !upright {
                    stuck.push(format!("{shown:?} {key}"));
                }
            }
        }
        assert!(stuck.is_empty(), "not upright on the floor: {stuck:?}");
    }

    #[test]
    fn an_i_lying_flat_on_the_floor_against_a_wall_turns_upright_on_it() {
        let mut tetris = with(Piece::I);
        tetris.at = (COLS - 4, ROWS - 2);
        tetris.tap("up");
        tetris.tick();
        assert_eq!(tetris.turn, 1);
        let mut cells = tetris.falling().to_vec();
        cells.sort();
        assert_eq!(
            cells,
            [
                (COLS - 1, ROWS - 4),
                (COLS - 1, ROWS - 3),
                (COLS - 1, ROWS - 2),
                (COLS - 1, ROWS - 1)
            ],
            "upright in the last column, on the floor"
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
        tetris.tap("up");
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
        tetris.tap("space");
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
        tetris.tap("space");
        tetris.tick();
        assert_eq!(tetris.score(), 2 * 16 + 1200);
        assert_eq!((tetris.lines, tetris.level), (12, 1));
        assert!(tetris.well.iter().flatten().all(Option::is_none));
    }

    #[test]
    fn a_soft_drop_scores_a_cell_and_on_the_floor_leaves_the_piece_to_its_delay() {
        let mut tetris = with(Piece::O);
        tetris.at = (3, ROWS - 3);
        tetris.tap("down");
        tetris.tick();
        assert_eq!((tetris.at.1, tetris.score()), (ROWS - 2, 1));
        tetris.tap("down");
        assert!(!tetris.tick(), "on the floor a soft drop does nothing");
        assert_eq!(tetris.score(), 1);
        for _ in 2..LOCK_DELAY {
            tetris.tick();
        }
        assert!(tetris.well[ROWS as usize - 1][4].is_some(), "locked");
        assert_eq!(tetris.at, (3, 0), "the next piece at the top");
    }

    #[test]
    fn a_landed_piece_locks_after_half_a_second_of_ticks() {
        assert_eq!(LOCK_DELAY, 31);
        let mut tetris = with(Piece::T);
        tetris.at = (3, ROWS - 2);
        for _ in 1..LOCK_DELAY {
            assert!(!tetris.tick(), "it lies there, not locked yet");
        }
        assert!(tetris.tick());
        assert!(tetris.well[ROWS as usize - 1][4].is_some(), "locked");
        assert_eq!(tetris.at, (3, 0), "the next piece at the top");
    }

    #[test]
    fn a_move_on_the_ground_starts_the_delay_again_15_times_and_the_16th_locks() {
        let mut tetris = with(Piece::O);
        tetris.at = (3, ROWS - 2);
        for step in 1..=RESETS {
            // all but the last tick of a delay, then a step aside
            for _ in 2..LOCK_DELAY {
                tetris.tick();
            }
            tetris.tap(if step % 2 == 1 { "left" } else { "right" });
            tetris.tick();
            assert!(
                tetris.well.iter().flatten().all(Option::is_none),
                "move {step} keeps it up"
            );
        }
        tetris.tap("left");
        tetris.tick();
        assert!(
            tetris.well[ROWS as usize - 1].iter().any(Option::is_some),
            "the 16th locks it"
        );
    }

    #[test]
    fn a_hard_drop_locks_at_once() {
        let mut tetris = with(Piece::O);
        tetris.tap("space");
        tetris.tick();
        assert!(tetris.well[ROWS as usize - 1][4].is_some());
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
        tetris.tap("space");
        tetris.tick();
        assert!(tetris.well[ROWS as usize - 1][8].is_some(), "the O settled");
        assert!(tetris.over());
        assert!(!tetris.tick(), "an ended game does not move");
    }

    #[test]
    fn a_held_arrow_slides_the_piece_after_das_ticks_then_a_cell_every_arr() {
        let mut tetris = with(Piece::T);
        tetris.at = (5, 0);
        tetris.key_down("left", false);
        let mut columns = Vec::new();
        for _ in 0..DAS + 2 * ARR {
            tetris.tick();
            columns.push(tetris.at.0);
        }
        let mut slid = vec![4; DAS as usize - 1];
        slid.extend([3, 3, 2, 2, 1]);
        assert_eq!(columns, slid, "once at once, again at DAS, then every ARR");
        tetris.key_up("left");
        for _ in 0..DAS {
            tetris.tick();
        }
        assert_eq!(tetris.at.0, 1, "up, it stops");
    }

    #[test]
    fn the_keyboards_own_repeat_is_not_read() {
        let mut tetris = with(Piece::T);
        tetris.at = (5, 0);
        for key in ["left", "up", "z", "down", "space", "c"] {
            tetris.key_down(key, true);
        }
        for _ in 0..DAS + ARR {
            assert!(!tetris.tick());
        }
        assert_eq!((tetris.at, tetris.turn, tetris.hold), ((5, 0), 0, None));
    }

    #[test]
    fn the_newest_arrow_held_slides_and_up_it_gives_the_other_its_own_count() {
        let mut tetris = with(Piece::T);
        tetris.at = (4, 0);
        tetris.key_down("left", false);
        tetris.key_down("right", false);
        for _ in 0..DAS {
            tetris.tick();
        }
        assert_eq!(tetris.at.0, 5, "a cell left, one back, then right's slide");
        tetris.key_up("right");
        for _ in 1..DAS {
            tetris.tick();
        }
        assert_eq!(tetris.at.0, 5, "left counts its DAS again");
        tetris.tick();
        assert_eq!(tetris.at.0, 4);
    }

    #[test]
    fn a_held_down_drops_a_row_every_soft_ticks_and_scores_each() {
        let mut tetris = with(Piece::O);
        tetris.key_down("down", false);
        for _ in 0..5 * SOFT {
            tetris.tick();
        }
        assert_eq!(
            (tetris.at.1, tetris.score()),
            (6, 6),
            "one at once, then five"
        );
        tetris.key_up("down");
        for _ in 0..5 * SOFT {
            tetris.tick();
        }
        assert_eq!(tetris.at.1, 6, "up, it falls by gravity's count alone");
    }

    #[test]
    fn letting_go_stops_what_a_held_key_did() {
        let mut tetris = with(Piece::T);
        tetris.at = (5, 0);
        tetris.key_down("left", false);
        tetris.key_down("down", false);
        tetris.let_go();
        for _ in 0..DAS + 2 * ARR {
            tetris.tick();
        }
        assert_eq!(tetris.at, (4, 1), "the presses alone, read on the tick");
    }

    #[test]
    fn c_puts_the_piece_in_hold_once_a_piece_and_brings_it_back_unturned_at_the_top() {
        let mut tetris = with(Piece::T);
        let next = tetris.next[0];
        tetris.turn = 1;
        tetris.at = (5, 7);
        tetris.tap("c");
        assert!(tetris.tick());
        assert_eq!(
            (tetris.hold, tetris.piece, tetris.turn, tetris.at),
            (Some(Piece::T), next, 0, (3, 0)),
            "an empty hold takes the T and the next comes in"
        );
        tetris.tap("c");
        assert!(!tetris.tick(), "not twice a piece");
        assert_eq!((tetris.piece, tetris.hold), (next, Some(Piece::T)));
        tetris.tap("space");
        tetris.tick();
        let falling = tetris.piece;
        tetris.tap("c");
        tetris.tick();
        assert_eq!(
            (tetris.piece, tetris.hold, tetris.at),
            (Piece::T, Some(falling), (3, 0)),
            "a new piece holds again and takes the T back"
        );
    }

    #[test]
    fn the_panel_shows_three_coming_and_they_come_in_that_order() {
        let mut tetris = Tetris::new(9);
        let coming = tetris.next;
        assert_eq!(coming.len(), 3);
        let mut came = Vec::new();
        for _ in 0..3 {
            tetris.tap("space");
            tetris.tick();
            came.push(tetris.piece);
        }
        assert_eq!(came, coming);
        // one held comes from the first in line, and the line moves up
        let after = tetris.next;
        tetris.tap("c");
        tetris.tick();
        assert_eq!(tetris.piece, after[0]);
        assert_eq!(tetris.next[..2], after[1..]);
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
