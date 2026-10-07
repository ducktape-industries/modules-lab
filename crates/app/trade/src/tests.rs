//! Every op, once as meant and once as an attack: each refusal leaves the
//! store exactly as it was. The harness is the [`Trade`] module over a
//! [`MockHost`] and a clock the test sets; the accounts that act on it each
//! sign the env of their op.
use std::collections::BTreeMap;

use guest::{Env, Error, MockHost, Module, QueryCtx, code};
use store::PageRequest;

use crate::state::{BALANCES, BOOK, Funds, OPEN, ORDERS, newest_first, rank};
use crate::{
    ASSETS, Balance, Bar, Book, Candle, Fill, History, LOT, Level, MARKETS, MAX_BARS, MAX_CANCELS,
    MAX_FILLS, MAX_ROWS, MODULE, Op, Order, OrderState, Query, Reply, SPANS, Side, Stats, Trade,
    Traded, UNIT, describe,
};

const ADA: u64 = 1;
const BO: u64 = 2;
const CY: u64 = 3;
const DEE: u64 = 4;

/// Places in `ASSETS`.
const USD: u8 = 0;
const BTC: u8 = 1;
/// Places in `MARKETS`; a test that names none trades BTC/USD.
const BTC_USD: u8 = 0;
const ETH_USD: u8 = 1;

const HOUR: u64 = 3_600_000;
const DAY: u64 = 24 * HOUR;

/// A price, or an amount of the quote, in whole dollars.
fn usd(n: u64) -> u64 {
    n * UNIT
}

/// An amount in lots: 10,000 of them are one of the base.
fn lots(n: u64) -> u64 {
    n * LOT
}

fn limit(side: Side, amount: u64, price: u64) -> Op {
    Op::Place {
        market: BTC_USD,
        side,
        amount,
        limit: Some(price),
    }
}

fn market(side: Side, amount: u64) -> Op {
    Op::Place {
        market: BTC_USD,
        side,
        amount,
        limit: None,
    }
}

/// A trade store and a clock: every op runs one block later, at the time
/// the test has set.
#[derive(Default)]
struct Desk {
    store: MockHost,
    height: u64,
    time: u64,
}

impl Desk {
    /// Ada, Bo and Cy, each with their practice funds: 50,000 USD and 1 BTC
    /// among them.
    fn funded() -> Desk {
        let mut desk = Desk::default();
        for who in [ADA, BO, CY] {
            desk.ok(who, Op::Fund);
        }
        desk
    }

    fn env(&self) -> Env {
        Env {
            height: self.height,
            time: self.time,
            ..MockHost::env(MODULE)
        }
    }

    /// A read at the current block.
    fn reads(&self) -> QueryCtx {
        self.store.query(self.env())
    }

    /// Runs `op` as `who` in the next block. A refusal has left the store
    /// as it found it, or this panics.
    #[track_caller]
    fn run(&mut self, who: u64, op: Op) -> Result<(), Error> {
        self.height += 1;
        let signed = self.env().signed(who.to_be_bytes(), Some(who));
        let ctx = self.store.exec(signed);
        self.store.attempt(|| Trade::execute(&ctx, op))
    }

    #[track_caller]
    fn ok(&mut self, who: u64, op: Op) {
        if let Err(refusal) = self.run(who, op) {
            panic!("refused: {refusal}");
        }
    }

    #[track_caller]
    fn refused(&mut self, who: u64, op: Op) -> Error {
        self.run(who, op).expect_err("the op was refused")
    }

    /// Places an order and answers its id: the op's output.
    #[track_caller]
    fn place(&mut self, who: u64, op: Op) -> u64 {
        self.ok(who, op);
        borsh::from_slice(&self.store.take_output()).expect("the output is the order's id")
    }

    /// One trade of `amount` at `price`: Ada's ask, taken whole by Bo.
    #[track_caller]
    fn trade(&mut self, amount: u64, price: u64) {
        self.place(ADA, limit(Side::Sell, amount, price));
        self.place(BO, limit(Side::Buy, amount, price));
    }

    fn ask(&self, question: Query) -> Reply {
        Trade::query(&self.reads(), question).unwrap()
    }

    /// What `who` has of `asset`: free, and held by its orders.
    fn funds(&self, who: u64, asset: u8) -> (u64, u64) {
        let Reply::Balances(balances) = self.ask(Query::Balances { owner: who }) else {
            panic!("balances answer balances");
        };
        let Balance { free, held, .. } = balances[usize::from(asset)];
        (free, held)
    }

    /// The BTC/USD book at its tick.
    fn book(&self) -> Book {
        self.book_by(1, u32::MAX)
    }

    fn book_by(&self, step: u64, depth: u32) -> Book {
        let Reply::Book(book) = self.ask(Query::Book {
            market: BTC_USD,
            step,
            depth,
        }) else {
            panic!("a book answers a book");
        };
        book
    }

    fn order(&self, id: u64) -> Order {
        ORDERS.get(&self.reads(), &id).unwrap().expect("the order")
    }

    /// One page of `who`'s orders, and the cursor of the next.
    fn orders(&self, who: u64, open: bool, page: PageRequest) -> (Vec<u64>, Option<Vec<u8>>) {
        let Reply::Orders(orders) = self.ask(Query::Orders {
            owner: who,
            open,
            page,
        }) else {
            panic!("orders answer orders");
        };
        let ids = orders.items.iter().map(|order| order.id).collect();
        (ids, orders.next)
    }

    fn open(&self, who: u64) -> Vec<u64> {
        self.orders(who, true, PageRequest::default()).0
    }

    fn tape(&self) -> Vec<Traded> {
        let Reply::Trades(trades) = self.ask(Query::Trades {
            market: BTC_USD,
            limit: MAX_ROWS,
        }) else {
            panic!("trades answer trades");
        };
        trades
    }

    fn fills(&self, who: u64) -> Vec<Fill> {
        let Reply::Fills(fills) = self.ask(Query::Fills {
            owner: who,
            page: PageRequest::default(),
        }) else {
            panic!("fills answer fills");
        };
        fills.items
    }

    fn candles(&self, span: u8, before: Option<u64>, limit: u32) -> Vec<Candle> {
        let Reply::Candles(candles) = self.ask(Query::Candles {
            market: BTC_USD,
            span,
            before,
            limit,
        }) else {
            panic!("candles answer candles");
        };
        candles
    }

