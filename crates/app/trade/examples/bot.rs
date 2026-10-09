//! The market maker of a live stage: a process off the chain that keeps
//! every market moving, so the book refills and the candles form without a
//! hand on them. It trades as one wallet, through the node's own binary, as
//! `live/live.sh` seeds the stage: `NODE submit trade` with an op's borsh
//! bytes, `NODE query trade` with a query's.
//!
//! `cargo run -p trade --example bot -- NODE STAGE WALLET` runs until it is
//! stopped. STAGE is the stage's directory as kit keeps it (`home/`, the
//! wallet's key in `home/keys/`, `password`); `make bot` runs it as `bo` on
//! the stage `make live` made.
//!
//! Each market's mid starts at its last price (`Query::Stats`: the last
//! trade, or the history's last close) and walks at random from there. A
//! round:
//!
//! - tops the wallet's funds up (`Op::Fund`) when an asset has under half
//!   its grant;
//! - walks every mid and rests [`LEVELS`] prices a side about each new
//!   one, the new ladder beside the old;
//! - takes in each market once, at the market, a buy or a sell. The program
//!   allows a self-trade, so most of these trade against its own ladders,
//!   and leave its funds where they were; what trades against another
//!   account's orders moves them;
//! - cancels, one by one (`Op::Cancel`), every open order of the wallet's
//!   that the round did not place: the old ladder, once the new one is on
//!   the book, so the book is never empty. Another account's orders are
//!   never touched.
//!
//! The ids: a `Place` returns its order's id, which the node's CLI prints
//! (`applied <hex>`), so the bot knows what it placed this round. What is
//! still open it reads (`Query::Orders`) after the takers, since a taker
//! may have filled an old order whole, and the program refuses to cancel a
//! filled one; it also finds the orders the wallet held before the bot
//! started (the seed's).
//!
//! Rounds follow each other with no pause: the cadence is the submit's,
//! 0.1 to 0.3 s on a kit stage (the CLI starts, opens the wallet's key and
//! waits for the op to preconfirm), so a round of about 50 ops takes 6 to
//! 9 s. An op the program refuses is printed and the round goes on. A node
//! that cannot be reached stops it.

