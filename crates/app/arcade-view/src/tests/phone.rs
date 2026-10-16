//! The phone's design (`ui/phone/`), as the phone boards draw it: the hub
//! and a game's board in each state, and each action, under a finger at
//! 390 wide. No game is played on a phone: no test here deals one. The
//! sweep holds every state at 360, 390 and 430.
use super::*;
use crate::words;

/// A place in `arcade::GAMES` whose board the default world leaves empty.
const MINES: usize = 3;

/// The program's refusal of a read, as a node in trouble says it.
const OUT_OF_FUEL: &str = "arcade ran out of fuel";

/// The view open over `world`, `account` seated, under a finger, 390 by
/// 844.
fn phone(world: World, account: Option<u64>) -> Opened {
    let mut opened = opened(world, account);
    opened.cx.simulate_touch(true);
    opened.cx.simulate_resize(390., 844.);
    opened
}

/// Ten others on Snake's board, every one over the reader's best of 120:
/// the accounts after the reader's.
fn outside_the_ten() -> World {
    let mut world = World::default();
    world.boards[SNAKE] = (READER + 1..=READER + 10)
        .map(|account| Entry {
            account,
            score: 1_000 - account,
        })
        .collect();
    world
}

/// Presses game `which`'s row on the hub.
fn open_board(cx: &mut TestAppContext, which: usize) {
    cx.simulate_click(&format!("arcade-game-{which}"));
    cx.run_until_parked();
}

/// The program refuses every read from now on.
fn refuse_reads(cx: &TestAppContext) {
    cx.host().refuse::<Ask<arcade::Arcade>>("trap", OUT_OF_FUEL);
}

#[test]
fn under_a_finger_the_hub_lists_the_six_games_with_the_readers_best_and_no_versus() {
    let opened = phone(World::default(), Some(READER));
    let cx = &opened.cx;
    for which in 0..arcade::GAMES.len() {
        let row = format!("arcade-game-{which}");
        assert!(cx.interactivity(&row).on_click.is_some(), "{row}");
    }
    assert!(
        cx.find("arcade-card-versus").is_none(),
        "versus is not read"
    );
    assert!(cx.find("arcade-card-0").is_none(), "no desktop card");
    shows(
        cx,
        &[
            words::ARCADE,
            words::HUB_CAPTION,
            words::PHONE_ONLY_READS,
            "Snake",
            words::BLURBS[SNAKE],
            "Your best 120",
            "No score yet",
        ],
    );
    assert!(!cx.has_text(words::UNSIGNED));
    assert!(cx.back_handler().is_none(), "the hub is the way out");
}

#[test]
fn bests_not_read_yet_are_a_dash() {
    let mut opened = phone(World::default(), None);
    opened.cx.host().never::<Ask<arcade::Arcade>>();
    // an account is seated: its bests are asked, and no answer comes
    opened
        .cx
        .host()
        .stream::<HostSession>()
        .send(session(Some(READER)));
    opened.cx.run_until_parked();
    shows(&opened.cx, &[words::NOT_KNOWN]);
    assert!(!opened.cx.has_text("Your best 120"));
}

#[test]
fn a_row_opens_its_board_without_dealing_a_game_and_back_closes_it() {
    let mut opened = phone(World::default(), Some(READER));
    open_board(&mut opened.cx, SNAKE);
    shows(
        &opened.cx,
        &[
            "Snake",
            &words::board_caption(words::BLURBS[SNAKE]),
            words::TOP_TEN,
            "ada",
            "420",
            "account 2",
            "reader",
        ],
    );
    // the board is read, and no game is dealt: no id, no clock, no keys
    assert_eq!(opened.tops(), 1);
    assert!(opened.cx.host().requests::<HostId>().is_empty());
    assert!(opened.clocks().is_empty());
    assert!(opened.view.read(|arcade| arcade.play.is_none()));
    // the phone's Back, then the page's own
    opened.cx.simulate_back();
    assert!(opened.cx.find("arcade-game-1").is_some());
    assert!(opened.cx.back_handler().is_none());
    open_board(&mut opened.cx, SNAKE);
    opened.cx.simulate_click("arcade-board-back");
    opened.cx.run_until_parked();
    assert!(opened.cx.find("arcade-hub").is_some());
    assert!(opened.view.read(|arcade| arcade.board.game.is_none()));
}

#[test]
fn the_readers_own_row_is_marked_you() {
    let mut opened = phone(World::default(), Some(READER));
    open_board(&mut opened.cx, SNAKE);
    // the reader is third, under Ada and Bo
    assert!(opened.cx.find("arcade-rank-2").is_some());
    shows(&opened.cx, &[words::YOU]);
    assert!(opened.cx.find("arcade-rank-you").is_none(), "in the ten");
}

#[test]
fn outside_the_ten_the_readers_best_shows_under_you_with_no_rank() {
    let mut opened = phone(outside_the_ten(), Some(READER));
    open_board(&mut opened.cx, SNAKE);
    assert!(opened.cx.find("arcade-rank-9").is_some());
    assert!(opened.cx.find("arcade-rank-you").is_some());
    shows(
        &opened.cx,
        &[words::NOT_IN_TOP_TEN, words::NOT_KNOWN, "120"],
    );
}

