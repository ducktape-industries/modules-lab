//! Tetris versus between two views, each on its fake host, linked by the
//! SDK's relay as one node links two apps: the lobby, a match on the same
//! pieces, a Tetris's garbage, a top-out, a rematch, and a side that goes.
//! The room's grant is the relay's to give: held, the lobby waits for it;
//! taken back, the lobby stops.
use view::methods::{
    HostLog, LiveAnnounce, LiveMode, LivePublishTrack, LiveTracks, Method, Publishing, Watching,
    refusal,
};
use view::testing::Relay;

use super::*;
use crate::tetris::Attack;
use crate::tetris::{self, COLS, Piece, ROWS};
use crate::versus::{Effect, End, Match, Said, basis, deal};

/// Ada, account 1, and account 2, who has no name: each on its own host,
/// linked, with the versus lobby open.
struct Two {
    relay: Relay,
    ada: Opened,
    two: Opened,
}

fn lobby() -> Two {
    lobby_in(Relay::new())
}

/// The lobby open on both sides of `relay`.
fn lobby_in(relay: Relay) -> Two {
    let ada = opened(World::default(), Some(1));
    let two = opened(World::default(), Some(2));
    // the relay's n-th link acts as account n
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
        f(&mut versus.game.as_mut().expect("a match").play)
    })
}

