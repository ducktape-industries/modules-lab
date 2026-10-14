//! Tetris, 2048, Minesweeper and Breakout on the fake host: what each asks
//! of the clock, and that a key or a press reaches its board.
use ducktape_view_guest::{MouseButton, MouseDownEvent, Point, px};

use super::*;
use crate::mines::{self, Mark};
use crate::{merge, tetris};

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
fn a_held_arrow_slides_the_tetris_piece_to_the_wall_and_c_puts_it_in_hold() {
    let mut opened = seated();
    opened.start(TETRIS);
    shows(&opened.cx, &["Hold", "Next"]);
    let piece = |opened: &mut Opened| {
        opened.game(|game| match game {
            Game::Tetris(tetris) => (tetris.piece, tetris.hold, tetris.falling()),
            _ => panic!("tetris is on screen"),
        })
    };
    let (first, _, _) = piece(&mut opened);
    // down and never up while the clock runs: within half a second
    opened.key("left");
    opened.tick(30);
    let (_, _, cells) = piece(&mut opened);
    assert_eq!(cells.iter().map(|(x, _)| *x).min(), Some(0), "at the wall");
    opened.cx.simulate_key_up("arcade-board", "left");
    opened.key("c");
    opened.tick(1);
    let (falling, held, _) = piece(&mut opened);
    assert_eq!(held, Some(first));
    assert_ne!(falling, first);
}

#[test]
fn shift_alone_holds_as_c_does_once_a_piece_and_only_as_it_goes_down() {
    let mut opened = seated();
    opened.start(TETRIS);
    let piece = |opened: &mut Opened| {
        opened.game(|game| match game {
            Game::Tetris(tetris) => (tetris.piece, tetris.hold),
            _ => panic!("tetris is on screen"),
        })
    };
    let (first, _) = piece(&mut opened);
    opened.modifiers(gpui::Modifiers::shift());
    opened.tick(1);
    let (second, held) = piece(&mut opened);
    assert_eq!(held, Some(first), "Shift put the first piece in hold");
    // up and down again: hold is used for this piece
    opened.modifiers(gpui::Modifiers::none());
    opened.modifiers(gpui::Modifiers::shift());
    opened.tick(1);
    assert_eq!(
        piece(&mut opened),
        (second, Some(first)),
        "not twice a piece"
    );
    opened.key("space");
    opened.tick(1);
    let (third, _) = piece(&mut opened);
    // Ctrl going down while Shift is held is no new press of Shift
    opened.modifiers(gpui::Modifiers {
        shift: true,
        control: true,
        ..gpui::Modifiers::none()
    });
    opened.tick(1);
    assert_eq!(piece(&mut opened), (third, Some(first)));
    opened.modifiers(gpui::Modifiers::none());
    opened.modifiers(gpui::Modifiers::shift());
    opened.tick(1);
    assert_eq!(
        piece(&mut opened),
        (first, Some(third)),
        "a new piece holds again and takes the first back"
    );
}

#[test]
fn a_pause_lets_go_of_a_held_tetris_arrow() {
    let mut opened = seated();
    opened.start(TETRIS);
    let at = |opened: &mut Opened| {
        opened.game(|game| match game {
            Game::Tetris(tetris) => tetris.at,
            _ => panic!("tetris is on screen"),
        })
    };
    let from = at(&mut opened);
    opened.key("right");
    opened.key("p");
    opened.key("p");
    opened.tick(30);
    assert_eq!(at(&mut opened), (from.0 + 1, from.1), "the press alone");
}

#[test]
fn a_2048_arrow_slides_at_once_and_asks_for_the_clock_only_while_tiles_move() {
    let mut opened = seated();
    opened.start(MERGE);
    assert!(opened.clocks().is_empty());
    assert!(!opened.ticks.subscribed());
    let place = |opened: &mut Opened, tiles| {
        opened.game(|game| match game {
            Game::Merge(merge) => merge.tiles = tiles,
            _ => panic!("2048 is on screen"),
        })
    };
    let tiles = |opened: &mut Opened| {
        opened.game(|game| match game {
            Game::Merge(merge) => merge.tiles,
            _ => panic!("2048 is on screen"),
        })
    };
    place(&mut opened, [[2, 2, 0, 0], [0; 4], [0; 4], [0; 4]]);
    opened.key("left");
    // the rules moved with the key; the clock draws the tiles getting there
    assert_eq!(tiles(&mut opened)[0][0], 4);
    shows(&opened.cx, &["Best tile", "4"]);
    assert_eq!(
        opened.view.read(|arcade| arcade
            .play
            .as_ref()
            .map(|play| play.game.ready().map(Game::score))),
        Some(Some(4))
    );
    assert_eq!(opened.clocks(), [merge::PERIOD]);
    let renders = opened.cx.renders();
    let frames = usize::from(merge::FRAMES);
    opened.tick(frames - 2);
    assert!(
        opened.ticks.subscribed(),
        "the new tile is still popping in"
    );
    opened.tick(1);
    assert!(!opened.ticks.subscribed(), "at rest: no clock");
    assert_eq!(
        opened.cx.renders(),
        renders + frames as u64 - 1,
        "a frame each"
    );
    // a key mid-motion moves at once and keeps the one clock
    place(&mut opened, [[0, 0, 0, 2], [0; 4], [0; 4], [0; 4]]);
    opened.key("left");
    opened.tick(2);
    place(&mut opened, [[0, 0, 0, 8], [0; 4], [0; 4], [0; 4]]);
    opened.key("left");
    assert_eq!(tiles(&mut opened)[0][0], 8);
    assert_eq!(opened.clocks(), [merge::PERIOD; 2]);
    opened.tick(frames - 1);
    assert!(!opened.ticks.subscribed());
    // a pause lands the tiles, and going on asks for no clock
    place(&mut opened, [[0, 0, 0, 2], [0; 4], [0; 4], [0; 4]]);
    opened.key("left");
    opened.key("p");
    assert!(!opened.ticks.subscribed());
    opened.key("p");
    assert!(!opened.ticks.subscribed(), "nothing left to move");
    assert_eq!(opened.clocks(), [merge::PERIOD; 3]);
}

#[test]
fn a_2048_board_with_no_move_left_posts_its_score() {
    let mut opened = opened(World::default(), Some(READER));
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
    shows(
        &opened.cx,
        &["Game over", "Sent to the board, which keeps your best."],
    );
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
    let mut opened = opened(World::default(), Some(READER));
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
            Game::Breakout(breakout) => (breakout.paddle, breakout.serving()),
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
    shows(&opened.cx, &["Stage", "1", "Lives", "3"]);
}
