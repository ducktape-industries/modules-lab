//! A market to draw the view on: the rows of the approved boards, served
//! by a fake host the way the program would answer them. The tests and the
//! `frames` example both draw on it; the view itself holds no sample.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use store::{PageRequest, PageResponse};
use trade::{
    Balance, Book, Candle, Fill, LOT, Level, Order, OrderState, Query, Reply, Side, Stats, Traded,
    UNIT,
};
use view::host::Error;
use view::methods::{HostSession, Query as Ask, Session, Submit, Submitted};
use view::testing::TestAppContext;
use view::{Entity, MouseMoveEvent, Point, View, px};

/// 2026-10-07 14:00 UTC, the hour the boards' newest candle opens at.
pub const NOW: u64 = 1_791_381_600_000;
const HOUR: u64 = 3_600_000;
/// The 1h span's place in `trade::SPANS`.
const SPAN: u8 = 3;
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
    /// What the program answers `query`.
    pub fn answer(&self, query: Query) -> Reply {
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
            // the program's refusal, as the node predicts it
            Some(message) => Ok(Submitted::Refused {
                frame: [0; 32],
                until: 0,
                refusal: Error::new("refused", message.clone()),
            }),
            None => Ok(view::testing::applied(Vec::new())),
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

/// Where the pane's top left corner is in the app's window: a pane is
/// laid out inside the window, never at its corner.
const PANE_AT: (f32, f32) = (312., 96.);

/// Where the host lays the chart out in a pane of `size`: its top left
/// corner in the window's pixels, under the bar and the tools, and its
/// size, what they, the panel and the other columns leave it.
pub fn chart_bounds(size: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let (over, size) = match size.0 >= 1100. {
        true => (48. + 40., (size.0 - 240. - 272., size.1 - 48. - 40. - 158.)),
        false => (40. + 34., (size.0 - 256., size.1 - 40. - 34. - 136.)),
    };
    ((PANE_AT.0, PANE_AT.1 + over), size)
}

/// The pointer moves to `at` of the chart, in the chart's own pixels, in
/// a pane of `size`: the host says where it is in the window.
pub fn point(cx: &mut TestAppContext, size: (f32, f32), at: (f32, f32)) {
    let (origin, _) = chart_bounds(size);
    let moved = MouseMoveEvent {
        position: Point::new(px(origin.0 + at.0), px(origin.1 + at.1)),
        ..Default::default()
    };
    cx.simulate_mouse_move("trade-plot", moved);
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
    let (origin, chart) = chart_bounds(size);
    cx.simulate_bounds("trade-chart", origin, chart);
    cx.run_until_parked();
    (cx, view, world)
}

/// A row of the 1h span, `back` hours before [`NOW`]: its prices in tenths
/// of a dollar, its volume in whole BTC.
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
        candles: HashMap::from([((0, SPAN), hours())]),
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

/// The boards' 1h candles as their chart draws them, the newest last: each
/// one's open, high, low and close in tenths of a dollar, and the BTC it
/// traded. The first 24 are before the wide board's left edge, where its
/// averages are of them.
#[rustfmt::skip]
const HOURS: [[u64; 5]; 112] = [
    [631456, 632056, 630856, 631456, 200], [631456, 634879, 630856, 634279, 200],
    [634279, 634879, 625050, 625650, 200], [625650, 629073, 625050, 628473, 200],
    [628473, 632992, 627873, 632392, 200], [632392, 632992, 631453, 632053, 200],
    [632053, 632653, 626744, 627344, 200], [627344, 634040, 626744, 633440, 200],
    [633440, 639734, 632840, 639134, 200], [639134, 640266, 638534, 639666, 200],
    [639666, 641007, 639066, 640407, 200], [640407, 644072, 639807, 643472, 200],
    [643472, 644072, 642050, 642650, 200], [642650, 644572, 642050, 643972, 200],
    [643972, 647233, 643372, 646633, 200], [646633, 647233, 644888, 645488, 200],
    [645488, 648717, 644888, 648117, 200], [648117, 648717, 642598, 643198, 200],
    [643198, 643798, 639064, 639664, 200], [639664, 641264, 639064, 640664, 200],
    [640664, 641441, 640064, 640841, 200], [640841, 643877, 640241, 643277, 200],
    [643277, 646038, 642677, 645438, 200], [645438, 646038, 641919, 642519, 200],
    [642519, 642677, 639726, 641145, 221], [641145, 645968, 640968, 645968, 206],
    [645968, 653629, 645968, 653629, 230], [653629, 654839, 652306, 653468, 213],
    [653468, 657194, 653468, 656694, 203], [656694, 660210, 655806, 660210, 200],
    [660210, 663500, 659823, 663500, 222], [663500, 666952, 663500, 666452, 218],
    [666452, 670468, 666452, 670129, 209], [670129, 671694, 668032, 670984, 215],
    [670984, 671645, 668855, 670710, 232], [670710, 672710, 669629, 672661, 187],
    [672661, 677097, 672661, 674919, 248], [674919, 674919, 669194, 672081, 182],
    [672081, 672081, 665371, 665742, 204], [665742, 671371, 665065, 670419, 220],
    [670419, 670758, 667548, 669274, 209], [669274, 669790, 667484, 668677, 233],
    [668677, 670613, 666952, 670613, 200], [670613, 679516, 670613, 675016, 259],
    [675016, 675016, 666919, 667629, 241], [667629, 669806, 664242, 667032, 257],
    [667032, 669339, 666790, 669339, 244], [669339, 670645, 666177, 666984, 185],
    [666984, 670323, 666984, 668629, 214], [668629, 671323, 668210, 668645, 212],
    [668645, 673290, 667742, 669177, 228], [669177, 670177, 662726, 662726, 217],
    [662726, 668097, 660565, 667016, 215], [667016, 667016, 662210, 662210, 247],
    [662210, 662677, 659661, 661484, 249], [661484, 662968, 660306, 662968, 215],
    [662968, 662968, 658436, 660226, 237], [660226, 660629, 655387, 655387, 266],
    [655387, 655548, 650065, 653597, 217], [653597, 657597, 653597, 655710, 214],
    [655710, 659968, 655306, 659290, 234], [659290, 661806, 658694, 660355, 275],
    [660355, 661823, 658339, 661306, 246], [661306, 663484, 655936, 663484, 244],
    [663484, 664403, 661839, 661968, 234], [661968, 664758, 660645, 664758, 276],
    [664758, 664758, 654403, 654403, 157], [654403, 655774, 653790, 655000, 214],
    [655000, 657694, 653468, 654484, 192], [654484, 656145, 652242, 655855, 242],
    [655855, 657048, 655000, 655161, 206], [655161, 657903, 654452, 655452, 245],
    [655452, 655581, 652436, 653032, 208], [653032, 654581, 652129, 652129, 266],
    [652129, 653290, 647565, 647565, 239], [647565, 647565, 641403, 643677, 247],
    [643677, 650936, 642613, 649548, 277], [649548, 649548, 643597, 646226, 233],
    [646226, 649613, 646226, 649613, 236], [649613, 649613, 647226, 648936, 251],
    [648936, 648936, 643661, 645897, 248], [645897, 647806, 645537, 645537, 232],
    [645537, 647161, 642968, 643968, 263], [643968, 643968, 636387, 639129, 242],
    [639129, 639919, 633290, 633290, 233], [633290, 634532, 628565, 629565, 223],
    [629565, 632581, 628597, 629790, 203], [629790, 631048, 627500, 629595, 262],
    [629595, 634000, 629595, 634000, 214], [634000, 635806, 633000, 633000, 242],
    [633000, 633000, 630016, 630016, 222], [630016, 632629, 629629, 629629, 276],
    [629629, 630694, 627387, 630226, 216], [630226, 633097, 629500, 630065, 218],
    [630065, 632339, 629436, 631468, 269], [631468, 635984, 631468, 634581, 247],
    [634581, 637339, 633194, 637339, 128], [637339, 637339, 634210, 635419, 185],
    [635419, 636194, 631161, 634871, 266], [634871, 638984, 634742, 634903, 166],
    [634903, 635952, 631032, 631032, 239], [631032, 631661, 630081, 630871, 199],
    [630871, 631694, 625839, 627258, 254], [627258, 629919, 627097, 627097, 205],
    [627097, 628032, 623519, 627742, 155], [627742, 631758, 627323, 630790, 204],
    [630790, 634129, 630274, 630274, 214], [630274, 631694, 627855, 627855, 257],
    [627855, 633597, 627855, 633371, 190], [633371, 637516, 630919, 637516, 236],
    [637516, 639814, 634823, 634825, 232], [634825, 637200, 634591, 635891, 253],
];

/// The 1h candles, oldest first: [`HOURS`], and before them a market that
/// went nowhere, for long enough that the chart pages back into it.
fn hours() -> Vec<Candle> {
    // a wave a day long and 300 dollars high
    let wave = |back: u64| 631_000 + (back % 24).abs_diff(12) * 250;
    let quiet = (HOURS.len() as u64..300).rev().map(|back| {
        let (open, close) = (wave(back + 1), wave(back));
        let (high, low) = (open.max(close) + 400, open.min(close) - 400);
        hour(back, open, high, low, close, 180)
    });
    let backs = (0..HOURS.len() as u64).rev();
    let board = HOURS.iter().zip(backs).map(|(row, back)| {
        let [open, high, low, close, volume] = *row;
        hour(back, open, high, low, close, volume)
    });
    quiet.chain(board).collect()
}
