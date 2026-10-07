//! One function per [`Op`](crate::Op), each named by [`Trade::execute`](crate::Trade)'s
//! match. Each reads and checks first, its sums included, then writes, so a
//! refused op leaves the store as it found it.
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use guest::{
    Error, ExecCtx, QueryCtx, capacity, code, corrupt, invalid, not_found, unauthorized,
    wrong_state,
};

use crate::state::{
    BALANCES, BOOK, CANDLES, FILLS, Funds, LAST_ORDER, LAST_TRADE, MINE, OPEN, ORDERS, TRADES,
    newest_first, rank,
};
use crate::{
    ASSETS, Candle, Fill, LOT, MARKETS, MAX_CANCELS, MAX_FILLS, Market, Order, OrderState, SPANS,
    Side, Traded, UNIT,
};

// ── money ───────────────────────────────────────────────────────────────────

pub(crate) fn market(id: u8) -> Result<&'static Market, Error> {
    MARKETS
        .get(usize::from(id))
        .ok_or_else(|| not_found(format!("there is no market {id}")))
}

fn asset_name(asset: u8) -> &'static str {
    ASSETS
        .get(usize::from(asset))
        .map_or("", |asset| asset.name)
}

/// An amount or a price as a person writes it: `1.5`, `0.0001`.
fn units(n: u64) -> String {
    let text = format!("{}.{:08}", n / UNIT, n % UNIT);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Money never wraps: a sum past what a `u64` holds is refused.
pub(crate) fn add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b)
        .ok_or_else(|| capacity("that is more than an account can hold"))
}

/// What is left of `a` after `b`. A rule takes only what it has checked is
/// there, so falling short means the tables disagree.
fn sub(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_sub(b)
        .ok_or_else(|| Error::new(code::CORRUPT, "an order moves more than there is"))
}

/// The next number of a counter that stands at `last`.
fn next(last: Option<u64>) -> Result<u64, Error> {
    last.unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| Error::new(code::EXHAUSTED, "there are no numbers left"))
}

/// The quote `amount` of the base comes to at `price`: `price * amount /
/// UNIT`, exact since a price is a multiple of a tick and an amount of a lot.
fn cost(price: u64, amount: u64) -> Result<u64, Error> {
    let quote = u128::from(price) * u128::from(amount) / u128::from(UNIT);
    u64::try_from(quote).map_err(|_| capacity("that order is too large"))
}

/// What an order of `side` puts up for `amount` at `price`: the quote a buy
/// pays, the base a sell gives.
fn stake(market: &Market, side: Side, price: u64, amount: u64) -> Result<(u8, u64), Error> {
    Ok(match side {
        Side::Buy => (market.quote, cost(price, amount)?),
        Side::Sell => (market.base, amount),
    })
}

/// The most of the base, in whole lots, that `free` of the quote pays for
/// at `price`. Nothing rests at a price of zero, and nothing is bought at it.
fn affordable(free: u64, price: u64) -> u64 {
    let amount = (u128::from(free) * u128::from(UNIT))
        .checked_div(u128::from(price))
        .unwrap_or(0);
    let amount = u64::try_from(amount).unwrap_or(u64::MAX);
    amount - amount % LOT
}

/// The balances an op moves, each read once and all stored when the op has
/// passed its checks. One account may be on both sides of a trade, or hold
/// several of the orders an op touches: its row is then moved here once
/// for each, and stored once.
#[derive(Default)]
struct Ledger(BTreeMap<(u64, u8), Funds>);

impl Ledger {
    fn funds(&mut self, ctx: &QueryCtx, account: u64, asset: u8) -> Result<&mut Funds, Error> {
        match self.0.entry((account, asset)) {
            Entry::Occupied(row) => Ok(row.into_mut()),
            Entry::Vacant(row) => {
                let funds = BALANCES.get(ctx, row.key())?.unwrap_or_default();
                Ok(row.insert(funds))
            }
        }
    }

    fn store(&self, ctx: &ExecCtx) {
        for (key, funds) in &self.0 {
            BALANCES.put(ctx, key, funds);
        }
    }
}

// ── funds ───────────────────────────────────────────────────────────────────

