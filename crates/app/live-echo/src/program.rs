//! The program: its name and types, which a view names it by, and the one
//! op, which writes the room's grant.
use ::program::live::{Grant, grant};
use ::program::{Error, ExecCtx, Program, QueryCtx, invalid};

use crate::{MAX_ROOM, Op, PROGRAM};

/// The one track each account may publish in an open room: its own line,
/// under its number.
const OWN: &str = "echo/{account}";

pub struct LiveEcho;

impl Program for LiveEcho {
    const NAME: &'static str = PROGRAM;
    type Op = Op;
    /// Nothing to ask: the program keeps no state of its own.
    type Query = ();
    type Reply = ();

    fn execute(ctx: &ExecCtx, op: Op) -> Result<(), Error> {
        // a room is opened by an account: a key that holds none opens nothing
        ctx.sender_account()?;
        match op {
            Op::Open { room } => {
                // the room is the last part of the namespace the node splits
                // at `/`, and its grant row's key is `live/<room>/*`
                let named = !room.is_empty() && room.len() <= MAX_ROOM && !room.contains('/');
                if !named {
                    return Err(invalid(format!(
                        "a room is 1 to {MAX_ROOM} bytes with no `/`"
                    )));
                }
                let open = Grant {
                    publish: vec![OWN.into()],
                    subscribe: vec!["*".into()],
                };
                grant(ctx, &room, None, open);
                Ok(())
            }
        }
    }

    fn query(_: &QueryCtx, (): ()) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(feature = "program")]
::program::export!(LiveEcho);
