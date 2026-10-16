//! The player: one key every [`Level::pace`] ticks, toward the best placement
//! there is from where the piece falls now. Every turn the piece reaches in
//! place and every column it slides to along its row, of the piece falling
//! and of the one hold would bring in, is dropped on a copy of the well and
//! scored by Pierre Dellacherie's six features with his own weights: landing
//! height -1, eroded piece cells +1, row transitions -1, column transitions
//! -1, holes -4, wells -1. Of equal scores the one fewer keys away wins.
//! The keys are the game's own (`Tetris::key_down`), read on the next tick
//! as a person's are: the piece turns, slides and drops, it never jumps.
//! The [`Level`] sets the pace, whether it holds, and how far off it judges.
use std::hash::{DefaultHasher, Hash, Hasher};

use arcade_view::tetris::{COLS, Piece, ROWS, Tetris, Well};

/// How well the player plays. A piece takes about four keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// 640 ms a key, about 3 s a piece; no hold, and placements misjudged.
    Easy,
    /// 352 ms a key, about 1.5 s a piece.
    Normal,
    /// 176 ms a key, about 0.7 s a piece.
    Hard,
    /// 96 ms a key, about 0.4 s a piece.
    Expert,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Easy, Level::Normal, Level::Hard, Level::Expert];

    pub fn name(self) -> &'static str {
        match self {
            Level::Easy => "easy",
            Level::Normal => "normal",
            Level::Hard => "hard",
            Level::Expert => "expert",
        }
    }

    /// Ticks of 16 ms between two keys.
    pub fn pace(self) -> u32 {
        match self {
            Level::Easy => 40,
            Level::Normal => 22,
            Level::Hard => 11,
            Level::Expert => 6,
        }
    }

    fn holds(self) -> bool {
        self != Level::Easy
    }

    /// The most a placement's score is misjudged by, either way. A hole
    /// costs 8, so easy now and then leaves one.
    fn blur(self) -> i32 {
        match self {
            Level::Easy => 10,
            _ => 0,
        }
    }
}

/// Where every piece comes into the well, unturned.
const SPAWN: (i32, i32) = (3, 0);

/// A placement's score, and the first key toward it.
type Choice = (i32, &'static str);

pub struct Player {
    level: Level,
    waited: u32,
}

impl Player {
    pub fn new(level: Level) -> Player {
        Player { level, waited: 0 }
    }

    /// One tick of the round's clock: on every [`Level::pace`]-th, one key.
    pub fn step(&mut self, tetris: &mut Tetris) {
        self.waited += 1;
        if self.waited >= self.level.pace() {
            self.waited = 0;
            press(tetris, self.level);
        }
    }
}

/// The key toward the best placement, down and up before the next tick.
pub fn press(tetris: &mut Tetris, level: Level) {
    if tetris.over() {
        return;
    }
    let key = choose(tetris, level);
    tetris.key_down(key, false);
    tetris.key_up(key);
}

/// Hold when the piece it brings in places better than the one falling.
fn choose(tetris: &Tetris, level: Level) -> &'static str {
    let falling = best(tetris, level, tetris.piece, tetris.turn, tetris.at);
    let swapped = match tetris.held || !level.holds() {
        true => None,
        false => best(
            tetris,
            level,
            tetris.hold.unwrap_or(tetris.next[0]),
            0,
            SPAWN,
        ),
    };
    match (falling, swapped) {
        (Some((score, _)), Some((swap, _))) if swap > score => "c",
        (Some((_, key)), _) => key,
        (None, Some(_)) => "c",
        (None, None) => "space",
    }
}