    /// BTC/USD's stats, asked at `time`.
    fn stats_at(&mut self, time: u64) -> Stats {
        self.time = time;
        let Reply::Stats(stats) = self.ask(Query::Stats) else {
            panic!("stats answer stats");
        };
        assert_eq!(stats.len(), MARKETS.len());
        assert_eq!(stats[usize::from(ETH_USD)], Stats::default());
        stats[usize::from(BTC_USD)]
    }
}

fn level(price: u64, amount: u64) -> Level {
    Level { price, amount }
}

// ── funds ───────────────────────────────────────────────────────────────────

#[test]
fn funding_tops_up_to_the_grant_and_never_lowers() {
    let mut desk = Desk::default();
    assert_eq!(desk.funds(ADA, USD), (0, 0));
    desk.ok(ADA, Op::Fund);
    for (asset, granted) in (0u8..).zip(&ASSETS) {
        assert_eq!(
            desk.funds(ADA, asset),
            (granted.grant, 0),
            "{}",
            granted.name
        );
    }
    let funded = desk.store.borrow().state.clone();
    desk.ok(ADA, Op::Fund);
    assert_eq!(desk.store.borrow().state, funded, "at the grant: nothing");

    // Ada's half a BTC on the book is still hers: nothing to top up
    desk.ok(BO, Op::Fund);
    desk.place(ADA, limit(Side::Sell, lots(5_000), usd(100)));
    desk.ok(ADA, Op::Fund);
    assert_eq!(desk.funds(ADA, BTC), (lots(5_000), lots(5_000)));

    // sold to Bo: she is below the BTC grant and above the USD one, he the
    // other way round
    desk.place(BO, market(Side::Buy, lots(5_000)));
    assert_eq!(desk.funds(ADA, BTC), (lots(5_000), 0));
    for who in [ADA, BO] {
        desk.ok(who, Op::Fund);
    }
    assert_eq!(desk.funds(ADA, BTC), (UNIT, 0));
    assert_eq!(desk.funds(ADA, USD), (usd(50_050), 0));
    assert_eq!(desk.funds(BO, BTC), (lots(15_000), 0));
    assert_eq!(desk.funds(BO, USD), (usd(50_000), 0));
}

#[test]
fn a_key_that_holds_no_account_writes_nothing() {
    let desk = Desk::default();
    let unheld = desk.env().signed([9u8; 32], None);
    let ctx = desk.store.exec(unheld);
    let refusal = desk.store.refused(|| Trade::execute(&ctx, Op::Fund));
    assert_eq!(refusal.code, code::UNAUTHORIZED);
}

// ── placing ─────────────────────────────────────────────────────────────────

#[test]
fn a_limit_order_that_does_not_cross_rests_with_its_funds_held() {
    let mut desk = Desk::funded();
    desk.time = 7_000;
    let id = desk.place(ADA, limit(Side::Buy, lots(5_000), usd(100)));
    assert_eq!(id, 1, "ids count from 1");
    let book = Book {
        bids: vec![level(usd(100), lots(5_000))],
        asks: vec![],
    };
    assert_eq!(desk.book(), book);
    assert_eq!(desk.open(ADA), [1]);
    let order = Order {
        id: 1,
        owner: ADA,
        market: BTC_USD,
        side: Side::Buy,
        limit: Some(usd(100)),
        amount: lots(5_000),
        filled: 0,
        quote: 0,
        placed: 7_000,
        state: OrderState::Open,
    };
    assert_eq!(desk.order(1), order);
    // half a BTC at 100 holds 50 of her 50,000
    assert_eq!(desk.funds(ADA, USD), (usd(49_950), usd(50)));
    assert_eq!(desk.funds(ADA, BTC), (UNIT, 0));

    // an ask above it does not cross either: both rest
    let id = desk.place(BO, limit(Side::Sell, lots(2_500), usd(101)));
    assert_eq!(id, 2);
    assert_eq!(desk.book().asks, [level(usd(101), lots(2_500))]);
    assert_eq!(desk.funds(BO, BTC), (lots(7_500), lots(2_500)));
    assert!(desk.tape().is_empty());
}

#[test]
fn a_crossing_limit_order_trades_at_the_makers_price() {
    let mut desk = Desk::funded();
    let ask = desk.place(ADA, limit(Side::Sell, lots(5_000), usd(100)));
    // Bo would pay 110: he pays Ada's 100 for her half, and what is left of
    // his order rests at his 110
    let bid = desk.place(BO, limit(Side::Buy, lots(8_000), usd(110)));
    assert_eq!(desk.funds(BO, BTC), (lots(15_000), 0));
    assert_eq!(desk.funds(BO, USD), (usd(50_000 - 50 - 33), usd(33)));
    assert_eq!(desk.funds(ADA, BTC), (lots(5_000), 0));
    assert_eq!(desk.funds(ADA, USD), (usd(50_050), 0));

    let sold = desk.order(ask);
    assert_eq!(
        (sold.state, sold.filled, sold.quote),
        (OrderState::Filled, lots(5_000), usd(50))
    );
    let bought = desk.order(bid);
    assert_eq!(
        (bought.state, bought.filled, bought.quote),
        (OrderState::Open, lots(5_000), usd(50))
    );
    assert_eq!(desk.open(ADA), []);
    assert_eq!(desk.open(BO), [bid]);
    let book = Book {
        bids: vec![level(usd(110), lots(3_000))],
        asks: vec![],
    };
    assert_eq!(desk.book(), book);
    assert_eq!(desk.tape().len(), 1);
    assert_eq!(desk.tape()[0].price, usd(100));

    // the same from the other side: Cy would take 90 and gets Bo's 110
    desk.place(CY, limit(Side::Sell, lots(1_000), usd(90)));
    assert_eq!(desk.funds(CY, USD), (usd(50_011), 0));
    assert_eq!(desk.funds(BO, USD), (usd(50_000 - 50 - 33), usd(22)));
    assert_eq!(desk.tape()[0].price, usd(110));
}

