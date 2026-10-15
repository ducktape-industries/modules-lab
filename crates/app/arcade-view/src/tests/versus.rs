//! Tetris versus between two views, each on its fake host, linked by the
//! SDK's relay as one node links two apps: the lobby, a match on the same
//! pieces, a Tetris's garbage, a top-out, a rematch, and a side that goes.
//! The room's grant is the relay's to give; the refusal for want of one is
//! the last test's, on a host with no relay.
use ducktape_view_guest::methods::{
    LiveAnnounce, LiveMode, LivePublishTrack, LiveTracks, Method, refusal,
};
use ducktape_view_guest::testing::Relay;

use super::*;
use crate::tetris::{self, COLS, Piece, ROWS};
use crate::versus::{End, Match};

/// Ada, account 1, and account 2, who has no name: each on its own host,
/// linked, with the versus lobby open.
struct Two {
    relay: Relay,
    ada: Opened,
    two: Opened,
}

fn lobby() -> Two {
    let ada = opened(World::default(), Some(1));
    let two = opened(World::default(), Some(2));
    // the relay's n-th link acts as account n
    let relay = Relay::new();
    relay.link(&ada.cx).link(&two.cx);
    let mut both = Two { relay, ada, two };
    both.ada.cx.simulate_click("arcade-card-versus");
    both.two.cx.simulate_click("arcade-card-versus");
    both.settle();
    both
}

/// Ada challenges account 2, who accepts: the match is on for both.
fn matched() -> Two {
    let mut both = lobby();
    both.ada.cx.simulate_click("versus-player-2");
    both.settle();
    both.two.cx.simulate_click("versus-player-1");
    both.settle();
    both
}

impl Two {
    /// Each view takes what the relay owes it, until neither is owed more.
    fn settle(&mut self) {
        for _ in 0..4 {
            self.ada.cx.run_until_parked();
            self.two.cx.run_until_parked();
        }
    }

    /// `n` ticks of each clock that runs.
    fn tick(&mut self, n: usize) {
        for _ in 0..n {
            for side in [&mut self.ada, &mut self.two] {
                if side.ticks.subscribed() {
                    side.ticks.send(());
                }
            }
            self.settle();
        }
    }
}

/// `keystroke` goes down on `side`'s well.
fn press(side: &mut Opened, keystroke: &str) {
    side.cx.simulate_key_down("versus-board", keystroke);
}

/// The match on `side`'s screen, changed as `f` changes it.
fn game<R>(side: &mut Opened, f: impl FnOnce(&mut Match) -> R) -> R {
    side.cx.update(&side.view, |arcade, _, _| {
        let versus = arcade.versus.as_mut().expect("versus on screen");
        f(versus.game.as_mut().expect("a match"))
    })
}

/// `side`'s well, its bottom `rows` rows as text: `.` empty, `G`
/// garbage, `#` a piece's block.
fn bottom(side: &mut Opened, rows: usize) -> Vec<String> {
    game(side, |game| {
        game.tetris.well[ROWS as usize - rows..]
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        None => '.',
                        Some(Piece::Garbage) => 'G',
                        Some(_) => '#',
                    })
                    .collect()
            })
            .collect()
    })
}

#[test]
fn two_in_the_lobby_see_each_other_and_a_challenge_accepted_starts_both_on_the_same_pieces() {
    let mut both = lobby();
    shows(
        &both.ada.cx,
        &["Who is waiting", "account 2", "Waiting", "Challenge"],
    );
    shows(&both.two.cx, &["ada", "Waiting"]);
    both.ada.cx.simulate_click("versus-player-2");
    both.settle();
    shows(&both.ada.cx, &["Asked. Waiting for them…", "Take back"]);
    shows(&both.two.cx, &["ada", "Challenges you", "Accept"]);
    assert!(
        both.ada
            .view
            .read(|arcade| arcade.versus.as_ref().unwrap().game.is_none())
    );
    both.two.cx.simulate_click("versus-player-1");
    both.settle();
    assert_eq!(game(&mut both.ada, |game| game.them), 2);
    assert_eq!(game(&mut both.two, |game| game.them), 1);
    shows(
        &both.ada.cx,
        &["against account 2", "HOLD", "NEXT", "SENT", "RECEIVED"],
    );
    assert!(!both.ada.cx.has_text("INCOMING"), "nothing on its way yet");
    shows(&both.two.cx, &["against ada"]);
    assert_eq!(both.ada.focused().as_deref(), Some("versus-board"));
    assert_eq!(both.two.focused().as_deref(), Some("versus-board"));
    // one deal for both: the piece falling and the five coming
    let dealt = |side: &mut Opened| game(side, |game| (game.tetris.piece, game.tetris.next));
    assert_eq!(dealt(&mut both.ada), dealt(&mut both.two));
    // a round runs on both clocks, and each sees the other's well
    assert!(both.ada.ticks.subscribed() && both.two.ticks.subscribed());
    both.tick(1);
    assert!(game(&mut both.ada, |game| game.their_well.is_some()));
    assert!(game(&mut both.two, |game| game.their_well.is_some()));
}

