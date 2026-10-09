//! Tetris, 2048, Minesweeper and Breakout on the fake host: what each asks
//! of the clock, and that a key or a press reaches its board.
use ducktape_view_guest::{MouseButton, MouseDownEvent, Point, px};

use super::*;
use crate::mines::{self, Mark};
use crate::tetris;

const TETRIS: usize = 0;
const MERGE: usize = 2;
const MINES: usize = 3;
const BREAKOUT: usize = 4;

#[test]
fn a_tetris_key_moves_the_piece_on_the_next_tick_and_an_idle_tick_draws_nothing() {
    let mut opened = seated();
    opened.start(TETRIS);
    assert_eq!(opened.clocks(), [tetris::PERIOD]);
    shows(&opened.cx, &["Level", "Lines", "Next"]);
    let at = |opened: &mut Opened| {
        opened.game(|game| match game {
            Game::Tetris(tetris) => tetris.at,
            _ => panic!("tetris is on screen"),
        })
    };
    let from = at(&mut opened);
    let renders = opened.cx.renders();
    // a frame of gravity's count that moves nothing is no render
    opened.tick(1);
    assert_eq!(opened.cx.renders(), renders);
    opened.key("left");
    assert_eq!(at(&mut opened), from, "not before the tick");
    opened.tick(1);
    assert_eq!(at(&mut opened), (from.0 - 1, from.1));
    assert_eq!(opened.cx.renders(), renders + 1);
}

#[test]
fn a_2048_arrow_slides_at_once_and_asks_for_no_clock() {
    let mut opened = seated();
    opened.start(MERGE);
    assert!(opened.clocks().is_empty());
    assert!(!opened.ticks.subscribed());
    opened.game(|game| match game {
        Game::Merge(merge) => merge.tiles = [[2, 2, 0, 0], [0; 4], [0; 4], [0; 4]],
        _ => panic!("2048 is on screen"),
    });
    opened.key("left");
    shows(&opened.cx, &["4", "Best tile"]);
    let tiles = opened.game(|game| match game {
        Game::Merge(merge) => merge.tiles,
        _ => panic!("2048 is on screen"),
    });
    assert_eq!(tiles[0][0], 4);
    assert_eq!(
        opened.view.read(|arcade| arcade
            .play
            .as_ref()
            .map(|play| play.game.ready().map(Game::score))),
        Some(Some(4))
    );
}

#[test]
fn a_2048_board_with_no_move_left_posts_its_score() {
    let mut world = World::default();
    world.bests[MERGE] = 0;
    let mut opened = opened(world, Some(READER));
    opened.start(MERGE);
    // the last move: two 2s merge, and nothing is left to pair
    opened.game(|game| match game {
        Game::Merge(merge) => {
            merge.tiles = [
                [8, 16, 8, 16],
                [16, 8, 16, 8],
                [8, 16, 8, 16],
                [2, 2, 32, 64],
            ];
        }
        _ => panic!("2048 is on screen"),
    });
    opened.key("left");
    // the new tile, a 2 or a 4, lands beside a 64 and under a 16: no pair
    assert_eq!(
        opened.ops(),
        [Op::Score {
            game: MERGE as u8,
            score: 4
        }]
    );
    shows(&opened.cx, &["Game over", "Posted to the board."]);
}

/// The field's top left corner in the window, as the host lays it out.
const FIELD_AT: (f32, f32) = (40., 80.);

/// A cell's pixels in the desk's pane: what the bar and the panel leave,
/// on sixteen cells.
fn field_cell() -> f32 {
    let (width, height) = ducktape_view_guest::testing::VIEWPORT;
    let room = (width - 216. - 32., height - 44. - 32.);
    (room.0 / 16.).min(room.1 / 16.).floor()
}

