//! The view against a fake host that answers as the program and identity
//! would (`World`): each test drives the frame the way a person would.
//! Every frame it sends is held to the host's sanitizer and accessibility
//! audit.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ::identity::Identity;
use abi::role::identity;
use arcade::{Entry, Op, Query, Reply, state};
use view::methods::{
    Change, Changes, ClockTicks, HostId, HostSession, HostVisible, Query as Ask, Session, Submit,
    Submitted,
};
use view::prelude::Capability;
use view::testing::{StreamSender, TestAppContext};
use view::{Entity, View, wire};

use crate::games::Game;
use crate::{Arcade, Post};

mod breakout;
mod games;
mod invaders;
mod snake;

/// The account the reader is seated as.
pub(crate) const READER: u64 = 7;
/// Places in `arcade::GAMES`.
pub(crate) const SNAKE: usize = 1;

/// What the program and identity hold, as far as the view asks.
pub(crate) struct World {
    pub(crate) boards: [Vec<Entry>; 6],
    pub(crate) bests: Vec<u64>,
    pub(crate) names: HashMap<u64, String>,
    /// What the program says to the next op in place of taking it.
    pub(crate) refusal: Option<String>,
}

impl Default for World {
    /// Ada (1) and Bo (2) on Snake's board, the reader (7) under them; a
    /// name for Ada and the reader, none for Bo.
    fn default() -> World {
        let entry = |account, score| Entry { account, score };
        let mut boards: [Vec<Entry>; 6] = Default::default();
        boards[SNAKE] = vec![entry(1, 420), entry(2, 300), entry(READER, 120)];
        World {
            boards,
            bests: vec![0, 120, 0, 0, 0, 0],
            names: HashMap::from([(1, "ada".into()), (READER, "reader".into())]),
            refusal: None,
        }
    }
}

pub(crate) type Shared = Rc<RefCell<World>>;

/// The view open on the fake host, the session said, and the feeds a test
/// speaks for the host through.
pub(crate) struct Opened {
    pub(crate) cx: TestAppContext,
    pub(crate) view: Entity<Arcade>,
    pub(crate) world: Shared,
    pub(crate) ticks: StreamSender<ClockTicks>,
    pub(crate) visible: StreamSender<HostVisible>,
    pub(crate) changes: StreamSender<Changes<arcade::Arcade>>,
}

/// A session with a link, and `account` seated or none.
pub(crate) fn session(account: Option<u64>) -> Session {
    Session {
        connected: true,
        chain_id: "lab#0a1b2c3d".into(),
        signer: "0102".into(),
        account,
        endpoint: String::new(),
    }
}

/// The view open over `world` in the desk's pane, `account` seated.
pub(crate) fn opened(world: World, account: Option<u64>) -> Opened {
    let mut cx = TestAppContext::new();
    let world = Rc::new(RefCell::new(world));
    let asked = world.clone();
    cx.host().handle::<Ask<arcade::Arcade>>(move |query| {
        let world = asked.borrow();
        Ok(match query {
            Query::Top { game, limit } => Reply::Top(
                world.boards[usize::from(game)]
                    .iter()
                    .take(limit as usize)
                    .copied()
                    .collect(),
            ),
            Query::Bests { .. } => Reply::Bests(world.bests.clone()),
        })
    });
    let named = world.clone();
    cx.host().handle::<Ask<Identity>>(move |query| {
        let ::identity::Query::Profile { number } = query else {
            panic!("the view asks identity for a profile alone");
        };
        let name = named.borrow().names.get(&number).cloned();
        Ok(::identity::Reply::Profile(name.map(|name| {
            identity::Profile {
                number,
                name,
                kind: identity::Kind::User,
            }
        })))
    });
    cx.host()
        .handle::<HostId>(|prefix| Ok(format!("{prefix}-18f2c0a1b2c3d4e5-1")));
    let told = world.clone();
    cx.host()
        .handle::<Submit<arcade::Arcade>>(move |_| match &told.borrow().refusal {
            // the program's refusal, as the node predicts it
            Some(message) => Ok(Submitted::Refused {
                frame: [0; 32],
                until: 0,
                refusal: view::host::Error::new("refused", message.clone()),
            }),
            None => Ok(view::testing::applied(Vec::new())),
        });
    let sessions = cx.host().stream::<HostSession>();
    let ticks = cx.host().stream::<ClockTicks>();
    let visible = cx.host().stream::<HostVisible>();
    let changes = cx.host().stream::<Changes<arcade::Arcade>>();
    let view = cx.open::<Arcade>();
    sessions.send(session(account));
    cx.run_until_parked();
    Opened {
        cx,
        view,
        world,
        ticks,
        visible,
        changes,
    }
}

/// The view with the reader seated over the default world.
pub(crate) fn seated() -> Opened {
    opened(World::default(), Some(READER))
}