/// Raises what the account has of each asset, free and held together, to
/// the asset's grant. An account at or above it keeps what it has.
pub(crate) fn fund(ctx: &ExecCtx, owner: u64) -> Result<(), Error> {
    let mut raised = Vec::new();
    for (asset, granted) in (0u8..).zip(&ASSETS) {
        let key = (owner, asset);
        let mut funds = BALANCES.get(ctx, &key)?.unwrap_or_default();
        let has = add(funds.free, funds.held)?;
        if has < granted.grant {
            funds.free = add(funds.free, granted.grant - has)?;
            raised.push((key, funds));
        }
    }
    for (key, funds) in raised {
        BALANCES.put(ctx, &key, &funds);
    }
    Ok(())
}

// ── orders ──────────────────────────────────────────────────────────────────

fn opposite(side: Side) -> Side {
    match side {
        Side::Buy => Side::Sell,
        Side::Sell => Side::Buy,
    }
}

/// Where an order rests. A side is keyed by its place in [`Side`]: the
/// bids under 0, the asks under 1.
fn resting(market: u8, side: Side, price: u64, id: u64) -> (u8, u8, u64, u64) {
    (market, side as u8, rank(side, price), id)
}

/// Whether an order of `side` trades at `price`: a market order at any, a
/// limit order at its limit or better.
fn reaches(side: Side, limit: Option<u64>, price: u64) -> bool {
    limit.is_none_or(|limit| match side {
        Side::Buy => price <= limit,
        Side::Sell => price >= limit,
    })
}

/// One maker an order trades against.
struct Match {
    /// The maker's order as the trade leaves it.
    maker: Order,
    price: u64,
    amount: u64,
    /// What is left of the maker on the book.
    left: u64,
    trade: u64,
    /// The maker's fill number, `2 * trade + 1`; the taker's is the one
    /// before it.
    fill: u64,
}

