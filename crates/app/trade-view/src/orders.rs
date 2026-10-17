//! The reader's orders and fills as both trees write them: the market
//! each is in, a price in that market's decimals, what an order rests at,
//! how much of it filled, and what its fills came to a unit.
use trade::{MARKETS, Market, Order, UNIT};

use crate::format;

/// The market `market` names. A market past the ones this build lists
/// wraps into them, so a newer program's row still draws.
pub(crate) fn market_of(market: u8) -> &'static Market {
    &MARKETS[market as usize % MARKETS.len()]
}

/// `price` in `market`'s decimals.
pub(crate) fn priced(market: &Market, price: u64) -> String {
    format::fixed(price, format::decimals(market.tick))
}

/// What `order` rests at, in its market's decimals; none at the market.
pub(crate) fn limit(order: &Order) -> Option<String> {
    let market = market_of(order.market);
    order.limit.map(|limit| priced(market, limit))
}

/// How much of `order` filled, in percent.
pub(crate) fn filled(order: &Order) -> u128 {
    u128::from(order.filled) * 100 / u128::from(order.amount.max(1))
}

/// What `order`'s fills came to a unit, the price it traded at, in its
/// market's decimals; none before a fill.
pub(crate) fn average(order: &Order) -> Option<String> {
    let average = u128::from(order.quote) * u128::from(UNIT);
    let average = average.checked_div(u128::from(order.filled))?;
    Some(priced(market_of(order.market), average as u64))
}
