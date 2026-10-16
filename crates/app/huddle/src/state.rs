//! The tables, beside the live grant rows (`live/<call>/<account>`) the
//! rules write through `::program::live`.
use store::{Map, Set};

use crate::Call;

/// Each call, by its id.
pub const CALLS: Map<String, Call> = Map::new("call/");

/// Each person's calls: `(account, started, id)`, so a scan of an
/// account's prefix backwards reads its newest call first. A person is
/// here from being asked in (or joining) until they decline or leave, or
/// the call ends.
pub const MINE: Set<(u64, u64, String)> = Set::new("mine/");
