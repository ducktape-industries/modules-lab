//! The `live-echo` program: the one behind the live echo view. It keeps
//! nothing of its own. Its one op opens a room: it writes the live grant
//! that lets every account on the chain subscribe to every track of the
//! room through its node, and publish its own line, `echo/<account>`. The
//! lines themselves never touch the chain; they are live tracks the view
//! publishes.
//!
//! - `lib.rs` (here): the op and [`describe()`]
//! - `program.rs`: [`LiveEcho`], the program
//! - `tests.rs`: the program natively over `::program::MockHost`
mod program;
#[cfg(test)]
mod tests;

pub use program::LiveEcho;

use borsh::{BorshDeserialize, BorshSerialize};

pub const PROGRAM: &str = "live-echo";

/// The room the view talks in.
pub const LOBBY: &str = "lobby";

/// The longest room name an op opens: the host stamps `<chain>/<program>/`
/// in front of it, the chain's name a byte at least, and the whole
/// namespace is the wire's [`MAX_NAMESPACE`](::program::live::MAX_NAMESPACE)
/// at most. A room past this fits on no chain; one that fits here and not
/// beside a longer chain's name is the host's to refuse when a view asks.
pub const MAX_ROOM: usize = ::program::live::MAX_NAMESPACE - "c/".len() - PROGRAM.len() - "/".len();

/// An open room's one row, every account's ([`Op::Open`]): each may
/// subscribe to every track and publish its own line alone, under its
/// number.
pub fn open_row() -> ::program::live::Grant {
    ::program::live::Grant {
        publish: vec!["echo/{account}".into()],
        subscribe: vec!["*".into()],
    }
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Op {
    /// Every account that acts on the chain may subscribe to every track
    /// of `room` and publish `echo/<its number>`, no one else's. Opening an
    /// open room writes the same row.
    Open { room: String },
}

/// An op as a person reads it: a title and its fields. The source of the
/// `ducktape.describe` module this program ships (`make wasm-describes`).
pub fn describe(op: &Op) -> describe::Description {
    use describe::{Value, field};
    match op {
        Op::Open { room } => describe::Description {
            title: format!("Open the live room {room}"),
            fields: vec![field("room", Value::text(room))],
        },
    }
}

describe::export!(Op, describe);

/// Old op bytes are described with the current code (`describe`): the op
/// enum only grows at its end. Append a new variant here; never reorder.
#[test]
fn op_variants_only_append() {
    assert_eq!(describe::variants::<Op>(), ["Open"]);
}
