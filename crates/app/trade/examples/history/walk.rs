//! A week of every market's minutes, made up: the history a lab stage is
//! founded with (`main.rs` writes it for `make live`) and Trade's live
//! example founds its fake node with.
//!
//! A market walks from its own start. A minute closes where it opened,
//! moved by the sum of three draws (so most moves are small) and by the
//! drift of the four hours it is in, both in millionths of the price. Its
//! wicks reach one draw past the body, and its volume is a draw of lots,
//! twice as heavy in the second half of each day. Integers only and a seed
//! a market: every run gives the same bars.

use trade::{Bar, History, LOT, MARKETS, UNIT};

/// A week of minutes: the bars of each market's history.
const MINUTES: usize = 7 * 24 * 60;
/// The most one of a minute's three draws moves the price, in millionths.
const MOVE: i64 = 400;
/// The most a minute drifts, and for how many minutes one drift holds.
const DRIFT: i64 = 60;
const DRIFT_HOLDS: usize = 240;
/// The most a wick reaches past the body, in millionths.
const WICK: u64 = 300;

/// A market's walk, by its place in [`MARKETS`]: the price it starts at
/// and the lots one draw of its volume counts.
const WALKS: [(u64, u64); MARKETS.len()] = [
    (61_000 * UNIT, 100),
    (2_400 * UNIT, 1_000),
    (UNIT / 4, 100_000),
];

/// splitmix64.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A draw from `-reach` to `reach`.
    fn around(&mut self, reach: i64) -> i64 {
        (self.next() % (2 * reach as u64 + 1)) as i64 - reach
    }
}

fn walk(market: u8) -> History {
    let (start, lots) = WALKS[usize::from(market)];
    let tick = MARKETS[usize::from(market)].tick;
    let on_tick = |price: u64| ((price + tick / 2) / tick).max(1) * tick;
    let part = |price: u64, millionths: u64| price / 1_000_000 * millionths;
    let mut draws = Draws(u64::from(market) + 1);
    let mut open = on_tick(start);
    let mut drift = 0;
    let mut bars = Vec::with_capacity(MINUTES);
    for minute in 0..MINUTES {
        if minute % DRIFT_HOLDS == 0 {
            drift = draws.around(DRIFT);
        }
        let by = drift + (0..3).map(|_| draws.around(MOVE)).sum::<i64>();
        let moved = part(open, by.unsigned_abs());
        let close = on_tick(if by < 0 { open - moved } else { open + moved });
        let high = on_tick(open.max(close) + part(open, draws.next() % WICK));
        let low = on_tick(open.min(close) - part(open, draws.next() % WICK));
        let heavy = if minute % 1_440 < 720 { 1 } else { 2 };
        let volume = (1 + draws.next() % 40) * heavy * lots * LOT;
        bars.push(Bar {
            open,
            high,
            low,
            close,
            volume,
        });
        open = close;
    }
    History {
        market,
        span: 0,
        bars,
    }
}

/// Every market's week, in [`MARKETS`]' order.
pub fn histories() -> Vec<History> {
    (0u8..).take(MARKETS.len()).map(walk).collect()
}