#[test]
fn the_best_price_trades_first_then_the_oldest_order() {
    let mut desk = Desk::funded();
    desk.ok(DEE, Op::Fund);
    let dear = desk.place(ADA, limit(Side::Sell, lots(1_000), usd(101)));
    let first = desk.place(BO, limit(Side::Sell, lots(1_000), usd(100)));
    let second = desk.place(CY, limit(Side::Sell, lots(1_000), usd(100)));
    desk.place(DEE, market(Side::Buy, lots(2_500)));
    let makers: Vec<u64> = [BO, CY, ADA]
        .iter()
        .map(|who| desk.fills(*who)[0].order)
        .collect();
    assert_eq!(makers, [first, second, dear]);
    // newest first: the last of it at the dearer price
    let tape: Vec<(u64, u64)> = desk
        .tape()
        .iter()
        .map(|traded| (traded.price, traded.amount))
        .collect();
    let traded = [
        (usd(101), lots(500)),
        (usd(100), lots(1_000)),
        (usd(100), lots(1_000)),
    ];
    assert_eq!(tape, traded);

    // bids the other way: the highest first
    desk.place(ADA, limit(Side::Buy, lots(100), usd(90)));
    let best = desk.place(BO, limit(Side::Buy, lots(100), usd(95)));
    desk.place(DEE, market(Side::Sell, lots(100)));
    assert_eq!(desk.order(best).state, OrderState::Filled);
    assert_eq!(desk.book().bids, [level(usd(90), lots(100))]);
}

#[test]
fn a_maker_filled_in_part_keeps_the_rest_on_the_book() {
    let mut desk = Desk::funded();
    let ask = desk.place(ADA, limit(Side::Sell, lots(1_000), usd(100)));
    let bid = desk.place(BO, limit(Side::Buy, lots(400), usd(100)));
    assert_eq!(desk.order(bid).state, OrderState::Filled);
    let maker = desk.order(ask);
    assert_eq!(
        (maker.state, maker.filled, maker.quote),
        (OrderState::Open, lots(400), usd(4))
    );
    assert_eq!(desk.book().asks, [level(usd(100), lots(600))]);
    assert_eq!(desk.open(ADA), [ask]);
    assert_eq!(desk.funds(ADA, BTC), (lots(9_000), lots(600)));
    // and trades again from where it stands
    desk.place(CY, market(Side::Buy, lots(600)));
    assert_eq!(desk.order(ask).state, OrderState::Filled);
    assert_eq!(desk.book(), Book::default());
    assert_eq!(desk.funds(ADA, BTC), (lots(9_000), 0));
    assert_eq!(desk.funds(ADA, USD), (usd(50_010), 0));
}

#[test]
fn a_market_buy_spends_only_what_the_account_has() {
    let mut desk = Desk::funded();
    desk.place(BO, limit(Side::Sell, lots(10_000), usd(30_000)));
    desk.place(CY, limit(Side::Sell, lots(10_000), usd(30_000)));
    // 50,000 buys Bo's whole BTC and 0.6666 of Cy's: a lot more would
    // take 3 dollars she does not have
    let id = desk.place(ADA, market(Side::Buy, lots(20_000)));
    let order = desk.order(id);
    assert_eq!(order.state, OrderState::Cancelled, "the rest is cancelled");
    assert_eq!(
        (order.filled, order.quote),
        (lots(16_666), usd(49_998)),
        "{order:?}"
    );
    assert_eq!(desk.funds(ADA, USD), (usd(2), 0));
    assert_eq!(desk.funds(ADA, BTC), (lots(26_666), 0));
    assert_eq!(desk.open(ADA), []);
    assert_eq!(desk.book().asks, [level(usd(30_000), lots(3_334))]);

    // with two dollars she buys nothing at 30,000, and is told so
    let refusal = desk.refused(ADA, market(Side::Buy, lots(1)));
    assert_eq!(refusal.code, code::CAPACITY);
    assert_eq!(
        refusal.message,
        "you have 2 USD free, which buys less than 0.0001 BTC at the best price"
    );
}

#[test]
fn a_market_sell_takes_the_bids_and_cancels_its_rest() {
    let mut desk = Desk::funded();
    desk.place(BO, limit(Side::Buy, lots(2_000), usd(100)));
    let id = desk.place(ADA, market(Side::Sell, lots(5_000)));
    let order = desk.order(id);
    assert_eq!(
        (order.state, order.filled, order.quote),
        (OrderState::Cancelled, lots(2_000), usd(20))
    );
    assert_eq!(desk.funds(ADA, BTC), (lots(8_000), 0));
    assert_eq!(desk.funds(BO, USD), (usd(49_980), 0));
    assert_eq!(desk.book(), Book::default());
}

#[test]
fn a_market_order_against_an_empty_side_is_refused() {
    let mut desk = Desk::funded();
    let refusal = desk.refused(ADA, market(Side::Buy, lots(1)));
    assert_eq!(refusal.code, code::WRONG_STATE);
    assert_eq!(refusal.message, "no one is selling on BTC/USD yet");
    // a bid is no one to buy from
    desk.place(BO, limit(Side::Buy, lots(1), usd(100)));
    let refusal = desk.refused(ADA, market(Side::Buy, lots(1)));
    assert_eq!(refusal.code, code::WRONG_STATE);
    desk.ok(BO, Op::CancelAll);
    let refusal = desk.refused(ADA, market(Side::Sell, lots(1)));
    assert_eq!(refusal.message, "no one is buying on BTC/USD yet");
}

#[test]
fn an_order_the_account_cannot_cover_is_refused() {
    let mut desk = Desk::funded();
    // 1 BTC at 50,000.1 is ten cents more than Ada has
    let dear = limit(Side::Buy, lots(10_000), usd(50_000) + UNIT / 10);
    let refusal = desk.refused(ADA, dear);
    assert_eq!(refusal.code, code::CAPACITY);
    assert_eq!(
        refusal.message,
        "this order needs 50000.1 USD and you have 50000 free"
    );
    for side in [Side::Sell, Side::Buy] {
        let much = limit(side, lots(10_001), usd(100_000));
        assert_eq!(desk.refused(ADA, much).code, code::CAPACITY, "{side:?}");
    }
    desk.place(BO, limit(Side::Buy, lots(20_000), usd(100)));
    let refusal = desk.refused(ADA, market(Side::Sell, lots(10_001)));
    assert_eq!(refusal.code, code::CAPACITY);

    // what her resting orders hold is not free for another
    desk.place(ADA, limit(Side::Sell, lots(6_000), usd(200)));
    let again = limit(Side::Sell, lots(6_000), usd(200));
    assert_eq!(desk.refused(ADA, again).code, code::CAPACITY);
    // an account that never took its funds has none
    let unfunded = limit(Side::Buy, lots(1), usd(100));
    assert_eq!(desk.refused(DEE, unfunded).code, code::CAPACITY);
    // a price times an amount past what a balance holds
    let huge = limit(
        Side::Buy,
        u64::MAX - u64::MAX % LOT,
        u64::MAX - u64::MAX % (UNIT / 10),
    );
    assert_eq!(desk.refused(ADA, huge).code, code::CAPACITY);
}

