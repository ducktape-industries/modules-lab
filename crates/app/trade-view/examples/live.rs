//! `cargo run -p trade-view --example live`: Trade in the real app over a
//! fake node (`view::rig`), founded and seeded as `make live` founds and
//! seeds a lab stage (`crates/tools/trade-stage`): a week of every
//! market's minutes, a maker's book around BTC/USD's last price, and the
//! reader's two market orders that trade against it. Then
//! the reader rests three limit orders and cancels a fourth, so every
//! board has its rows: the book and the tape, the orders' lines on the
//! chart, open orders, an order history with every state, fills and
//! balances.
use trade::{History, LOT, MARKETS, Op, Side, UNIT};
use trade_stage::{book, walk};
use view::rig::{self, Network};

/// ETH/USD and DUCK/USD, by their places in [`MARKETS`].
const ETH: u8 = 1;
const DUCK: u8 = 2;

fn main() {
    rig::run(trade::PROGRAM, |net| {
        net.seat::<trade::Trade>(trade::PROGRAM);
        let histories = walk::histories();
        net.init(trade::PROGRAM, &histories)?;
        let (me, maker) = (net.me(), net.person("bo"));
        for who in [me, maker] {
            submit(net, who, &Op::Fund)?;
        }
        for op in book::rests(last(&histories, book::MARKET)) {
            submit(net, maker, &op)?;
        }
        for op in book::takes() {
            submit(net, me, &op)?;
        }
        // the reader's own: a bid and an ask a few percent off BTC/USD's
        // price, which rest on its chart, a bid on ETH/USD, and a bid on
        // DUCK/USD they take back
        let btc = last(&histories, book::MARKET);
        submit(
            net,
            me,
            &limit(book::MARKET, Side::Buy, btc * 98 / 100, 2_500),
        )?;
        submit(
            net,
            me,
            &limit(book::MARKET, Side::Sell, btc * 102 / 100, 1_000),
        )?;
        let eth = last(&histories, ETH);
        submit(net, me, &limit(ETH, Side::Buy, eth * 97 / 100, 20_000))?;
        let duck = last(&histories, DUCK);
        let taken_back = submit(net, me, &limit(DUCK, Side::Buy, duck * 9 / 10, 50_000_000))?;
        let order = borsh::from_slice(&taken_back)
            .map_err(|_| rig::Error::new("invalid", "the order's id is a borsh u64"))?;
        submit(net, me, &Op::Cancel { order }).map(drop)
    })
}

/// `op` to the program, signed by `who`: what it answered.
fn submit(net: &Network, who: u64, op: &Op) -> Result<Vec<u8>, rig::Error> {
    net.submit(who, trade::PROGRAM, op)
}

/// The price `market`'s history ends at.
fn last(histories: &[History], market: u8) -> u64 {
    histories
        .iter()
        .find(|history| history.market == market)
        .and_then(|history| history.bars.last())
        .map_or(UNIT, |bar| bar.close)
}

/// A limit order of `lots` on `market` at `price`, down to its tick.
fn limit(market: u8, side: Side, price: u64, lots: u64) -> Op {
    let tick = MARKETS[usize::from(market)].tick;
    Op::Place {
        market,
        side,
        amount: lots * LOT,
        limit: Some(price / tick * tick),
    }
}
