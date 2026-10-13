//! Breakout's walls: eight stages, each a wall laid out by hand, then the
//! eight again. A wall is thirteen bricks across and ten rows down; a
//! brick takes one to three hits, or is steel and never breaks.

/// The wall's bricks across and its rows.
pub(crate) const COLS: usize = 13;
pub(crate) const ROWS: usize = 10;
/// A brick that never breaks: the stage is clear without it.
pub(crate) const STEEL: u8 = u8::MAX;
/// The stages before the walls come round again.
pub(crate) const COUNT: u32 = STAGES.len() as u32;

/// Each brick's hits left, the top row first; none is a gap.
pub(crate) type Wall = [[u8; COLS]; ROWS];

/// Each stage's wall, the top row first: `.` a gap, a digit a brick of that
/// many hits, `#` steel.
const STAGES: [[&str; ROWS]; 8] = [
    // rows, as Atari laid them
    [
        ".............",
        ".............",
        "1111111111111",
        "1111111111111",
        "1111111111111",
        "1111111111111",
        "1111111111111",
        "1111111111111",
        ".............",
        ".............",
    ],
    // a pyramid with a core of two hits
    [
        ".............",
        "......1......",
        ".....111.....",
        "....11211....",
        "...1122211...",
        "..111222111..",
        ".11112221111.",
        "1111111111111",
        ".............",
        ".............",
    ],
    // battlements: teeth of steel and three hits under the wall, four ways
    // in; no two steel teeth side by side, so a ball over them falls past
    [
        ".............",
        "2222222222222",
        "1111111111111",
        "1111111111111",
        "1111111111111",
        "1111111111111",
        ".............",
        "3#.#3.3.3#.#3",
        ".............",
        ".............",
    ],
    // a checkerboard over steel posts, clear of the side walls
    [
        "2.2.2.2.2.2.2",
        ".1.1.1.1.1.1.",
        "2.2.2.2.2.2.2",
        ".1.1.1.1.1.1.",
        "2.2.2.2.2.2.2",
        ".1.1.1.1.1.1.",
        ".............",
        "..#...#...#..",
        ".............",
        ".............",
    ],
    // an invader with a body of three hits
    [
        ".............",
        "...1.....1...",
        "....1...1....",
        "...2222222...",
        "..22.222.22..",
        ".33333333333.",
        ".3.3333333.3.",
        ".3.1.....1.3.",
        "....11.11....",
        ".............",
    ],
    // a steel vault wall to wall, a door under it, a row in front: no way
    // round it to its roof
    [
        ".............",
        "#############",
        "#.111111111.#",
        "#.122222221.#",
        "#.123333321.#",
        "#.122222221.#",
        "#.111111111.#",
        "#####...#####",
        ".............",
        "1111111111111",
    ],
    // stairs: steel ledges the ball climbs from side to side, three
    // bricks wide, so one over a ledge soon falls past its end
    [
        "3333333333333",
        "2222222222222",
        "###..........",
        "2222222222222",
        "1111111111111",
        "..........###",
        "2222222222222",
        "1111111111111",
        "###..........",
        ".............",
    ],
    // a fortress: pockets of three hits in steel over a wall of three hits
    // with two gates; a steel wall there would shut a ball in over it
    [
        "2222222222222",
        "2#3#3#3#3#3#2",
        "2333333333332",
        "2#2#2#2#2#2#2",
        "2222222222222",
        "33..33333..33",
        ".............",
        "2222222222222",
        ".............",
        ".............",
    ],
];

/// Stage `stage`'s wall (the first is 0): the eight in turn, round and
/// round.
pub(crate) fn wall(stage: u32) -> Wall {
    let mut wall = [[0; COLS]; ROWS];
    for (row, line) in STAGES[(stage % COUNT) as usize].iter().enumerate() {
        for (col, brick) in line.bytes().enumerate() {
            wall[row][col] = match brick {
                b'.' => 0,
                b'#' => STEEL,
                hits => hits - b'0',
            };
        }
    }
    wall
}

/// A brick that breaks and still stands.
pub(crate) fn breakable(brick: u8) -> bool {
    brick != 0 && brick != STEEL
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bricks the ball can come to from under the wall, gap by gap and
    /// brick by breakable brick, across and down but never through steel.
    fn reached(wall: &Wall) -> [[bool; COLS]; ROWS] {
        let mut reached = [[false; COLS]; ROWS];
        // the bottom row is open from below, and the top row from the room
        // over the wall once the ball is there
        let mut open: Vec<(usize, usize)> = (0..COLS).map(|col| (ROWS - 1, col)).collect();
        while let Some((row, col)) = open.pop() {
            if reached[row][col] || wall[row][col] == STEEL {
                continue;
            }
            reached[row][col] = true;
            if row > 0 {
                open.push((row - 1, col));
            } else {
                open.extend((0..COLS).map(|col| (0, col)));
            }
            if row + 1 < ROWS {
                open.push((row + 1, col));
            }
            if col > 0 {
                open.push((row, col - 1));
            }
            if col + 1 < COLS {
                open.push((row, col + 1));
            }
        }
        reached
    }

    #[test]
    fn every_stage_is_its_own_wall_and_the_ball_can_reach_each_brick_that_breaks() {
        for stage in 0..COUNT {
            let line = &STAGES[stage as usize];
            for row in line {
                assert_eq!(row.len(), COLS, "stage {stage}: {row:?}");
                assert!(row.bytes().all(|b| b"#.123".contains(&b)), "{row:?}");
            }
            let wall = wall(stage);
            let reached = reached(&wall);
            let breakable: Vec<_> = (0..ROWS)
                .flat_map(|row| (0..COLS).map(move |col| (row, col)))
                .filter(|(row, col)| breakable(wall[*row][*col]))
                .collect();
            assert!(breakable.len() > 20, "stage {stage}");
            for (row, col) in breakable {
                assert!(
                    reached[row][col],
                    "stage {stage}: ({row}, {col}) is shut in"
                );
            }
            for other in 0..stage {
                assert_ne!(super::wall(other), wall, "stages {other} and {stage}");
            }
        }
    }

    #[test]
    fn a_brick_shut_in_by_steel_is_found() {
        let mut wall = wall(5);
        // the vault's door closed
        wall[7][5..8].fill(STEEL);
        assert!(!reached(&wall)[4][6]);
    }

    #[test]
    fn after_the_last_stage_the_walls_come_round_again() {
        assert_eq!(COUNT, 8);
        assert_eq!(wall(COUNT), wall(0));
        assert_eq!(wall(2 * COUNT + 5), wall(5));
    }
}
