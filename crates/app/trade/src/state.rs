//! The tables. A key's bytes sort as its values do, so a scan of a prefix
//! reads in key order: the book best price first, a listing newest first.

use store::{Item, Map, Set};

use crate::{Candle, Fill, Order, Side, Traded};

/// What an account has of an asset.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct Funds {
    /// Free to place an order with.
    pub free: u64,
    /// Held by its resting orders.
    pub held: u64,
}

/// `(account, asset)`
pub const BALANCES: Map<(u64, u8), Funds> = Map::new("balance/");

/// The last order id given out; ids count from 1, so an older order has the
/// smaller id.
pub const LAST_ORDER: Item<u64> = Item::new("last/order");
/// The last trade's number, from 1.
pub const LAST_TRADE: Item<u64> = Item::new("last/trade");

/// Every order ever placed, by id.
pub const ORDERS: Map<u64, Order> = Map::new("order/");

/// The resting orders: `(market, side, rank, order id)` to the amount left.
/// [`rank`] puts a side's best price first, and the id after it puts the
/// oldest order first at one price.
pub const BOOK: Map<(u8, u8, u64, u64), u64> = Map::new("book/");

/// Where a price sits on its side of the book: the lowest ask first, the
/// highest bid first.
pub fn rank(side: Side, price: u64) -> u64 {
    match side {
        Side::Sell => price,
        Side::Buy => newest_first(price),
    }
}

/// The key under which a larger number reads first.
pub fn newest_first(number: u64) -> u64 {
    u64::MAX - number
}

/// An account's open orders, newest first: `(owner, newest_first(id))`.
pub const OPEN: Set<(u64, u64)> = Set::new("open/");
/// Every order of an account, newest first: `(owner, newest_first(id))`.
pub const MINE: Set<(u64, u64)> = Set::new("mine/");

/// An account's fills, newest first: `(owner, newest_first(2 * trade + maker))`.
/// A trade is two fills, the taker's and the maker's, which one account may
/// both hold.
pub const FILLS: Map<(u64, u64), Fill> = Map::new("fill/");

/// A market's tape, newest first: `(market, newest_first(trade))`.
pub const TRADES: Map<(u8, u64), Traded> = Map::new("trade/");

/// `(market, span, bucket)`: the candle of the span's `bucket`th stretch of
/// time since the epoch. A stretch with no trade has no row.
pub const CANDLES: Map<(u8, u8, u64), Candle> = Map::new("candle/");
