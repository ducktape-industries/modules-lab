//! A market that does not exist: each symbol's candles, made from a seed.
//! No program holds prices yet, so what the view draws comes from here.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

pub struct Symbol {
    pub name: &'static str,
    /// Where its walk starts.
    pub price: f64,
    seed: u64,
}

pub const SYMBOLS: [Symbol; 3] = [
    Symbol {
        name: "BTC/USD",
        price: 64_000.,
        seed: 0x9E37_79B9_7F4A_7C15,
    },
    Symbol {
        name: "ETH/USD",
        price: 3_200.,
        seed: 0xBF58_476D_1CE4_E5B9,
    },
    Symbol {
        name: "DUCK/USD",
        price: 1.25,
        seed: 0x94D0_49BB_1331_11EB,
    },
];

/// A candle's span.
pub struct Frame {
    pub name: &'static str,
    pub minutes: u64,
}

pub const FRAMES: [Frame; 5] = [
    Frame {
        name: "1m",
        minutes: 1,
    },
    Frame {
        name: "5m",
        minutes: 5,
    },
    Frame {
        name: "1h",
        minutes: 60,
    },
    Frame {
        name: "4h",
        minutes: 240,
    },
    Frame {
        name: "1D",
        minutes: 1440,
    },
];

/// The candles a series starts with.
pub const HISTORY: usize = 1000;
/// The ticks a live candle takes before the next one opens: a candle of any
/// span closes in this many seconds, so the chart is seen to move.
const TICKS_PER_CANDLE: u32 = 20;

/// One symbol at one span: its candles, oldest first, the last one live.
#[derive(Default)]
pub struct Series {
    pub candles: Vec<Candle>,
    /// The largest move of one trade, as a share of the price.
    reach: f64,
    random: u64,
    ticks: u32,
}

impl Series {
    pub fn new(symbol: &Symbol, frame: &Frame) -> Self {
        let mut series = Self {
            candles: Vec::with_capacity(HISTORY + 64),
            // A minute moves a price about 0.1%, and a longer candle by the
            // square root of its minutes, as a random walk does.
            reach: 0.0005 * (frame.minutes as f64).sqrt(),
            random: symbol.seed ^ frame.minutes,
            // The last candle is live and half way through.
            ticks: TICKS_PER_CANDLE / 2,
        };
        let mut open = symbol.price;
        for index in 0..HISTORY {
            let mut candle = Candle {
                open,
                high: open,
                low: open,
                close: open,
                volume: 0.,
            };
            let live = index + 1 == HISTORY;
            for _ in 0..TICKS_PER_CANDLE - if live { series.ticks } else { 0 } {
                series.trade(&mut candle);
            }
            open = candle.close;
            series.candles.push(candle);
        }
        series
    }

    /// A second of the market: a trade lands on the live candle, and every
    /// `TICKS_PER_CANDLE`th closes it and opens the next at its close.
    pub fn tick(&mut self) {
        let Some(mut last) = self.candles.pop() else {
            return;
        };
        self.trade(&mut last);
        self.candles.push(last);
        self.ticks += 1;
        if self.ticks == TICKS_PER_CANDLE {
            self.ticks = 0;
            self.candles.push(Candle {
                open: last.close,
                high: last.close,
                low: last.close,
                close: last.close,
                volume: 0.,
            });
        }
    }

    fn trade(&mut self, candle: &mut Candle) {
        candle.close *= 1. + self.signed() * self.reach;
        candle.high = candle.high.max(candle.close);
        candle.low = candle.low.min(candle.close);
        candle.volume += (self.signed() + 1.) * 40.;
    }

    /// The next number in `[-1, 1)`: xorshift64*, whose top 53 bits are a
    /// uniform fraction.
    fn signed(&mut self) -> f64 {
        self.random ^= self.random >> 12;
        self.random ^= self.random << 25;
        self.random ^= self.random >> 27;
        let bits = self.random.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11;
        bits as f64 / (1u64 << 52) as f64 - 1.
    }
}

/// When candle `index` of a span opened, counted from 2026-01-01 00:00:
/// `(year, month, day, hour, minute)`.
pub fn opened(index: usize, frame: &Frame) -> (i64, u32, u32, u32, u32) {
    let minutes = index as u64 * frame.minutes;
    // Days since 1970-01-01, then the civil date (Howard Hinnant's
    // `civil_from_days`), which is right across leap years.
    const EPOCH_2026: i64 = 20_454;
    let days = EPOCH_2026 + (minutes / 1440) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    let minute_of_day = (minutes % 1440) as u32;
    (year, month, day, minute_of_day / 60, minute_of_day % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_series_is_the_same_every_time_and_its_candles_hold_their_trades() {
        let series = Series::new(&SYMBOLS[0], &FRAMES[2]);
        assert_eq!(series.candles, Series::new(&SYMBOLS[0], &FRAMES[2]).candles);
        assert_eq!(series.candles.len(), HISTORY);
        for pair in series.candles.windows(2) {
            assert_eq!(pair[1].open, pair[0].close);
        }
        for candle in &series.candles {
            assert!(candle.low <= candle.open.min(candle.close), "{candle:?}");
            assert!(candle.high >= candle.open.max(candle.close), "{candle:?}");
            assert!(candle.low > 0. && candle.volume > 0., "{candle:?}");
        }
    }

    #[test]
    fn the_live_candle_closes_and_the_next_opens_at_its_close() {
        let mut series = Series::new(&SYMBOLS[1], &FRAMES[0]);
        for _ in 0..TICKS_PER_CANDLE / 2 {
            assert_eq!(series.candles.len(), HISTORY, "still the live candle");
            series.tick();
        }
        assert_eq!(series.candles.len(), HISTORY + 1);
        let [.., closed, live] = series.candles[..] else {
            panic!("two candles")
        };
        assert_eq!(live.open, closed.close);
    }

    #[test]
    fn a_candle_opens_on_the_calendar() {
        assert_eq!(opened(0, &FRAMES[0]), (2026, 1, 1, 0, 0));
        assert_eq!(opened(61, &FRAMES[0]), (2026, 1, 1, 1, 1));
        assert_eq!(opened(31, &FRAMES[4]), (2026, 2, 1, 0, 0));
        // 2028 is a leap year: day 789 is its 29th of February.
        assert_eq!(opened(789, &FRAMES[4]), (2028, 2, 29, 0, 0));
    }
}