#[test]
fn signed_out_the_hub_says_so_and_a_board_says_whose_best_would_show() {
    let mut opened = phone(World::default(), None);
    shows(&opened.cx, &[words::UNSIGNED]);
    assert!(!opened.cx.has_text("No score yet"), "no best is anyone's");
    open_board(&mut opened.cx, SNAKE);
    shows(&opened.cx, &["ada", words::OWN_BEST_NEEDS_AN_ACCOUNT]);
    assert!(!opened.cx.has_text(words::YOU));
    let asked = opened.cx.host().requests::<Ask<arcade::Arcade>>();
    assert!(
        asked.iter().all(|query| matches!(query, Query::Top { .. })),
        "no bests are read: {asked:?}"
    );
}

#[test]
fn an_empty_board_says_no_scores_yet_and_what_puts_one_there() {
    let mut opened = phone(World::default(), Some(READER));
    open_board(&mut opened.cx, MINES);
    assert!(opened.cx.find("arcade-board-empty").is_some());
    shows(
        &opened.cx,
        &[words::NO_SCORES, &words::first_score("Minesweeper")],
    );
    assert!(opened.cx.find("arcade-top-head").is_none());
}

#[test]
fn a_board_being_read_says_so() {
    let mut opened = phone(World::default(), Some(READER));
    opened.cx.host().never::<Ask<arcade::Arcade>>();
    open_board(&mut opened.cx, SNAKE);
    assert!(opened.cx.find("arcade-board-reading").is_some());
    shows(&opened.cx, &[words::TOP_TEN, words::READING_BOARD]);
}

#[test]
fn a_refused_board_says_why_and_retry_reads_it_again() {
    let mut opened = phone(World::default(), Some(READER));
    refuse_reads(&opened.cx);
    open_board(&mut opened.cx, SNAKE);
    assert!(opened.cx.find("arcade-board-refused").is_some());
    shows(&opened.cx, &[OUT_OF_FUEL]);
    answer_reads(&opened.cx, &opened.world);
    opened.cx.simulate_click("arcade-board-retry");
    opened.cx.run_until_parked();
    assert!(opened.cx.find("arcade-board-refused").is_none());
    shows(&opened.cx, &["ada", "420"]);
}

#[test]
fn refused_bests_leave_the_rows_bare_and_retry_reads_them_again() {
    let mut opened = phone(World::default(), Some(READER));
    refuse_reads(&opened.cx);
    // a reopened link reads the bests again, and they are refused
    opened.changes.send(None);
    opened.cx.run_until_parked();
    assert!(opened.cx.find("arcade-bests-refused").is_some());
    assert!(!opened.cx.has_text("Your best 120"));
    answer_reads(&opened.cx, &opened.world);
    opened.cx.simulate_click("arcade-bests-retry");
    opened.cx.run_until_parked();
    shows(&opened.cx, &["Your best 120"]);
}

#[test]
fn a_block_that_wrote_the_open_board_reads_it_again() {
    let mut opened = phone(World::default(), Some(READER));
    open_board(&mut opened.cx, SNAKE);
    let tops = opened.tops();
    let ranked = state::BOARD.key(&(SNAKE as u8, 5u64, 9u64));
    opened.changes.send(Some(Change {
        height: 2,
        keys: vec![ranked],
    }));
    opened.cx.run_until_parked();
    assert_eq!(opened.tops(), tops + 1);
}

/// The pointer picks the design, not the width: a finger gets the phone's
/// hub on a wide window, a mouse the desktop's cards on a narrow one.
#[test]
fn the_pointer_picks_the_design_not_the_width() {
    let mut opened = phone(World::default(), Some(READER));
    opened.cx.simulate_resize(820., 800.);
    assert!(opened.cx.find("arcade-game-0").is_some());
    opened.cx.simulate_touch(false);
    opened.cx.simulate_resize(390., 844.);
    assert!(opened.cx.find("arcade-card-0").is_some());
}

/// The sweep: every board's state, every frame audited and fitted at each
/// phone width.
#[test]
fn sweep() {
    // boards 10, 11 and 13: the hub, a board with the reader in it, one
    // with the reader under it, an empty one
    let mut seated = phone(outside_the_ten(), Some(READER));
    view::testing::sweep(&mut seated.cx, |cx| {
        for which in [0, SNAKE, MINES] {
            open_board(cx, which);
            cx.simulate_back();
        }
    });
    // board 10 signed out, and a board without an account
    let mut unsigned = phone(World::default(), None);
    view::testing::sweep(&mut unsigned.cx, |cx| {
        open_board(cx, SNAKE);
        cx.simulate_back();
    });
    // board 12: a refused board, refused bests, then a board being read
    let mut troubled = phone(World::default(), Some(READER));
    refuse_reads(&troubled.cx);
    troubled.changes.send(None);
    troubled.cx.run_until_parked();
    view::testing::sweep(&mut troubled.cx, |cx| {
        open_board(cx, SNAKE);
        cx.simulate_back();
    });
    troubled.cx.host().never::<Ask<arcade::Arcade>>();
    view::testing::sweep(&mut troubled.cx, |cx| {
        open_board(cx, SNAKE);
        cx.simulate_back();
    });
}
