//! The program (its name and types, which a view names it by) and the
//! module: every op and every query, each handed to its function in
//! `rules.rs` or `queries.rs`.

use guest::{Error, ExecCtx, Module, Program, QueryCtx};

use crate::queries::{balances, book, candles, fills, orders, stats, trades};
use crate::rules::{cancel, cancel_all, found, fund, place};
use crate::{MODULE, Op, Query, Reply};

pub struct Trade;

impl Program for Trade {
    const NAME: &'static str = MODULE;
    type Op = Op;
    type Query = Query;
    type Reply = Reply;
}

impl Module for Trade {
    fn init(ctx: &ExecCtx, params: &[u8]) -> Result<(), Error> {
        found(ctx, params)
    }

    fn execute(ctx: &ExecCtx, op: Op) -> Result<(), Error> {
        // every op acts as an account: the funds and the orders are its own
        let account = ctx.sender_account()?;
        match op {
            Op::Fund => fund(ctx, account),
            Op::Place {
                market,
                side,
                amount,
                limit,
            } => place(ctx, account, market, side, amount, limit),
            Op::Cancel { order } => cancel(ctx, account, order),
            Op::CancelAll => cancel_all(ctx, account),
        }
    }

    fn query(ctx: &QueryCtx, query: Query) -> Result<Reply, Error> {
        Ok(match query {
            Query::Stats => Reply::Stats(stats(ctx)?),
            Query::Candles {
                market,
                span,
                before,
                limit,
            } => Reply::Candles(candles(ctx, market, span, before, limit)?),
            Query::Book {
                market,
                step,
                depth,
            } => Reply::Book(book(ctx, market, step, depth)?),
            Query::Trades { market, limit } => Reply::Trades(trades(ctx, market, limit)?),
            Query::Balances { owner } => Reply::Balances(balances(ctx, owner)?),
            Query::Orders { owner, open, page } => Reply::Orders(orders(ctx, owner, open, &page)?),
            Query::Fills { owner, page } => Reply::Fills(fills(ctx, owner, &page)?),
        })
    }
}

#[cfg(feature = "module")]
guest::export!(Trade);
