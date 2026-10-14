//! The op once as meant and once as an attack: a refusal leaves the store
//! exactly as it was.
use guest::live::{Grant, key};
use guest::{Env, Error, MockHost, Module};

use crate::{LOBBY, LiveEcho, MAX_ROOM, MODULE, Op};

const ADA: u64 = 1;

fn env() -> Env {
    MockHost::env(MODULE)
}

/// `who`'s `Open { room }`, signed with an account or, `None`, without.
fn open(store: &MockHost, who: Option<u64>, room: &str) -> Result<(), Error> {
    let signed = env().signed(7u64.to_be_bytes(), who);
    let ctx = store.exec(signed);
    store.attempt(|| {
        LiveEcho::execute(
            &ctx,
            Op::Open {
                room: room.to_owned(),
            },
        )
    })
}

fn row(store: &MockHost, room: &str) -> Option<Grant> {
    store.query(env()).record(key(room, None)).unwrap()
}

#[test]
fn opening_a_room_writes_the_grant_every_account_reads() {
    let store = MockHost::default();
    assert_eq!(row(&store, LOBBY), None);
    open(&store, Some(ADA), LOBBY).unwrap();
    assert_eq!(row(&store, LOBBY), Some(Grant::all()));
    // again: the same row
    let kept = store.borrow().state.clone();
    open(&store, Some(ADA), LOBBY).unwrap();
    assert_eq!(store.borrow().state, kept);
}

#[test]
fn a_key_with_no_account_or_a_bad_room_opens_nothing() {
    let store = MockHost::default();
    let long = "r".repeat(MAX_ROOM + 1);
    for (who, room) in [
        (None, LOBBY),
        (Some(ADA), ""),
        (Some(ADA), "a/b"),
        (Some(ADA), &long),
    ] {
        assert!(open(&store, who, room).is_err(), "{who:?} {room:?}");
    }
    assert!(store.borrow().state.is_empty());
}
