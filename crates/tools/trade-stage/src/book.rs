//! The book a lab stage is seeded with after its founding, so the view
//! opens on a market that is alive: one account's orders resting on both
//! sides of BTC/USD around the price the history ends at, and two market
//! orders of another's that trade against them. The `seed` binary writes them as
//! the bytes the node's CLI submits (`make live`); Trade's live example
//! submits them to its fake node.

use trade::{ASSETS, LOT, MARKETS, Op, Side, UNIT};

/// BTC/USD, by its place in [`MARKETS`].
pub const MARKET: u8 = 0;
/// The prices a side rests at.
const LEVELS: u64 = 18;
/// In ticks: how far the best bid and the best ask are from the price the
/// history ends at, rounded down to [`ROUND`], and the step between two
/// prices of a side.
const ROUND: u64 = 10;
const HALF_SPREAD: u64 = 40;
const STEP: u64 = 25;
/// What the taker buys and then sells at the market, in lots: each takes
/// the best price whole and a part of the next.
const TAKES: [(Side, u64); 2] = [(Side::Buy, 200), (Side::Sell, 150)];

/// What rests at the `level`th price of a side, in lots: 100 to 480 in an
/// order that looks uneven, `turn` being a side's own (odd, and not 5's).
fn lots(level: u64, turn: u64) -> u64 {
    100 + 20 * (level * turn % 20)
}
const BID_TURN: u64 = 7;
const ASK_TURN: u64 = 11;

fn place(side: Side, lots: u64, limit: Option<u64>) -> Op {
    Op::Place {
        market: MARKET,
        side,
        amount: lots * LOT,
        limit,
    }
}

/// The maker's limit orders around `last`, the price the history ends at:
/// best price first, a buy and a sell in turn.
pub fn rests(last: u64) -> Vec<Op> {
    let market = &MARKETS[usize::from(MARKET)];
    let tick = market.tick;
    let middle = last / tick / ROUND * ROUND;
    let bid = |level: u64| (middle - HALF_SPREAD - STEP * level) * tick;
    let ask = |level: u64| (middle + HALF_SPREAD + STEP * level) * tick;

    // The maker's funds are one grant: every order it rests is covered, or
    // the program refuses it.
    let quote: u128 = (0..LEVELS)
        .map(|level| {
            u128::from(bid(level)) * u128::from(lots(level, BID_TURN) * LOT) / u128::from(UNIT)
        })
        .sum();
    let base: u64 = (0..LEVELS).map(|level| lots(level, ASK_TURN) * LOT).sum();
    assert!(
        quote <= u128::from(ASSETS[usize::from(market.quote)].grant),
        "the bids are covered by one grant"
    );
    assert!(
        base <= ASSETS[usize::from(market.base)].grant,
        "the asks are covered by one grant"
    );
    (0..LEVELS)
        .flat_map(|level| {
            [
                place(Side::Buy, lots(level, BID_TURN), Some(bid(level))),
                place(Side::Sell, lots(level, ASK_TURN), Some(ask(level))),
            ]
        })
        .collect()
}

/// The taker's market orders: a buy, then a sell.
pub fn takes() -> Vec<Op> {
    TAKES
        .iter()
        .map(|(side, lots)| place(*side, *lots, None))
        .collect()
}
