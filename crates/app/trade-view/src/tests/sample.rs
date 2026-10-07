//! A market to draw the view on: the rows of the approved boards, served
//! by a fake host the way the program would answer them. The tests and the
//! `frames` example both draw on it; the view itself holds no sample.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ducktape_view_guest::host::Error;
use ducktape_view_guest::methods::{HostSession, Query as Ask, Session, Submit};
use ducktape_view_guest::testing::TestAppContext;
use ducktape_view_guest::{Entity, View};
use store::{PageRequest, PageResponse};
use trade::{
    Balance, Book, Candle, Fill, LOT, Level, Order, OrderState, Query, Reply, Side, Stats, Traded,
    UNIT,
};

/// 2026-10-07 14:00 UTC, the hour the boards' newest candle opens at.
pub const NOW: u64 = 1_791_381_600_000;
const HOUR: u64 = 3_600_000;
/// The 1h span's place in `trade::SPANS`.
const HOURS: u8 = 3;
/// The account the boards are an account's.
pub const READER: u64 = 7;

/// What the program holds, as far as the view asks.
#[derive(Default)]
pub struct World {
    pub stats: Vec<Stats>,
    /// `(market, span)` to its candles, oldest first.
    pub candles: HashMap<(u8, u8), Vec<Candle>>,
    /// The book and the tape of market 0; the others have none.
    pub book: Book,
    pub trades: Vec<Traded>,
    pub balances: Vec<Balance>,
    /// Every order of the reader, newest first.
    pub orders: Vec<Order>,
    pub fills: Vec<Fill>,
    /// What the program says to the next op in place of taking it.
    pub refusal: Option<String>,
}

pub type Shared = Rc<RefCell<World>>;

/// A price in tenths of a quote unit, BTC/USD's tick.
const fn tenths(price: u64) -> u64 {
    price * (UNIT / 10)
}

/// The cursor of the page from row `at`.
fn cursor(at: usize) -> Vec<u8> {
    (at as u64).to_le_bytes().to_vec()
}

fn page<T: Clone>(rows: Vec<T>, request: &PageRequest) -> PageResponse<T> {
    let from = request.after.as_ref().map_or(0, |cursor| {
        u64::from_le_bytes(cursor[..8].try_into().expect("a cursor of this listing")) as usize
    });
    let to = (from + request.limit() as usize).min(rows.len());
    PageResponse {
        height: 1,
        items: rows[from..to].to_vec(),
        next: (to < rows.len()).then(|| cursor(to)),
    }
}

impl World {
    fn answer(&self, query: Query) -> Reply {
        match query {
            Query::Stats => Reply::Stats(self.stats.clone()),
            Query::Candles {
                market,
                span,
                before,
                limit,
            } => {
                let rows = self.candles.get(&(market, span));
                let rows = rows.into_iter().flatten().rev();
                let rows = rows.filter(|row| before.is_none_or(|before| row.bucket < before));
                Reply::Candles(
                    rows.take(limit.min(trade::MAX_ROWS) as usize)
                        .copied()
                        .collect(),
                )
            }
            Query::Book { market: 0, .. } => Reply::Book(self.book.clone()),
            Query::Book { .. } => Reply::Book(Book::default()),
            Query::Trades { market: 0, limit } => {
                Reply::Trades(self.trades.iter().take(limit as usize).copied().collect())
            }
            Query::Trades { .. } => Reply::Trades(Vec::new()),
            Query::Balances { .. } => Reply::Balances(self.balances.clone()),
            Query::Orders { open, page: at, .. } => {
                let rows = self.orders.iter().copied();
                let rows = rows.filter(|order| !open || order.state == OrderState::Open);
                Reply::Orders(page(rows.collect(), &at))
            }
            Query::Fills { page: at, .. } => Reply::Fills(page(self.fills.clone(), &at)),
        }
    }
}

/// Answers the view's queries and ops from `world`, which a test goes on
/// changing through the handle it gets back.
pub fn serve(cx: &TestAppContext, world: World) -> Shared {
    let world = Rc::new(RefCell::new(world));
    let asked = world.clone();
    cx.host()
        .handle::<Ask<trade::Trade>>(move |query| Ok(asked.borrow().answer(query)));
    let told = world.clone();
    cx.host()
        .handle::<Submit<trade::Trade>>(move |_| match &told.borrow().refusal {
            Some(message) => Err(Error::new("refused", message.clone())),
            None => Ok(Vec::new()),
        });
    world
}

/// A session with a link, and `account` seated or none.
pub fn session(account: Option<u64>) -> Session {
    Session {
        connected: true,
        chain_id: "lab#0a1b2c3d".into(),
        signer: "0102".into(),
        account,
        endpoint: String::new(),
    }
}

