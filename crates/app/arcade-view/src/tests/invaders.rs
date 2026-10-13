//! Space Invaders on the fake host: the clock it runs on, and a shot of the
//! cannon's that scores.
use super::*;
use crate::invaders;

const INVADERS: usize = 5;

/// The board's canvases that draw something: the ground, one an invader
/// standing, and the cannon's.
fn drawn(opened: &Opened) -> usize {
    let mut board = opened.cx.node("arcade-board").clone();
    let mut drawn = 0;
    board.for_each_mut(&mut |node| {
        if let wire::Node::Canvas { commands, .. } = node {
            drawn += usize::from(!commands.is_empty());
        }
    });
    drawn
}

fn bottom_left(opened: &mut Opened) -> bool {
    opened.game(|game| match game {
        Game::Invaders(invaders) => invaders.alive[4][0],
        _ => panic!("invaders is on screen"),
    })
}

#[test]
fn space_fires_the_cannon_and_a_hit_invader_scores_its_points() {
    let mut opened = seated();
    opened.start(INVADERS);
    assert_eq!(opened.clocks(), [invaders::PERIOD]);
    shows(&opened.cx, &["Invaders", "Lives", "3", "Wave", "1"]);
    // the rack brought low, its bottom row just over the cannon, which
    // starts under the first column
    opened.game(|game| match game {
        Game::Invaders(invaders) => invaders.rack.1 += 88,
        _ => panic!("invaders is on screen"),
    });
    opened.key("space");
    opened.tick(2);
    assert!(bottom_left(&mut opened), "the shot is on its way");
    assert_eq!(drawn(&opened), 57);
    opened.tick(1);
    assert!(!bottom_left(&mut opened), "hit");
    assert_eq!(drawn(&opened), 56, "its canvas is empty");
    shows(&opened.cx, &["10"]);
}