#[test]
fn shift_alone_holds_on_the_versus_well_too() {
    let mut both = matched();
    let first = game(&mut both.ada, |game| game.tetris.piece);
    both.ada.cx.simulate_modifiers_changed(
        "versus-board",
        gpui::ModifiersChangedEvent {
            modifiers: gpui::Modifiers::shift(),
            capslock: gpui::Capslock { on: false },
        },
    );
    both.tick(1);
    assert_eq!(game(&mut both.ada, |game| game.tetris.hold), Some(first));
}

#[test]
fn a_tetris_sends_four_rows_that_rise_in_the_other_well_on_its_next_lock() {
    let mut both = matched();
    game(&mut both.ada, |game| {
        let tetris = &mut game.tetris;
        // four rows full but for the first column, an I upright over it,
        // and a block over them: no all clear
        for y in ROWS as usize - 4..ROWS as usize {
            for x in 1..COLS as usize {
                tetris.well[y][x] = Some(Piece::O);
            }
        }
        tetris.well[ROWS as usize - 5][9] = Some(Piece::O);
        tetris.piece = Piece::I;
        tetris.turn = 1;
        tetris.at = (-2, 0);
    });
    press(&mut both.ada, "space");
    both.tick(1);
    assert_eq!(game(&mut both.ada, |game| game.tetris.lines), 4);
    let queued = game(&mut both.two, |game| {
        game.tetris.versus.as_ref().unwrap().incoming.clone()
    });
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].lines, 4);
    shows(&both.two.cx, &["INCOMING", "4"]);
    let counts = |side: &mut Opened| {
        game(side, |game| {
            let garbage = game.tetris.versus.as_ref().unwrap();
            (garbage.sent, garbage.received)
        })
    };
    assert_eq!(counts(&mut both.ada), (4, 0));
    assert_eq!(counts(&mut both.two), (0, 4));
    // a drop that clears nothing: the four rise under it
    press(&mut both.two, "space");
    both.tick(1);
    let hole = usize::from(queued[0].hole);
    let mut row: Vec<char> = "GGGGGGGGGG".chars().collect();
    row[hole] = '.';
    let row: String = row.into_iter().collect();
    assert_eq!(
        bottom(&mut both.two, 4),
        [row.clone(), row.clone(), row.clone(), row]
    );
    // and ada sees them in its view of the other well
    let seen = game(&mut both.ada, |game| game.their_well.expect("their well"));
    let floor = seen[ROWS as usize - 1];
    assert_eq!(floor[hole], None);
    assert_eq!(
        floor
            .iter()
            .filter(|cell| **cell == Some(Piece::Garbage))
            .count(),
        9
    );
}

#[test]
fn a_top_out_ends_the_round_for_both_and_a_rematch_deals_the_next_alike() {
    let mut both = matched();
    game(&mut both.two, |game| {
        // blocks where every piece comes in, beside a column the drop
        // falls down
        for y in 0..2 {
            for x in 3..7 {
                game.tetris.well[y][x] = Some(Piece::Z);
            }
        }
        game.tetris.piece = Piece::O;
        game.tetris.at = (7, 0);
    });
    press(&mut both.two, "space");
    both.tick(1);
    assert_eq!(game(&mut both.two, |game| game.end), Some(End::Lost));
    assert_eq!(game(&mut both.ada, |game| game.end), Some(End::Won));
    // each well flashes first, its clock running until it is done
    for side in [&mut both.ada, &mut both.two] {
        assert!(!side.cx.has_text("Rematch"), "the card waits");
        assert!(side.ticks.subscribed());
    }
    both.tick(tetris::ENDING as usize);
    shows(
        &both.ada.cx,
        &["You win", "account 2 topped out.", "Rematch", "Leave"],
    );
    shows(&both.two.cx, &["You lose", "Your well topped out."]);
    assert!(!both.ada.ticks.subscribed() && !both.two.ticks.subscribed());
    both.two.cx.simulate_click("versus-again");
    both.settle();
    shows(&both.two.cx, &["Waiting for ada to play again…"]);
    shows(&both.ada.cx, &["account 2 wants a rematch."]);
    both.ada.cx.simulate_click("versus-again");
    both.settle();
    for side in [&mut both.ada, &mut both.two] {
        assert_eq!(game(side, |game| game.end), None, "round two runs");
        assert!(side.ticks.subscribed());
    }
    let dealt = |side: &mut Opened| game(side, |game| (game.tetris.piece, game.tetris.next));
    assert_eq!(dealt(&mut both.ada), dealt(&mut both.two));
    assert!(
        bottom(&mut both.two, ROWS as usize)
            .iter()
            .all(|row| row == "..........")
    );
}

