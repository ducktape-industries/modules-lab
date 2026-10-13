//! Breakout on the fake host: the panel names the stage, the lives and the
//! items on, and a wall broken whole brings the next stage.
use super::*;
use crate::breakout::{Ball, Breakout, Capsule, Item, SUB, UP};

const BREAKOUT: usize = 4;

fn breakout<R>(opened: &mut Opened, f: impl FnOnce(&mut Breakout) -> R) -> R {
    opened.game(|game| match game {
        Game::Breakout(breakout) => f(breakout),
        _ => panic!("breakout is on screen"),
    })
}

#[test]
fn breakouts_panel_names_the_item_caught_and_a_wall_broken_whole_brings_the_next_stage() {
    let mut opened = seated();
    opened.start(BREAKOUT);
    shows(&opened.cx, &["Stage", "1", "Lives", "3"]);
    // a laser capsule falls onto the waiting paddle
    breakout(&mut opened, |breakout| {
        breakout.capsules.push(Capsule {
            item: Item::Laser,
            at: (breakout.paddle * SUB, 300 * SUB),
        });
    });
    opened.tick(20);
    shows(&opened.cx, &["Laser", "12 s", "10"]);
    // the wall down to the bottom row's sixth brick (x 160 to 192 px, its
    // bottom at 144 px), the ball rising under it
    breakout(&mut opened, |breakout| {
        breakout.bricks = Default::default();
        breakout.bricks[7][5] = 1;
        breakout.balls = vec![Ball {
            at: (176 * SUB, 150 * SUB),
            way: (0, -UP),
            held: None,
        }];
    });
    opened.tick(2);
    shows(&opened.cx, &["Stage", "2", "111"]);
    assert!(!opened.cx.has_text("Laser"), "a new wall starts clean");
    let (serving, stage) = breakout(&mut opened, |breakout| (breakout.serving(), breakout.stage));
    assert_eq!((serving, stage), (true, 1));
    assert!(opened.ticks.subscribed());
}
