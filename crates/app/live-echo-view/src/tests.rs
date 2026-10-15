//! The view on fake hosts: two or three readers linked by a
//! [`Relay`] that answers `live.*` as one node would, the lobby held
//! until it is opened, and its row taken back. Every frame is held to the
//! host's sanitizer and accessibility audit.
use live_echo::{LOBBY, LiveEcho, Op, open_row};
use view::methods::{HostSession, LivePublishTrack, Session, Submit};
use view::testing::{Relay, TestAppContext};

use crate::Echo;

fn seat(cx: &mut TestAppContext, account: u64) {
    cx.host().stream::<HostSession>().send(Session {
        connected: true,
        account: Some(account),
        ..Session::default()
    });
    cx.run_until_parked();
}

fn say(cx: &mut TestAppContext, line: &str) {
    cx.simulate_input("line", line);
    cx.simulate_submit("line");
    cx.run_until_parked();
}

#[test]
fn a_line_typed_in_one_app_reaches_the_other_and_a_late_reader_hears_the_last() {
    let (mut a, mut b, mut c) = (
        TestAppContext::new(),
        TestAppContext::new(),
        TestAppContext::new(),
    );
    // the relay's n-th view is account n
    Relay::new().link(&a).link(&b).link(&c);
    for (cx, account) in [(&mut a, 1), (&mut b, 2)] {
        cx.open::<Echo>();
        seat(cx, account);
    }
    a.run_until_parked();
    say(&mut a, "hello from one");
    b.run_until_parked();
    assert!(b.has_text("hello from one"), "{:?}", b.texts());
    assert!(b.has_text("Account 1"), "{:?}", b.texts());
    a.run_until_parked();
    assert!(a.has_text("1 listening"), "{:?}", a.texts());
    // the field is empty again, and the line is on the reader's own list
    assert!(a.has_text("hello from one"), "{:?}", a.texts());
    // a third, later: each one's last line is replayed to it
    say(&mut a, "and again");
    c.open::<Echo>();
    seat(&mut c, 3);
    assert!(c.has_text("and again"), "{:?}", c.texts());
    assert!(!c.has_text("hello from one"), "{:?}", c.texts());
    say(&mut b, "two here");
    a.run_until_parked();
    c.run_until_parked();
    assert!(a.has_text("two here"), "{:?}", a.texts());
    assert!(c.has_text("two here"), "{:?}", c.texts());
}

#[test]
fn a_reader_whose_app_went_away_shows_publisher_gone() {
    let (mut a, mut b) = (TestAppContext::new(), TestAppContext::new());
    let relay = Relay::new();
    relay.link(&a).link(&b);
    a.open::<Echo>();
    seat(&mut a, 1);
    b.open::<Echo>();
    seat(&mut b, 2);
    say(&mut a, "bye");
    b.run_until_parked();
    relay.gone(&a);
    b.run_until_parked();
    assert!(b.has_text("left: publisher_gone"), "{:?}", b.texts());
    // what it said stays
    assert!(b.has_text("bye"), "{:?}", b.texts());
}

#[test]
fn a_lobby_nobody_opened_is_opened_once_and_the_readers_go_in_on_its_block() {
    let (mut a, mut b) = (TestAppContext::new(), TestAppContext::new());
    let relay = Relay::new();
    relay.hold(LOBBY).link(&a).link(&b);
    for (cx, account) in [(&mut a, 1), (&mut b, 2)] {
        cx.host().handle::<Submit<LiveEcho>>(|_| Ok(Vec::new()));
        cx.open::<Echo>();
        seat(cx, account);
    }
    let open = Op::Open {
        room: LOBBY.to_owned(),
    };
    // the track and the room's list each heard `Waiting`: one `Open`
    assert_eq!(
        a.host().requests::<Submit<LiveEcho>>(),
        std::slice::from_ref(&open)
    );
    assert!(a.has_text("Opening the lobby…"), "{:?}", a.texts());
    // the block that writes the grant lets the waiting asks in
    relay.grant(LOBBY, None, open_row());
    a.run_until_parked();
    b.run_until_parked();
    a.run_until_parked();
    assert!(a.has_text("1 listening"), "{:?}", a.texts());
    say(&mut a, "first in");
    b.run_until_parked();
    assert!(b.has_text("first in"), "{:?}", b.texts());
    // asked once: nothing asked again
    assert_eq!(a.host().requests::<LivePublishTrack>().len(), 1);
    assert_eq!(
        a.host().requests::<Submit<LiveEcho>>(),
        std::slice::from_ref(&open)
    );
}

#[test]
fn a_grant_taken_back_is_shown_and_opens_nothing() {
    let (mut a, mut b) = (TestAppContext::new(), TestAppContext::new());
    let relay = Relay::new();
    relay.link(&a).link(&b);
    for (cx, account) in [(&mut a, 1), (&mut b, 2)] {
        cx.host().handle::<Submit<LiveEcho>>(|_| Ok(Vec::new()));
        cx.open::<Echo>();
        seat(cx, account);
    }
    a.run_until_parked();
    assert!(a.has_text("1 listening"), "{:?}", a.texts());
    // the block that removes the lobby's row
    relay.revoke(LOBBY, None);
    a.run_until_parked();
    assert!(
        a.has_text("The lobby refused: not_granted"),
        "{:?}",
        a.texts()
    );
    assert!(a.host().requests::<Submit<LiveEcho>>().is_empty());
}

#[test]
fn a_key_with_no_account_publishes_nothing() {
    let mut cx = TestAppContext::new();
    cx.open::<Echo>();
    cx.host().stream::<HostSession>().send(Session {
        connected: true,
        ..Session::default()
    });
    cx.run_until_parked();
    assert!(cx.host().requests::<LivePublishTrack>().is_empty());
    assert!(
        cx.has_text("Take an account to talk in the lobby."),
        "{:?}",
        cx.texts()
    );
}
