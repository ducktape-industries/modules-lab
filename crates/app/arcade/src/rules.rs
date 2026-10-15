//! What each [`Op`](crate::Op) checks and writes: it reads first, then
//! writes, so a refused op leaves the store as it found it.
use ::program::live::{Grant, grant};
use ::program::{Error, ExecCtx, not_found};

use crate::state::{BEST, BOARD, highest_first};
use crate::{GAMES, VERSUS};

/// The game a number names on the wire.
pub(crate) fn game(id: u8) -> Result<u8, Error> {
    match GAMES.get(usize::from(id)) {
        Some(_) => Ok(id),
        None => Err(not_found(format!("there is no game {id}"))),
    }
}

/// `posted` becomes `account`'s best of `game` when it is higher than the
/// best held; at or under it, nothing is written.
pub(crate) fn score(ctx: &ExecCtx, account: u64, game_id: u8, posted: u64) -> Result<(), Error> {
    let game = game(game_id)?;
    let best = BEST.get(ctx, &(game, account))?;
    if posted <= best.unwrap_or(0) {
        return Ok(());
    }
    if let Some(best) = best {
        BOARD.remove(ctx, &(game, highest_first(best), account));
    }
    BOARD.insert(ctx, &(game, highest_first(posted), account));
    BEST.put(ctx, &(game, account), &posted);
    Ok(())
}

/// The versus room's grant row: every account that acts on the chain may
/// publish and subscribe to every track in it. The node reads the row; the
/// view sends this op when the node refused it for want of one.
pub(crate) fn open_versus(ctx: &ExecCtx) -> Result<(), Error> {
    grant(ctx, VERSUS, None, Grant::all());
    Ok(())
}