/// The size the host measures the chart at in a pane of `size`: what the
/// bar, the tools, the panel and the other columns leave it.
pub fn chart_size(size: (f32, f32)) -> (f32, f32) {
    match size.0 >= 1100. {
        true => (size.0 - 240. - 272., size.1 - 48. - 40. - 158.),
        false => (size.0 - 256., size.1 - 40. - 34. - 136.),
    }
}

/// The view `V` open over `world` in a pane of `size`, the session said
/// with `account` seated, every first read landed and the chart measured.
pub fn opened<V: View>(
    world: World,
    size: (f32, f32),
    account: Option<u64>,
) -> (TestAppContext, Entity<V>, Shared) {
    let mut cx = TestAppContext::new();
    cx.simulate_resize(size.0, size.1);
    let world = serve(&cx, world);
    let sessions = cx.host().stream::<HostSession>();
    let view = cx.open::<V>();
    sessions.send(session(account));
    cx.run_until_parked();
    let chart = chart_size(size);
    cx.simulate_measure("trade-chart", chart.0, chart.1);
    cx.run_until_parked();
    (cx, view, world)
}

/// A row of the 1h span, `back` hours before [`NOW`].
pub fn hour(back: u64, open: u64, high: u64, low: u64, close: u64, volume: u64) -> Candle {
    Candle {
        bucket: NOW / HOUR - back,
        open: tenths(open),
        high: tenths(high),
        low: tenths(low),
        close: tenths(close),
        volume: volume * UNIT,
    }
}

/// A market with every table empty: nobody has traded, nobody is funded.
pub fn empty() -> World {
    World {
        stats: vec![Stats::default(); trade::MARKETS.len()],
        balances: (0..trade::ASSETS.len() as u8)
            .map(|asset| Balance {
                asset,
                free: 0,
                held: 0,
            })
            .collect(),
        ..World::default()
    }
}

/// The boards' market: BTC/USD at 63,589.1, three days of 1h candles and
/// some before them, the book and the tape of the wide board, and the
/// reader's three open orders, their history, fills and balances.
pub fn board() -> World {
    let stat = |last: u64, before: u64, high: u64, low: u64, volume: u64| Stats {
        last,
        before,
        high,
        low,
        volume,
    };
    let stats = vec![
        stat(
            tenths(635_891),
            tenths(629_595),
            tenths(639_814),
            tenths(623_519),
            527_888 * (UNIT / 100),
        ),
        stat(
            316_420 * (UNIT / 100),
            317_755 * (UNIT / 100),
            319_800 * (UNIT / 100),
            314_050 * (UNIT / 100),
            41_250 * UNIT,
        ),
        stat(
            12_648 * LOT,
            12_267 * LOT,
            12_700 * LOT,
            12_105 * LOT,
            2_480_000 * UNIT,
        ),
    ];
    let level = |(price, amount): (u64, u64)| Level {
        price: tenths(price),
        amount: amount * LOT,
    };
    let asks = [
        (635_897, 4504),
        (635_911, 5998),
        (635_912, 8283),
        (635_926, 23_626),
        (635_935, 2616),
        (635_949, 1521),
        (635_952, 959),
        (635_954, 127),
        (635_956, 505),
        (635_959, 14_860),
        (635_960, 160),
        (635_974, 1682),
        (635_979, 14_145),
        (635_984, 1135),
        (635_993, 1519),
    ];
    let bids = [
        (635_888, 1106),
        (635_887, 13_031),
        (635_873, 20_119),
        (635_868, 134),
        (635_867, 3453),
        (635_853, 3540),
        (635_850, 13_668),
        (635_849, 3152),
        (635_846, 8476),
        (635_837, 685),
        (635_832, 122),
        (635_830, 7617),
        (635_828, 601),
        (635_826, 729),
        (635_823, 114),
    ];
    // price in tenths, amount in lots, seconds after 13:59:00, the taker
    let tape = [
        (635_891, 361, 221, Side::Sell),
        (635_879, 281, 204, Side::Sell),
        (635_880, 6376, 189, Side::Sell),
        (635_889, 1505, 181, Side::Buy),
        (635_891, 4160, 175, Side::Buy),
        (635_879, 84, 166, Side::Sell),
        (635_867, 107, 149, Side::Buy),
        (635_869, 26, 131, Side::Buy),
        (635_870, 117, 110, Side::Buy),
        (635_879, 378, 94, Side::Sell),
        (635_888, 364, 88, Side::Sell),
        (635_876, 2710, 79, Side::Sell),
        (635_885, 4355, 56, Side::Buy),
        (635_886, 3582, 55, Side::Buy),
    ];
    let trades = tape.map(|(price, amount, seconds, taker)| Traded {
        price: tenths(price),
        amount: amount * LOT,
        time: NOW - 60_000 + seconds * 1000,
        taker,
    });
    let order = |id: u64, market: u8, side: Side, limit: u64, amount: u64, placed: u64| Order {
        id,
        owner: READER,
        market,
        side,
        limit: Some(limit),
        amount: amount * LOT,
        filled: 0,
        quote: 0,
        placed,
        state: OrderState::Open,
    };
    let at = |hour: u64, minute: u64, second: u64| {
        NOW - 14 * HOUR + (hour * 3600 + minute * 60 + second) * 1000
    };
    let orders = vec![
        order(6, 0, Side::Buy, tenths(626_350), 2500, at(14, 2, 10)),
        order(5, 0, Side::Sell, tenths(643_520), 1000, at(13, 40, 55)),
        Order {
            filled: 7000 * LOT,
            quote: 2_205 * UNIT,
            ..order(4, 1, Side::Buy, 3150 * UNIT, 20_000, at(11, 12, 3))
        },
        Order {
            filled: 1500 * LOT,
            quote: 9_550_500 * (UNIT / 1000),
            state: OrderState::Filled,
            ..order(3, 0, Side::Buy, tenths(636_700), 1500, at(9, 30, 12))
        },
        Order {
            limit: None,
            filled: 500 * LOT,
            quote: 3_190_250 * (UNIT / 1000),
            state: OrderState::Filled,
            ..order(2, 0, Side::Sell, 0, 500, at(8, 5, 40))
        },
        Order {
            state: OrderState::Cancelled,
            ..order(1, 2, Side::Buy, 12_000 * LOT, 5_000_000, at(7, 1, 0))
        },
    ];
    let filled = |order: u64, side: Side, price: u64, amount: u64, time: u64, maker: bool| Fill {
        order,
        market: 0,
        side,
        price: tenths(price),
        amount: amount * LOT,
        time,
        maker,
    };
    let fills = vec![
        filled(3, Side::Buy, 636_700, 1000, at(9, 41, 3), true),
        filled(3, Side::Buy, 636_700, 500, at(9, 30, 12), true),
        filled(2, Side::Sell, 638_050, 500, at(8, 5, 40), false),
    ];
    let funds = |asset: u8, free: u64, held: u64| Balance { asset, free, held };
    let balances = vec![
        funds(0, 2_528_000 * (UNIT / 100), 1_975_375 * (UNIT / 100)),
        funds(1, 6500 * LOT, 1000 * LOT),
        funds(2, 7000 * LOT, 0),
        funds(3, 0, 0),
    ];
    World {
        stats,
        candles: HashMap::from([((0, HOURS), hours())]),
        book: Book {
            bids: bids.map(level).to_vec(),
            asks: asks.map(level).to_vec(),
        },
        trades: trades.to_vec(),
        balances,
        orders,
        fills,
        refusal: None,
    }
}

