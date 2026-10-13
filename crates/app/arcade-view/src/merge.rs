//! 2048: tiles on a board of four by four slide all the way as an arrow
//! says; two equal tiles that meet become one of their sum, which scores
//! it. Each tile merges once a move, the pair nearest the wall first.
//! A move that changes the board brings a new tile, a 2 nine times in
//! ten and a 4 the tenth; when no move changes it, the game is over.
//!
//! A key changes the board at once; the clock only draws it getting there
//! (`Motion`): the tiles slide, then a merged tile pulses and the new one
//! pops in, about 150 ms, and a key before the end lands them at once. A
//! board at rest asks for no clock.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Canvas, Div, FontWeight};

use crate::games::{Fact, Rng, square};

pub(crate) const SIDE: usize = 4;
pub(crate) const KEYS: &str = "Arrows slide the tiles";
/// The clock's period while tiles move: a frame, in milliseconds.
pub(crate) const PERIOD: i64 = 16;
/// The frames a move is drawn over: its tiles slide for `SLIDE`, then a
/// merged tile pulses and the new one pops in for `POP`. The first is
/// drawn with the key; the last is the board at rest, which ends it.
const SLIDE: u8 = 6;
const POP: u8 = 4;
pub(crate) const FRAMES: u8 = SLIDE + POP;
/// How much a merged tile grows at its pulse's height: a cell's twelfth
/// is its gap, so it stays inside the board.
const PULSE: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slide {
    Left,
    Right,
    Up,
    Down,
}

pub(crate) struct Merge {
    /// Rows top first; 0 is an empty cell.
    pub(crate) tiles: [[u32; SIDE]; SIDE],
    score: u64,
    over: bool,
    rng: Rng,
    /// How the last move is drawn, until it ends; `tiles` is already
    /// where it ends.
    pub(crate) motion: Option<Motion>,
}

/// A move as it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Motion {
    /// Every tile before the move.
    pub(crate) glides: Vec<Glide>,
    /// The cells where two tiles became one, (x, y).
    pub(crate) merged: Vec<(usize, usize)>,
    /// The new tile's cell.
    pub(crate) new: Option<(usize, usize)>,
    /// The frame on screen, from 1.
    pub(crate) frame: u8,
}

/// A tile in a move: its value before it, the cell it was on and the one
/// it lands on, (x, y).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Glide {
    pub(crate) value: u32,
    pub(crate) from: (usize, usize),
    pub(crate) to: (usize, usize),
}

/// A tile as a frame draws it: its value, where it lies in cells (x, y),
/// and its scale.
type Piece = (u32, (f32, f32), f32);

impl Merge {
    pub(crate) fn new(seed: u64) -> Merge {
        let mut merge = Merge {
            tiles: [[0; SIDE]; SIDE],
            score: 0,
            over: false,
            rng: Rng::new(seed),
            motion: None,
        };
        merge.spawn();
        merge.spawn();
        merge
    }

    /// A 2, or a 4 one time in ten, on an empty cell: which.
    fn spawn(&mut self) -> Option<(usize, usize)> {
        let empty: Vec<(usize, usize)> = (0..SIDE)
            .flat_map(|y| (0..SIDE).map(move |x| (x, y)))
            .filter(|(x, y)| self.tiles[*y][*x] == 0)
            .collect();
        if empty.is_empty() {
            return None;
        }
        let (x, y) = empty[self.rng.below(empty.len() as u64) as usize];
        self.tiles[y][x] = if self.rng.below(10) == 0 { 4 } else { 2 };
        Some((x, y))
    }

    /// An arrow slides the board at once, and its motion starts over from
    /// there: one still drawn lands. Whether it changed.
    pub(crate) fn key(&mut self, key: &str) -> bool {
        let slide = match key {
            "left" | "a" => Slide::Left,
            "right" | "d" => Slide::Right,
            "up" | "w" => Slide::Up,
            "down" | "s" => Slide::Down,
            _ => return false,
        };
        let (tiles, points, glides) = slid(self.tiles, slide);
        if tiles == self.tiles {
            return false;
        }
        self.tiles = tiles;
        self.score += points;
        let new = self.spawn();
        self.over = stuck(self.tiles);
        // a tile whose cell holds another value merged there; a merge's two
        // tiles are neighbours in their line, and so in `glides`
        let mut merged: Vec<_> = glides
            .iter()
            .filter(|glide| tiles[glide.to.1][glide.to.0] != glide.value)
            .map(|glide| glide.to)
            .collect();
        merged.dedup();
        // the clock stops with the game: its last move is drawn at rest,
        // under its end
        self.motion = (!self.over).then_some(Motion {
            glides,
            merged,
            new,
            frame: 1,
        });
        true
    }