#[test]
fn an_amount_off_the_lot_or_a_price_off_the_tick_is_refused() {
    let mut desk = Desk::funded();
    for amount in [0, 1, LOT - 1, LOT + 1] {
        for op in [
            limit(Side::Buy, amount, usd(100)),
            market(Side::Sell, amount),
        ] {
            assert_eq!(desk.refused(ADA, op).code, code::INVALID_INPUT, "{amount}");
        }
    }
    // BTC/USD moves by ten cents
    for price in [1, usd(100) + UNIT / 100] {
        let refusal = desk.refused(ADA, limit(Side::Sell, lots(1), price));
        assert_eq!(refusal.code, code::INVALID_INPUT, "{price}");
        assert_eq!(refusal.message, "a BTC/USD price is a multiple of 0.1");
    }
    let free = desk.refused(ADA, limit(Side::Sell, lots(1), 0));
    assert_eq!(free.code, code::INVALID_INPUT);
    assert_eq!(free.message, "a limit order needs a price");
    // a cent is a price of ETH/USD
    let cents = Op::Place {
        market: ETH_USD,
        side: Side::Sell,
        amount: lots(1),
        limit: Some(usd(100) + UNIT / 100),
    };
    desk.ok(ADA, cents);
    let nowhere = Op::Place {
        market: MARKETS.len() as u8,
        side: Side::Buy,
        amount: lots(1),
        limit: None,
    };
    assert_eq!(desk.refused(ADA, nowhere).code, code::NOT_FOUND);
}

#[test]
fn an_order_stops_at_max_fills_makers_and_never_leaves_the_book_crossed() {
    let makers = MAX_FILLS as u64;
    let mut desk = Desk::funded();
    for _ in 0..makers + 2 {
        desk.place(ADA, limit(Side::Sell, lots(1), usd(100)));
    }
    // two asks at Bo's price are left when he has traded with all he may:
    // resting, his bid would cross them
    let id = desk.place(BO, limit(Side::Buy, lots(makers + 5), usd(100)));
    let order = desk.order(id);
    assert_eq!(
        (order.state, order.filled),
        (OrderState::Cancelled, lots(makers))
    );
    let book = Book {
        bids: vec![],
        asks: vec![level(usd(100), lots(2))],
    };
    assert_eq!(desk.book(), book);
    assert_eq!(desk.open(BO), []);
    assert_eq!(desk.funds(BO, BTC), (UNIT + lots(makers), 0));
    assert_eq!(desk.funds(BO, USD).1, 0, "nothing of it is held");
    assert_eq!(desk.tape().len(), MAX_FILLS);

    // fewer than the cap: it takes them all and the rest rests
    let id = desk.place(BO, limit(Side::Buy, lots(5), usd(100)));
    assert_eq!(desk.order(id).state, OrderState::Open);
    let book = Book {
        bids: vec![level(usd(100), lots(3))],
        asks: vec![],
    };
    assert_eq!(desk.book(), book);

    // at the cap with the next ask out of reach: nothing to cross, it rests
    let mut desk = Desk::funded();
    for _ in 0..makers {
        desk.place(ADA, limit(Side::Sell, lots(1), usd(100)));
    }
    desk.place(ADA, limit(Side::Sell, lots(1), usd(101)));
    let id = desk.place(BO, limit(Side::Buy, lots(makers + 5), usd(100)));
    let order = desk.order(id);
    assert_eq!(
        (order.state, order.filled),
        (OrderState::Open, lots(makers))
    );
    let book = Book {
        bids: vec![level(usd(100), lots(5))],
        asks: vec![level(usd(101), lots(1))],
    };
    assert_eq!(desk.book(), book);

    // a market order stops there too
    for _ in 0..makers + 1 {
        desk.place(BO, limit(Side::Buy, lots(1), usd(99)));
    }
    let id = desk.place(CY, market(Side::Sell, lots(makers + 9)));
    let order = desk.order(id);
    assert_eq!(
        (order.state, order.filled),
        (OrderState::Cancelled, lots(5 + makers - 1))
    );
}

#[test]
fn a_sum_past_what_a_balance_holds_is_refused_not_wrapped() {
    let mut desk = Desk::funded();
    desk.place(BO, limit(Side::Sell, lots(1), usd(100)));
    // Bo's dollars stand at the most a balance holds: the cent Ada would
    // pay him has nowhere to go
    let full = Funds {
        free: u64::MAX,
        held: 0,
    };
    BALANCES.put(&desk.store.exec(desk.env()), &(BO, USD), &full);
    assert_eq!(
        desk.refused(ADA, market(Side::Buy, lots(1))).code,
        code::CAPACITY
    );
    assert_eq!(
        desk.refused(ADA, limit(Side::Buy, lots(1), usd(100))).code,
        code::CAPACITY
    );
    assert_eq!(desk.book().asks, [level(usd(100), lots(1))]);
}

// ── cancelling ──────────────────────────────────────────────────────────────

#[test]
fn a_cancel_frees_the_held_funds_and_only_the_owner_cancels() {
    let mut desk = Desk::funded();
    let bid = desk.place(ADA, limit(Side::Buy, lots(5_000), usd(100)));
    let ask = desk.place(ADA, limit(Side::Sell, lots(2_000), usd(120)));
    assert_eq!(
        desk.refused(BO, Op::Cancel { order: bid }).code,
        code::UNAUTHORIZED
    );
    assert_eq!(
        desk.refused(ADA, Op::Cancel { order: 99 }).code,
        code::NOT_FOUND
    );
    desk.ok(ADA, Op::Cancel { order: bid });
    assert_eq!(desk.funds(ADA, USD), (usd(50_000), 0));
    assert_eq!(desk.order(bid).state, OrderState::Cancelled);
    assert_eq!(desk.open(ADA), [ask]);
    assert_eq!(desk.book().bids, []);
    let twice = desk.refused(ADA, Op::Cancel { order: bid });
    assert_eq!(twice.code, code::WRONG_STATE);
    assert_eq!(twice.message, "order 1 is already cancelled");

    // an order filled in part frees what is left of it; a filled one is done
    desk.place(BO, limit(Side::Buy, lots(500), usd(120)));
    desk.ok(ADA, Op::Cancel { order: ask });
    assert_eq!(desk.funds(ADA, BTC), (lots(9_500), 0));
    let order = desk.order(ask);
    assert_eq!(
        (order.state, order.filled),
        (OrderState::Cancelled, lots(500))
    );
    assert_eq!(desk.book(), Book::default());
    let taken = desk.place(ADA, limit(Side::Sell, lots(1), usd(100)));
    desk.place(BO, market(Side::Buy, lots(1)));
    let done = desk.refused(ADA, Op::Cancel { order: taken });
    assert_eq!(done.message, format!("order {taken} is already filled"));
}

