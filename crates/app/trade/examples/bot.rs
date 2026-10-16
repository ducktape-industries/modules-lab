//! The market maker: a process off the chain that keeps every market's book
//! two-sided and its price moving, at a load a live network carries. It
//! trades as one wallet, through the node's own binary: `NODE submit
//! --preconfirm trade` with an op's borsh bytes, `NODE query trade` with a
//! query's.
//!
//! `cargo run -p trade --example bot -- NODE HOME WALLET` runs until it is
//! stopped. HOME is the node's workspace (`DUCKTAPE_HOME`, the wallet's key
//! in `HOME/keys/`), the wallet's password is `DUCKTAPE_PASSWORD`. `make bot`
//! runs it as `bo` on the stage `make live` made; on a network, a service
//! runs it against a resident node's workspace.
//!
//! Each market has a fair price, moved once a round ([`ROUND`] apart) by a
//! [`Regime`]: calm, a trend up or down, or a storm, each lasting minutes,
//! with a jump now and then. A pull towards an anchor that itself drifts
//! slowly keeps a day from running off. A round:
//!
//! - reads the wallet's open orders, its funds and every market's last
//!   price (`Query::Orders`, `Balances`, `Stats`: the confirmed state, which
//!   holds the round before, [`ROUND`] ago);
//! - tops the funds up (`Op::Fund`) when an asset has under half its grant;
//! - moves every fair price;
//! - cancels each open order that is not on the round's ladder ([`LEVELS`]
//!   prices a side on the market's [`grid`], about the fair price), then
//!   places the ladder's prices no order holds. A fair price that stays
//!   between two lines of the grid moves nothing;
//! - takes once in each market at the market, towards the fair price: a
//!   buy when the last trade is under it. The program allows a self-trade,
//!   so most takers trade against the bot's own ladder, which is how the
//!   tape follows the fair price.
//!
//! It starts from a clean book (`Op::CancelAll`). Then a round waits out
//! its time. That is about three takers and a few
//! refills every 20 s, each a frame of some 30 bytes: under one frame a
//! second for all three markets. An op the program refuses is printed and
//! the round goes on; a node that cannot be reached stops it.