/// The boards' 1h candles, oldest first: a climb to 67,900, a slide to
/// 62,400 and the turn back up to 63,589.1, on a walk that repeats.
fn hours() -> Vec<Candle> {
    // the close, in dollars, some hours before the newest candle
    const PATH: [(u64, u64); 12] = [
        (300, 63_800),
        (87, 64_300),
        (68, 67_700),
        (60, 67_300),
        (46, 66_700),
        (30, 64_554),
        (24, 63_100),
        (14, 63_250),
        (8, 63_400),
        (3, 62_700),
        (1, 63_400),
        (0, 63_589),
    ];
    let mut seed = 0x5eed_u64;
    let mut noise = |range: u64| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % range
    };
    let mut close = tenths(638_000);
    let mut rows = Vec::new();
    for back in (0..PATH[0].0).rev() {
        let leg = PATH
            .windows(2)
            .find(|leg| back <= leg[0].0 && back >= leg[1].0);
        let [(from, start), (to, end)] = *leg.expect("the path covers every hour") else {
            unreachable!()
        };
        let along = (from - back) as f64 / (from - to).max(1) as f64;
        let target = start as f64 + (end as f64 - start as f64) * along;
        let open = close;
        // within 250 dollars of the path, on the tick
        close = tenths((target * 10.) as u64 + noise(5000)) - tenths(2500);
        let (high, low) = (open.max(close), open.min(close));
        rows.push(Candle {
            bucket: NOW / HOUR - back,
            open,
            high: high + tenths(noise(2800)),
            low: low - tenths(noise(2800)),
            close,
            volume: (150 + noise(140)) * UNIT,
        });
    }
    // the candle the boards read out, and the newest at the last price
    let hovered = rows.len() - 31;
    rows[hovered] = Candle {
        open: tenths(645_897),
        high: tenths(647_806),
        low: tenths(645_537),
        close: tenths(645_537),
        volume: 232 * UNIT,
        ..rows[hovered]
    };
    let next = &mut rows[hovered + 1];
    next.open = tenths(645_537);
    next.high = next.high.max(next.open);
    next.low = next.low.min(next.open);
    let newest = rows.last_mut().expect("a newest candle");
    newest.close = tenths(635_891);
    newest.high = newest.high.max(newest.close);
    newest.low = newest.low.min(newest.close);
    rows
}
