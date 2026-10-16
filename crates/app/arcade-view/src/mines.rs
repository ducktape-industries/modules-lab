//! Minesweeper: forty mines under a field of sixteen by sixteen. A shown
//! cell tells how many of its eight neighbours hold a mine; a cell with
//! none shows its neighbours too, and so on out. The mines are laid at the
//! first reveal, never on that cell or around it, so the first press always
//! opens ground. Flag a cell you think holds a mine; a shown count with as
//! many flags around it shows the rest of its neighbours when it is pressed
//! again (a chord). Showing a mine ends the game, showing every other cell
//! wins it. The score is a point a cell shown, and a cleared field adds
//! 1,000 less a point a second. The clock runs from the first reveal, a
//! tick a second, only to count them.
use std::collections::VecDeque;

use gpui::point;
use view::prelude::*;
use view::{Canvas, Div, FontWeight};

use crate::games::{Area, Fact, Rng, Still, square};

pub(crate) const COLS: i32 = 16;
pub(crate) const ROWS: i32 = 16;
pub(crate) const MINES: usize = 40;
pub(crate) const KEYS: &str = "Click shows, right click flags; or arrows, Space shows, F flags";
pub(crate) const PERIOD: i64 = 1000;
/// What a cleared field adds, less a point a second.
const CLEARED: u64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Hidden,
    Shown,
    Flagged,
}

pub(crate) struct Mines {
    pub(crate) mine: [[bool; COLS as usize]; ROWS as usize],
    /// How many neighbours hold a mine.
    pub(crate) near: [[u8; COLS as usize]; ROWS as usize],
    pub(crate) mark: [[Mark; COLS as usize]; ROWS as usize],
    /// Whether the mines are laid: at the first reveal.
    laid: bool,
    /// The cell the keys are on.
    pub(crate) cursor: (i32, i32),
    shown: usize,
    flags: usize,
    seconds: u64,
    /// The mine shown, which ended the game.
    pub(crate) hit: Option<(i32, i32)>,
    won: bool,
    rng: Rng,
}

impl Mines {
    pub(crate) fn new(seed: u64) -> Mines {
        Mines {
            mine: [[false; COLS as usize]; ROWS as usize],
            near: [[0; COLS as usize]; ROWS as usize],
            mark: [[Mark::Hidden; COLS as usize]; ROWS as usize],
            laid: false,
            cursor: (COLS / 2, ROWS / 2),
            shown: 0,
            flags: 0,
            seconds: 0,
            hit: None,
            won: false,
            rng: Rng::new(seed),
        }
    }

    /// The mines, anywhere but on `(x, y)` and its neighbours, each place
    /// alike; then each cell's count.
    fn lay(&mut self, (x, y): (i32, i32)) {
        let mut free: Vec<(i32, i32)> = cells()
            .filter(|(cx, cy)| (cx - x).abs() > 1 || (cy - y).abs() > 1)
            .collect();
        for _ in 0..MINES {
            let at = self.rng.below(free.len() as u64) as usize;
            let (mx, my) = free.swap_remove(at);
            self.mine[my as usize][mx as usize] = true;
        }
        for (cx, cy) in cells() {
            self.near[cy as usize][cx as usize] = neighbours((cx, cy))
                .filter(|(nx, ny)| self.mine[*ny as usize][*nx as usize])
                .count() as u8;
        }
        self.laid = true;
    }

    /// Shows `(x, y)`: a mine ends the game; a cell with no mine around it
    /// shows its neighbours, and theirs, out to the cells that count one.
    /// A shown count chords.
    pub(crate) fn reveal(&mut self, (x, y): (i32, i32)) -> bool {
        if self.over() {
            return false;
        }
        match self.mark[y as usize][x as usize] {
            Mark::Hidden => {}
            Mark::Shown => return self.chord((x, y)),
            Mark::Flagged => return false,
        }
        if !self.laid {
            self.lay((x, y));
        }
        if self.mine[y as usize][x as usize] {
            self.hit = Some((x, y));
            for (cx, cy) in cells() {
                if self.mine[cy as usize][cx as usize] {
                    self.mark[cy as usize][cx as usize] = Mark::Shown;
                }
            }
            return true;
        }
        let mut open = VecDeque::from([(x, y)]);
        while let Some((cx, cy)) = open.pop_front() {
            let mark = &mut self.mark[cy as usize][cx as usize];
            if *mark != Mark::Hidden {
                continue;
            }
            *mark = Mark::Shown;
            self.shown += 1;
            if self.near[cy as usize][cx as usize] == 0 {
                open.extend(neighbours((cx, cy)));
            }
        }
        let cleared = cells().all(|(cx, cy)| {
            self.mine[cy as usize][cx as usize]
                || self.mark[cy as usize][cx as usize] == Mark::Shown
        });
        if cleared {
            self.won = true;
            for (cx, cy) in cells() {
                if self.mine[cy as usize][cx as usize] {
                    self.mark[cy as usize][cx as usize] = Mark::Flagged;
                }
            }
            self.flags = MINES;
        }
        true
    }

