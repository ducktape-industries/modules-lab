//! The `arcade` module: the shared high scores of six classic games. The
//! games themselves run in the view and never touch the chain; an account
//! posts the score a game ended on, and the program keeps each account's
//! best of each game and ranks them. Nothing checks a score: the client
//! says it, so a board is practice, not proof. The layout, in reading
//! order:
//!
//! - `lib.rs` (here): the games, the types on the wire, all borsh, and
//!   [`describe()`]
//! - `state.rs`: the tables
//! - `program.rs`: [`Arcade`], the program (its name and types, which a view
//!   names it by) and the module: one match over every op and one over
//!   every query
//! - `rules.rs`: what the op checks and writes
//! - `queries.rs`: one function per question
//! - `tests.rs`: the module natively over `guest::MockHost`
mod program;
mod queries;
mod rules;
pub mod state;
#[cfg(test)]
mod tests;

pub use program::Arcade;

use borsh::{BorshDeserialize, BorshSerialize};

pub const MODULE: &str = "arcade";

/// A game is named on the wire by its place here.
pub const GAMES: [&str; 6] = [
    "Tetris",
    "Snake",
    "2048",
    "Minesweeper",
    "Breakout",
    "Invaders",
];

/// The most rows one [`Query::Top`] answers.
pub const MAX_TOP: u32 = 50;

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Op {
    /// The score a game of the sender's ended on. It becomes the sender's
    /// best of that game when it is higher; a score at or under the best
    /// changes nothing.
    Score { game: u8, score: u64 },
}

/// A read. Each variant names the reply that answers it, so a view asks it
/// alone (`host.query(arcade::ask::Top { .. })`) and gets that reply, and
/// the tables it reads, so a view asks again only when a block wrote one.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, ::program::Ask)]
#[ask(Arcade)]
pub enum Query {
    /// A game's best scores, highest first, one an account; at one score
    /// the smaller account first. At least one row is asked for and at
    /// most [`MAX_TOP`].
    #[ask(Reply::Top(Vec<Entry>))]
    #[reads(state::BOARD)]
    Top { game: u8, limit: u32 },
    /// An account's best of every game, in [`GAMES`]' order; zero where it
    /// has posted none.
    #[ask(Reply::Bests(Vec<u64>))]
    #[reads(state::BEST)]
    Bests { owner: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Reply {
    Top(Vec<Entry>),
    Bests(Vec<u64>),
}

/// One row of a game's board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Entry {
    pub account: u64,
    pub score: u64,
}

/// An op as a person reads it: a title and its fields. The source of the
/// `ducktape.describe` module this module ships (`make wasm-describes`).
pub fn describe(op: &Op) -> describe::Description {
    use describe::{Value, field};
    let (title, fields) = match op {
        Op::Score { game, score } => {
            let game = match GAMES.get(usize::from(*game)) {
                Some(name) => (*name).to_owned(),
                None => format!("game {game}"),
            };
            (
                format!("Post a {game} score"),
                vec![
                    field("game", Value::text(&game)),
                    field("score", Value::count(*score)),
                ],
            )
        }
    };
    describe::Description { title, fields }
}

describe::export!(Op, describe);

/// Old op bytes are described with the current code (`describe`): the op
/// enum only grows at its end. Append a new variant here; never reorder.
#[test]
fn op_variants_only_append() {
    assert_eq!(describe::variants::<Op>(), ["Score"]);
}