use std::{
    collections::BTreeSet,
    env, fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use borsh::BorshSerialize;
use store::PageRequest;
use trade::{ASSETS, LOT, MARKETS, Op, Order, Query, Reply, Side, UNIT};

/// The prices a side rests at.
const LEVELS: u64 = 4;
/// In millionths of the mid, each at least one tick: how far the best bid
/// and the best ask are from it, and the step between two prices of a side.
const SPREAD: u64 = 100;
const STEP: u64 = 100;
/// The most a mid walks in a round, in millionths. At the 9 rounds a
/// minute a kit stage takes, a minute moves about as far as one of
/// `examples/history.rs` (400 millionths, one standard deviation).
const WALK: i64 = 230;
/// What one unit of a market's amounts is, in lots, by its place in
/// [`MARKETS`]: 0.01 BTC, 0.1 ETH, 10 DUCK. A price rests 1 to 4 of them,
/// and a taker takes 1 or 2.
const UNITS: [u64; MARKETS.len()] = [100, 1_000, 100_000];

/// splitmix64, as `examples/history.rs` draws.
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

/// `millionths` of `price`, in whole ticks of `market`: at least one.
fn ticks(market: u8, price: u64, millionths: u64) -> u64 {
    let tick = MARKETS[usize::from(market)].tick;
    (price / 1_000_000 * millionths / tick).max(1) * tick
}

/// A mid moved by a draw of at most [`WALK`] millionths. It is kept finer
/// than the tick, so moves smaller than a tick add up: DUCK's tick is 438
/// millionths of its price.
fn walk(market: u8, mid: u64, draws: &mut Draws) -> u64 {
    let tick = MARKETS[usize::from(market)].tick;
    let by = draws.around(WALK);
    let moved = mid / 1_000_000 * by.unsigned_abs();
    let moved = if by < 0 { mid - moved } else { mid + moved };
    moved.max(tick)
}

/// The bid and the ask of a ladder's `level`th price about `mid`, on the
/// market's tick nearest it, the first being the best.
fn level(market: u8, mid: u64, level: u64, draws: &mut Draws) -> [Op; 2] {
    let tick = MARKETS[usize::from(market)].tick;
    let mid = (mid + tick / 2) / tick * tick;
    let away = ticks(market, mid, SPREAD) + level * ticks(market, mid, STEP);
    let unit = UNITS[usize::from(market)];
    let mut rest = |side: Side, limit: u64| Op::Place {
        market,
        side,
        amount: (1 + draws.next() % 4) * unit * LOT,
        limit: Some(limit),
    };
    [
        rest(Side::Buy, mid.saturating_sub(away)),
        rest(Side::Sell, mid + away),
    ]
}

/// A market order of one or two units, a buy or a sell.
fn take(market: u8, draws: &mut Draws) -> Op {
    let side = match draws.next() % 2 {
        0 => Side::Buy,
        _ => Side::Sell,
    };
    let lots = (1 + draws.next() % 2) * UNITS[usize::from(market)];
    Op::Place {
        market,
        side,
        amount: lots * LOT,
        limit: None,
    }
}

/// A round, op by op through `send`, which answers the id of the order an
/// op placed (none for a refusal): every mid walks; every market's new
/// ladder rests about its new mid, a level at a time across the markets;
/// a taker in each market; then every order of the wallet's still open
/// (`open`, read after the takers) that the round did not place is
/// cancelled by its id. The old ladder stays until the new one is on the
/// book. Answers how many ops it sent.
fn round(
    mids: &mut [u64],
    draws: &mut Draws,
    mut send: impl FnMut(&Op) -> Option<u64>,
    open: impl FnOnce() -> Vec<Order>,
) -> usize {
    for (market, mid) in (0u8..).zip(mids.iter_mut()) {
        *mid = walk(market, *mid, draws);
    }
    let mut placed = BTreeSet::new();
    let mut sent = 0;
    for at in 0..LEVELS {
        for (market, mid) in (0u8..).zip(mids.iter()) {
            for op in level(market, *mid, at, draws) {
                placed.extend(send(&op));
                sent += 1;
            }
        }
    }
    for market in (0u8..).take(mids.len()) {
        send(&take(market, draws));
        sent += 1;
    }
    for order in open().iter().filter(|order| !placed.contains(&order.id)) {
        send(&Op::Cancel { order: order.id });
        sent += 1;
    }
    sent
}

/// `text` as hex, two digits a byte.
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// The node's binary as one wallet of one stage.
struct Node {
    binary: String,
    stage: PathBuf,
    wallet: String,
    password: String,
}

impl Node {
    /// `ARGS` with `input` on stdin: what it printed. A node that cannot be
    /// reached, or a request it cannot read, stops the bot.
    fn run(&self, args: &[&str], input: &[u8]) -> Vec<u8> {
        let home = self.stage.join("home");
        let key = home.join("keys").join(format!("{}.key", self.wallet));
        let mut child = Command::new(&self.binary)
            .args(args)
            .env("DUCKTAPE_HOME", &home)
            .env("DUCKTAPE_PASSWORD", &self.password)
            .env("DUCKTAPE_USER_KEY", key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("NODE runs");
        let mut stdin = child.stdin.take().expect("its stdin");
        stdin.write_all(input).expect("the request is sent");
        drop(stdin);
        let out = child.wait_with_output().expect("NODE ends");
        if !out.status.success() {
            panic!(
                "`{} {}` failed: {}",
                self.binary,
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        out.stdout
    }

    fn query<R: borsh::BorshDeserialize>(&self, program: &str, query: &impl BorshSerialize) -> R {
        let request = borsh::to_vec(query).expect("a query encodes");
        let answer = self.run(&["query", program], &request);
        borsh::from_slice(&answer).expect("the program's reply")
    }

    /// Sends `op`: its return data (`applied <hex>`), or the program's
    /// refusal when it refuses it.
    fn submit(&self, op: &Op) -> Result<Vec<u8>, String> {
        let payload = borsh::to_vec(op).expect("an op encodes");
        let said = String::from_utf8(self.run(&["submit", trade::MODULE], &payload))
            .expect("the node's answer is text");
        let first = said.lines().next().unwrap_or_default();
        match first.strip_prefix("applied") {
            Some(output) => Ok(hex(output.trim())),
            None => Err(first.to_owned()),
        }
    }

    /// The wallet's account: its key as identity knows it.
    fn account(&self) -> u64 {
        let listed = String::from_utf8(self.run(&["wallet", "list"], &[])).expect("text");
        let key = listed
            .lines()
            .map(|row| {
                row.trim_start_matches(['*', ' '])
                    .split('\t')
                    .collect::<Vec<_>>()
            })
            .find(|row| row.first() == Some(&self.wallet.as_str()))
            .and_then(|row| row.get(1).map(|key| key.to_string()))
            .expect("the wallet is listed");
        match self.query("identity", &identity::Query::OfKey { key: hex(&key) }) {
            identity::Reply::Number(Some(account)) => account,
            reply => panic!("{} has no account: {reply:?}", self.wallet),
        }
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let (Some(binary), Some(stage), Some(wallet)) = (args.next(), args.next(), args.next()) else {
        panic!("usage: bot NODE STAGE WALLET");
    };
    let stage = PathBuf::from(stage);
    let password = fs::read_to_string(stage.join("password")).expect("the stage's password");
    let node = Node {
        binary,
        stage,
        wallet,
        password: password.trim_end().to_owned(),
    };
    let owner = node.account();
    let Reply::Stats(stats) = node.query(trade::MODULE, &Query::Stats) else {
        panic!("Stats answers with the stats");
    };
    let mut mids: Vec<u64> = stats.iter().map(|stats| stats.last).collect();
    assert!(
        mids.iter().all(|mid| *mid > 0),
        "every market needs a price to start from: a stage founded with its history has one"
    );
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970");
    let mut draws = Draws(seed.as_nanos() as u64);
    let shown = |price: u64| price as f64 / UNIT as f64;
    println!("bot: wallet {} (account {owner})", node.wallet);
    for n in 1.. {
        let started = Instant::now();
        let mut refused = 0;
        let mut send = |op: &Op| match node.submit(op) {
            Ok(output) => match op {
                Op::Place { .. } => {
                    Some(borsh::from_slice(&output).expect("a Place returns its order's id"))
                }
                _ => None,
            },
            Err(refusal) => {
                refused += 1;
                println!("bot: {op:?} {refusal}");
                None
            }
        };
        let Reply::Balances(balances) = node.query(trade::MODULE, &Query::Balances { owner })
        else {
            panic!("Balances answers with the balances");
        };
        let low = balances.iter().any(|balance| {
            balance.free + balance.held < ASSETS[usize::from(balance.asset)].grant / 2
        });
        if low {
            send(&Op::Fund);
        }
        let open = || {
            let page = PageRequest::first(PageRequest::MAX_LIMIT);
            match node.query(
                trade::MODULE,
                &Query::Orders {
                    owner,
                    open: true,
                    page,
                },
            ) {
                Reply::Orders(open) => open.items,
                _ => panic!("Orders answers with the orders"),
            }
        };
        let sent = usize::from(low) + round(&mut mids, &mut draws, &mut send, open);
        let mids: Vec<String> = MARKETS
            .iter()
            .zip(&mids)
            .map(|(market, mid)| format!("{} {}", market.name, shown(*mid)))
            .collect();
        println!(
            "bot: round {n}: {sent} ops ({refused} refused) in {:.1} s; {}",
            started.elapsed().as_secs_f64(),
            mids.join(", ")
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use trade::OrderState;

    use super::*;

    /// The program's book as far as the bot's own orders go: a `Place`
    /// takes from the other side's best prices its limit reaches (a market
    /// order's, any), and what is left of a limit order rests; it answers
    /// the order's id. A `Cancel` or a `CancelAll` takes orders off. Every
    /// op sent is kept, in order.
    #[derive(Default)]
    struct Fake {
        resting: Vec<Order>,
        sent: Vec<Op>,
        next: u64,
    }

    impl Fake {
        fn apply(&mut self, op: &Op) -> Option<u64> {
            self.sent.push(op.clone());
            match *op {
                Op::Place {
                    market,
                    side,
                    mut amount,
                    limit,
                } => {
                    let price = |order: &Order| order.limit.unwrap_or_default();
                    let reached = |order: &&mut Order| {
                        order.market == market
                            && order.side != side
                            && limit.is_none_or(|limit| match side {
                                Side::Buy => price(order) <= limit,
                                Side::Sell => price(order) >= limit,
                            })
                    };
                    let mut makers: Vec<&mut Order> =
                        self.resting.iter_mut().filter(reached).collect();
                    match side {
                        Side::Buy => makers.sort_by_key(|order| price(order)),
                        Side::Sell => makers.sort_by_key(|order| u64::MAX - price(order)),
                    }
                    for maker in makers {
                        let took = amount.min(maker.amount - maker.filled);
                        maker.filled += took;
                        amount -= took;
                    }
                    self.resting.retain(|order| order.filled < order.amount);
                    self.next += 1;
                    if limit.is_some() && amount > 0 {
                        self.resting.push(Order {
                            id: self.next,
                            owner: 9,
                            market,
                            side,
                            limit,
                            amount,
                            filled: 0,
                            quote: 0,
                            placed: 0,
                            state: OrderState::Open,
                        });
                    }
                    Some(self.next)
                }
                Op::Cancel { order } => {
                    self.resting.retain(|resting| resting.id != order);
                    None
                }
                Op::CancelAll => {
                    self.resting.clear();
                    None
                }
                Op::Fund => None,
            }
        }

        /// Whether every market has a bid and an ask of the bot's resting.
        fn two_sided(&self) -> bool {
            (0..MARKETS.len() as u8).all(|market| {
                let side = |side| {
                    let mut mine = self.resting.iter();
                    mine.any(|order| order.market == market && order.side == side)
                };
                side(Side::Buy) && side(Side::Sell)
            })
        }
    }

    /// A round on `book`: the ops it sent, in order.
    fn sent(mids: &mut [u64], draws: &mut Draws, book: &RefCell<Fake>) -> Vec<Op> {
        let before = book.borrow().sent.len();
        round(
            mids,
            draws,
            |op| book.borrow_mut().apply(op),
            || book.borrow().resting.clone(),
        );
        book.borrow().sent[before..].to_vec()
    }

    /// Market `of`'s limit orders among `ops`, as (side, limit, amount).
    fn rested(ops: &[Op], of: usize) -> Vec<(Side, u64, u64)> {
        let rested = ops.iter().filter_map(|op| match op {
            Op::Place {
                market,
                side,
                amount,
                limit: Some(limit),
            } if usize::from(*market) == of => Some((*side, *limit, *amount)),
            _ => None,
        });
        rested.collect()
    }

    /// The headline: round after round, every market keeps a bid and an
    /// ask of the bot's on the book after every op, and once a round is
    /// over what rests is that round's ladder alone.
    #[test]
    fn the_book_keeps_a_bid_and_an_ask_at_every_step_of_a_round() {
        let book = RefCell::new(Fake::default());
        let draws = &mut Draws(5);
        let mut mids = [66_050 * UNIT, 2_400 * UNIT, UNIT / 4];
        // the first ladders, on an empty book
        sent(&mut mids, draws, &book);
        for n in 0..20 {
            let first = book.borrow().next + 1;
            let ops = round(
                &mut mids,
                draws,
                |op| {
                    let id = book.borrow_mut().apply(op);
                    assert!(book.borrow().two_sided(), "round {n}, after {op:?}");
                    id
                },
                || book.borrow().resting.clone(),
            );
            assert!(ops > 3 * 8 + 3, "the old ladder was cancelled: {ops} ops");
            let resting = &book.borrow().resting;
            assert!(resting.iter().all(|order| order.id >= first), "round {n}");
        }
    }

    /// From given mids, a round walks each mid a little, rests four prices
    /// a side about it, best first, never crossing and a level at a time
    /// across the markets, takes once in each market at the market, and
    /// then cancels by its id each of the wallet's open orders it did not
    /// place: the ones the round before left, not the ones a taker filled.
    #[test]
    fn a_round_moves_the_ladder_with_the_mid_and_takes_in_every_market() {
        let book = RefCell::new(Fake::default());
        let draws = &mut Draws(7);
        let given = [66_050 * UNIT, 2_400 * UNIT, UNIT / 4];
        let mut mids = given;
        let ops = sent(&mut mids, draws, &book);
        assert_eq!(ops.len(), 3 * 8 + 3, "nothing to cancel on an empty book");
        assert_ne!(mids, given, "the mids walked");
        // every market's best bid and ask come before any second price
        for market in 0..3 {
            assert_eq!(rested(&ops[..6], market).len(), 2, "market {market}");
        }
        for (market, (mid, given)) in mids.iter().zip(given).enumerate() {
            let tick = MARKETS[market].tick;
            let moved = mid.abs_diff(given);
            assert!(moved <= given / 1_000_000 * WALK as u64 + tick, "{moved}");
            let ladder = rested(&ops, market);
            let side = |side: Side| -> Vec<u64> {
                let prices = ladder.iter().filter(|(of, ..)| *of == side);
                prices.map(|(_, limit, _)| *limit).collect()
            };
            let (bids, asks) = (side(Side::Buy), side(Side::Sell));
            assert_eq!((bids.len(), asks.len()), (4, 4));
            // best first, about the walked mid, as far from it on each side
            assert!(bids.windows(2).all(|two| two[0] > two[1]));
            assert!(asks.windows(2).all(|two| two[0] < two[1]));
            let center = (mid + tick / 2) / tick * tick;
            assert!(bids[0] < center && asks[0] > center);
            assert_eq!(center - bids[0], asks[0] - center);
            for (_, limit, amount) in &ladder {
                assert_eq!(limit % tick, 0);
                assert_eq!(amount % LOT, 0);
                let units = amount / LOT / UNITS[market];
                assert!((1..=4).contains(&units), "{units}");
            }
            match &ops[3 * 8 + market] {
                Op::Place {
                    market: at,
                    limit: None,
                    amount,
                    ..
                } => {
                    assert_eq!(usize::from(*at), market);
                    assert!((1..=2).contains(&(amount / LOT / UNITS[market])));
                }
                op => panic!("{op:?} is no taker"),
            }
        }
        // BTC/USD: 0.01% of a mid within WALK of 66,050 is 6.60 and a
        // little: 66 whole ticks from the mid to the best bid and to the best
        // ask, and 66 more to the next
        let btc = rested(&ops, 0);
        let center = (mids[0] + MARKETS[0].tick / 2) / MARKETS[0].tick * MARKETS[0].tick;
        let away = btc.iter().map(|(_, limit, _)| center.abs_diff(*limit));
        let ticks: Vec<u64> = away.map(|away| away / MARKETS[0].tick).collect();
        assert_eq!(ticks[..4], [66, 66, 132, 132]);
        // another account takes BTC's best ask whole; the next round, after
        // its own ladder and takers, cancels by id what is left of this
        // round's orders, and not the one filled
        let best = |book: &Fake| {
            let asks = book.resting.iter().filter(|order| order.market == 0);
            let asks = asks.filter(|order| order.side == Side::Sell);
            asks.min_by_key(|order| order.limit)
                .map(|order| (order.id, order.amount - order.filled))
        };
        let (filled, amount) = best(&book.borrow()).expect("an ask");
        let take = Op::Place {
            market: 0,
            side: Side::Buy,
            amount,
            limit: None,
        };
        book.borrow_mut().apply(&take);
        let left: BTreeSet<u64> = book.borrow().resting.iter().map(|order| order.id).collect();
        assert!(!left.contains(&filled) && left.len() == 23, "{left:?}");
        let ops = sent(&mut mids, draws, &book);
        let cancelled: Vec<u64> = ops[3 * 8 + 3..]
            .iter()
            .map(|op| match op {
                Op::Cancel { order } => *order,
                op => panic!("{op:?} is no cancel"),
            })
            .collect();
        assert!(
            cancelled.iter().all(|id| left.contains(id)),
            "{cancelled:?}"
        );
        assert!(!cancelled.contains(&filled));
        let resting = &book.borrow().resting;
        assert!(!resting.iter().any(|order| left.contains(&order.id)));
    }

    /// DUCK's tick is 400 millionths of its 0.25: the spread and the step
    /// are a tick at the least, so no two prices are one.
    #[test]
    fn a_ladder_is_a_tick_apart_at_the_least() {
        let tick = MARKETS[2].tick;
        let mid = UNIT / 4;
        let draws = &mut Draws(1);
        let prices: Vec<u64> = (0..LEVELS)
            .flat_map(|at| level(2, mid, at, draws))
            .map(|op| match op {
                Op::Place {
                    limit: Some(limit), ..
                } => limit,
                op => panic!("{op:?}"),
            })
            .collect();
        let away: Vec<i64> = prices
            .iter()
            .map(|price| (*price as i64 - mid as i64) / tick as i64)
            .collect();
        assert_eq!(away, [-1, 1, -2, 2, -3, 3, -4, 4]);
    }

    /// A mid is kept finer than its tick, so every draw counts: DUCK's tick
    /// is 438 millionths of its price, and a mid rounded to it each round
    /// would move only on the draws past half a tick.
    #[test]
    fn a_mid_is_kept_finer_than_the_tick() {
        let draws = &mut Draws(3);
        let tick = MARKETS[2].tick;
        let mut mid = UNIT / 4;
        let mut off = 0;
        for _ in 0..20 {
            mid = walk(2, mid, draws);
            off += usize::from(!mid.is_multiple_of(tick));
        }
        assert!(off > 10, "{off} of 20 mids between two ticks");
    }
}