    /// A shown count with as many flags around it shows each hidden
    /// neighbour as a press would: a flag in the wrong place leaves a mine
    /// among them, and that ends the game. A neighbour an earlier one's
    /// flood showed is passed over, not chorded in turn: a chord opens
    /// round the count pressed and nowhere else.
    fn chord(&mut self, at: (i32, i32)) -> bool {
        let marked = |mark| {
            neighbours(at)
                .filter(|(x, y)| self.mark[*y as usize][*x as usize] == mark)
                .collect::<Vec<_>>()
        };
        let near = usize::from(self.near[at.1 as usize][at.0 as usize]);
        if marked(Mark::Flagged).len() != near {
            return false;
        }
        let mut shown = false;
        for (x, y) in marked(Mark::Hidden) {
            if self.mark[y as usize][x as usize] == Mark::Hidden {
                shown |= self.reveal((x, y));
            }
        }
        shown
    }

    /// Flags a hidden cell, or takes its flag off.
    pub(crate) fn flag(&mut self, (x, y): (i32, i32)) -> bool {
        if self.over() {
            return false;
        }
        let mark = &mut self.mark[y as usize][x as usize];
        match *mark {
            Mark::Hidden => {
                *mark = Mark::Flagged;
                self.flags += 1;
            }
            Mark::Flagged => {
                *mark = Mark::Hidden;
                self.flags -= 1;
            }
            Mark::Shown => return false,
        }
        true
    }

    /// The arrows move the cursor, held ones on the keyboard's repeat;
    /// Space or Enter shows its cell, F flags it, once a press.
    pub(crate) fn key(&mut self, key: &str, repeat: bool) -> bool {
        let (x, y) = self.cursor;
        let step = match key {
            "up" | "w" => (0, -1),
            "down" | "s" => (0, 1),
            "left" | "a" => (-1, 0),
            "right" | "d" => (1, 0),
            _ if repeat => return false,
            "space" | "enter" => return self.reveal(self.cursor),
            "f" => return self.flag(self.cursor),
            _ => return false,
        };
        let to = (
            (x + step.0).clamp(0, COLS - 1),
            (y + step.1).clamp(0, ROWS - 1),
        );
        let moved = to != self.cursor;
        self.cursor = to;
        moved
    }

    /// A press on cell `(x, y)`: the left button shows it, the right flags it.
    pub(crate) fn press(&mut self, at: (i32, i32), right: bool) -> bool {
        if !(0..COLS).contains(&at.0) || !(0..ROWS).contains(&at.1) {
            return false;
        }
        self.cursor = at;
        match right {
            true => self.flag(at),
            false => self.reveal(at),
        }
    }

    /// A second of the game.
    pub(crate) fn tick(&mut self) -> bool {
        self.seconds += 1;
        true
    }

    /// A tick a second from the first reveal; none before it or after the end.
    pub(crate) fn period(&self) -> Option<i64> {
        self.laid.then_some(PERIOD)
    }

    pub(crate) fn score(&self) -> u64 {
        let bonus = match self.won {
            true => CLEARED - self.seconds.min(CLEARED - 1),
            false => 0,
        };
        self.shown as u64 + bonus
    }

    pub(crate) fn over(&self) -> bool {
        self.won || self.hit.is_some()
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        vec![
            ("Mines left", (MINES as i64 - self.flags as i64).to_string()),
            ("Time", format!("{} s", self.seconds)),
        ]
    }