#[test]
fn cancel_all_cancels_the_senders_open_orders_max_cancels_an_op() {
    let mut desk = Desk::funded();
    desk.place(ADA, limit(Side::Buy, lots(100), usd(90)));
    desk.place(ADA, limit(Side::Sell, lots(100), usd(110)));
    let eth = Op::Place {
        market: ETH_USD,
        side: Side::Sell,
        amount: lots(100),
        limit: Some(usd(10)),
    };
    desk.place(ADA, eth);
    let other = desk.place(BO, limit(Side::Buy, lots(100), usd(91)));
    desk.ok(ADA, Op::CancelAll);
    assert_eq!(desk.open(ADA), []);
    assert_eq!(desk.open(BO), [other]);
    for (asset, granted) in (0u8..).zip(&ASSETS) {
        assert_eq!(
            desk.funds(ADA, asset),
            (granted.grant, 0),
            "{}",
            granted.name
        );
    }
    assert_eq!(desk.book().bids, [level(usd(91), lots(100))]);
    assert_eq!(desk.book().asks, []);
    // with nothing open it changes nothing
    let before = desk.store.borrow().state.clone();
    desk.ok(ADA, Op::CancelAll);
    assert_eq!(desk.store.borrow().state, before);

    // more than one op cancels: the rest stays open for the next
    for _ in 0..MAX_CANCELS + 3 {
        desk.place(ADA, limit(Side::Sell, lots(1), usd(110)));
    }
    desk.ok(ADA, Op::CancelAll);
    assert_eq!(desk.open(ADA).len(), 3);
    assert_eq!(desk.funds(ADA, BTC), (UNIT - lots(3), lots(3)));
    desk.ok(ADA, Op::CancelAll);
    assert_eq!(desk.funds(ADA, BTC), (UNIT, 0));
}

// ── the tape, the fills, the candles ────────────────────────────────────────

#[test]
fn the_tape_lists_trades_newest_first() {
    let mut desk = Desk::funded();
    desk.time = 1_000;
    desk.trade(lots(100), usd(100));
    desk.time = 2_000;
    desk.place(BO, limit(Side::Buy, lots(300), usd(99)));
    desk.place(ADA, market(Side::Sell, lots(200)));
    let tape = [
        Traded {
            price: usd(99),
            amount: lots(200),
            time: 2_000,
            taker: Side::Sell,
        },
        Traded {
            price: usd(100),
            amount: lots(100),
            time: 1_000,
            taker: Side::Buy,
        },
    ];
    assert_eq!(desk.tape(), tape);
    let Reply::Trades(newest) = desk.ask(Query::Trades {
        market: BTC_USD,
        limit: 1,
    }) else {
        panic!("trades answer trades");
    };
    assert_eq!(newest, tape[..1]);
    let Reply::Trades(elsewhere) = desk.ask(Query::Trades {
        market: ETH_USD,
        limit: 9,
    }) else {
        panic!("trades answer trades");
    };
    assert_eq!(elsewhere, []);
}

#[test]
fn a_trade_is_a_fill_for_each_side_and_two_for_an_account_on_both() {
    let mut desk = Desk::funded();
    desk.time = 5_000;
    let ask = desk.place(ADA, limit(Side::Sell, lots(300), usd(100)));
    let bid = desk.place(BO, limit(Side::Buy, lots(200), usd(105)));
    let taken = Fill {
        order: bid,
        market: BTC_USD,
        side: Side::Buy,
        price: usd(100),
        amount: lots(200),
        time: 5_000,
        maker: false,
    };
    let made = Fill {
        order: ask,
        side: Side::Sell,
        maker: true,
        ..taken
    };
    assert_eq!(desk.fills(BO), [taken]);
    assert_eq!(desk.fills(ADA), [made]);
    assert_eq!(desk.fills(CY), []);

    // Ada takes the rest of her own ask: both fills are hers, and she has
    // what she had
    let own = desk.place(ADA, market(Side::Buy, lots(100)));
    let fills = desk.fills(ADA);
    assert_eq!(fills.len(), 3);
    assert_eq!(
        (fills[0].order, fills[0].side, fills[0].maker),
        (ask, Side::Sell, true)
    );
    assert_eq!(
        (fills[1].order, fills[1].side, fills[1].maker),
        (own, Side::Buy, false)
    );
    assert_eq!(fills[2], made);
    assert_eq!(desk.order(ask).state, OrderState::Filled);
    assert_eq!(desk.order(own).state, OrderState::Filled);
    assert_eq!(desk.funds(ADA, BTC), (lots(9_800), 0));
    assert_eq!(desk.funds(ADA, USD), (usd(50_002), 0));
    assert_eq!(desk.tape().len(), 2);
}

