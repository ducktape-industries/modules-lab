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
//! - walks every mid, cancels the wallet's orders (`Op::CancelAll`: its
//!   own, never another account's) and rests [`LEVELS`] prices a side
//!   around each new mid;
//! - takes in each market once, at the market, a buy or a sell. The program
//!   allows a self-trade, so most of these trade against its own ladder,
//!   and leave its funds where they were; what trades against another
//!   account's orders moves them.
//!
//! Rounds follow each other with no pause: the cadence is the submit's,
//! about 0.2 s on a kit stage (the CLI starts, opens the wallet's key and
//! waits for the op to preconfirm), so a round of 28 ops takes about 5 s.
//! An op the program refuses is printed and the round goes on. A node that
//! cannot be reached stops it.

use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use borsh::BorshSerialize;
use trade::{ASSETS, LOT, MARKETS, Op, Query, Reply, Side, UNIT};

/// The prices a side rests at.
const LEVELS: u64 = 4;
/// In millionths of the mid, each at least one tick: how far the best bid
/// and the best ask are from it, and the step between two prices of a side.
const SPREAD: u64 = 100;
const STEP: u64 = 100;
/// The most a mid walks in a round, in millionths. At the 13 rounds a
/// minute a kit stage takes, a minute moves about as far as one of
/// `examples/history.rs` (400 millionths, one standard deviation).
const WALK: i64 = 200;
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

/// A round's ops, after every mid has walked: the wallet's orders
/// cancelled, every market's ladder about its new mid, then a taker in
/// each. The ladders go up a level at a time across the markets, so each
/// market has its best prices back after a few ops, not after the other
/// markets' ladders.
fn round(mids: &mut [u64], draws: &mut Draws) -> Vec<Op> {
    for (market, mid) in (0u8..).zip(mids.iter_mut()) {
        *mid = walk(market, *mid, draws);
    }
    let mut ops = vec![Op::CancelAll];
    for at in 0..LEVELS {
        for (market, mid) in (0u8..).zip(mids.iter()) {
            ops.extend(level(market, *mid, at, draws));
        }
    }
    ops.extend((0u8..).take(mids.len()).map(|market| take(market, draws)));
    ops
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

    /// Sends `op`: the program's refusal when it refuses it.
    fn submit(&self, op: &Op) -> Result<(), String> {
        let payload = borsh::to_vec(op).expect("an op encodes");
        let said = String::from_utf8(self.run(&["submit", trade::MODULE], &payload))
            .expect("the node's answer is text");
        match said.lines().next() {
            Some(line) if line.starts_with("applied") => Ok(()),
            line => Err(line.unwrap_or_default().to_owned()),
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
        let key: Vec<u8> = (0..key.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&key[at..at + 2], 16).expect("hex"))
            .collect();
        match self.query("identity", &identity::Query::OfKey { key }) {
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
        let mut ops = round(&mut mids, &mut draws);
        let Reply::Balances(balances) = node.query(trade::MODULE, &Query::Balances { owner })
        else {
            panic!("Balances answers with the balances");
        };
        let low = balances.iter().any(|balance| {
            balance.free + balance.held < ASSETS[usize::from(balance.asset)].grant / 2
        });
        if low {
            ops.insert(0, Op::Fund);
        }
        let mut refused = 0;
        for op in &ops {
            if let Err(refusal) = node.submit(op) {
                refused += 1;
                println!("bot: {op:?} {refusal}");
            }
        }
        let mids: Vec<String> = MARKETS
            .iter()
            .zip(&mids)
            .map(|(market, mid)| format!("{} {}", market.name, shown(*mid)))
            .collect();
        println!(
            "bot: round {n}: {} ops ({refused} refused) in {:.1} s; {}",
            ops.len(),
            started.elapsed().as_secs_f64(),
            mids.join(", ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// From given mids, a round cancels the wallet's orders, walks each mid
    /// a little on its tick, rests four prices a side about it, best first,
    /// never crossing and a level at a time across the markets, then takes
    /// once in each market at the market.
    #[test]
    fn a_round_moves_the_ladder_with_the_mid_and_takes_in_every_market() {
        let given = [66_050 * UNIT, 2_400 * UNIT, UNIT / 4];
        let mut mids = given;
        let ops = round(&mut mids, &mut Draws(7));
        assert_eq!(ops.len(), 1 + 3 * 8 + 3);
        assert_eq!(ops[0], Op::CancelAll);
        assert_ne!(mids, given, "the mids walked");
        // every market's best bid and ask come before any second price
        for market in 0..3 {
            assert_eq!(rested(&ops[1..7], market).len(), 2, "market {market}");
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
            match &ops[1 + 3 * 8 + market] {
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
        // BTC/USD: 0.01% of a mid within 200 millionths of 66,050 is 6.60
        // and a little: 66 whole ticks from the mid to the best bid and to
        // the best ask, and 66 more to the next
        let btc = rested(&ops, 0);
        let center = (mids[0] + MARKETS[0].tick / 2) / MARKETS[0].tick * MARKETS[0].tick;
        let away = btc.iter().map(|(_, limit, _)| center.abs_diff(*limit));
        let ticks: Vec<u64> = away.map(|away| away / MARKETS[0].tick).collect();
        assert_eq!(ticks[..4], [66, 66, 132, 132]);
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

    /// A walk of less than a tick adds up: DUCK's mid moves its ladder,
    /// though no one round's draw reaches half its tick.
    #[test]
    fn moves_smaller_than_a_tick_add_up() {
        let draws = &mut Draws(3);
        let mut mid = UNIT / 4;
        let mut best = std::collections::BTreeSet::new();
        for _ in 0..100 {
            mid = walk(2, mid, draws);
            best.extend(
                rested(&level(2, mid, 0, draws), 2)
                    .into_iter()
                    .map(|(_, limit, _)| limit),
            );
        }
        assert!(best.len() > 2, "{best:?}");
    }
}
