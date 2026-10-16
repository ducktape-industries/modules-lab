//! Writes the founding's params of a lab stage, a borsh `Vec<History>`:
//! the week of every market's minutes `walk.rs` makes up.
//! `cargo run -p trade --example history -- OUT`.

use std::{env, fs};

use trade::{MARKETS, UNIT};

mod walk;

fn main() {
    let out = env::args().nth(1).expect("usage: history OUT");
    let histories = walk::histories();
    for history in &histories {
        let (Some(first), Some(last)) = (history.bars.first(), history.bars.last()) else {
            continue;
        };
        let high = history.bars.iter().map(|bar| bar.high).max().unwrap_or(0);
        let low = history.bars.iter().map(|bar| bar.low).min().unwrap_or(0);
        let shown = |price: u64| price as f64 / UNIT as f64;
        println!(
            "{}: {} bars, from {} to {}, between {} and {}",
            MARKETS[usize::from(history.market)].name,
            history.bars.len(),
            shown(first.open),
            shown(last.close),
            shown(low),
            shown(high),
        );
    }
    fs::write(&out, borsh::to_vec(&histories).expect("a history encodes")).expect("OUT is written");
}