    /// A frame of the clock: the motion goes on, and ends at rest. Whether
    /// the drawing changed.
    pub(crate) fn tick(&mut self) -> bool {
        let Some(motion) = &mut self.motion else {
            return false;
        };
        motion.frame += 1;
        if motion.frame == FRAMES {
            self.motion = None;
        }
        true
    }

    /// The clock while tiles move; none at rest.
    pub(crate) fn period(&self) -> Option<i64> {
        self.motion.as_ref().map(|_| PERIOD)
    }

    pub(crate) fn score(&self) -> u64 {
        self.score
    }

    pub(crate) fn over(&self) -> bool {
        self.over
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        let best = self.tiles.iter().flatten().max().copied().unwrap_or(0);
        vec![("Best tile", design::grouped(u64::from(best)))]
    }

    /// The board: a tile is an element, so its number is text. `cell` px a
    /// tile, with a gap between them. The tiles lie over a grid of empty
    /// cells, where this frame of the motion has them.
    pub(crate) fn draw(&self, cell: f32, theme: &Theme) -> Div {
        let theme = *theme;
        let tile = |value: u32, side: f32| {
            let (fill, ink) = shade(value, &theme);
            div()
                .size(px(side))
                .flex()
                .items_center()
                .justify_center()
                .bg(fill)
                .text_color(ink)
                .font_family(design::fonts::FAMILY_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(if value >= 1024 {
                    side * 0.26
                } else {
                    side * 0.32
                }))
                .when(value > 0, |tile| tile.child(value.to_string()))
        };
        let gap = (cell / 12.).round();
        let row = |_| {
            div()
                .flex()
                .gap(px(gap))
                .children([0; SIDE].map(|_| tile(0, cell)))
        };
        let grid = div()
            .p(px(gap))
            .flex()
            .flex_col()
            .gap(px(gap))
            .bg(theme.border)
            .children((0..SIDE).map(row));
        let piece = |(value, at, scale): Piece| {
            let (left, top, side) = placed(cell, at, scale);
            tile(value, side).absolute().left(px(left)).top(px(top))
        };
        div()
            .relative()
            .child(grid)
            .children(self.pieces().into_iter().map(piece))
    }

    /// Each tile as the frame on screen draws it: sliding from its cell to
    /// the one it lands on, then the board after the move, a merged tile
    /// pulsing and the new one popping in.
    fn pieces(&self) -> Vec<Piece> {
        let cells = |(x, y): (usize, usize)| (x as f32, y as f32);
        if let Some(motion) = self.motion.as_ref().filter(|motion| motion.frame <= SLIDE) {
            let t = eased(f32::from(motion.frame) / f32::from(SLIDE));
            let between = |from: f32, to: f32| from + (to - from) * t;
            return motion
                .glides
                .iter()
                .map(|glide| {
                    let (from, to) = (cells(glide.from), cells(glide.to));
                    (
                        glide.value,
                        (between(from.0, to.0), between(from.1, to.1)),
                        1.,
                    )
                })
                .collect();
        }
        let scale = |at| {
            let Some(motion) = &self.motion else {
                return 1.;
            };
            let t = f32::from(motion.frame - SLIDE) / f32::from(POP);
            if motion.new == Some(at) {
                eased(t)
            } else if motion.merged.contains(&at) {
                // up to its height halfway, and back
                1. + PULSE * (1. - (2. * t - 1.).abs())
            } else {
                1.
            }
        };
        (0..SIDE)
            .flat_map(|y| (0..SIDE).map(move |x| (x, y)))
            .filter(|(x, y)| self.tiles[*y][*x] > 0)
            .map(|at| (self.tiles[at.1][at.0], cells(at), scale(at)))
            .collect()
    }
}

/// Where a tile `scale` times its size, centred on cell `at`, lies on a
/// board of `cell` px cells: its left, its top and its side, in whole px.
fn placed(cell: f32, (x, y): (f32, f32), scale: f32) -> (f32, f32, f32) {
    let gap = (cell / 12.).round();
    let side = (cell * scale).round();
    let at = |at: f32| (gap + at * (cell + gap) + (cell - side) / 2.).round();
    (at(x), at(y), side)
}

/// Fast from the start, slow onto the end.
fn eased(t: f32) -> f32 {
    1. - (1. - t) * (1. - t)
}

