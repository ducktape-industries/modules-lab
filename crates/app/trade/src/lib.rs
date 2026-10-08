//! The `trade` module: spot markets for practice funds. An account takes
//! its practice funds, places limit and market orders, and the program
//! matches them: best price first, then oldest first. Nothing here runs
//! without an op, so a price moves only when an order trades. The layout,
//! in reading order:
//!
//! - `lib.rs` (here): the assets and markets, the types on the wire, all
//!   borsh, and [`describe()`]
//! - `state.rs`: the tables
//! - `program.rs`: [`Trade`], the program (its name and types, which a view
//!   names it by) and the module: one match over every op and one over
//!   every query
//! - `rules.rs`: what each op checks and writes
//! - `queries.rs`: one function per question
//! - `tests.rs`: the module natively over `guest::MockHost`
mod program;
mod queries;
mod rules;
pub mod state;
#[cfg(test)]
mod tests;

pub use program::Trade;

use borsh::{BorshDeserialize, BorshSerialize};
use store::{PageRequest, PageResponse};

pub const MODULE: &str = "trade";

/// Every balance, amount and price counts in this many parts of one: an
/// amount of `UNIT` is 1 of its asset, a price of `UNIT` is 1 of the quote
/// for 1 of the base.
pub const UNIT: u64 = 100_000_000;
/// The step of an amount: 0.0001. A price is a multiple of its market's
/// tick, which is no smaller, so `price * amount / UNIT` is exact.
pub const LOT: u64 = UNIT / 10_000;

pub struct Asset {
    pub name: &'static str,
    /// What `Op::Fund` tops an account up to.
    pub grant: u64,
}

/// An asset is named on the wire by its place here.
pub const ASSETS: [Asset; 4] = [
    Asset {
        name: "USD",
        grant: 50_000 * UNIT,
    },
    Asset {
        name: "BTC",
        grant: UNIT,
    },
    Asset {
        name: "ETH",
        grant: 10 * UNIT,
    },
    Asset {
        name: "DUCK",
        grant: 10_000 * UNIT,
    },
];

pub struct Market {
    pub name: &'static str,
    /// What is bought and sold: a place in [`ASSETS`].
    pub base: u8,
    /// What it is paid in.
    pub quote: u8,
    /// The step of a price.
    pub tick: u64,
}

/// A market is named on the wire by its place here.
pub const MARKETS: [Market; 3] = [
    Market {
        name: "BTC/USD",
        base: 1,
        quote: 0,
        tick: UNIT / 10,
    },
    Market {
        name: "ETH/USD",
        base: 2,
        quote: 0,
        tick: UNIT / 100,
    },
    Market {
        name: "DUCK/USD",
        base: 3,
        quote: 0,
        tick: UNIT / 10_000,
    },
];

pub struct Span {
    pub name: &'static str,
    pub millis: u64,
}

/// The candle spans kept; a span is named on the wire by its place here.
pub const SPANS: [Span; 6] = [
    Span {
        name: "1m",
        millis: 60_000,
    },
    Span {
        name: "5m",
        millis: 300_000,
    },
    Span {
        name: "15m",
        millis: 900_000,
    },
    Span {
        name: "1h",
        millis: 3_600_000,
    },
    Span {
        name: "4h",
        millis: 14_400_000,
    },
    Span {
        name: "1D",
        millis: 86_400_000,
    },
];

/// The makers one order may trade against. What is left of an order that
/// still crosses the book after them is cancelled.
pub const MAX_FILLS: usize = 32;
/// The orders one `CancelAll` cancels.
pub const MAX_CANCELS: usize = 64;
/// The resting orders a `Book` query reads on each side.
pub const BOOK_SCAN: u64 = 512;
/// The most candles or trades one query answers.
pub const MAX_ROWS: u32 = 256;

/// The most bars one market's [`History`] holds: a week of minutes. Every
/// market's at once is a founding inside the limits core's README sets
/// (1e9 fuel, 256 MB).
pub const MAX_BARS: usize = 10_080;

/// A market's past, given at the founding so a new network's chart is not
/// empty. The founding's params are a borsh `Vec<History>`, one a market
/// at most; with no params every market starts with no past.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct History {
    pub market: u8,
    /// The span of `bars`. The candles of every coarser span are folded
    /// from them; a finer span starts with none.
    pub span: u8,
    /// Oldest first. The last is the stretch before the founding block's.
    pub bars: Vec<Bar>,
}

/// One stretch of a [`History`]: a candle not yet placed in time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Bar {
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Op {
    /// Tops the sender's practice funds up to each asset's grant.
    Fund,
    /// Places an order for `amount` of the market's base. With a `limit` it
    /// trades at that price or better and what is left rests on the book;
    /// with none it trades at the best prices there are and what is left
    /// is cancelled. The op's output is the order's id, a borsh `u64`.
    Place {
        market: u8,
        side: Side,
        amount: u64,
        limit: Option<u64>,
    },
    /// Cancels the sender's open order.
    Cancel { order: u64 },
    /// Cancels the sender's open orders, [`MAX_CANCELS`] an op.
    CancelAll,
}

