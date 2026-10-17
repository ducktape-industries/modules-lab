//! What a lab stage is seeded with after its founding ([`book`]): two
//! accounts with their practice funds, one's orders resting on both sides
//! of BTC/USD, and two market orders of the other's that trade against
//! them. `cargo run -p trade-stage --bin seed -- HISTORY OUT` reads the
//! founding's params (the `history` binary writes them) for the price the
//! history ends at, and writes each op as the borsh bytes the node's CLI
//! submits, into OUT, a directory it makes anew:
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
use trade::{History, MARKETS, Op, UNIT};
use trade_stage::book;

/// The names of the two accounts: the taker's, whose wallet the app opens
/// with, and the maker's.
const ACCOUNTS: [&str; 2] = ["Ann", "Bo"];

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
        .find(|history| history.market == book::MARKET)
        .and_then(|history| history.bars.last())
        .expect("the history holds the market")
        .close;
    let (rests, takes) = (book::rests(last), book::takes());

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
    for (n, op) in rests.iter().enumerate() {
        write(out, &format!("rest-{n:02}.bin"), op);
    }
    for (n, op) in takes.iter().enumerate() {
        write(out, &format!("take-{n}.bin"), op);
    }

    println!(
        "{}: the history ends at {}; {} orders rest around it and {} trade against them",
        MARKETS[usize::from(book::MARKET)].name,
        last as f64 / UNIT as f64,
        rests.len(),
        takes.len(),
    );
}