#[test]
fn a_side_that_goes_mid_match_hands_the_other_the_win_and_no_rematch() {
    let mut both = matched();
    both.tick(3);
    both.relay.gone(&both.two.cx);
    both.settle();
    assert_eq!(
        game(&mut both.ada, |game| (game.end, game.gone)),
        (Some(End::Won), true)
    );
    both.tick(tetris::ENDING as usize);
    shows(
        &both.ada.cx,
        &["You win", "account 2 left the match.", "Leave"],
    );
    assert!(!both.ada.cx.has_text("Rematch"));
    assert!(!both.ada.ticks.subscribed());
    // her challenge ended with the match: account 2 back in the lobby
    // finds no stale one to accept
    let asking = both.ada.cx.update(&both.ada.view, |arcade, _, _| {
        arcade
            .versus
            .as_ref()
            .expect("versus on screen")
            .ask
            .is_some()
    });
    assert!(!asking);
    // leaving goes back to the lobby, waiting again
    both.ada.cx.simulate_click("versus-lobby-back");
    both.ada.cx.run_until_parked();
    shows(&both.ada.cx, &["Who is waiting"]);
}

#[test]
fn an_object_the_node_drops_leaves_the_round_running() {
    let mut opened = opened(World::default(), Some(1));
    let tracks = opened.cx.host().stream::<LiveTracks>();
    opened.cx.simulate_click("arcade-card-versus");
    opened.cx.run_until_parked();
    // account 2 waits and asks; ada accepts
    for name in ["wait/2", "ask/2/1/0b"] {
        tracks.send(LiveAnnounce {
            name: name.into(),
            by: 2,
            mode: LiveMode::Datagram,
            up: true,
        });
    }
    opened.cx.run_until_parked();
    opened.cx.simulate_click("versus-player-2");
    let board = opened
        .cx
        .last_frame()
        .requests
        .iter()
        .find(|request| {
            request.kind == LivePublishTrack::KIND
                && LivePublishTrack::decode_request(&request.payload)
                    .is_ok_and(|publish| publish.track.name.starts_with("board/1/"))
        })
        .map(|request| request.id)
        .expect("the match publishes its board");
    opened.cx.run_until_parked();
    assert!(opened.ticks.subscribed(), "the round runs");
    // too many objects a second: the node drops one, says so, and the
    // track goes on
    opened.cx.simulate_event(wire::Event::Response {
        id: board,
        result: Err(ducktape_view_guest::host::Error::new(
            refusal::RATE_LIMIT,
            "too many objects a second",
        )),
        done: false,
    });
    opened.cx.run_until_parked();
    assert!(opened.ticks.subscribed(), "still running");
    assert!(
        !opened
            .cx
            .has_text("Versus stopped: too many objects a second")
    );
}

#[test]
fn the_room_opens_through_the_program_once_when_the_node_refuses_for_want_of_a_grant() {
    let mut opened = opened(World::default(), Some(1));
    let host = opened.cx.host();
    host.refuse::<LiveTracks>(refusal::NOT_GRANTED, "no grant covers this room");
    host.refuse::<LivePublishTrack>(refusal::NOT_GRANTED, "no grant covers this room");
    opened.cx.simulate_click("arcade-card-versus");
    opened.cx.run_until_parked();
    assert_eq!(
        opened.ops(),
        [Op::OpenVersus],
        "the room was asked open once"
    );
    // asked again once it landed, and refused again: no third time
    assert_eq!(opened.cx.host().requests::<LiveTracks>().len(), 2);
    assert_eq!(opened.cx.host().requests::<LivePublishTrack>().len(), 2);
    shows(&opened.cx, &["Versus stopped: no grant covers this room"]);
}