#[test]
fn trades_in_one_bucket_fold_into_one_candle_of_every_span() {
    let mut desk = Desk::funded();
    // where a candle of every span opens
    let opens = 3 * DAY;
    desk.time = opens;
    desk.trade(lots(1_000), usd(100));
    desk.time += 10_000;
    desk.trade(lots(2_000), usd(102));
    for (span, kept) in (0u8..).zip(&SPANS) {
        let candle = Candle {
            bucket: opens / kept.millis,
            open: usd(100),
            high: usd(102),
            low: usd(100),
            close: usd(102),
            volume: lots(3_000),
        };
        assert_eq!(
            desk.candles(span, None, MAX_ROWS),
            [candle],
            "{}",
            kept.name
        );
    }

    // a minute on: the next 1m candle, the same candle of every longer span
    desk.time = opens + 60_000;
    desk.place(ADA, limit(Side::Sell, lots(500), usd(99)));
    desk.place(ADA, limit(Side::Sell, lots(500), usd(101)));
    desk.place(BO, market(Side::Buy, lots(1_000)));
    let minute = Candle {
        bucket: opens / 60_000 + 1,
        open: usd(99),
        high: usd(101),
        low: usd(99),
        close: usd(101),
        volume: lots(1_000),
    };
    let minutes = desk.candles(0, None, MAX_ROWS);
    assert_eq!(minutes.len(), 2, "newest first");
    assert_eq!(minutes[0], minute);
    assert_eq!(minutes[1].bucket, opens / 60_000);
    for (span, kept) in (1u8..).zip(&SPANS[1..]) {
        let candle = Candle {
            bucket: opens / kept.millis,
            open: usd(100),
            high: usd(102),
            low: usd(99),
            close: usd(101),
            volume: lots(4_000),
        };
        assert_eq!(
            desk.candles(span, None, MAX_ROWS),
            [candle],
            "{}",
            kept.name
        );
    }

    // a page of them: the newest, then the ones before a bucket
    assert_eq!(desk.candles(0, None, 1), [minute]);
    assert_eq!(desk.candles(0, Some(minute.bucket), 9), minutes[1..]);
    assert_eq!(desk.candles(0, Some(opens / 60_000), 9), []);
    let Reply::Candles(elsewhere) = desk.ask(Query::Candles {
        market: ETH_USD,
        span: 0,
        before: Some(u64::MAX),
        limit: 9,
    }) else {
        panic!("candles answer candles");
    };
    assert_eq!(elsewhere, [], "another market's candles are not below it");
    let no_span = Query::Candles {
        market: BTC_USD,
        span: SPANS.len() as u8,
        before: None,
        limit: 9,
    };
    let refusal = Trade::query(&desk.reads(), no_span).unwrap_err();
    assert_eq!(refusal.code, code::NOT_FOUND);
}

#[test]
fn stats_cover_the_last_24_hours_of_the_1h_candles() {
    let mut desk = Desk::funded();
    assert_eq!(desk.stats_at(0), Stats::default(), "before its first trade");
    desk.time = HOUR / 2;
    desk.trade(lots(1_000), usd(100));
    desk.time = 5 * HOUR;
    desk.trade(lots(1_000), usd(110));
    desk.trade(lots(1_000), usd(105));
    desk.time = 20 * HOUR + 1;
    desk.trade(lots(2_000), usd(90));

    // the market is younger than a day: all of it, and no price before
    let young = Stats {
        last: usd(90),
        before: 0,
        high: usd(110),
        low: usd(90),
        volume: lots(5_000),
    };
    assert_eq!(desk.stats_at(23 * HOUR + 59 * 60_000), young);
    // an hour on, the first hour is the price 24 hours before
    let aged = Stats {
        before: usd(100),
        volume: lots(4_000),
        ..young
    };
    assert_eq!(desk.stats_at(24 * HOUR), aged);
    assert_eq!(desk.stats_at(28 * HOUR + HOUR - 1), aged);
    // hour 5 leaves the day: its last price is the one before
    let later = Stats {
        last: usd(90),
        before: usd(105),
        high: usd(90),
        low: usd(90),
        volume: lots(2_000),
    };
    assert_eq!(desk.stats_at(29 * HOUR), later);
    // a quiet day: the last price stands, nothing traded
    let quiet = Stats {
        last: usd(90),
        before: usd(90),
        ..Stats::default()
    };
    assert_eq!(desk.stats_at(2 * DAY), quiet);
}

// ── the book and the listings ───────────────────────────────────────────────

#[test]
fn a_book_sums_by_price_rounded_away_from_the_spread() {
    let mut desk = Desk::funded();
    let dimes = |n: u64| n * UNIT / 10;
    for (price, amount) in [(999, 100), (995, 200), (995, 50), (982, 400)] {
        desk.place(ADA, limit(Side::Buy, lots(amount), dimes(price)));
    }
    for (price, amount) in [(1_001, 100), (1_006, 200), (1_010, 50), (1_013, 400)] {
        desk.place(BO, limit(Side::Sell, lots(amount), dimes(price)));
    }
    // at the tick: a level for each price, best first
    let book = Book {
        bids: vec![
            level(dimes(999), lots(100)),
            level(dimes(995), lots(250)),
            level(dimes(982), lots(400)),
        ],
        asks: vec![
            level(dimes(1_001), lots(100)),
            level(dimes(1_006), lots(200)),
            level(dimes(1_010), lots(50)),
            level(dimes(1_013), lots(400)),
        ],
    };
    assert_eq!(desk.book(), book);
    assert_eq!(desk.book_by(0, u32::MAX), book, "no step is a step of one");
    // by the dollar: a bid down to it, an ask up to it
    let dollars = Book {
        bids: vec![level(usd(99), lots(350)), level(usd(98), lots(400))],
        asks: vec![level(usd(101), lots(350)), level(usd(102), lots(400))],
    };
    assert_eq!(desk.book_by(10, u32::MAX), dollars);
    let top = Book {
        bids: dollars.bids[..1].to_vec(),
        asks: dollars.asks[..1].to_vec(),
    };
    assert_eq!(desk.book_by(10, 1), top);
    assert_eq!(desk.book_by(10, 0), Book::default());

    // a step no price is a multiple of: every bid at zero, and the asks
    // where a price ends
    let wide = desk.book_by(u64::MAX / (UNIT / 10), 9);
    assert_eq!(wide.bids, [level(0, lots(750))]);
    assert_eq!(wide.asks.len(), 1);
    assert_eq!(wide.asks[0].amount, lots(750));
    let refusal = Trade::query(
        &desk.reads(),
        Query::Book {
            market: BTC_USD,
            step: u64::MAX,
            depth: 9,
        },
    )
    .unwrap_err();
    assert_eq!(refusal.code, code::INVALID_INPUT);

    // an ask at the highest price there is, which no multiple of three
    // ticks is above: the book still answers, with it at the top
    let highest = u64::MAX - u64::MAX % (UNIT / 10);
    desk.place(CY, limit(Side::Sell, lots(1), highest));
    let asks = desk.book_by(3, u32::MAX).asks;
    assert_eq!(asks.last(), Some(&level(u64::MAX, lots(1))));
}