/// The best placement of `piece`, turned `from` at `(x0, y)`: each turn it
/// reaches there a quarter at a time with no kick, then each column it
/// slides to along row `y`, dropped.
fn best(
    tetris: &Tetris,
    level: Level,
    piece: Piece,
    from: u8,
    (x0, y): (i32, i32),
) -> Option<Choice> {
    let turns: [(&[u8], Option<&'static str>); 4] = [
        (&[0], None),
        (&[0, 1], Some("up")),
        (&[0, 3], Some("z")),
        (&[0, 1, 2], Some("up")),
    ];
    let mut best: Option<Choice> = None;
    for (way, turning) in turns {
        let turn = (from + way[way.len() - 1]) % 4;
        if !way
            .iter()
            .all(|quarter| tetris.fits(piece, (from + quarter) % 4, (x0, y)))
        {
            continue;
        }
        let reach = |step: i32| {
            (1..)
                .map(move |far| x0 + step * far)
                .take_while(|x| tetris.fits(piece, turn, (*x, y)))
        };
        let mut columns: Vec<i32> = std::iter::once(x0)
            .chain(reach(-1))
            .chain(reach(1))
            .collect();
        columns.sort_by_key(|x| (x - x0).abs());
        for x in columns {
            let mut landed = y;
            while tetris.fits(piece, turn, (x, landed + 1)) {
                landed += 1;
            }
            let score = score(&tetris.well, piece, turn, (x, landed))
                + misjudged(level, &tetris.well, piece, turn, x);
            let key = turning.unwrap_or(match x.cmp(&x0) {
                std::cmp::Ordering::Less => "left",
                std::cmp::Ordering::Greater => "right",
                std::cmp::Ordering::Equal => "space",
            });
            if best.is_none_or(|(held, _)| score > held) {
                best = Some((score, key));
            }
        }
    }
    best
}

/// How far `level` misjudges `piece` turned `turn` at column `x` over
/// `well`: the same all the while the piece falls, as the well stays put
/// till it locks, and another for the next piece.
fn misjudged(level: Level, well: &Well, piece: Piece, turn: u8, x: i32) -> i32 {
    let blur = level.blur();
    if blur == 0 {
        return 0;
    }
    let mut hash = DefaultHasher::new();
    well.iter()
        .flatten()
        .map(Option::is_some)
        .for_each(|cell| cell.hash(&mut hash));
    (piece as u8, turn, x).hash(&mut hash);
    (hash.finish() % (2 * blur as u64 + 1)) as i32 - blur
}

/// Dellacherie's evaluation of `piece` locked at `(x, y)` in `well`,
/// doubled so the landing height's half rows stay whole.
fn score(well: &Well, piece: Piece, turn: u8, (x, y): (i32, i32)) -> i32 {
    let cells = piece
        .cells(turn)
        .map(|(dx, dy)| ((x + dx) as usize, (y + dy) as usize));
    let mut locked = *well;
    for (cx, cy) in cells {
        locked[cy][cx] = Some(piece);
    }
    let full: Vec<usize> = (0..ROWS as usize)
        .filter(|row| locked[*row].iter().all(Option::is_some))
        .collect();
    let eroded = full.len() * cells.iter().filter(|(_, cy)| full.contains(cy)).count();
    let kept: Vec<_> = locked
        .iter()
        .filter(|row| row.iter().any(Option::is_none))
        .copied()
        .collect();
    let mut after = [[None; COLS as usize]; ROWS as usize];
    after[ROWS as usize - kept.len()..].copy_from_slice(&kept);
    let top = cells.iter().map(|(_, cy)| *cy).min().expect("four cells");
    let bottom = cells.iter().map(|(_, cy)| *cy).max().expect("four cells");
    // twice the height of the piece's middle over the floor
    let landing = (2 * (ROWS as usize - 1) - top - bottom) as i32;
    -landing + 2 * eroded as i32
        - 2 * (row_transitions(&after) + column_transitions(&after) + wells(&after))
        - 8 * holes(&after)
}

/// A wall counts as a block.
fn filled(well: &Well, x: i32, y: usize) -> bool {
    !(0..COLS).contains(&x) || well[y][x as usize].is_some()
}

/// Block to empty and back along every row that holds a block.
fn row_transitions(well: &Well) -> i32 {
    (0..ROWS as usize)
        .filter(|y| well[*y].iter().any(Option::is_some))
        .map(|y| {
            (0..=COLS)
                .filter(|x| filled(well, x - 1, y) != filled(well, *x, y))
                .count() as i32
        })
        .sum()
}

/// Block to empty and back up every column, the floor a block.
fn column_transitions(well: &Well) -> i32 {
    (0..COLS as usize)
        .map(|x| {
            let mut below = true;
            let mut count = 0;
            for row in well.iter().rev() {
                let here = row[x].is_some();
                count += i32::from(here != below);
                below = here;
            }
            count
        })
        .sum()
}

/// Empty cells under a block of their column.
fn holes(well: &Well) -> i32 {
    (0..COLS as usize)
        .map(|x| {
            let mut roofed = false;
            let mut count = 0;
            for row in well {
                match row[x] {
                    Some(_) => roofed = true,
                    None if roofed => count += 1,
                    None => {}
                }
            }
            count
        })
        .sum()
}

/// Every well, 1 + 2 + … + its depth: empty cells walled in on both sides.
fn wells(well: &Well) -> i32 {
    (0..COLS)
        .map(|x| {
            let (mut depth, mut sum) = (0, 0);
            for y in 0..ROWS as usize {
                match well[y][x as usize].is_none()
                    && filled(well, x - 1, y)
                    && filled(well, x + 1, y)
                {
                    true => {
                        depth += 1;
                        sum += depth;
                    }
                    false => depth = 0,
                }
            }
            sum
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces locked so far: four cells each, ten gone with each row.
    fn placed(tetris: &Tetris) -> u64 {
        let cells = tetris
            .well
            .iter()
            .flatten()
            .filter(|cell| cell.is_some())
            .count() as u64;
        (cells + 10 * tetris.lines) / 4
    }

    #[test]
    fn every_piece_comes_in_where_the_player_plans_from() {
        let tetris = Tetris::new(1);
        assert_eq!((tetris.at, tetris.turn), (SPAWN, 0));
    }

    #[test]
    fn five_hundred_pieces_on_a_seed_never_top_out_and_clear_rows() {
        let mut tetris = Tetris::new(42);
        let mut ticks = 0;
        while placed(&tetris) < 500 {
            press(&mut tetris, Level::Normal);
            tetris.tick();
            ticks += 1;
            assert!(
                !tetris.over(),
                "topped out after {} pieces",
                placed(&tetris)
            );
            assert!(ticks < 100_000, "stalled at {} pieces", placed(&tetris));
        }
        // 500 pieces are 200 rows of cells: nearly all of them cleared
        assert!(tetris.lines >= 190, "{} rows cleared", tetris.lines);
    }

    #[test]
    fn hold_brings_in_the_piece_that_places_better() {
        let mut tetris = Tetris::new(7);
        tetris.well = [[None; COLS as usize]; ROWS as usize];
        // four rows full but for the first column: an I's
        for row in &mut tetris.well[ROWS as usize - 4..] {
            *row = [Some(Piece::O); COLS as usize];
            row[0] = None;
        }
        tetris.piece = Piece::S;
        tetris.hold = Some(Piece::I);
        assert_eq!(choose(&tetris, Level::Normal), "c");
        while tetris.lines == 0 {
            press(&mut tetris, Level::Normal);
            tetris.tick();
        }
        assert_eq!(tetris.lines, 4, "the I went down the well");
        assert_eq!(tetris.hold, Some(Piece::S));
    }

    #[test]
    fn at_its_pace_each_level_presses_one_key_every_pace_ticks() {
        for level in Level::ALL {
            let mut tetris = Tetris::new(3);
            let mut player = Player::new(level);
            let keyed = |tetris: &Tetris| (tetris.at.0, tetris.turn, tetris.hold, tetris.next);
            let before = keyed(&tetris);
            for _ in 0..level.pace() - 1 {
                player.step(&mut tetris);
                tetris.tick();
            }
            assert_eq!(keyed(&tetris), before, "{level:?}: no key yet");
            player.step(&mut tetris);
            tetris.tick();
            assert_ne!(
                keyed(&tetris),
                before,
                "{level:?}: a key on the pace-th tick"
            );
        }
    }

    #[test]
    fn the_levels_play_slower_to_faster() {
        let paces: Vec<u32> = Level::ALL.iter().map(|level| level.pace()).collect();
        assert!(paces.is_sorted_by(|slow, fast| slow > fast), "{paces:?}");
    }

    #[test]
    fn easy_never_holds_and_misjudges_some_placements() {
        let mut tetris = Tetris::new(7);
        tetris.well = [[None; COLS as usize]; ROWS as usize];
        for row in &mut tetris.well[ROWS as usize - 4..] {
            *row = [Some(Piece::O); COLS as usize];
            row[0] = None;
        }
        tetris.piece = Piece::S;
        tetris.hold = Some(Piece::I);
        assert_ne!(choose(&tetris, Level::Easy), "c");
        // the first key of 200 pieces' plans, easy against normal
        let mut differs = 0;
        let mut tetris = Tetris::new(11);
        for _ in 0..200 {
            let easy = best(&tetris, Level::Easy, tetris.piece, tetris.turn, tetris.at);
            let normal = best(&tetris, Level::Normal, tetris.piece, tetris.turn, tetris.at);
            differs += usize::from(easy.map(|(_, key)| key) != normal.map(|(_, key)| key));
            tetris.key_down("space", false);
            tetris.key_up("space");
            tetris.tick();
            if tetris.over() {
                tetris = Tetris::new(12);
            }
        }
        assert!(
            (20..180).contains(&differs),
            "{differs} of 200 plans differ"
        );
    }
}
