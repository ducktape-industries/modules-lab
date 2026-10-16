//! The program (its name and types, which a view names it by) and its
//! dispatch: the op and every query, each handed to its function in
//! `rules.rs` or `queries.rs`.
use ::program::{Error, ExecCtx, Program, QueryCtx};

use crate::queries::{bests, top};
use crate::rules::{open_versus, score};
use crate::{Op, PROGRAM, Query, Reply};

pub struct Arcade;

impl Program for Arcade {
    const NAME: &'static str = PROGRAM;
    type Op = Op;
    type Query = Query;
    type Reply = Reply;

    fn execute(ctx: &ExecCtx, op: Op) -> Result<(), Error> {
        // a score is an account's, and so is a live room's door: a key that
        // holds none posts nothing and opens nothing
        let account = ctx.sender_account()?;
        match op {
            Op::Score {
                game,
                score: posted,
            } => score(ctx, account, game, posted),
            Op::OpenVersus => open_versus(ctx),
        }
    }

    fn query(ctx: &QueryCtx, query: Query) -> Result<Reply, Error> {
        Ok(match query {
            Query::Top { game, limit } => Reply::Top(top(ctx, game, limit)?),
            Query::Bests { owner } => Reply::Bests(bests(ctx, owner)?),
        })
    }
}

#[cfg(feature = "program")]
::program::export!(Arcade);
