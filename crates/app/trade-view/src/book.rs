//! The book and the tape as both trees list them: the best prices a side,
//! each with what rests at it and the running total out from the best,
//! the spread between the sides and the bids' share of the whole, the
//! name of the book's price step, and the last trade with the way it
//! moved. Drawing them is each tree's.
use std::cmp::Ordering;

use trade::{Book, Level};

use crate::data::STEPS;
use crate::{Trade, format};

/// A price of the book, as a tree lists it.
pub(crate) struct Rung {
    pub(crate) price: u64,
    pub(crate) amount: u64,
    /// What rests from the side's best price out to this one.
    pub(crate) running: u64,
}

/// The best prices a side, laid out.
pub(crate) struct Depth {
    /// Each side best first.
    pub(crate) asks: Vec<Rung>,
    pub(crate) bids: Vec<Rung>,
    /// The decimals of a price at the book's step, and of an amount
    /// among the rungs, so each column's figures line up.
    pub(crate) places: u32,
    pub(crate) sized: u32,
    /// The best ask less the best bid, while both sides have one.
    pub(crate) spread: Option<u64>,
}

impl Depth {
    /// What a side's rungs hold in all.
    fn whole(rungs: &[Rung]) -> u64 {
        rungs.last().map_or(0, |rung| rung.running)
    }

    /// How far a rung's bar reaches across its row: its running total
    /// against the larger side's whole.
    pub(crate) fn reach(&self, rung: &Rung) -> f32 {
        let most = Self::whole(&self.asks).max(Self::whole(&self.bids)).max(1);
        rung.running as f32 / most as f32
    }

    /// The bids' share of what the rungs of both sides hold, in percent.
    pub(crate) fn bid_share(&self) -> u32 {
        let (asked, bid) = (Self::whole(&self.asks), Self::whole(&self.bids));
        (u128::from(bid) * 100 / u128::from(asked + bid).max(1)) as u32
    }
}

/// A side's levels as rungs, the running total added up from the best.
fn rungs(levels: &[Level]) -> Vec<Rung> {
    let mut running = 0u64;
    let rungs = levels.iter().map(|level| {
        running += level.amount;
        Rung {
            price: level.price,
            amount: level.amount,
            running,
        }
    });
    rungs.collect()
}

impl Trade {
    /// The best `fit` prices a side of `book`.
    pub(crate) fn depth(&self, book: &Book, fit: usize) -> Depth {
        let asks = &book.asks[..book.asks.len().min(fit)];
        let bids = &book.bids[..book.bids.len().min(fit)];
        let step = STEPS[self.step].saturating_mul(self.market().tick);
        let spread = match (asks.first(), bids.first()) {
            (Some(ask), Some(bid)) => Some(ask.price.saturating_sub(bid.price)),
            _ => None,
        };
        Depth {
            asks: rungs(asks),
            bids: rungs(bids),
            places: format::decimals(step),
            sized: format::places(asks.iter().chain(bids).map(|level| level.amount)),
            spread,
        }
    }

    /// A price step of the book, `step` ticks of the market, written in
    /// the step's own decimals: `0.1`, `1`, `10`.
    pub(crate) fn step_name(&self, step: u64) -> String {
        let step = step.saturating_mul(self.market().tick);
        format::fixed(step, format::decimals(step))
    }

    /// The last price traded (the market's last price while the tape is
    /// still read), and how it stands against the trade before it.
    pub(crate) fn last_trade(&self) -> (Option<u64>, Option<Ordering>) {
        let trades = self.trades.ready().map(Vec::as_slice).unwrap_or_default();
        let last = trades.first().map(|trade| trade.price);
        let last = last.or(self
            .market_stats()
            .map(|stats| stats.last)
            .filter(|last| *last > 0));
        let before = trades.get(1).map(|trade| trade.price);
        (
            last,
            last.zip(before).map(|(last, before)| last.cmp(&before)),
        )
    }
}
