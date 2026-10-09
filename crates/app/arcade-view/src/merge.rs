//! 2048: tiles on a board of four by four slide all the way as an arrow
//! says; two equal tiles that meet become one of their sum, which scores
//! it. Each tile merges once a move, the pair nearest the wall first.
//! A move that changes the board brings a new tile, a 2 nine times in
//! ten and a 4 the tenth; when no move changes it, the game is over. It
//! moves on a key alone: no clock.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Canvas, Div, FontWeight};

use crate::games::{Fact, Rng, square};

pub(crate) const SIDE: usize = 4;
pub(crate) const KEYS: &str = "Arrows slide the tiles";

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
}

impl Merge {
    pub(crate) fn new(seed: u64) -> Merge {
        let mut merge = Merge {
            tiles: [[0; SIDE]; SIDE],
            score: 0,
            over: false,
            rng: Rng::new(seed),
        };
        merge.spawn();
        merge.spawn();
        merge
    }

    /// A 2, or a 4 one time in ten, on an empty cell.
    fn spawn(&mut self) {
        let empty: Vec<(usize, usize)> = (0..SIDE)
            .flat_map(|y| (0..SIDE).map(move |x| (x, y)))
            .filter(|(x, y)| self.tiles[*y][*x] == 0)
            .collect();
        if empty.is_empty() {
            return;
        }
        let (x, y) = empty[self.rng.below(empty.len() as u64) as usize];
        self.tiles[y][x] = if self.rng.below(10) == 0 { 4 } else { 2 };
    }

    /// An arrow slides the board at once. Whether it changed.
    pub(crate) fn key(&mut self, key: &str) -> bool {
        let slide = match key {
            "left" | "a" => Slide::Left,
            "right" | "d" => Slide::Right,
            "up" | "w" => Slide::Up,
            "down" | "s" => Slide::Down,
            _ => return false,
        };
        let (tiles, points) = slid(self.tiles, slide);
        if tiles == self.tiles {
            return false;
        }
        self.tiles = tiles;
        self.score += points;
        self.spawn();
        self.over = stuck(self.tiles);
        true
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
    /// tile, with a gap between them.
    pub(crate) fn draw(&self, cell: f32, theme: &Theme) -> Div {
        let theme = *theme;
        let gap = (cell / 12.).round();
        let tile = |value: u32| {
            let (fill, ink) = shade(value, &theme);
            div()
                .size(px(cell))
                .flex()
                .items_center()
                .justify_center()
                .bg(fill)
                .text_color(ink)
                .font_family(design::fonts::FAMILY_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(if value >= 1024 {
                    cell * 0.26
                } else {
                    cell * 0.32
                }))
                .when(value > 0, |tile| tile.child(value.to_string()))
        };
        let row = |cells: &[u32; SIDE]| div().flex().gap(px(gap)).children(cells.map(tile));
        div()
            .p(px(gap))
            .flex()
            .flex_col()
            .gap(px(gap))
            .bg(theme.border)
            .children(self.tiles.iter().map(row))
    }
}

/// `tiles` slid one way, and what its merges scored.
fn slid(mut tiles: [[u32; SIDE]; SIDE], slide: Slide) -> ([[u32; SIDE]; SIDE], u64) {
    let mut points = 0;
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
        let (merged, scored) = merged(read);
        points += scored;
        for (at, value) in merged.into_iter().enumerate() {
            let (x, y) = cell(at);
            tiles[y][x] = value;
        }
    }
    (tiles, points)
}

/// Whether no slide changes `tiles`: the game is over.
fn stuck(tiles: [[u32; SIDE]; SIDE]) -> bool {
    [Slide::Left, Slide::Right, Slide::Up, Slide::Down]
        .into_iter()
        .all(|slide| slid(tiles, slide).0 == tiles)
}

/// One line slid toward its start: equal neighbours merge, the pair
/// nearest the start first, each tile once.
fn merged(line: [u32; SIDE]) -> ([u32; SIDE], u64) {
    let mut out = [0; SIDE];
    let (mut at, mut points) = (0, 0);
    let mut waiting: Option<u32> = None;
    for value in line.into_iter().filter(|value| *value > 0) {
        match waiting {
            Some(held) if held == value => {
                out[at] = held * 2;
                points += u64::from(held * 2);
                at += 1;
                waiting = None;
            }
            Some(held) => {
                out[at] = held;
                at += 1;
                waiting = Some(value);
            }
            None => waiting = Some(value),
        }
    }
    if let Some(held) = waiting {
        out[at] = held;
    }
    (out, points)
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
        assert_eq!(merged([2, 2, 2, 2]), ([4, 4, 0, 0], 8));
        assert_eq!(merged([2, 2, 2, 0]), ([4, 2, 0, 0], 4));
        assert_eq!(
            merged([4, 4, 8, 0]),
            ([8, 8, 0, 0], 8),
            "an 8 made is not merged again"
        );
        assert_eq!(merged([2, 0, 2, 4]), ([4, 4, 0, 0], 4));
        assert_eq!(merged([0, 0, 0, 2]), ([2, 0, 0, 0], 0));
        assert_eq!(merged([2, 4, 2, 4]), ([2, 4, 2, 4], 0));
    }

    #[test]
    fn a_slide_reads_each_line_from_the_wall_it_slides_to() {
        let tiles = [[2, 2, 2, 0], [0; 4], [0; 4], [0; 4]];
        assert_eq!(slid(tiles, Slide::Right).0[0], [0, 0, 2, 4]);
        assert_eq!(slid(tiles, Slide::Left).0[0], [4, 2, 0, 0]);
        let column = [[2, 0, 0, 0], [2, 0, 0, 0], [4, 0, 0, 0], [0; 4]];
        let (down, points) = slid(column, Slide::Down);
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
}