/// `tiles` slid one way, what its merges scored, and where each of its
/// tiles went.
fn slid(mut tiles: [[u32; SIDE]; SIDE], slide: Slide) -> ([[u32; SIDE]; SIDE], u64, Vec<Glide>) {
    let mut points = 0;
    let mut glides = Vec::new();
    for line in 0..SIDE {
        // the line read from the wall the tiles slide to
        let cell = |at: usize| match slide {
            Slide::Left => (at, line),
            Slide::Right => (SIDE - 1 - at, line),
            Slide::Up => (line, at),
            Slide::Down => (line, SIDE - 1 - at),
        };
        let read: [u32; SIDE] = std::array::from_fn(|at| {
            let (x, y) = cell(at);
            tiles[y][x]
        });
        let (merged, scored, to) = merged(read);
        points += scored;
        glides.extend((0..SIDE).filter(|at| read[*at] > 0).map(|at| Glide {
            value: read[at],
            from: cell(at),
            to: cell(to[at]),
        }));
        for (at, value) in merged.into_iter().enumerate() {
            let (x, y) = cell(at);
            tiles[y][x] = value;
        }
    }
    (tiles, points, glides)
}

/// Whether no slide changes `tiles`: the game is over.
fn stuck(tiles: [[u32; SIDE]; SIDE]) -> bool {
    [Slide::Left, Slide::Right, Slide::Up, Slide::Down]
        .into_iter()
        .all(|slide| slid(tiles, slide).0 == tiles)
}

/// One line slid toward its start: equal neighbours merge, the pair
/// nearest the start first, each tile once. Also the place each place's
/// tile lands on.
fn merged(line: [u32; SIDE]) -> ([u32; SIDE], u64, [usize; SIDE]) {
    let (mut out, mut to) = ([0; SIDE], [0; SIDE]);
    let (mut at, mut points) = (0, 0);
    let mut waiting: Option<u32> = None;
    for (from, value) in line.into_iter().enumerate().filter(|(_, value)| *value > 0) {
        match waiting {
            Some(held) if held == value => {
                out[at] = held * 2;
                points += u64::from(held * 2);
                to[from] = at;
                at += 1;
                waiting = None;
            }
            Some(held) => {
                out[at] = held;
                at += 1;
                to[from] = at;
                waiting = Some(value);
            }
            None => {
                to[from] = at;
                waiting = Some(value);
            }
        }
    }
    if let Some(held) = waiting {
        out[at] = held;
    }
    (out, points, to)
}

/// A tile's fill and the ink of its number: soft tones for the small ones,
/// the strong ones as they grow.
fn shade(value: u32, theme: &Theme) -> (Hsla, Hsla) {
    let soft = |fill| (fill, theme.foreground);
    let strong = |fill| (fill, theme.background);
    match value {
        0 => soft(theme.surface),
        2 => soft(theme.background),
        4 => soft(theme.surface_raised),
        8 => soft(theme.warning_soft),
        16 => strong(theme.warning.opacity(0.7)),
        32 => strong(theme.warning),
        64 => soft(theme.danger_soft),
        128 => strong(theme.danger.opacity(0.75)),
        256 => strong(theme.danger),
        512 => soft(theme.agent_soft),
        1024 => strong(theme.agent.opacity(0.8)),
        2048 => strong(theme.agent),
        _ => strong(theme.foreground),
    }
}