    /// The field, `cell` px a cell, on a canvas, and each shown count as
    /// text laid over it.
    pub(crate) fn draw(&self, cell: f32, theme: &Theme) -> Div {
        let (width, height) = (COLS as f32 * cell, ROWS as f32 * cell);
        let mut drawn = canvas()
            .absolute()
            .top_0()
            .left_0()
            .w(px(width))
            .h(px(height))
            .rect(square(0., 0., width, height), theme.border_strong);
        let mut counts = Vec::new();
        for (x, y) in cells() {
            let (left, top) = (x as f32 * cell, y as f32 * cell);
            let inner = square(left + 1., top + 1., cell - 1., cell - 1.);
            let (mark, near) = (
                self.mark[y as usize][x as usize],
                self.near[y as usize][x as usize],
            );
            let mine = self.mine[y as usize][x as usize];
            drawn = match mark {
                Mark::Hidden => drawn.rect(inner, theme.border),
                Mark::Flagged => {
                    let pole = left + cell * 0.4;
                    drawn
                        .rect(inner, theme.surface_raised)
                        .rect(
                            square(pole, top + cell * 0.25, 1.5, cell * 0.5),
                            theme.foreground,
                        )
                        .rect(
                            square(pole, top + cell * 0.25, cell * 0.3, cell * 0.2),
                            theme.danger,
                        )
                }
                Mark::Shown if mine => {
                    let ground = match self.hit == Some((x, y)) {
                        true => theme.danger_soft,
                        false => theme.background,
                    };
                    let middle = point(px(left + cell / 2.), px(top + cell / 2.));
                    drawn
                        .rect(inner, ground)
                        .circle(middle, px(cell * 0.22), theme.foreground)
                }
                Mark::Shown => {
                    if near > 0 {
                        counts.push((left, top, near));
                    }
                    drawn.rect(inner, theme.background)
                }
            };
        }
        if !self.over() {
            let (x, y) = self.cursor;
            let (left, top) = (x as f32 * cell, y as f32 * cell);
            for edge in [
                square(left, top, cell + 1., 2.),
                square(left, top + cell - 1., cell + 1., 2.),
                square(left, top, 2., cell + 1.),
                square(left + cell - 1., top, 2., cell + 1.),
            ] {
                drawn = drawn.rect(edge, theme.foreground);
            }
        }
        let counts = counts.into_iter().map(|(left, top, near)| {
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .size(px(cell))
                .flex()
                .items_center()
                .justify_center()
                .font_family(design::fonts::FAMILY_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(cell * 0.55))
                .text_color(ink(near, theme))
                .child(near.to_string())
        });
        div()
            .relative()
            .w(px(width + 1.))
            .h(px(height + 1.))
            .child(drawn)
            .children(counts)
    }
}

/// The colour a count is told in.
fn ink(near: u8, theme: &Theme) -> Hsla {
    match near {
        1 => theme.agent,
        2 => theme.success,
        3 => theme.danger,
        4 => theme.warning,
        _ => theme.foreground,
    }
}

/// Every cell, rows top first.
fn cells() -> impl Iterator<Item = (i32, i32)> {
    (0..ROWS).flat_map(|y| (0..COLS).map(move |x| (x, y)))
}

