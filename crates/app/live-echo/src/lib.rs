//! The `live-echo` module: the program behind the live echo view. It keeps
//! nothing of its own. Its one op opens a room: it writes the live grant
//! that lets every account on the chain publish and subscribe to every
//! track of the room through its node. The lines themselves never touch
//! the chain; they are live tracks the view publishes.
//!
//! - `lib.rs` (here): the op and [`describe()`]
//! - `program.rs`: [`LiveEcho`], the program and the module
//! - `tests.rs`: the module natively over `guest::MockHost`
mod program;
#[cfg(test)]
mod tests;

pub use program::LiveEcho;

use borsh::{BorshDeserialize, BorshSerialize};

pub const MODULE: &str = "live-echo";

/// The room the view talks in.
pub const LOBBY: &str = "lobby";

/// The longest room name an op opens: the host stamps the chain and the
/// module in front of it, and the whole namespace is at most 192 bytes.
pub const MAX_ROOM: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Op {
    /// Every account that acts on the chain may publish and subscribe to
    /// every track of `room`. Opening an open room writes the same row.
    Open { room: String },
}

/// An op as a person reads it: a title and its fields. The source of the
/// `ducktape.describe` module this module ships (`make wasm-describes`).
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