#[test]
fn orders_page_newest_first_all_of_them_or_the_open_ones() {
    let mut desk = Desk::funded();
    for n in 1..=5 {
        desk.place(ADA, limit(Side::Buy, lots(1), usd(90 + n)));
    }
    desk.place(BO, limit(Side::Buy, lots(1), usd(80)));
    desk.ok(ADA, Op::Cancel { order: 4 });

    let (ids, next) = desk.orders(ADA, false, PageRequest::first(2));
    assert_eq!(ids, [5, 4]);
    let (ids, next) = desk.orders(ADA, false, PageRequest::resume(next, 2));
    assert_eq!(ids, [3, 2]);
    let (ids, last) = desk.orders(ADA, false, PageRequest::resume(next.clone(), 2));
    assert_eq!((ids, last), (vec![1], None));

    let (ids, open) = desk.orders(ADA, true, PageRequest::first(2));
    assert_eq!(ids, [5, 3]);
    let (ids, last) = desk.orders(ADA, true, PageRequest::resume(open, 2));
    assert_eq!((ids, last), (vec![2, 1], None));
    assert_eq!(desk.orders(BO, false, PageRequest::default()).0, [6]);
    assert_eq!(desk.orders(CY, true, PageRequest::default()).0, []);

    // a cursor resumes the listing it came from, no other
    let question = Query::Orders {
        owner: ADA,
        open: true,
        page: PageRequest::resume(next, 2),
    };
    let refusal = Trade::query(&desk.reads(), question).unwrap_err();
    assert_eq!(refusal.code, code::STALE);
}

#[test]
fn an_op_reads_as_a_person_would_say_it() {
    use ::describe::{Value, field};
    let units = |value: u64| Value::Amount {
        value: u128::from(value),
        decimals: 8,
    };
    let said = describe(&limit(Side::Buy, lots(5_000), usd(100)));
    assert_eq!(said.title, "Buy BTC/USD at a limit");
    let fields = [
        field("market", Value::text("BTC/USD")),
        field("side", Value::text("Buy")),
        field("amount", units(50_000_000)),
        field("price", units(10_000_000_000)),
    ];
    assert_eq!(said.fields, fields);
    let said = describe(&Op::Place {
        market: ETH_USD,
        side: Side::Sell,
        amount: lots(1),
        limit: None,
    });
    assert_eq!(said.title, "Sell ETH/USD at market");
    assert_eq!(said.fields.len(), 3, "a market order names no price");
    let nowhere = describe(&Op::Place {
        market: 9,
        side: Side::Buy,
        amount: 0,
        limit: None,
    });
    assert_eq!(nowhere.title, "Buy market 9 at market");
    assert_eq!(describe(&Op::Fund).title, "Get practice funds");
    let said = describe(&Op::Cancel { order: 7 });
    assert_eq!(said.title, "Cancel order 7");
    assert_eq!(said.fields, [field("order", Value::count(7))]);
    assert_eq!(describe(&Op::CancelAll).title, "Cancel all open orders");
}

// ── any walk ────────────────────────────────────────────────────────────────

/// xorshift64*: the same seed walks the same way.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % n.max(1)
    }
}

/// Random walks of ops by four accounts over every market: whatever order
/// they come in, a refused op writes nothing, no asset is made or lost but
/// by a grant, and the book, the open sets and the held funds agree with
/// the orders.
#[test]
fn any_walk_conserves_every_asset_and_keeps_the_book_in_step() {
    for seed in 1..=12 {
        let mut rng = Rng(seed);
        let mut desk = Desk::default();
        let mut granted = [0u64; ASSETS.len()];
        for step in 0..400 {
            let who = 1 + rng.below(4);
            desk.time += rng.below(90_000);
            let op = match rng.below(16) {
                0 | 1 => Op::Fund,
                2 | 3 => Op::Cancel {
                    order: 1 + rng.below(step),
                },
                4 => Op::CancelAll,
                n => {
                    let market = rng.below(MARKETS.len() as u64) as u8;
                    // a limit within a few ticks of a thousand of them
                    // crosses about as often as it rests
                    let price = MARKETS[usize::from(market)].tick * (995 + rng.below(10));
                    Op::Place {
                        market,
                        side: if n % 2 == 0 { Side::Buy } else { Side::Sell },
                        amount: lots(1 + rng.below(3_000)),
                        limit: (rng.below(4) > 0).then_some(price),
                    }
                }
            };
            let had = holdings(&desk, who);
            // a refusal that wrote would panic in `run`
            let funded = desk.run(who, op.clone()).is_ok() && op == Op::Fund;
            if funded {
                let has = holdings(&desk, who);
                for (granted, (has, had)) in granted.iter_mut().zip(has.iter().zip(had)) {
                    *granted += has - had;
                }
            }
            in_step(&desk, &granted, seed);
        }
        assert!(!desk.tape().is_empty(), "seed {seed}: the walk trades");
    }
}

/// What `who` has of each asset, free and held together.
fn holdings(desk: &Desk, who: u64) -> Vec<u64> {
    (0u8..)
        .zip(&ASSETS)
        .map(|(asset, _)| {
            let (free, held) = desk.funds(who, asset);
            free + held
        })
        .collect()
}

fn in_step(desk: &Desk, granted: &[u64], seed: u64) {
    let ctx = desk.reads();
    let balances = BALANCES.all(&ctx).unwrap();
    for (asset, granted) in (0u8..).zip(granted) {
        let all: u64 = balances
            .iter()
            .filter(|((_, of), _)| *of == asset)
            .map(|(_, funds)| funds.free + funds.held)
            .sum();
        assert_eq!(all, *granted, "seed {seed}: asset {asset} is conserved");
    }

    let orders = ORDERS.all(&ctx).unwrap();
    let mut held = BTreeMap::<(u64, u8), u64>::new();
    let mut open = 0;
    for (id, order) in &orders {
        assert_eq!(*id, order.id);
        assert!(order.filled <= order.amount, "seed {seed}: {order:?}");
        let done = order.filled == order.amount;
        assert_eq!(done, order.state == OrderState::Filled, "seed {seed}");
        if order.state != OrderState::Open {
            continue;
        }
        // an open order rests for what is left of it, in its owner's open
        // set, holding its stake
        open += 1;
        let market = &MARKETS[usize::from(order.market)];
        let limit = order.limit.expect("only a limit order rests");
        let left = order.amount - order.filled;
        let key = (order.market, order.side as u8, rank(order.side, limit), *id);
        assert_eq!(BOOK.get(&ctx, &key).unwrap(), Some(left), "seed {seed}");
        assert!(OPEN.has(&ctx, &(order.owner, newest_first(*id))));
        let (asset, stake) = match order.side {
            Side::Buy => {
                let quote = u128::from(limit) * u128::from(left) / u128::from(UNIT);
                (market.quote, quote as u64)
            }
            Side::Sell => (market.base, left),
        };
        *held.entry((order.owner, asset)).or_default() += stake;
    }
    assert_eq!(BOOK.all(&ctx).unwrap().len(), open, "seed {seed}");
    assert_eq!(OPEN.all(&ctx).unwrap().len(), open, "seed {seed}");
    for (key, funds) in &balances {
        let orders_hold = held.get(key).copied().unwrap_or(0);
        assert_eq!(funds.held, orders_hold, "seed {seed}: {key:?}");
    }

    for market in (0u8..).take(MARKETS.len()) {
        let Reply::Book(book) = desk.ask(Query::Book {
            market,
            step: 1,
            depth: 1,
        }) else {
            panic!("a book answers a book");
        };
        if let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) {
            assert!(
                bid.price < ask.price,
                "seed {seed}: market {market} crossed"
            );
        }
    }
}