/// A read. Each variant names the reply that answers it, so a view asks it
/// alone (`host.query(trade::ask::Book { .. })`) and gets that reply, and
/// the tables it reads, so a view asks again only when a block wrote one.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, ::program::Ask)]
#[ask(Trade)]
pub enum Query {
    /// Every market's last price and last 24 hours, in [`MARKETS`]' order.
    #[ask(Reply::Stats(Vec<Stats>))]
    #[reads(state::CANDLES)]
    Stats,
    /// A market's candles of one span, newest first, from the one before
    /// bucket `before` (from the newest with none); at most [`MAX_ROWS`].
    #[ask(Reply::Candles(Vec<Candle>))]
    #[reads(state::CANDLES)]
    Candles {
        market: u8,
        span: u8,
        before: Option<u64>,
        limit: u32,
    },
    /// A market's book, best price first on each side: resting orders
    /// summed by price, a price rounded away from the spread to a multiple
    /// of `step` ticks; at most `depth` prices a side.
    #[ask(Reply::Book(Book))]
    #[reads(state::BOOK)]
    Book { market: u8, step: u64, depth: u32 },
    /// A market's last trades, newest first; at most [`MAX_ROWS`].
    #[ask(Reply::Trades(Vec<Traded>))]
    #[reads(state::TRADES)]
    Trades { market: u8, limit: u32 },
    /// What an account has of every asset, in [`ASSETS`]' order.
    #[ask(Reply::Balances(Vec<Balance>))]
    #[reads(state::BALANCES)]
    Balances { owner: u64 },
    /// An account's orders, newest first: the open ones alone, or all.
    #[ask(Reply::Orders(PageResponse<Order>))]
    #[reads(state::ORDERS)]
    Orders {
        owner: u64,
        open: bool,
        page: PageRequest,
    },
    /// An account's fills, newest first.
    #[ask(Reply::Fills(PageResponse<Fill>))]
    #[reads(state::FILLS)]
    Fills { owner: u64, page: PageRequest },
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Reply {
    Stats(Vec<Stats>),
    Candles(Vec<Candle>),
    Book(Book),
    Trades(Vec<Traded>),
    Balances(Vec<Balance>),
    Orders(PageResponse<Order>),
    Fills(PageResponse<Fill>),
}

/// A market now and over its last 24 hours. All zero before its first trade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Stats {
    pub last: u64,
    /// The price 24 hours before; zero when the market is younger.
    pub before: u64,
    pub high: u64,
    pub low: u64,
    /// The base traded.
    pub volume: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Candle {
    /// Which stretch of its span, counted from the epoch: it opened at
    /// `bucket * span.millis`.
    pub bucket: u64,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    /// The base traded.
    pub volume: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Level {
    pub price: u64,
    /// The amount resting at it.
    pub amount: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Book {
    /// Highest price first.
    pub bids: Vec<Level>,
    /// Lowest price first.
    pub asks: Vec<Level>,
}

/// One trade on a market's tape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Traded {
    pub price: u64,
    pub amount: u64,
    /// The block's time, in milliseconds.
    pub time: u64,
    /// The side of the order that took it.
    pub taker: Side,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Balance {
    pub asset: u8,
    pub free: u64,
    /// Held by resting orders.
    pub held: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum OrderState {
    Open,
    Filled,
    /// By its owner, or what a market order or a capped match left over.
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Order {
    pub id: u64,
    pub owner: u64,
    pub market: u8,
    pub side: Side,
    /// None for a market order.
    pub limit: Option<u64>,
    pub amount: u64,
    pub filled: u64,
    /// The quote its fills came to: `quote * UNIT / filled` is the average
    /// price.
    pub quote: u64,
    /// The block's time when it was placed, in milliseconds.
    pub placed: u64,
    pub state: OrderState,
}

/// One side's record of a trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Fill {
    pub order: u64,
    pub market: u8,
    pub side: Side,
    pub price: u64,
    pub amount: u64,
    pub time: u64,
    /// Whether this side's order was resting on the book.
    pub maker: bool,
}

/// An op as a person reads it: a title and its fields. The source of the
/// `ducktape.describe` module this module ships (`make wasm-describes`).
pub fn describe(op: &Op) -> describe::Description {
    use describe::{Value, field};
    // an amount or a price, in the parts of one it counts in
    let units = |value: u64| Value::Amount {
        value: u128::from(value),
        decimals: 8,
    };
    let (title, fields) = match op {
        Op::Fund => ("Get practice funds".to_owned(), vec![]),
        Op::Place {
            market,
            side,
            amount,
            limit,
        } => {
            let market = match MARKETS.get(usize::from(*market)) {
                Some(market) => market.name.to_owned(),
                None => format!("market {market}"),
            };
            let side = match side {
                Side::Buy => "Buy",
                Side::Sell => "Sell",
            };
            let mut fields = vec![
                field("market", Value::text(&market)),
                field("side", Value::text(side)),
                field("amount", units(*amount)),
            ];
            let how = match limit {
                Some(limit) => {
                    fields.push(field("price", units(*limit)));
                    "at a limit"
                }
                None => "at market",
            };
            (format!("{side} {market} {how}"), fields)
        }
        Op::Cancel { order } => (
            format!("Cancel order {order}"),
            vec![field("order", Value::count(*order))],
        ),
        Op::CancelAll => ("Cancel all open orders".to_owned(), vec![]),
    };
    describe::Description { title, fields }
}

describe::export!(Op, describe);

/// Old op bytes are described with the current code (`describe`): the op
/// enum only grows at its end. Append a new variant here; never reorder.
#[test]
fn op_variants_only_append() {
    assert_eq!(
        describe::variants::<Op>(),
        ["Fund", "Place", "Cancel", "CancelAll"]
    );
}
