//! One function per [`Query`](crate::Query), each named by [`Arcade::query`](crate::Arcade)'s
//! match and each a bounded read of the tables in `state.rs`.
use ::program::{Error, QueryCtx};

use crate::rules::game;
use crate::state::{BEST, BOARD, highest_first};
use crate::{Entry, GAMES};

pub(crate) fn top(ctx: &QueryCtx, game_id: u8, limit: u32) -> Result<Vec<Entry>, Error> {
    let game = game(game_id)?;
    let ranked = BOARD.prefix_of(&game).limit(u64::from(limit.max(1)));
    Ok(BOARD
        .scan(ctx, ranked)?
        .into_iter()
        .map(|(_, ranked, account)| Entry {
            account,
            score: highest_first(ranked),
        })
        .collect())
}

pub(crate) fn bests(ctx: &QueryCtx, owner: u64) -> Result<Vec<u64>, Error> {
    (0u8..)
        .zip(&GAMES)
        .map(|(game, _)| Ok(BEST.get(ctx, &(game, owner))?.unwrap_or(0)))
        .collect()
}