use std::{
    collections::BTreeSet,
    env,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use borsh::BorshSerialize;
use store::PageRequest;
use trade::{ASSETS, LOT, MARKETS, Op, Order, Query, Reply, Side, UNIT};

/// How long a round takes, its wait included.
const ROUND: Duration = Duration::from_secs(20);
/// The prices a side rests at.
const LEVELS: u64 = 3;
/// The step of a market's grid, in millionths of its first fair price, at
/// least one tick: the spread, and the gap between two prices of a side.
const STEP: u64 = 1_000;
/// What one unit of a market's amounts is, in lots, by its place in
/// [`MARKETS`]: 0.03 BTC, 0.4 ETH, 400 DUCK. A ladder's `k`th price rests
/// `k + 3` of them, twelve a side, and a taker takes one: sized to what
/// `Op::Fund` grants (1 BTC, 10 ETH, 10,000 DUCK, 50,000 USD), the three
/// markets' bids hold some 36,000 USD at today's prices and each side's asks
/// under half of its asset.
const UNITS: [u64; MARKETS.len()] = [300, 4_000, 4_000_000];

/// splitmix64.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1].
    fn unit(&mut self) -> f64 {
        ((self.next() >> 11) + 1) as f64 / (1u64 << 53) as f64
    }

    /// A standard normal draw (Box-Muller).
    fn normal(&mut self) -> f64 {
        let (a, b) = (self.unit(), self.unit());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

/// What moves a fair price for a while: per round, a drift and a spread of
/// its log, and how many rounds it lasts at the least and at the most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Regime {
    Calm,
    Up,
    Down,
    Storm,
}

impl Regime {
    fn drift(self) -> f64 {
        match self {
            Regime::Up => 0.0008,
            Regime::Down => -0.0008,
            Regime::Calm | Regime::Storm => 0.0,
        }
    }

    fn sigma(self) -> f64 {
        match self {
            Regime::Calm => 0.0008,
            Regime::Up | Regime::Down => 0.0012,
            Regime::Storm => 0.004,
        }
    }

    /// The next regime: calm two times in five, a storm one in five, and a
    /// trend the rest, up less often the further `above` the anchor the
    /// price is (a log), down likewise below it.
    fn next(above: f64, draws: &mut Draws) -> Regime {
        match draws.next() % 5 {
            0 | 1 => Regime::Calm,
            2 => Regime::Storm,
            _ if draws.unit() < 0.5 - (above * 5.0).clamp(-0.4, 0.4) => Regime::Up,
            _ => Regime::Down,
        }
    }
}

/// The pull towards the anchor, per round: half-way back in about 230.
const PULL: f64 = 0.003;
/// How far the anchor drifts per round (a log's spread).
const ANCHOR: f64 = 0.0005;
/// How far a price may stray from its anchor, as a log: 0.67 to 1.5 times.
const STRAY: f64 = 0.405;
/// A jump's odds per round, and its size at the least and at the most.
const JUMP: f64 = 0.004;
const JUMP_SIZE: (f64, f64) = (0.02, 0.06);

/// A market's fair price, as the log of its price in parts of [`UNIT`].
struct Fair {
    log: f64,
    anchor: f64,
    regime: Regime,
    left: u64,
}

impl Fair {
    fn new(price: u64) -> Fair {
        let log = (price as f64).ln();
        Fair {
            log,
            anchor: log,
            regime: Regime::Calm,
            left: 0,
        }
    }

    /// One round's move.
    fn step(&mut self, draws: &mut Draws) {
        if self.left == 0 {
            self.regime = Regime::next(self.log - self.anchor, draws);
            self.left = 10 + draws.next() % 51;
        }
        self.left -= 1;
        self.anchor += ANCHOR * draws.normal();
        let mut moved = self.regime.drift() + self.regime.sigma() * draws.normal()
            - PULL * (self.log - self.anchor);
        if draws.unit() < JUMP {
            let size = JUMP_SIZE.0 + (JUMP_SIZE.1 - JUMP_SIZE.0) * draws.unit();
            moved += if draws.next().is_multiple_of(2) {
                size
            } else {
                -size
            };
        }
        self.log = (self.log + moved).clamp(self.anchor - STRAY, self.anchor + STRAY);
    }

    fn price(&self) -> u64 {
        self.log.exp() as u64
    }
}

/// The step of `market`'s ladder about a first fair price of `price`: on
/// the market's tick, at least one. It is fixed for the bot's run, so a
/// ladder whose fair price stays between two of its lines stays put.
fn grid(market: u8, price: u64) -> u64 {
    let tick = MARKETS[usize::from(market)].tick;
    (price / 1_000_000 * STEP / tick).max(1) * tick
}

/// The ladder about `fair` on a grid of `step`: [`LEVELS`] (side, price,
/// amount) a side, best first, the bids from the grid line at or under
/// `fair` and the asks from the line above it.
fn ladder(market: u8, fair: u64, step: u64) -> Vec<(Side, u64, u64)> {
    let unit = UNITS[usize::from(market)] * LOT;
    let bid = fair / step * step;
    let mut rungs = Vec::new();
    for k in 0..LEVELS {
        let away = k * step;
        if bid > away {
            rungs.push((Side::Buy, bid - away, (k + 3) * unit));
        }
        rungs.push((Side::Sell, bid + step + away, (k + 3) * unit));
    }
    rungs
}

/// One market's ops of a round, in the order they go: a cancel for each
/// of the wallet's `open` orders on it that is off the ladder about `fair`
/// (or a second at one price, or one takers left half or less of), a place
/// for each price of the ladder no open order holds, and then the taker, a
/// buy when `last` is under `fair` and a sell otherwise, of `units`. A
/// taker of one unit leaves the best price two thirds of its three, so the
/// best price is never taken whole: it is put back whole every second
/// round.
fn plan(market: u8, fair: u64, step: u64, open: &[Order], last: u64, units: u64) -> Vec<Op> {
    let rungs = ladder(market, fair, step);
    let mut held = BTreeSet::new();
    let mut ops = Vec::new();
    for order in open.iter().filter(|order| order.market == market) {
        let on = |(side, price, amount): &&(Side, u64, u64)| {
            *side == order.side
                && Some(*price) == order.limit
                && 2 * (order.amount - order.filled) > *amount
        };
        match rungs.iter().find(on) {
            Some(&(side, price, _)) if held.insert((side == Side::Buy, price)) => {}
            _ => ops.push(Op::Cancel { order: order.id }),
        }
    }
    for &(side, price, amount) in &rungs {
        if !held.contains(&(side == Side::Buy, price)) {
            ops.push(Op::Place {
                market,
                side,
                amount,
                limit: Some(price),
            });
        }
    }
    ops.push(Op::Place {
        market,
        side: if last < fair { Side::Buy } else { Side::Sell },
        amount: units * UNITS[usize::from(market)] * LOT,
        limit: None,
    });
    ops
}

/// `text` as hex, two digits a byte.
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// The node's binary as one wallet of one workspace.
struct Node {
    binary: String,
    home: PathBuf,
    wallet: String,
}

impl Node {
    /// `ARGS` with `input` on stdin: what it printed. A node that cannot be
    /// reached, or a request it cannot read, stops the bot.
    fn run(&self, args: &[&str], input: &[u8]) -> Vec<u8> {
        let key = self.home.join("keys").join(format!("{}.key", self.wallet));
        let mut child = Command::new(&self.binary)
            .args(args)
            .env("DUCKTAPE_HOME", &self.home)
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

    /// Sends `op`: `Err` with the node's words when the program refuses it.
    /// The node runs it only when asked to preconfirm, and an applied op's
    /// first line then says so (`preconfirmed applied <hex>`); without it
    /// every op answers `pooled` and reads as refused.
    fn submit(&self, op: &Op) -> Result<(), String> {
        let payload = borsh::to_vec(op).expect("an op encodes");
        let args = ["submit", "--preconfirm", trade::PROGRAM];
        let said = String::from_utf8(self.run(&args, &payload)).expect("the node's answer is text");
        let first = said.lines().next().unwrap_or_default();
        match first.split_whitespace().any(|word| word == "applied") {
            true => Ok(()),
            false => Err(first.to_owned()),
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

    fn stats(&self) -> Vec<trade::Stats> {
        match self.query(trade::PROGRAM, &Query::Stats) {
            Reply::Stats(stats) => stats,
            reply => panic!("Stats answered {reply:?}"),
        }
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let (Some(binary), Some(home), Some(wallet)) = (args.next(), args.next(), args.next()) else {
        panic!("usage: bot NODE HOME WALLET, with the wallet's password in DUCKTAPE_PASSWORD");
    };
    let node = Node {
        binary,
        home: PathBuf::from(home),
        wallet,
    };
    let owner = node.account();
    let firsts: Vec<u64> = node.stats().iter().map(|stats| stats.last).collect();
    assert!(
        firsts.iter().all(|price| *price > 0),
        "every market needs a price to start from: a network founded with trade's history has one"
    );
    let steps: Vec<u64> = (0u8..).zip(&firsts).map(|(m, p)| grid(m, *p)).collect();
    let mut fairs: Vec<Fair> = firsts.iter().map(|price| Fair::new(*price)).collect();
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970");
    let mut draws = Draws(seed.as_nanos() as u64);
    let shown = |price: u64| price as f64 / UNIT as f64;
    println!("bot: wallet {} (account {owner})", node.wallet);
    // a clean book to start from: an order the bot left at another size
    // (an older bot's) would otherwise stay while it holds half a price
    if let Err(refusal) = node.submit(&Op::CancelAll) {
        println!("bot: CancelAll {refusal}");
    }
    for n in 1.. {
        let started = Instant::now();
        let (mut sent, mut refused) = (0, 0);
        let mut send = |op: &Op| {
            sent += 1;
            if let Err(refusal) = node.submit(op) {
                refused += 1;
                println!("bot: {op:?} {refusal}");
            }
        };
        let Reply::Balances(balances) = node.query(trade::PROGRAM, &Query::Balances { owner })
        else {
            panic!("Balances answers with the balances");
        };
        if balances.iter().any(|balance| {
            balance.free + balance.held < ASSETS[usize::from(balance.asset)].grant / 2
        }) {
            send(&Op::Fund);
        }
        let page = PageRequest::default();
        let open = match node.query(
            trade::PROGRAM,
            &Query::Orders {
                owner,
                open: true,
                page,
            },
        ) {
            Reply::Orders(open) => open.items,
            reply => panic!("Orders answered {reply:?}"),
        };
        let lasts = node.stats();
        for (market, fair) in (0u8..).zip(fairs.iter_mut()) {
            fair.step(&mut draws);
            let units = 1;
            let at = usize::from(market);
            let ops = plan(
                market,
                fair.price(),
                steps[at],
                &open,
                lasts[at].last,
                units,
            );
            ops.iter().for_each(&mut send);
        }
        let prices: Vec<String> = MARKETS
            .iter()
            .zip(&fairs)
            .map(|(market, fair)| {
                format!(
                    "{} {} ({:?})",
                    market.name,
                    shown(fair.price()),
                    fair.regime
                )
            })
            .collect();
        println!(
            "bot: round {n}: {sent} ops ({refused} refused) in {:.1} s; {}",
            started.elapsed().as_secs_f64(),
            prices.join(", ")
        );
        thread::sleep(ROUND.saturating_sub(started.elapsed()));
    }
}

#[cfg(test)]
mod tests {
    use trade::OrderState;

    use super::*;

    /// What the ladder ops among `ops` rest, as open orders of ids from
    /// `first` on, beside the `kept` ones.
    fn rest(kept: &[Order], ops: &[Op], first: u64) -> Vec<Order> {
        let cancelled: BTreeSet<u64> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Cancel { order } => Some(*order),
                _ => None,
            })
            .collect();
        let mut open: Vec<Order> = kept
            .iter()
            .filter(|order| !cancelled.contains(&order.id))
            .cloned()
            .collect();
        let placed = ops.iter().filter_map(|op| match *op {
            Op::Place {
                market,
                side,
                amount,
                limit: Some(limit),
            } => Some((market, side, amount, limit)),
            _ => None,
        });
        for (id, (market, side, amount, limit)) in (first..).zip(placed) {
            open.push(Order {
                id,
                owner: 9,
                market,
                side,
                limit: Some(limit),
                amount,
                filled: 0,
                quote: 0,
                placed: 0,
                state: OrderState::Open,
            });
        }
        open
    }

    fn count(ops: &[Op], pick: fn(&Op) -> bool) -> usize {
        ops.iter().filter(|op| pick(op)).count()
    }

    fn is_cancel(op: &Op) -> bool {
        matches!(op, Op::Cancel { .. })
    }

    fn is_rest(op: &Op) -> bool {
        matches!(op, Op::Place { limit: Some(_), .. })
    }

    /// The headline of the load: on an empty book a round rests the whole
    /// ladder and takes once; a fair price that stays between two grid
    /// lines then sends the taker alone, and one that crosses a line moves
    /// one price a side: a cancel and a place each.
    #[test]
    fn a_round_sends_only_what_the_fair_price_moved() {
        let step = grid(0, 66_050 * UNIT);
        // a quarter of a step above a grid line
        let price = 66_050 * UNIT / step * step + step / 4;
        let first = plan(0, price, step, &[], price, 1);
        assert_eq!(count(&first, is_rest), 2 * LEVELS as usize);
        assert_eq!(
            (count(&first, is_cancel), first.len()),
            (0, 2 * LEVELS as usize + 1)
        );
        let open = rest(&[], &first, 1);
        let still = plan(0, price + step / 3, step, &open, price, 1);
        assert_eq!(still.len(), 1, "the taker alone: {still:?}");
        let crossed = plan(0, price + step, step, &open, price, 1);
        assert_eq!(count(&crossed, is_cancel), 2, "{crossed:?}");
        assert_eq!(count(&crossed, is_rest), 2, "{crossed:?}");
        // and the book it leaves is that fair price's ladder, whole
        let after = rest(&open, &crossed, 100);
        assert_eq!(plan(0, price + step, step, &after, price, 1).len(), 1);
    }

    /// A taker's unit leaves the best price two thirds, which stays; a
    /// second leaves a third, and the round puts it back whole.
    #[test]
    fn a_best_price_takers_drained_is_put_back_whole() {
        let step = grid(0, 66_050 * UNIT);
        let price = 66_050 * UNIT / step * step + step / 4;
        let mut open = rest(&[], &plan(0, price, step, &[], price, 1), 1);
        let best = |open: &[Order]| {
            let bids = open.iter().filter(|order| order.side == Side::Buy);
            bids.max_by_key(|order| order.limit).map(|order| order.id)
        };
        let id = best(&open).expect("a bid");
        let unit = UNITS[0] * LOT;
        let at = open.iter().position(|order| order.id == id).expect("held");
        open[at].filled = unit;
        assert_eq!(
            plan(0, price, step, &open, price, 1).len(),
            1,
            "two thirds stay"
        );
        open[at].filled = 2 * unit;
        let ops = plan(0, price, step, &open, price, 1);
        assert!(
            matches!(ops[0], Op::Cancel { order } if order == id),
            "{ops:?}"
        );
        assert!(
            matches!(ops[1], Op::Place { amount, .. } if amount == 3 * unit),
            "{ops:?}"
        );
        assert_eq!(ops.len(), 3);
    }

    /// Every ladder is on the market's tick and grid, best first, the bids
    /// under the asks with one grid step between the best two, and amounts
    /// in whole lots; DUCK's grid is two ticks at its 0.25.
    #[test]
    fn a_ladder_never_crosses_and_sits_on_the_grid() {
        for (market, price) in [(0u8, 66_050 * UNIT), (1, 2_400 * UNIT), (2, UNIT / 4)] {
            let step = grid(market, price);
            let tick = MARKETS[usize::from(market)].tick;
            assert_eq!(step % tick, 0);
            let rungs = ladder(market, price + step / 2, step);
            let side = |want: Side| -> Vec<u64> {
                let of = rungs.iter().filter(|(side, ..)| *side == want);
                of.map(|(_, price, _)| *price).collect()
            };
            let (bids, asks) = (side(Side::Buy), side(Side::Sell));
            assert_eq!((bids.len(), asks.len()), (LEVELS as usize, LEVELS as usize));
            assert!(bids.windows(2).all(|two| two[0] > two[1]));
            assert!(asks.windows(2).all(|two| two[0] < two[1]));
            assert_eq!(asks[0] - bids[0], step);
            assert!(
                rungs
                    .iter()
                    .all(|(_, p, amount)| p % step == 0 && amount % LOT == 0)
            );
        }
        assert_eq!(grid(2, UNIT / 4), 2 * MARKETS[2].tick);
    }

    /// A second order at one price, or one off the ladder, is cancelled;
    /// the taker goes towards the fair price.
    #[test]
    fn a_duplicate_is_cancelled_and_the_taker_leans_to_the_fair_price() {
        let price = 2_400 * UNIT;
        let step = grid(1, price);
        let open = rest(&[], &plan(1, price, step, &[], price, 1), 1);
        let mut twice = open.clone();
        twice.push(Order { id: 50, ..open[0] });
        let ops = plan(1, price, step, &twice, price - step, 2);
        assert_eq!(ops.len(), 2, "{ops:?}");
        assert!(matches!(ops[0], Op::Cancel { order: 50 }));
        let taker = |ops: &[Op]| match ops.last() {
            Some(&Op::Place {
                side,
                amount,
                limit: None,
                ..
            }) => (side, amount),
            op => panic!("{op:?} is no taker"),
        };
        assert_eq!(taker(&ops), (Side::Buy, 2 * UNITS[1] * LOT));
        assert_eq!(
            taker(&plan(1, price, step, &open, price + step, 1)).0,
            Side::Sell
        );
    }

    /// A day of rounds (4,320 at 20 s) from many seeds stays within the
    /// stray of its start and its anchor, and moves: a day sees an hour
    /// that swings more than 2%.
    #[test]
    fn a_day_stays_in_its_band_and_swings() {
        let rounds_an_hour = 180;
        for seed in 0..40 {
            let draws = &mut Draws(seed);
            let start = 66_050 * UNIT;
            let mut fair = Fair::new(start);
            let mut prices = Vec::new();
            for _ in 0..24 * rounds_an_hour {
                fair.step(draws);
                prices.push(fair.price() as f64);
            }
            let start = start as f64;
            assert!(
                prices.iter().all(|p| *p > start * 0.4 && *p < start * 2.5),
                "seed {seed} ran off"
            );
            let swing = prices
                .windows(rounds_an_hour)
                .map(|hour| {
                    let low = hour.iter().cloned().fold(f64::MAX, f64::min);
                    let high = hour.iter().cloned().fold(0.0, f64::max);
                    high / low - 1.0
                })
                .fold(0.0, f64::max);
            assert!(swing > 0.02, "seed {seed}: its widest hour swung {swing}");
        }
    }
}