/// The hub's picture of the game: a board under way, in the middle of its
/// 208 x 120 px.
pub(crate) fn preview(theme: &Theme) -> Canvas {
    let values = [[0, 2, 0, 0], [2, 4, 8, 0], [4, 16, 32, 2], [8, 64, 128, 4]];
    let (tile, gap, left, top) = (22., 4., 50., 6.);
    let side = 4. * tile + 5. * gap;
    let mut drawn = canvas()
        .size_full()
        .rect(square(left, top, side, side), theme.border);
    for (y, row) in values.iter().enumerate() {
        for (x, value) in row.iter().enumerate() {
            let at = square(
                left + gap + x as f32 * (tile + gap),
                top + gap + y as f32 * (tile + gap),
                tile,
                tile,
            );
            drawn = drawn.rect(at, shade(*value, theme).0);
        }
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_merges_the_pair_nearest_the_wall_first_and_each_tile_once() {
        let values = |line| {
            let (out, points, _) = merged(line);
            (out, points)
        };
        assert_eq!(values([2, 2, 2, 2]), ([4, 4, 0, 0], 8));
        assert_eq!(values([2, 2, 2, 0]), ([4, 2, 0, 0], 4));
        assert_eq!(
            values([4, 4, 8, 0]),
            ([8, 8, 0, 0], 8),
            "an 8 made is not merged again"
        );
        assert_eq!(values([2, 0, 2, 4]), ([4, 4, 0, 0], 4));
        assert_eq!(values([0, 0, 0, 2]), ([2, 0, 0, 0], 0));
        assert_eq!(values([2, 4, 2, 4]), ([2, 4, 2, 4], 0));
        // where each tile went: a pair to one place, the next after it
        assert_eq!(merged([2, 2, 2, 2]).2, [0, 0, 1, 1]);
        assert_eq!(merged([2, 0, 2, 4]).2, [0, 0, 0, 1]);
        assert_eq!(merged([0, 4, 0, 8]).2, [0, 0, 0, 1]);
    }

    #[test]
    fn a_slide_reads_each_line_from_the_wall_it_slides_to() {
        let tiles = [[2, 2, 2, 0], [0; 4], [0; 4], [0; 4]];
        assert_eq!(slid(tiles, Slide::Right).0[0], [0, 0, 2, 4]);
        assert_eq!(slid(tiles, Slide::Left).0[0], [4, 2, 0, 0]);
        let column = [[2, 0, 0, 0], [2, 0, 0, 0], [4, 0, 0, 0], [0; 4]];
        let (down, points, _) = slid(column, Slide::Down);
        assert_eq!(down.map(|row| row[0]), [0, 0, 4, 4]);
        assert_eq!(points, 4);
    }

    #[test]
    fn a_move_that_changes_the_board_brings_one_tile_and_one_that_does_not_none() {
        let mut merge = Merge::new(3);
        merge.tiles = [[2, 0, 0, 0], [0; 4], [0; 4], [0; 4]];
        assert!(!merge.key("left"), "against the wall: nothing changes");
        assert_eq!(merge.tiles.iter().flatten().filter(|v| **v > 0).count(), 1);
        assert!(merge.key("right"));
        assert_eq!(merge.tiles[0][3], 2);
        assert_eq!(merge.tiles.iter().flatten().filter(|v| **v > 0).count(), 2);
        let new = merge.tiles.iter().flatten().copied().filter(|v| *v > 0);
        assert!(new.into_iter().all(|v| v == 2 || v == 4));
    }

    #[test]
    fn merges_score_their_sums() {
        let mut merge = Merge::new(3);
        merge.tiles = [[2, 2, 4, 4], [0; 4], [0; 4], [0; 4]];
        merge.key("left");
        assert_eq!(merge.tiles[0][..2], [4, 8]);
        assert_eq!(merge.score(), 12);
    }

    #[test]
    fn a_full_board_with_no_pair_is_over() {
        let checkered = [[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [4, 2, 4, 2]];
        assert!(stuck(checkered));
        let mut paired = checkered;
        paired[3][3] = 4;
        assert!(!stuck(paired), "two 4s side by side");
        let mut gap = checkered;
        gap[0][0] = 0;
        assert!(!stuck(gap));
        // a move that fills the board ends the game when it leaves no pair
        let mut merge = Merge::new(3);
        merge.tiles = [[2, 4, 2, 4], [4, 2, 4, 2], [2, 4, 2, 4], [0, 8, 16, 8]];
        assert!(merge.key("left"));
        assert_eq!(merge.tiles[3][..3], [8, 16, 8]);
        assert_eq!(merge.over(), merge.tiles[3][3] == 2, "{:?}", merge.tiles);
    }

    #[test]
    fn a_move_records_each_tiles_slide_its_merges_and_the_new_tile() {
        let mut merge = Merge::new(3);
        merge.tiles = [[2, 0, 2, 4], [0, 8, 0, 0], [0; 4], [0; 4]];
        assert_eq!(merge.period(), None, "at rest");
        assert!(merge.key("left"));
        let motion = merge.motion.clone().expect("the move is drawn");
        let glide = |value, from, to| Glide { value, from, to };
        assert_eq!(
            motion.glides,
            [
                glide(2, (0, 0), (0, 0)),
                glide(2, (2, 0), (0, 0)),
                glide(4, (3, 0), (1, 0)),
                glide(8, (1, 1), (0, 1)),
            ]
        );
        assert_eq!(motion.merged, [(0, 0)]);
        let (x, y) = motion.new.expect("a new tile");
        assert!(
            ![(0, 0), (1, 0), (0, 1)].contains(&(x, y)),
            "on an empty cell"
        );
        assert!(matches!(merge.tiles[y][x], 2 | 4));
        assert_eq!((motion.frame, merge.period()), (1, Some(PERIOD)));
        // a column reads from the wall it slides to
        merge.tiles = [[4, 0, 0, 0], [0; 4], [4, 0, 0, 0], [2, 0, 0, 0]];
        assert!(merge.key("down"));
        let motion = merge.motion.clone().expect("the move is drawn");
        assert_eq!(
            motion.glides,
            [
                glide(2, (0, 3), (0, 3)),
                glide(4, (0, 2), (0, 2)),
                glide(4, (0, 0), (0, 2)),
            ]
        );
        assert_eq!(motion.merged, [(0, 2)]);
    }

    #[test]
    fn a_key_mid_motion_lands_it_and_moves_again_and_the_game_never_waits() {
        let start = [[2, 2, 0, 0], [0; 4], [0; 4], [4, 0, 0, 0]];
        let (mut fast, mut slow) = (Merge::new(5), Merge::new(5));
        (fast.tiles, slow.tiles) = (start, start);
        assert!(fast.key("left"));
        let landed = fast.tiles;
        fast.tick();
        assert!(fast.key("down"), "the next move plays mid-slide");
        let motion = fast.motion.as_ref().expect("the next move is drawn");
        assert_eq!(motion.frame, 1, "from its start");
        // it slides from where the first move landed, not from mid-way
        let mut from: Vec<_> = motion.glides.iter().map(|g| (g.from, g.value)).collect();
        let mut held: Vec<_> = (0..SIDE)
            .flat_map(|y| (0..SIDE).map(move |x| (x, y)))
            .filter(|(x, y)| landed[*y][*x] > 0)
            .map(|(x, y)| ((x, y), landed[y][x]))
            .collect();
        from.sort();
        held.sort();
        assert_eq!(from, held);
        // the same keys with every motion run out play the same game
        slow.key("left");
        while slow.tick() {}
        slow.key("down");
        let mut frames = 1;
        while slow.tick() {
            frames += 1;
        }
        assert_eq!(frames, FRAMES, "the last frame is the board at rest");
        assert_eq!((fast.tiles, fast.score), (slow.tiles, slow.score));
        assert_eq!(slow.period(), None, "at rest");
        // an arrow that changes nothing leaves the motion as it was
        assert!(slow.key("up"));
        slow.tick();
        slow.tiles = [[0, 0, 0, 2], [0; 4], [0; 4], [0; 4]];
        assert!(!slow.key("right"));
        assert_eq!(slow.motion.as_ref().map(|m| m.frame), Some(2));
    }

    #[test]
    fn a_frame_draws_the_slide_then_the_pulse_and_the_pop_inside_the_board() {
        let mut merge = Merge::new(3);
        merge.tiles = [[0, 2, 0, 2], [0; 4], [0; 4], [0; 4]];
        assert!(merge.key("left"));
        let new = merge
            .motion
            .as_ref()
            .and_then(|m| m.new)
            .expect("a new tile");
        let mut slid = Vec::new();
        while let Some(frame) = merge.motion.as_ref().map(|m| m.frame) {
            let pieces = merge.pieces();
            if frame <= SLIDE {
                // the two 2s on their way to the wall; the far one the farther
                assert_eq!(pieces.iter().map(|p| p.0).collect::<Vec<_>>(), [2, 2]);
                slid.push(pieces[1].1.0);
            } else {
                let scale = |at: (f32, f32)| pieces.iter().find(|p| p.1 == at).map(|p| p.2);
                assert!(
                    scale((0., 0.)).is_some_and(|s| s > 1.),
                    "the 4 pulses: {pieces:?}"
                );
                let new = (new.0 as f32, new.1 as f32);
                assert!(
                    scale(new).is_some_and(|s| s < 1.),
                    "the new tile pops: {pieces:?}"
                );
            }
            merge.tick();
        }
        assert_eq!(slid.len(), usize::from(SLIDE));
        assert!(
            slid.windows(2).all(|w| w[1] < w[0]),
            "toward the wall: {slid:?}"
        );
        assert_eq!(slid.last(), Some(&0.), "on its cell when the slide ends");
        assert!(merge.pieces().iter().all(|p| p.2 == 1.), "at rest");
        // a tile at its largest stays on the board at every size play draws
        for cell in 40..=120 {
            let cell = cell as f32;
            let gap = (cell / 12.).round();
            let board = 4. * cell + 5. * gap;
            for at in [0., 3.] {
                let (left, top, side) = placed(cell, (at, at), 1. + PULSE);
                assert!(left >= 0. && top >= 0., "{cell}: {left}");
                assert!(left + side <= board, "{cell}: {left} + {side} > {board}");
            }
        }
    }
}