impl Opened {
    /// Presses game `which`'s card: the game is dealt and on screen.
    pub(crate) fn start(&mut self, which: usize) {
        self.cx.simulate_click(&format!("arcade-card-{which}"));
        self.cx.run_until_parked();
    }

    /// `keystroke` goes down on the board, which holds the keys.
    pub(crate) fn key(&mut self, keystroke: &str) {
        self.cx.simulate_key_down("arcade-board", keystroke);
    }

    /// The keyboard repeats `keystroke`, held down on the board.
    pub(crate) fn held(&mut self, keystroke: &str) {
        let handler = self.cx.interactivity("arcade-board").on_key_down;
        let state = gpui::Keystroke::parse(keystroke).expect("a keystroke");
        self.cx.simulate_event(wire::Event::KeyDown {
            handler: handler.expect("the board hears keys"),
            phase: wire::DispatchPhase::Bubble,
            event: wire::interactivity::KeyDown {
                state: (&state).into(),
                repeat: true,
                prefer_character_input: false,
            },
        });
    }

    /// `n` ticks of the clock, each run to the end before the next.
    pub(crate) fn tick(&mut self, n: usize) {
        for _ in 0..n {
            self.ticks.send(());
            self.cx.run_until_parked();
        }
    }

    /// The game on screen, changed as `f` changes it: a test that places
    /// what it needs on a board.
    pub(crate) fn game<R>(&mut self, f: impl FnOnce(&mut Game) -> R) -> R {
        self.cx.update(&self.view, |arcade, _, _| {
            let play = arcade.play.as_mut().expect("a game on screen");
            f(play.game.ready_mut().expect("a dealt game"))
        })
    }

    /// The periods of every clock the view asked for, in order.
    pub(crate) fn clocks(&self) -> Vec<i64> {
        self.cx.host().requests::<ClockTicks>()
    }

    pub(crate) fn ops(&self) -> Vec<Op> {
        self.cx.host().requests::<Submit<arcade::Arcade>>()
    }

    pub(crate) fn post(&self) -> Post {
        self.view
            .read(|arcade| arcade.play.as_ref().expect("a game").post.clone())
    }

    pub(crate) fn focused(&self) -> Option<String> {
        self.cx
            .focused()
            .and_then(|node| node.key())
            .map(str::to_owned)
    }

    pub(crate) fn tops(&self) -> usize {
        let asked = self.cx.host().requests::<Ask<arcade::Arcade>>();
        asked
            .iter()
            .filter(|query| matches!(query, Query::Top { .. }))
            .count()
    }
}

#[track_caller]
pub(crate) fn shows(cx: &TestAppContext, texts: &[&str]) {
    for text in texts {
        assert!(cx.has_text(text), "no {text:?} in {:?}", cx.texts());
    }
}

#[test]
fn the_manifest_names_the_program_identity_and_the_clock() {
    assert_eq!(<Arcade as View>::NAME, "Arcade");
    assert_eq!(<Arcade as View>::TARGETS, [arcade::PROGRAM, "identity"]);
    assert_eq!(
        <Arcade as View>::CAPABILITIES,
        [
            Capability::Program,
            Capability::Op,
            Capability::Host,
            Capability::Clock
        ]
    );
}

#[test]
fn the_hub_shows_every_game_with_the_readers_best_and_asks_for_no_clock() {
    let opened = seated();
    let cx = &opened.cx;
    shows(
        cx,
        &[
            "Arcade",
            "Tetris",
            "Snake",
            "2048",
            "Minesweeper",
            "Breakout",
            "Invaders",
        ],
    );
    shows(cx, &["Your best 120", "No score yet"]);
    for which in 0..arcade::GAMES.len() {
        let card = format!("arcade-card-{which}");
        assert!(cx.interactivity(&card).on_click.is_some(), "{card}");
    }
    assert!(opened.clocks().is_empty());
    assert!(!opened.ticks.subscribed());
    // nothing to say about posting with an account seated
    assert!(!cx.has_text(
        "Scores go on the board from an account. Without one the games play and nothing is posted."
    ));
}

#[test]
fn without_an_account_the_hub_says_scores_stay_here_and_reads_no_bests() {
    let opened = opened(World::default(), None);
    shows(
        &opened.cx,
        &[
            "Scores go on the board from an account. Without one the games play and nothing is posted.",
        ],
    );
    let asked = opened.cx.host().requests::<Ask<arcade::Arcade>>();
    assert!(asked.is_empty(), "{asked:?}");
}

