//! What a lab stage is seeded with after its founding, so the view opens on
//! a market that is alive: two accounts with their practice funds, one's
//! orders resting on both sides of BTC/USD, and two market orders of the
//! other's that trade against them.
//! `cargo run -p trade --example seed -- HISTORY OUT` reads the founding's
//! params (`examples/history.rs` writes them) for the price the history ends
//! at, and writes each op as the borsh bytes the node's CLI submits, into
//! OUT, a directory it makes anew:
//!
//! - `account-NAME.bin`: identity's op that gives a key its account;
//! - `fund.bin`: `Op::Fund`, for both accounts;
//! - `rest-NN.bin`: the maker's limit orders, best price first, a buy and a
//!   sell in turn;
//! - `take-N.bin`: the taker's market orders, a buy and a sell.
//!
//! `live/live.sh` sends them in that order. The bytes are made from the
//! types on the wire, so they change when those do.

use std::{env, fs, path::Path};

use borsh::BorshSerialize;
use trade::{ASSETS, History, LOT, MARKETS, Op, Side, UNIT};

/// BTC/USD, by its place in [`MARKETS`].
const MARKET: u8 = 0;
/// The names of the two accounts: the taker's, whose wallet the app opens
/// with, and the maker's.
const ACCOUNTS: [&str; 2] = ["Ann", "Bo"];
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

fn write(out: &Path, name: &str, op: &impl BorshSerialize) {
    let bytes = borsh::to_vec(op).expect("an op encodes");
    fs::write(out.join(name), bytes).expect("OUT is written");
}

fn main() {
    let mut args = env::args().skip(1);
    let (Some(history), Some(out)) = (args.next(), args.next()) else {
        panic!("usage: seed HISTORY OUT");
    };
    let histories: Vec<History> =
        borsh::from_slice(&fs::read(&history).expect("HISTORY is read")).expect("a history");
    let last = histories
        .iter()
        .find(|history| history.market == MARKET)
        .and_then(|history| history.bars.last())
        .expect("the history holds the market")
        .close;
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

    let out = Path::new(&out);
    // anew: a file an earlier run wrote is not a part of this seed
    let _ = fs::remove_dir_all(out);
    fs::create_dir_all(out).expect("OUT is made");
    for name in ACCOUNTS {
        let account = identity::Op::Create {
            name: name.to_owned(),
            scheme: ::program::Scheme::Ed25519,
        };
        write(
            out,
            &format!("account-{}.bin", name.to_lowercase()),
            &account,
        );
    }
    write(out, "fund.bin", &Op::Fund);
    for level in 0..LEVELS {
        let buy = place(Side::Buy, lots(level, BID_TURN), Some(bid(level)));
        let sell = place(Side::Sell, lots(level, ASK_TURN), Some(ask(level)));
        write(out, &format!("rest-{:02}.bin", 2 * level), &buy);
        write(out, &format!("rest-{:02}.bin", 2 * level + 1), &sell);
    }
    for (n, (side, lots)) in TAKES.into_iter().enumerate() {
        write(out, &format!("take-{n}.bin"), &place(side, lots, None));
    }

    let shown = |price: u64| price as f64 / UNIT as f64;
    println!(
        "{}: the history ends at {}; {LEVELS} bids from {} down to {}, {LEVELS} asks from {} up to {}",
        market.name,
        shown(last),
        shown(bid(0)),
        shown(bid(LEVELS - 1)),
        shown(ask(0)),
        shown(ask(LEVELS - 1)),
    );
}
