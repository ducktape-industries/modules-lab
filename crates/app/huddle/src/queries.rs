//! One function per [`Query`](crate::Query), each named by [`Huddle::query`](crate::Huddle)'s
//! match and each a bounded read of the tables in `state.rs`, at the
//! chain's time `now` (seconds).
use ::program::{Error, QueryCtx, corrupt};

use crate::state::{CALLS, MINE};
use crate::{Call, Listed};

/// The most of an account's calls, newest first, a list reads.
// ponytail: a call that went over with no write (no one answered, or
// everyone left without Leave) keeps its row in MINE; past this many newer
// ones an open call is not listed. Prune MINE in the op that finds a call
// over if lists ever miss one.
const NEWEST: u64 = 32;

pub(crate) fn call(ctx: &QueryCtx, id: &str) -> Result<Option<Call>, Error> {
    CALLS.get(ctx, &id.to_owned())
}

pub(crate) fn mine(ctx: &QueryCtx, account: u64, now: u64) -> Result<Vec<Listed>, Error> {
    listed(ctx, account, |call| !call.over(now))
}

pub(crate) fn ringing(ctx: &QueryCtx, account: u64, now: u64) -> Result<Vec<Listed>, Error> {
    listed(ctx, account, |call| call.rings(account, now))
}

/// `account`'s newest calls that `keep` keeps.
fn listed(
    ctx: &QueryCtx,
    account: u64,
    keep: impl Fn(&Call) -> bool,
) -> Result<Vec<Listed>, Error> {
    let newest = MINE.prefix_of(&account).reverse().limit(NEWEST);
    let mut calls = Vec::new();
    for (_, _, id) in MINE.scan(ctx, newest)? {
        // a call is never deleted, so its index row never outlives it
        let call = CALLS
            .get(ctx, &id)?
            .ok_or_else(|| corrupt(MINE.prefix(), id.as_bytes(), "an index row with no call"))?;
        if keep(&call) {
            calls.push(Listed { id, call });
        }
    }
    Ok(calls)
}
