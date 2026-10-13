//! Claude Love on the fake host: the clock runs only while a line reveals,
//! a click answers a choice, and a playthrough to an ending posts its
//! score as game 5.
use super::*;
use crate::dating::{self, Ending};

const DATING: usize = 5;

fn at(opened: &mut Opened) -> (&'static str, usize) {
    opened.game(|game| match game {
        Game::Dating(dating) => (dating.node().id, dating.shown),
        _ => panic!("the dating sim is on screen"),
    })
}

#[test]
fn a_line_reveals_on_the_clock_and_the_clock_drops_once_it_is_shown() {
    let mut opened = seated();
    opened.start(DATING);
    assert_eq!(opened.clocks(), [dating::PERIOD]);
    assert!(opened.ticks.subscribed());
    shows(
        &opened.cx,
        &[
            "Claude Love",
            "Affection",
            "0",
            "Chapter",
            "1 / 5",
            "시스템",
        ],
    );
    opened.tick(3);
    assert_eq!(at(&mut opened), ("1-hello", 3));
    shows(&opened.cx, &["새 세"]);
    opened.key("space");
    assert!(!opened.ticks.subscribed(), "shown: no clock");
    assert!(!opened.cx.has_text("새 세"));
    opened.key("space");
    assert_eq!(at(&mut opened), ("1-hi", 0));
    assert_eq!(opened.clocks(), [dating::PERIOD, dating::PERIOD]);
}

#[test]
fn a_click_on_an_answer_takes_it() {
    let mut opened = seated();
    opened.start(DATING);
    opened.key("space");
    opened.key("space");
    assert_eq!(at(&mut opened), ("1-hi", 0));
    assert!(
        !opened.cx.has_text("1. 안녕"),
        "not before the line is shown"
    );
    opened.key("space");
    shows(
        &opened.cx,
        &["1. 안녕", "2. 버그 하나만 고쳐줘", "3. 나랑 사귀자"],
    );
    opened.cx.simulate_click("arcade-choice-2");
    opened.cx.run_until_parked();
    assert_eq!(at(&mut opened), ("1-date", 0));
    shows(&opened.cx, &["6"]);
    assert_eq!(opened.focused().as_deref(), Some("arcade-board"));
    // the next choice, the permission prompt, taken by its number
    opened.key("space");
    opened.key("space");
    opened.key("space");
    opened.key("space");
    assert_eq!(at(&mut opened), ("1-perm", 0));
    opened.key("space");
    opened.key("3");
    assert_eq!(at(&mut opened), ("1-perm-always", 0));
    shows(&opened.cx, &["14"]);
}

#[test]
fn a_playthrough_to_an_ending_posts_its_score_as_game_five() {
    let mut opened = seated();
    opened.start(DATING);
    // Space through every line and every choice, the first answer each
    // time: autopilot burns the quota, and the usage limit ends it
    let mut presses = 0;
    while opened.post() == Post::Playing {
        opened.key("space");
        presses += 1;
        assert!(presses < 200, "no ending at {:?}", at(&mut opened));
    }
    let ending = opened.game(|game| match game {
        Game::Dating(dating) => (dating.ending(), dating.affection),
        _ => panic!("the dating sim is on screen"),
    });
    assert_eq!(ending, (Some(Ending::Quota), 24));
    assert_eq!(
        opened.ops(),
        [Op::Score {
            game: DATING as u8,
            score: 440
        }]
    );
    assert!(!opened.ticks.subscribed());
    shows(
        &opened.cx,
        &["Game over", "Ending", "사용량 한도 엔딩", "Top 10"],
    );
}