/// The other side's well, as `side` last saw it.
fn their_well(side: &mut Opened) -> Option<tetris::Well> {
    side.cx.update(&side.view, |arcade, _, _| {
        let versus = arcade.versus.as_ref().expect("versus on screen");
        versus.game.as_ref().expect("a match").their_well
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
    assert!(their_well(&mut both.ada).is_some());
    assert!(their_well(&mut both.two).is_some());
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
    let seen = their_well(&mut both.ada).expect("their well");
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

/// Ada alone on her fake host, in a match account 2 asked for: the room's
/// list is the test's to feed.
struct Alone {
    ada: Opened,
    /// The ids of ada's `ask/` and `board/` publications.
    ask: u64,
    board: u64,
}

fn alone() -> Alone {
    let mut ada = opened(World::default(), Some(1));
    let tracks = ada.cx.host().stream::<LiveTracks>();
    ada.cx.simulate_click("arcade-card-versus");
    ada.cx.run_until_parked();
    // account 2 waits and asks; ada accepts
    for name in ["wait/2", "ask/2/1/0b"] {
        tracks.send(Watching::Announce(LiveAnnounce {
            name: name.into(),
            by: 2,
            mode: LiveMode::Stream,
            up: true,
        }));
    }
    ada.cx.run_until_parked();
    ada.cx.simulate_click("versus-player-2");
    // the challenge and the match publish in the click's frame
    let published = |prefix: &str| {
        ada.cx
            .last_frame()
            .requests
            .iter()
            .find(|request| {
                request.kind == LivePublishTrack::KIND
                    && LivePublishTrack::decode_request(&request.payload)
                        .is_ok_and(|publish| publish.track.name.starts_with(prefix))
            })
            .map(|request| request.id)
            .expect("ada publishes it")
    };
    let (ask, board) = (published("ask/1/2/"), published("board/1/"));
    ada.cx.run_until_parked();
    assert!(ada.ticks.subscribed(), "the round runs");
    Alone { ada, ask, board }
}

impl Alone {
    /// The host says `item` of ada's publication `id`, the track going on.
    fn tell(&mut self, id: u64, item: Result<Publishing, view::host::Error>) {
        let result = item.map(|item| LivePublishTrack::encode_reply(&item));
        self.ada.cx.simulate_event(wire::Event::Response {
            id,
            result,
            done: false,
        });
        self.ada.cx.run_until_parked();
    }

    fn still_running(&mut self) {
        assert!(self.ada.ticks.subscribed(), "the round runs");
        assert_eq!(game(&mut self.ada, |game| game.end), None);
        assert!(
            !self.ada.cx.has_text("Versus stopped"),
            "{:?}",
            self.ada.cx.texts()
        );
    }
}

#[test]
fn an_object_the_node_drops_leaves_the_round_running() {
    let mut alone = alone();
    // an object over its cap: the node drops it, says so, and the track
    // goes on
    let refused = view::host::Error::new(refusal::TOO_LARGE, "an object over its cap");
    alone.tell(alone.board, Err(refused));
    alone.still_running();
}

#[test]
fn a_group_the_host_cut_leaves_the_round_running() {
    let mut alone = alone();
    alone.tell(alone.ask, Ok(Publishing::Cut { group: 0 }));
    alone.still_running();
    let logged = alone.ada.cx.host().requests::<HostLog>();
    assert_eq!(
        logged,
        ["Arcade: the host cut group 0 of this side's versus ask"]
    );
}

#[test]
fn a_room_not_open_yet_is_opened_through_the_program_once_and_the_lobby_goes_in_on_its_grant() {
    let relay = Relay::new();
    relay.hold(arcade::VERSUS);
    let mut both = lobby_in(relay);
    // the watch and `wait/<me>` each heard `Waiting`: one op a side
    for side in [&both.ada, &both.two] {
        assert_eq!(side.ops(), [Op::OpenVersus], "the room was asked open once");
    }
    assert!(!both.ada.cx.has_text("account 2"));
    assert!(
        !both.ada.cx.has_text("Versus stopped"),
        "{:?}",
        both.ada.cx.texts()
    );
    // the block that writes the row lets the waiting asks in
    both.relay
        .grant(arcade::VERSUS, None, arcade::versus_room());
    both.settle();
    shows(
        &both.ada.cx,
        &["Who is waiting", "account 2", "Waiting", "Challenge"],
    );
    shows(&both.two.cx, &["ada", "Waiting"]);
    // asked once: nothing asked again
    assert_eq!(both.ada.cx.host().requests::<LiveTracks>().len(), 1);
    assert_eq!(both.ada.cx.host().requests::<LivePublishTrack>().len(), 1);
    assert_eq!(both.ada.ops(), [Op::OpenVersus]);
}

#[test]
fn a_grant_taken_back_stops_versus_and_asks_nothing_open() {
    let mut both = lobby();
    shows(&both.ada.cx, &["account 2"]);
    both.relay.revoke(arcade::VERSUS, None);
    both.settle();
    shows(
        &both.ada.cx,
        &["Versus stopped: the room's program took the grant back"],
    );
    assert_eq!(both.ada.ops(), []);
}

#[test]
fn the_match_step_alone_counts_an_attack_heard_and_deals_the_next_round_once_both_ask() {
    let (mut play, shown) = Match::new(2, 1, basis(2, "b", 1, "a"));
    assert_eq!(shown, [Effect::Show]);
    let attack = Attack { lines: 3, hole: 4 };
    assert_eq!(play.heard(Said::Attack(attack)), []);
    let garbage = play.tetris.versus.as_ref().expect("a versus well");
    assert_eq!((garbage.queued(), garbage.received), (3, 3));
    // they top out: this side won, and wants another
    assert_eq!(play.heard(Said::Out), []);
    assert_eq!(play.end, Some(End::Won));
    assert_eq!(play.tick(), [], "nothing steps after the round");
    assert_eq!(play.rematch(), [Effect::Say(Said::Rematch)]);
    assert_eq!(play.rematch(), [], "asked once");
    // theirs deals round 1, from its number
    assert_eq!(play.heard(Said::Rematch), [Effect::NewRound, Effect::Show]);
    assert_eq!(
        (play.round, play.end, play.again),
        (1, None, (false, false))
    );
    let dealt = deal(&basis(2, "b", 1, "a"), 1, 2);
    assert_eq!(
        (play.tetris.piece, play.tetris.next),
        (dealt.piece, dealt.next)
    );
    // a side that left: the other wins, and no round follows
    play.left();
    assert_eq!((play.end, play.gone), (Some(End::Won), true));
    assert_eq!(play.rematch(), []);
}