#[test]
fn a_card_deals_its_game_onto_a_focused_board_and_its_first_arrow_asks_for_the_clock() {
    let mut opened = seated();
    opened.start(SNAKE);
    assert_eq!(opened.cx.host().requests::<HostId>(), ["arcade"]);
    assert_eq!(opened.focused().as_deref(), Some("arcade-board"));
    assert!(opened.clocks().is_empty(), "the snake waits for an arrow");
    opened.key("right");
    assert_eq!(opened.clocks(), [160]);
    assert!(opened.ticks.subscribed());
    shows(
        &opened.cx,
        &["Snake", "Score", "Your best", "120", "Length", "3"],
    );
    // the board is read for the overlays, and stays off the panel in play
    assert_eq!(opened.tops(), 1);
    assert!(!opened.cx.has_text("Top 10"));
}

#[test]
fn a_pause_drops_the_clock_and_shows_the_top_ten_and_going_on_asks_again() {
    let mut opened = seated();
    opened.start(SNAKE);
    opened.key("right");
    opened.key("p");
    assert!(!opened.ticks.subscribed(), "paused: no clock");
    shows(&opened.cx, &["Paused", "Resume", "Top 10", "ada", "420"]);
    // Bo has no name: his number stands in for it
    shows(&opened.cx, &["account 2", "reader"]);
    opened.key("p");
    assert!(opened.ticks.subscribed());
    assert_eq!(opened.clocks(), [160, 160]);
    // the bar's button pauses too, and the overlay's goes on
    opened.cx.simulate_click("arcade-pause");
    opened.cx.run_until_parked();
    assert!(!opened.ticks.subscribed());
    opened.cx.simulate_click("arcade-resume");
    opened.cx.run_until_parked();
    assert!(opened.ticks.subscribed());
}

#[test]
fn a_held_p_or_escape_pauses_once_and_the_game_stays_on_screen() {
    let mut opened = seated();
    opened.start(SNAKE);
    opened.key("right");
    opened.key("p");
    // the keyboard repeats it: the pause stays
    opened.held("p");
    assert!(!opened.ticks.subscribed(), "still paused");
    shows(&opened.cx, &["Paused"]);
    opened.key("p");
    assert!(opened.ticks.subscribed());
    // Escape pauses, and held does not go on to leave the game
    opened.key("escape");
    opened.held("escape");
    opened.held("enter");
    assert!(
        opened.view.read(|arcade| arcade.play.is_some()),
        "on the game"
    );
    assert!(!opened.ticks.subscribed(), "still paused");
    shows(&opened.cx, &["Paused", "Resume"]);
}

#[test]
fn off_screen_a_game_pauses_and_stays_paused() {
    let mut opened = seated();
    opened.start(SNAKE);
    opened.key("right");
    opened.visible.send(false);
    opened.cx.run_until_parked();
    assert!(!opened.ticks.subscribed());
    opened.visible.send(true);
    opened.cx.run_until_parked();
    assert!(
        !opened.ticks.subscribed(),
        "the reader goes on, not the host"
    );
    shows(&opened.cx, &["Paused"]);
    opened.key("enter");
    assert!(opened.ticks.subscribed());
}

#[test]
fn back_goes_to_the_hub_onto_the_games_card_and_drops_the_clock() {
    let mut opened = seated();
    opened.start(SNAKE);
    opened.cx.simulate_click("arcade-back");
    opened.cx.run_until_parked();
    assert!(!opened.ticks.subscribed());
    assert_eq!(opened.focused().as_deref(), Some("arcade-card-1"));
    shows(&opened.cx, &["Minesweeper"]);
}

#[test]
fn a_block_that_wrote_the_board_reads_it_again_and_one_that_did_not_reads_nothing() {
    let mut opened = seated();
    opened.start(SNAKE);
    let (tops, asked) = (
        opened.tops(),
        opened.cx.host().requests::<Ask<arcade::Arcade>>().len(),
    );
    let ranked = state::BOARD.key(&(1u8, 5u64, 9u64));
    opened.changes.send(Some(Change {
        height: 2,
        keys: vec![ranked],
    }));
    opened.cx.run_until_parked();
    assert_eq!(opened.tops(), tops + 1);
    let elsewhere = b"elsewhere/".to_vec();
    opened.changes.send(Some(Change {
        height: 3,
        keys: vec![elsewhere],
    }));
    opened.cx.run_until_parked();
    assert_eq!(
        opened.cx.host().requests::<Ask<arcade::Arcade>>().len(),
        asked + 1
    );
}

#[test]
fn a_snapshot_comes_back_to_the_hub() {
    let mut opened = seated();
    opened.start(SNAKE);
    let bytes = opened.cx.snapshot().expect("a snapshot");
    let view = opened.cx.restore::<Arcade>(&bytes).expect("restored");
    opened.cx.run_until_parked();
    view.read(|arcade| assert!(arcade.play.is_none()));
    shows(&opened.cx, &["Tetris", "Breakout"]);
}