fn press(opened: &mut Opened, (x, y): (i32, i32), button: MouseButton) {
    let cell = field_cell();
    let position = Point::new(
        px(FIELD_AT.0 + (x as f32 + 0.5) * cell),
        px(FIELD_AT.1 + (y as f32 + 0.5) * cell),
    );
    opened.cx.simulate_mouse_down(
        "arcade-board",
        MouseDownEvent {
            button,
            position,
            click_count: 1,
            ..Default::default()
        },
    );
}

#[test]
fn minesweeper_opens_where_it_is_pressed_and_runs_its_clock_from_then() {
    let mut opened = seated();
    opened.start(MINES);
    assert!(
        !opened.ticks.subscribed(),
        "no clock before the first reveal"
    );
    let cell = field_cell();
    opened
        .cx
        .simulate_bounds("arcade-field", FIELD_AT, (16. * cell + 1., 16. * cell + 1.));
    press(&mut opened, (3, 4), MouseButton::Left);
    let shown = opened.game(|game| match game {
        Game::Mines(mines) => (mines.mark[4][3], mines.cursor),
        _ => panic!("minesweeper is on screen"),
    });
    assert_eq!(shown, (Mark::Shown, (3, 4)));
    assert_eq!(opened.clocks(), [mines::PERIOD]);
    opened.tick(3);
    shows(&opened.cx, &["3 s", "Mines left", "40"]);
    // a right press flags a hidden cell
    let hidden = opened.game(|game| match game {
        Game::Mines(mines) => (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .find(|(x, y)| mines.mark[*y as usize][*x as usize] == Mark::Hidden),
        _ => panic!("minesweeper is on screen"),
    });
    let hidden = hidden.expect("a hidden cell");
    press(&mut opened, hidden, MouseButton::Right);
    shows(&opened.cx, &["39"]);
}

#[test]
fn a_mine_shown_ends_the_game_and_posts_the_cells_shown() {
    let mut world = World::default();
    world.bests[MINES] = 0;
    let mut opened = opened(world, Some(READER));
    opened.start(MINES);
    // open ground with the keys, then walk onto a mine
    opened.key("space");
    let shown = opened.game(|game| match game {
        Game::Mines(mines) => {
            let shown = mines
                .mark
                .iter()
                .flatten()
                .filter(|m| **m == Mark::Shown)
                .count();
            let mine = (0..16)
                .flat_map(|y| (0..16).map(move |x| (x, y)))
                .find(|(x, y)| mines.mine[*y as usize][*x as usize])
                .expect("a mine");
            mines.cursor = mine;
            shown as u64
        }
        _ => panic!("minesweeper is on screen"),
    });
    opened.key("enter");
    assert_eq!(
        opened.ops(),
        [Op::Score {
            game: MINES as u8,
            score: shown
        }]
    );
    assert!(!opened.ticks.subscribed());
    shows(&opened.cx, &["Game over"]);
}

#[test]
fn breakouts_paddle_moves_while_an_arrow_is_held_and_space_serves() {
    let mut opened = seated();
    opened.start(BREAKOUT);
    assert_eq!(opened.clocks(), [crate::breakout::PERIOD]);
    let paddle = |opened: &mut Opened| {
        opened.game(|game| match game {
            Game::Breakout(breakout) => (breakout.paddle, breakout.serving),
            _ => panic!("breakout is on screen"),
        })
    };
    let (from, _) = paddle(&mut opened);
    opened.key("right");
    opened.tick(3);
    assert!(paddle(&mut opened).0 > from);
    opened.cx.simulate_key_up("arcade-board", "right");
    let (held, _) = paddle(&mut opened);
    let renders = opened.cx.renders();
    opened.tick(2);
    assert_eq!(paddle(&mut opened).0, held, "let go, it stops");
    assert_eq!(opened.cx.renders(), renders, "a ball at rest draws nothing");
    opened.key("space");
    opened.tick(1);
    assert!(!paddle(&mut opened).1, "served");
    shows(&opened.cx, &["Lives", "3", "Wall", "1"]);
}