/// A founding at `time` with `histories` for its params.
fn founded(time: u64, histories: &[History]) -> Result<Desk, Error> {
    let desk = Desk {
        time,
        ..Desk::default()
    };
    let params = borsh::to_vec(histories).expect("a history encodes");
    let ctx = desk.store.exec(desk.env().root());
    desk.store.attempt(|| Trade::init(&ctx, &params))?;
    Ok(desk)
}

fn bar(open: u64, high: u64, low: u64, close: u64, lots_traded: u64) -> Bar {
    Bar {
        open: usd(open),
        high: usd(high),
        low: usd(low),
        close: usd(close),
        volume: lots(lots_traded),
    }
}

/// A founding's history is the market's past: its bars are the candles of
/// their span up to the stretch before the founding's, each coarser span
/// is folded from them, a finer one has none, and the next trade carries on
/// from there.
#[test]
fn a_foundings_history_is_the_candles_before_it() {
    const QUARTER: u64 = HOUR / 4;
    // founded 20 minutes into an hour: the last bar is that hour's first quarter
    let now = 500 * DAY + 7 * HOUR + 20 * 60_000;
    let bars = vec![
        bar(100, 104, 99, 103, 10),
        bar(103, 110, 102, 108, 20),
        bar(108, 109, 101, 102, 30),
        bar(102, 106, 100, 105, 40),
        bar(105, 107, 104, 106, 50),
    ];
    let history = History {
        market: BTC_USD,
        span: 2,
        bars: bars.clone(),
    };
    let mut desk = founded(now, &[history]).unwrap();
    let quarters = desk.candles(2, None, MAX_ROWS);
    let newest = now / QUARTER - 1;
    assert_eq!(quarters.len(), 5);
    for (back, (candle, bar)) in quarters.iter().zip(bars.iter().rev()).enumerate() {
        assert_eq!(candle.bucket, newest - back as u64);
        assert_eq!(
            (
                candle.open,
                candle.high,
                candle.low,
                candle.close,
                candle.volume
            ),
            (bar.open, bar.high, bar.low, bar.close, bar.volume)
        );
    }
    // the hour before the founding's holds four bars, the founding's hour one
    let hours = desk.candles(3, None, MAX_ROWS);
    assert_eq!(hours.len(), 2);
    let (this, last) = (hours[0], hours[1]);
    assert_eq!((this.bucket, last.bucket), (now / HOUR, now / HOUR - 1));
    assert_eq!(
        (last.open, last.high, last.low, last.close, last.volume),
        (usd(100), usd(110), usd(99), usd(105), lots(100))
    );
    assert_eq!(
        (this.open, this.close, this.volume),
        (usd(105), usd(106), lots(50))
    );
    assert!(
        desk.candles(0, None, MAX_ROWS).is_empty() && desk.candles(1, None, MAX_ROWS).is_empty()
    );
    assert_eq!(desk.candles(5, None, MAX_ROWS).len(), 1);
    assert_eq!(desk.stats_at(now).last, usd(106));
    // a trade in the founding's hour moves that hour's candle on
    for who in [ADA, BO] {
        desk.ok(who, Op::Fund);
    }
    desk.trade(lots(10), usd(120));
    let this = desk.candles(3, None, 1)[0];
    assert_eq!(
        (this.open, this.high, this.close, this.volume),
        (usd(105), usd(120), usd(120), lots(60))
    );
}

#[test]
fn a_history_that_is_not_candles_founds_nothing() {
    let now = 500 * DAY;
    let told = |market: u8, span: u8, bars: Vec<Bar>| History { market, span, bars };
    let good = bar(100, 104, 99, 103, 10);
    // no params at all is a market with no past
    let desk = Desk::default();
    Trade::init(&desk.store.exec(desk.env().root()), &[]).unwrap();
    assert!(desk.candles(3, None, MAX_ROWS).is_empty());
    let refused = |histories: &[History]| founded(now, histories).err().expect("refused").code;
    // a high under the close, a price off the tick, an amount off the lot
    let off_tick = Bar {
        close: usd(103) + 1,
        ..good
    };
    let off_lot = Bar { volume: 1, ..good };
    for bad in [bar(100, 102, 99, 103, 10), off_tick, off_lot] {
        assert_eq!(
            refused(&[told(BTC_USD, 2, vec![good, bad])]),
            code::INVALID_INPUT
        );
    }
    assert_eq!(refused(&[told(9, 2, vec![good])]), code::NOT_FOUND);
    assert_eq!(refused(&[told(BTC_USD, 6, vec![good])]), code::NOT_FOUND);
    let twice = [told(BTC_USD, 2, vec![good]), told(BTC_USD, 3, vec![good])];
    assert_eq!(refused(&twice), code::INVALID_INPUT);
    assert_eq!(
        refused(&[told(BTC_USD, 0, vec![good; MAX_BARS + 1])]),
        code::CAPACITY
    );
    // bytes that are no history
    let desk = Desk::default();
    let ctx = desk.store.exec(desk.env().root());
    let garbled = desk.store.attempt(|| Trade::init(&ctx, &[7])).unwrap_err();
    assert_eq!(garbled.code, code::INVALID_INPUT);
}