/// The cells around `(x, y)` on the field.
fn neighbours((x, y): (i32, i32)) -> impl Iterator<Item = (i32, i32)> {
    (-1..=1)
        .flat_map(move |dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
        .filter(move |cell| *cell != (x, y))
        .filter(|(x, y)| (0..COLS).contains(x) && (0..ROWS).contains(y))
}

/// The hub's picture of the game: a corner of a field half cleared, a
/// `#` a covered cell.
const PICTURE: [&str; 6] = [
    "##....##", "#.....##", "#......#", "##.....#", "###..###", "########",
];

/// A cell of [`PICTURE`], and its top left in
/// [`STILL`](crate::games::STILL).
const PICTURE_CELL: f32 = 14.;
const PICTURE_AT: (f32, f32) = (48., 18.);

/// The part of [`STILL`](crate::games::STILL) the picture draws on.
pub(crate) const DRAWN: Area = (
    PICTURE_AT.0,
    PICTURE_AT.1,
    PICTURE[0].len() as f32 * PICTURE_CELL,
    PICTURE.len() as f32 * PICTURE_CELL,
);

/// [`PICTURE`] with a flag on it, where `still` puts it.
pub(crate) fn preview(still: Still, theme: &Theme) -> Canvas {
    let (cell, (left, top)) = (PICTURE_CELL, PICTURE_AT);
    let mut drawn = canvas().size_full();
    for (y, row) in PICTURE.iter().enumerate() {
        for (x, mark) in row.chars().enumerate() {
            let at = still.square(
                left + x as f32 * cell,
                top + y as f32 * cell,
                cell - 1.,
                cell - 1.,
            );
            drawn = drawn.rect(
                at,
                match mark {
                    '#' => theme.border_strong,
                    _ => theme.background,
                },
            );
        }
    }
    let pole = left + 7. * cell + 5.;
    drawn
        .rect(
            still.square(pole, top + 4. * cell + 3., 1.5, 7.),
            theme.foreground,
        )
        .rect(
            still.square(pole, top + 4. * cell + 3., 4., 3.),
            theme.danger,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(mines: &Mines, mark: Mark) -> usize {
        mines.mark.iter().flatten().filter(|m| **m == mark).count()
    }

    #[test]
    fn the_first_reveal_never_lands_on_or_beside_a_mine() {
        for seed in 0..64 {
            let mut mines = Mines::new(seed);
            let first = ((seed % 16) as i32, (seed / 4 % 16) as i32);
            mines.reveal(first);
            assert!(!mines.over(), "seed {seed}");
            assert_eq!(mines.mine.iter().flatten().filter(|m| **m).count(), MINES);
            let around = neighbours(first).chain([first]);
            assert!(
                around
                    .into_iter()
                    .all(|(x, y)| !mines.mine[y as usize][x as usize])
            );
            // a first cell with no mine around it opens more than itself
            assert!(count(&mines, Mark::Shown) > 1, "seed {seed}");
        }
    }

    /// A field laid by hand: mines where `rows` has `*`.
    fn laid(rows: &[&str]) -> Mines {
        let mut mines = Mines::new(1);
        mines.laid = true;
        for (cx, cy) in cells() {
            mines.mark[cy as usize][cx as usize] = Mark::Hidden;
        }
        for (y, row) in rows.iter().enumerate() {
            for (x, mark) in row.chars().enumerate() {
                mines.mine[y][x] = mark == '*';
            }
        }
        for (cx, cy) in cells() {
            mines.near[cy as usize][cx as usize] = neighbours((cx, cy))
                .filter(|(nx, ny)| mines.mine[*ny as usize][*nx as usize])
                .count() as u8;
        }
        mines
    }

    #[test]
    fn a_cell_with_no_mine_around_shows_out_to_the_counted_cells() {
        // one mine in the top left corner: everything else opens
        let mut mines = laid(&["*"]);
        assert!(mines.reveal((10, 10)));
        assert_eq!(count(&mines, Mark::Shown), (COLS * ROWS) as usize - 1);
        assert!(mines.won && mines.over());
        assert_eq!(
            mines.mark[0][0],
            Mark::Flagged,
            "a won field flags its mines"
        );
        // the three counted cells stop the flood but are shown
        assert_eq!(mines.near[1][1], 1);
    }

    #[test]
    fn the_flood_stops_at_counted_cells() {
        // the top left corner fenced in by mines
        let mut mines = laid(&["..*", "..*", "***"]);
        mines.reveal((0, 0));
        // the corner's four cells open, the first with no count
        assert_eq!(count(&mines, Mark::Shown), 4);
        assert_eq!(mines.near[0][0], 0);
        assert_eq!(mines.near[1][1], 5);
        assert_eq!(mines.mark[0][3], Mark::Hidden);
        assert!(!mines.over());
    }

    #[test]
    fn a_flag_keeps_its_cell_from_a_reveal_and_counts_against_the_mines() {
        let mut mines = laid(&["*"]);
        mines.flag((0, 0));
        assert_eq!(mines.facts()[0].1, "39");
        assert!(!mines.reveal((0, 0)), "a flagged cell does not open");
        assert!(!mines.over());
        mines.flag((0, 0));
        assert_eq!(mines.facts()[0].1, "40");
    }

    #[test]
    fn showing_a_mine_ends_the_game_and_shows_them_all() {
        let mut mines = laid(&["*..*"]);
        mines.reveal((3, 0));
        assert!(mines.over());
        assert_eq!(mines.hit, Some((3, 0)));
        assert_eq!(mines.mark[0][0], Mark::Shown);
        assert_eq!(mines.score(), 0);
        assert!(!mines.reveal((8, 8)), "an ended field does not open");
    }

    #[test]
    fn a_cleared_field_scores_its_cells_and_the_seconds_under_1000() {
        let mut mines = laid(&["*"]);
        for _ in 0..90 {
            mines.tick();
        }
        mines.reveal((10, 10));
        assert_eq!(mines.score(), 255 + 1000 - 90);
    }

    #[test]
    fn the_clock_runs_from_the_first_reveal() {
        let mut mines = Mines::new(5);
        assert_eq!(mines.period(), None);
        mines.key("space", false);
        assert_eq!(mines.period(), Some(PERIOD));
    }

    /// Two mines over a count of two at (1, 1), and a fence of mines that
    /// stops a flood three rows down.
    const CHORD: [&str; 4] = ["*.*", "", "", "****************"];

    #[test]
    fn a_count_pressed_again_with_its_flags_placed_shows_the_rest_around_it() {
        let mut mines = laid(&CHORD);
        assert!(mines.press((1, 1), false));
        assert_eq!(count(&mines, Mark::Shown), 1);
        mines.flag((0, 0));
        assert!(!mines.press((1, 1), false), "one flag of two: nothing");
        assert_eq!(count(&mines, Mark::Shown), 1);
        mines.flag((2, 0));
        assert!(mines.press((1, 1), false));
        // its six other neighbours, each a count
        assert_eq!(count(&mines, Mark::Shown), 7);
        assert_eq!(mines.mark[2][0], Mark::Shown);
        assert!(!mines.over());
        // the keys chord the same
        let mut keyed = laid(&CHORD);
        keyed.cursor = (1, 1);
        keyed.key("space", false);
        keyed.flag((0, 0));
        keyed.flag((2, 0));
        assert!(keyed.key("space", false));
        assert_eq!(count(&keyed, Mark::Shown), 7);
    }

    #[test]
    fn a_chord_round_a_flag_in_the_wrong_place_shows_the_mine_and_ends_the_game() {
        let mut mines = laid(&CHORD);
        mines.reveal((1, 1));
        mines.flag((0, 0));
        mines.flag((1, 0));
        assert!(mines.reveal((1, 1)));
        assert!(mines.over());
        assert_eq!(mines.hit, Some((2, 0)));
        assert_eq!(mines.mark[0][2], Mark::Shown);
    }

    #[test]
    fn a_chord_opens_round_the_count_pressed_and_not_round_one_its_flood_showed() {
        // the chord at (1, 1) floods out from (2, 0) and shows (2, 2), a 1
        // with a wrong flag at (3, 3) beside it: (2, 2) is not chorded
        let mut mines = laid(&["*", "", "", "..*", "", "", "****************"]);
        mines.reveal((1, 1));
        mines.flag((0, 0));
        mines.flag((3, 3));
        assert!(mines.reveal((1, 1)));
        assert_eq!(mines.mark[2][2], Mark::Shown);
        assert!(!mines.over());
        assert_eq!(mines.mark[3][2], Mark::Hidden, "the mine under (2, 2)");
    }

    #[test]
    fn a_held_f_or_space_acts_once_and_a_held_arrow_goes_on() {
        let mut mines = laid(&["*"]);
        mines.cursor = (0, 0);
        assert!(mines.key("f", false));
        for _ in 0..3 {
            assert!(!mines.key("f", true), "the keyboard's repeat");
        }
        assert_eq!(mines.mark[0][0], Mark::Flagged, "one flag, kept");
        assert!(mines.key("right", true));
        assert!(mines.key("right", true));
        assert_eq!(mines.cursor, (2, 0));
        assert!(!mines.key("space", true));
        assert_eq!(mines.mark[0][2], Mark::Hidden);
    }

    #[test]
    fn the_arrows_move_the_cursor_inside_the_field() {
        let mut mines = Mines::new(5);
        mines.cursor = (0, 0);
        assert!(!mines.key("left", false));
        assert!(mines.key("right", false));
        assert_eq!(mines.cursor, (1, 0));
        assert!(mines.press((4, 5), true));
        assert_eq!(mines.mark[5][4], Mark::Flagged);
        assert!(!mines.press((COLS, 0), false), "off the field");
    }
}