pub(crate) fn place(
    ctx: &ExecCtx,
    owner: u64,
    market_id: u8,
    side: Side,
    amount: u64,
    limit: Option<u64>,
) -> Result<(), Error> {
    let market = market(market_id)?;
    if amount == 0 {
        return Err(invalid("an order needs an amount"));
    }
    if !amount.is_multiple_of(LOT) {
        return Err(invalid(format!(
            "an amount is a multiple of {}",
            units(LOT)
        )));
    }
    if limit == Some(0) {
        return Err(invalid("a limit order needs a price"));
    }
    if limit.is_some_and(|limit| !limit.is_multiple_of(market.tick)) {
        return Err(invalid(format!(
            "a {} price is a multiple of {}",
            market.name,
            units(market.tick)
        )));
    }
    // A limit order has its whole stake free, at its limit, and a market
    // sell what it sells. A market buy has no price to count a stake at: it
    // spends what the account has.
    let mut ledger = Ledger::default();
    let (staked, needs) = stake(market, side, limit.unwrap_or(0), amount)?;
    let free = ledger.funds(ctx, owner, staked)?.free;
    if free < needs {
        return Err(capacity(format!(
            "this order needs {} {} and you have {} free",
            units(needs),
            asset_name(staked),
            units(free)
        )));
    }

    // The other side of the book, best price first and oldest first at a
    // price; one more than an order may trade against, which tells whether
    // the book still crosses the order when it has traded against them all.
    let book = opposite(side);
    let makers = BOOK.prefix_of(&(market_id, book as u8));
    let makers = BOOK.scan(ctx, makers.limit(MAX_FILLS as u64 + 1))?;
    if limit.is_none() && makers.is_empty() {
        let doing = match book {
            Side::Buy => "buying",
            Side::Sell => "selling",
        };
        return Err(wrong_state(format!(
            "no one is {doing} on {} yet",
            market.name
        )));
    }
    let id = next(LAST_ORDER.get(ctx)?)?;
    let last_trade = LAST_TRADE.get(ctx)?;
    let time = ctx.env().time;
    let mut order = Order {
        id,
        owner,
        market: market_id,
        side,
        limit,
        amount,
        filled: 0,
        quote: 0,
        placed: time,
        state: OrderState::Open,
    };
    let mut matches: Vec<Match> = Vec::new();
    // whether matching stopped at `MAX_FILLS` makers with the next in reach
    let mut crossed = false;
    for &((_, _, at, maker_id), rests) in &makers {
        // `rank` is its own inverse: a place on the book back to its price
        let price = rank(book, at);
        let left = sub(amount, order.filled)?;
        if left == 0 || !reaches(side, limit, price) {
            break;
        }
        if matches.len() == MAX_FILLS {
            crossed = true;
            break;
        }
        let mut take = left.min(rests);
        if side == Side::Buy && limit.is_none() {
            let has = ledger.funds(ctx, owner, market.quote)?.free;
            take = take.min(affordable(has, price));
        }
        if take == 0 {
            break;
        }
        let key = resting(market_id, book, price, maker_id);
        let mut maker = ORDERS
            .get(ctx, &maker_id)?
            .ok_or_else(|| corrupt(BOOK.prefix(), &BOOK.key(&key), "its order is gone"))?;
        // The trade is at the maker's price. The taker pays out of its free
        // funds, the maker out of what its order holds, and each gets the
        // other's, free.
        let quote = cost(price, take)?;
        let (pays, paid, gets, got) = match side {
            Side::Buy => (market.quote, quote, market.base, take),
            Side::Sell => (market.base, take, market.quote, quote),
        };
        let funds = ledger.funds(ctx, owner, pays)?;
        funds.free = sub(funds.free, paid)?;
        let funds = ledger.funds(ctx, owner, gets)?;
        funds.free = add(funds.free, got)?;
        let funds = ledger.funds(ctx, maker.owner, gets)?;
        funds.held = sub(funds.held, got)?;
        let funds = ledger.funds(ctx, maker.owner, pays)?;
        funds.free = add(funds.free, paid)?;

        order.filled = add(order.filled, take)?;
        order.quote = add(order.quote, quote)?;
        maker.filled = add(maker.filled, take)?;
        maker.quote = add(maker.quote, quote)?;
        let rests = sub(rests, take)?;
        if rests == 0 {
            maker.state = OrderState::Filled;
        }
        let trade = next(matches.last().map(|last| last.trade).or(last_trade))?;
        let fill = trade
            .checked_mul(2)
            .and_then(|taker| taker.checked_add(1))
            .ok_or_else(|| Error::new(code::EXHAUSTED, "there are no numbers left"))?;
        matches.push(Match {
            maker,
            price,
            amount: take,
            left: rests,
            trade,
            fill,
        });
    }
    // A market order that trades nothing is refused, as one against an
    // empty side is: here a buy with less than the best price of one lot.
    if limit.is_none() && matches.is_empty() {
        return Err(capacity(format!(
            "you have {} {} free, which buys less than {} {} at the best price",
            units(free),
            asset_name(staked),
            units(LOT),
            asset_name(market.base)
        )));
    }

    // What is left of a limit order rests with its funds held, unless the
    // book still crosses it: then it is cancelled, as a market order's is.
    let left = sub(amount, order.filled)?;
    order.state = match limit {
        _ if left == 0 => OrderState::Filled,
        Some(_) if !crossed => OrderState::Open,
        _ => OrderState::Cancelled,
    };
    if let (OrderState::Open, Some(limit)) = (order.state, limit) {
        let (staked, held) = stake(market, side, limit, left)?;
        let funds = ledger.funds(ctx, owner, staked)?;
        funds.free = sub(funds.free, held)?;
        funds.held = add(funds.held, held)?;
    }

    // Every trade of one op is in one block, so in one candle of each span.
    let mut candles = Vec::new();
    if let Some(first) = matches.first() {
        for (span, kept) in (0u8..).zip(&SPANS) {
            let bucket = time / kept.millis;
            let key = (market_id, span, bucket);
            let mut candle = CANDLES.get(ctx, &key)?.unwrap_or(Candle {
                bucket,
                open: first.price,
                high: first.price,
                low: first.price,
                close: first.price,
                volume: 0,
            });
            for traded in &matches {
                candle.high = candle.high.max(traded.price);
                candle.low = candle.low.min(traded.price);
                candle.close = traded.price;
                candle.volume = add(candle.volume, traded.amount)?;
            }
            candles.push((key, candle));
        }
    }

    // Every check has passed: the writes.
    LAST_ORDER.put(ctx, &id);
    ORDERS.put(ctx, &id, &order);
    MINE.insert(ctx, &(owner, newest_first(id)));
    if let (OrderState::Open, Some(limit)) = (order.state, limit) {
        BOOK.put(ctx, &resting(market_id, side, limit, id), &left);
        OPEN.insert(ctx, &(owner, newest_first(id)));
    }
    if let Some(last) = matches.last() {
        LAST_TRADE.put(ctx, &last.trade);
    }
    for (key, candle) in candles {
        CANDLES.put(ctx, &key, &candle);
    }
    for traded in matches {
        let Match {
            maker,
            price,
            amount,
            trade,
            fill,
            ..
        } = traded;
        let key = resting(market_id, book, price, maker.id);
        if traded.left == 0 {
            BOOK.remove(ctx, &key);
            OPEN.remove(ctx, &(maker.owner, newest_first(maker.id)));
        } else {
            BOOK.put(ctx, &key, &traded.left);
        }
        ORDERS.put(ctx, &maker.id, &maker);
        let tape = Traded {
            price,
            amount,
            time,
            taker: side,
        };
        TRADES.put(ctx, &(market_id, newest_first(trade)), &tape);
        let taken = Fill {
            order: id,
            market: market_id,
            side,
            price,
            amount,
            time,
            maker: false,
        };
        let made = Fill {
            order: maker.id,
            side: book,
            maker: true,
            ..taken
        };
        FILLS.put(ctx, &(owner, newest_first(fill - 1)), &taken);
        FILLS.put(ctx, &(maker.owner, newest_first(fill)), &made);
    }
    ledger.store(ctx);
    ctx.set_return_data(abi::encode(&id));
    Ok(())
}

