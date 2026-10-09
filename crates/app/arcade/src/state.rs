//! The tables. A key's bytes sort as its values do, so a scan of a game's
//! prefix of [`BOARD`] reads its best score first.
use store::{Map, Set};

/// `(game, account)` to the account's best score of the game.
pub const BEST: Map<(u8, u64), u64> = Map::new("best/");

/// Every best score, ranked: `(game, highest_first(score), account)`. One
/// row an account and game, the one [`BEST`] holds.
pub const BOARD: Set<(u8, u64, u64)> = Set::new("board/");

/// The key under which a higher score reads first.
pub fn highest_first(score: u64) -> u64 {
    u64::MAX - score
}
