//! One function per [`Query`](crate::Query), each named by [`Trade::query`](crate::Trade)'s
//! match and each a bounded read of the tables in `state.rs`.
use guest::{Error, QueryCtx, Range, corrupt, invalid, not_found};
use store::{PageRequest, PageResponse};

use crate::rules::{add, market};
use crate::state::{
    BALANCES, BOOK, CANDLES, FILLS, Funds, MINE, OPEN, ORDERS, TRADES, newest_first, rank,
};
use crate::{
    ASSETS, BOOK_SCAN, Balance, Book, Candle, Fill, Level, MARKETS, MAX_ROWS, Order, SPANS, Side,
    Stats, Traded,
};

/// The 1h span's place in [`SPANS`]: what [`stats`] reads.
const HOUR: u8 = 3;
const HOUR_MILLIS: u64 = 3_600_000;
const _: () = assert!(SPANS[HOUR as usize].millis == HOUR_MILLIS);

/// Each market's last price and its last 24 hours, as of this block's time.
pub(crate) fn stats(ctx: &QueryCtx) -> Result<Vec<Stats>, Error> {
    // the day is the 24 one-hour buckets that end with this block's
    let day = (ctx.env().time / HOUR_MILLIS).saturating_sub(23);
    (0u8..)
        .zip(&MARKETS)
        .map(|(market, _)| {
            // Newest first: the day's candles, 24 at most, then the one
            // before the day.
            let newest = CANDLES.prefix_of(&(market, HOUR)).reverse().limit(25);
            let newest = CANDLES.scan(ctx, newest)?;
            let mut stats = Stats {
                last: newest.first().map_or(0, |(_, candle)| candle.close),
                ..Stats::default()
            };
            for (_, candle) in newest {
                if candle.bucket < day {
                    stats.before = candle.close;
                    break;
                }
                stats.high = stats.high.max(candle.high);
                stats.low = match stats.low {
                    0 => candle.low,
                    low => low.min(candle.low),
                };
                stats.volume = add(stats.volume, candle.volume)?;
            }
            Ok(stats)
        })
        .collect()
}

pub(crate) fn candles(
    ctx: &QueryCtx,
    market_id: u8,
    span: u8,
    before: Option<u64>,
    limit: u32,
) -> Result<Vec<Candle>, Error> {
    market(market_id)?;
    if usize::from(span) >= SPANS.len() {
        return Err(not_found(format!("there is no span {span}")));
    }
    let span = (market_id, span);
    let older = match before {
        // from the span's first candle up to `before`, which is left out
        Some(before) => Range::new(
            CANDLES.key(&span),
            Some(CANDLES.key(&(span.0, span.1, before))),
        ),
        None => CANDLES.prefix_of(&span),
    };
    let rows = CANDLES.scan(ctx, older.reverse().limit(rows(limit)))?;
    Ok(rows.into_iter().map(|(_, candle)| candle).collect())
}

/// How many candles or trades a query answers: what it asks for, one at
/// least and [`MAX_ROWS`] at most, as a page's limit is.
fn rows(limit: u32) -> u64 {
    u64::from(limit.clamp(1, MAX_ROWS))
}

pub(crate) fn book(ctx: &QueryCtx, market_id: u8, step: u64, depth: u32) -> Result<Book, Error> {
    let tick = market(market_id)?.tick;
    let grain = step
        .max(1)
        .checked_mul(tick)
        .ok_or_else(|| invalid("that price step is too large"))?;
    Ok(Book {
        bids: levels(ctx, market_id, Side::Buy, grain, depth)?,
        asks: levels(ctx, market_id, Side::Sell, grain, depth)?,
    })
}

/// One side of a book, best price first: the amounts resting at each
/// multiple of `grain`.
// ponytail: the deepest level may be short when more than `BOOK_SCAN`
// orders rest on the side; read the level out before answering it if a
// book ever grows that deep.
fn levels(
    ctx: &QueryCtx,
    market_id: u8,
    side: Side,
    grain: u64,
    depth: u32,
) -> Result<Vec<Level>, Error> {
    let resting = BOOK.prefix_of(&(market_id, side as u8)).limit(BOOK_SCAN);
    let mut levels: Vec<Level> = Vec::new();
    for ((_, _, at, _), amount) in BOOK.scan(ctx, resting)? {
        let price = rank(side, at);
        // Away from the spread: a bid down, an ask up. An ask at the top
        // of what a price holds has no multiple above it.
        let price = match side {
            Side::Buy => price - price % grain,
            Side::Sell => price.checked_next_multiple_of(grain).unwrap_or(u64::MAX),
        };
        // the scan is in price order, and rounding keeps it
        if let Some(level) = levels.last_mut().filter(|level| level.price == price) {
            level.amount = add(level.amount, amount)?;
        } else if levels.len() >= depth as usize {
            break;
        } else {
            levels.push(Level { price, amount });
        }
    }
    Ok(levels)
}

pub(crate) fn trades(ctx: &QueryCtx, market_id: u8, limit: u32) -> Result<Vec<Traded>, Error> {
    market(market_id)?;
    let newest = TRADES.prefix_of(&market_id).limit(rows(limit));
    let rows = TRADES.scan(ctx, newest)?;
    Ok(rows.into_iter().map(|(_, traded)| traded).collect())
}

pub(crate) fn balances(ctx: &QueryCtx, owner: u64) -> Result<Vec<Balance>, Error> {
    (0u8..)
        .zip(&ASSETS)
        .map(|(asset, _)| {
            let Funds { free, held } = BALANCES.get(ctx, &(owner, asset))?.unwrap_or_default();
            Ok(Balance { asset, free, held })
        })
        .collect()
}

pub(crate) fn orders(
    ctx: &QueryCtx,
    owner: u64,
    open: bool,
    page: &PageRequest,
) -> Result<PageResponse<Order>, Error> {
    let listed = if open { &OPEN } else { &MINE };
    listed.range_of(ctx, &owner, page)?.try_map(|key| {
        ORDERS
            .get(ctx, &newest_first(key.1))?
            .ok_or_else(|| corrupt(listed.prefix(), &listed.key(&key), "its order is gone"))
    })
}

pub(crate) fn fills(
    ctx: &QueryCtx,
    owner: u64,
    page: &PageRequest,
) -> Result<PageResponse<Fill>, Error> {
    Ok(FILLS.range_of(ctx, &owner, page)?.map(|(_, fill)| fill))
}