pub(crate) fn cancel(ctx: &ExecCtx, owner: u64, id: u64) -> Result<(), Error> {
    let order = ORDERS
        .get(ctx, &id)?
        .ok_or_else(|| not_found(format!("there is no order {id}")))?;
    if order.owner != owner {
        return Err(unauthorized(format!("order {id} is not yours")));
    }
    match order.state {
        OrderState::Open => withdraw(ctx, vec![order]),
        OrderState::Filled => Err(wrong_state(format!("order {id} is already filled"))),
        OrderState::Cancelled => Err(wrong_state(format!("order {id} is already cancelled"))),
    }
}

/// The account's newest open orders, [`MAX_CANCELS`] of them: one with more
/// sends the op again.
pub(crate) fn cancel_all(ctx: &ExecCtx, owner: u64) -> Result<(), Error> {
    let open = OPEN.scan(ctx, OPEN.prefix_of(&owner).limit(MAX_CANCELS as u64))?;
    let orders = open
        .into_iter()
        .map(|key| {
            ORDERS
                .get(ctx, &newest_first(key.1))?
                .ok_or_else(|| corrupt(OPEN.prefix(), &OPEN.key(&key), "its order is gone"))
        })
        .collect::<Result<Vec<Order>, Error>>()?;
    withdraw(ctx, orders)
}

/// Takes open orders off the book, frees what they hold and leaves each
/// `Cancelled`.
fn withdraw(ctx: &ExecCtx, orders: Vec<Order>) -> Result<(), Error> {
    let mut ledger = Ledger::default();
    let mut off = Vec::new();
    for mut order in orders {
        let market = market(order.market)?;
        // only a limit order rests
        let limit = order.limit.ok_or_else(|| {
            corrupt(
                ORDERS.prefix(),
                &ORDERS.key(&order.id),
                "open with no limit",
            )
        })?;
        let left = sub(order.amount, order.filled)?;
        let (staked, held) = stake(market, order.side, limit, left)?;
        let funds = ledger.funds(ctx, order.owner, staked)?;
        funds.held = sub(funds.held, held)?;
        funds.free = add(funds.free, held)?;
        order.state = OrderState::Cancelled;
        off.push((resting(order.market, order.side, limit, order.id), order));
    }
    for (key, order) in off {
        BOOK.remove(ctx, &key);
        OPEN.remove(ctx, &(order.owner, newest_first(order.id)));
        ORDERS.put(ctx, &order.id, &order);
    }
    ledger.store(ctx);
    Ok(())
}
