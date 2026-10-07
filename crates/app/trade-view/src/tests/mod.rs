//! The view against a fake host that answers as the program would
//! (`sample.rs`): each test drives the frame the way a person would. Every
//! frame it sends is held to the host's sanitizer and accessibility audit.
use ducktape_view_guest::methods::{Change, Changes, Query as Ask, Submit};
use ducktape_view_guest::prelude::Capability;
use ducktape_view_guest::testing::TestAppContext;
use ducktape_view_guest::{Entity, View};
use trade::{MAX_ROWS, Op, Query, UNIT, state};

use crate::{Column, Panel, Trade, format};
use sample::{READER, Shared, World};

mod chart;
mod panel;
pub(crate) mod sample;
mod ticket;

/// The two boards' panes.
const WIDE: (f32, f32) = (1280., 760.);
const PANE: (f32, f32) = (768., 500.);

type Opened = (TestAppContext, Entity<Trade>, Shared);

fn opened(world: World, size: (f32, f32), account: Option<u64>) -> Opened {
    sample::opened::<Trade>(world, size, account)
}

/// The boards' market in the wide layout, the reader seated.
fn wide() -> Opened {
    opened(sample::board(), WIDE, Some(READER))
}

fn asked(cx: &TestAppContext) -> Vec<Query> {
    cx.host().requests::<Ask<trade::Trade>>()
}

fn ops(cx: &TestAppContext) -> Vec<Op> {
    cx.host().requests::<Submit<trade::Trade>>()
}

/// How often the view has asked each read: the stats, the candles, the
/// book, the tape, the balances, the orders and the fills.
fn reads(cx: &TestAppContext) -> [usize; 7] {
    let mut reads = [0; 7];
    for query in asked(cx) {
        let read = match query {
            Query::Stats => 0,
            Query::Candles { .. } => 1,
            Query::Book { .. } => 2,
            Query::Trades { .. } => 3,
            Query::Balances { .. } => 4,
            Query::Orders { .. } => 5,
            Query::Fills { .. } => 6,
        };
        reads[read] += 1;
    }
    reads
}

/// Whether the control `key` hears a press.
fn pressable(cx: &TestAppContext, key: &str) -> bool {
    cx.interactivity(key).on_click.is_some()
}

#[track_caller]
fn shows(cx: &TestAppContext, texts: &[&str]) {
    for text in texts {
        assert!(cx.has_text(text), "no {text:?} in {:?}", cx.texts());
    }
}

#[test]
fn the_manifest_names_the_program_and_what_the_view_asks_of_the_host() {
    assert_eq!(<Trade as View>::NAME, "Trade");
    assert_eq!(<Trade as View>::TARGETS, [trade::MODULE]);
    assert_eq!(
        <Trade as View>::CAPABILITIES,
        [Capability::Module, Capability::Op, Capability::Host]
    );
    // narrower than the desk's 768 px pane, which the view is laid out in
    assert_eq!(<Trade as View>::MIN_WINDOW_WIDTH, 736);
}

#[test]
fn a_first_load_asks_every_read_and_draws_every_region() {
    let (cx, ..) = wide();
    let asked = asked(&cx);
    let newest = Query::Candles {
        market: 0,
        span: 3,
        before: None,
        limit: MAX_ROWS,
    };
    assert!(asked.contains(&Query::Stats));
    assert!(asked.contains(&newest), "{asked:?}");
    assert!(asked.contains(&Query::Book {
        market: 0,
        step: 1,
        depth: 15
    }));
    let has = |pick: fn(&Query) -> bool| asked.iter().any(pick);
    assert!(has(|query| matches!(
        query,
        Query::Trades { market: 0, .. }
    )));
    assert!(has(|query| matches!(
        query,
        Query::Balances { owner: READER }
    )));
    assert!(has(|query| matches!(
        query,
        Query::Orders {
            owner: READER,
            open: true,
            ..
        }
    )));
    assert!(has(|query| matches!(
        query,
        Query::Orders {
            owner: READER,
            open: false,
            ..
        }
    )));
    assert!(has(|query| matches!(
        query,
        Query::Fills { owner: READER, .. }
    )));
    // the bar
    shows(
        &cx,
        &["BTC/USD", "PRACTICE", "+1.00%", "24h high", "63,981.4"],
    );
    shows(&cx, &["24h low", "62,351.9", "24h volume", "5,278.88 BTC"]);
    // the chart: its tools, the last price's tag, an order on its line
    shows(&cx, &["1h", "MA 7", "MA 25", "Volume", "Candles", "Line"]);
    shows(&cx, &["Buy 0.2500", "62,635.0", "Sell 0.1000", "64,352.0"]);
    assert!(cx.find("trade-plot").is_some());
    // the book, best ask and best bid, and the tape
    shows(&cx, &["Order book", "Price (USD)", "Amount (BTC)", "Total"]);
    shows(&cx, &["63,589.7", "0.4504", "63,588.8", "0.1106", "Spread"]);
    shows(&cx, &["Trades", "Time", "0.0361", "14:02:41"]);
    // the ticket
    shows(&cx, &["Buy", "Sell", "Limit", "Market", "Available"]);
    shows(&cx, &["25,280.00 USD", "Buy BTC"]);
    // the panel: the open orders, counted
    shows(
        &cx,
        &["Open orders", "3", "Order history", "Fills", "Balances"],
    );
    shows(
        &cx,
        &["Cancel all", "14:02:10", "ETH/USD", "3,150.00", "35%"],
    );
}

/// A block reads again only what it wrote to, as the program declares it:
/// a resting order that moved is the book's alone. A reopened link reads
/// everything.
#[test]
fn a_block_that_wrote_the_book_asks_the_book_alone() {
    let (mut cx, ..) = wide();
    let changes = cx.host().stream::<Changes<trade::Trade>>();
    let before = reads(&cx);
    let rested = state::BOOK.key(&(0u8, 0u8, 5u64, 9u64));
    changes.send(Some(Change {
        height: 2,
        keys: vec![rested],
    }));
    cx.run_until_parked();
    let mut after = before;
    after[2] += 1;
    assert_eq!(reads(&cx), after);
    changes.send(None);
    cx.run_until_parked();
    let all = reads(&cx);
    assert!(
        all.iter().zip(after).all(|(all, after)| *all > after),
        "{all:?} after {after:?}"
    );
}

/// A block that changed nothing on screen: every read is asked again, the
/// same rows land, and the view draws nothing for them.
#[test]
fn a_block_whose_reads_land_the_same_rows_draws_nothing() {
    let (mut cx, ..) = wide();
    let changes = cx.host().stream::<Changes<trade::Trade>>();
    let (before, renders) = (reads(&cx), cx.renders());
    changes.send(None);
    cx.run_until_parked();
    assert_ne!(reads(&cx), before);
    assert_eq!(cx.renders(), renders);
}

#[test]
fn another_market_and_another_span_are_asked_for() {
    let (mut cx, view, _) = wide();
    cx.simulate_click("trade-market");
    cx.run_until_parked();
    shows(&cx, &["Search markets", "3,164.20", "-0.42%", "1.2648"]);
    shows(&cx, &["+3.10%"]);
    let before = reads(&cx);
    cx.simulate_click("market-ETH/USD");
    cx.run_until_parked();
    assert!(cx.find("trade-markets-list").is_none(), "the list closed");
    view.read(|trade| assert_eq!(trade.market, 1));
    let asked = asked(&cx);
    let has = |pick: fn(&Query) -> bool| asked.iter().any(pick);
    assert!(has(|query| matches!(
        query,
        Query::Candles { market: 1, .. }
    )));
    assert!(has(|query| matches!(query, Query::Book { market: 1, .. })));
    assert!(has(|query| matches!(
        query,
        Query::Trades { market: 1, .. }
    )));
    let mut after = before;
    for read in [1, 2, 3] {
        after[read] += 1;
    }
    assert_eq!(reads(&cx), after, "the market's three reads alone");
    // the bar turns to the market; nothing has traded on its chart
    shows(&cx, &["ETH/USD", "3,164.20", "No trades yet"]);
    shows(&cx, &["No orders in the book", "Buy ETH"]);
    cx.simulate_click("15m");
    cx.run_until_parked();
    let asked = self::asked(&cx);
    assert_eq!(
        asked.last(),
        Some(&Query::Candles {
            market: 1,
            span: 2,
            before: None,
            limit: MAX_ROWS
        })
    );
}

#[test]
fn the_markets_list_is_searched_and_enter_takes_the_first_found() {
    let (mut cx, view, _) = wide();
    cx.simulate_click("trade-market");
    cx.run_until_parked();
    let focused = cx.focused().and_then(|node| node.key());
    assert_eq!(
        focused,
        Some("trade-markets-search"),
        "the keys are the search's"
    );
    cx.simulate_input("trade-markets-search", "duck");
    cx.run_until_parked();
    assert!(cx.find("market-DUCK/USD").is_some());
    assert!(cx.find("market-ETH/USD").is_none());
    cx.simulate_input("trade-markets-search", "zzz");
    cx.run_until_parked();
    shows(&cx, &["No market by that name"]);
    cx.simulate_input("trade-markets-search", "duck");
    cx.simulate_submit("trade-markets-search");
    cx.run_until_parked();
    view.read(|trade| assert_eq!(trade.market, 2));
    // DUCK's tick is 0.0001: its prices take four places
    shows(&cx, &["DUCK/USD", "1.2648"]);
}

/// The layout is the view's own width: three columns from 1100 px, and
/// under that the pane's one tabbed column, its bar without the volume and
/// the chart's switches folded into a menu.
#[test]
fn the_layout_is_wide_from_1100_and_the_pane_under_it() {
    let (mut cx, view, _) = wide();
    let is_wide = |cx: &TestAppContext| {
        let wide = cx.find("trade-column-tabs").is_none();
        assert_eq!(cx.has_text("24h volume"), wide);
        assert_eq!(cx.find("trade-ma7").is_some(), wide);
        assert_eq!(cx.find("trade-indicators").is_none(), wide);
        assert_eq!(cx.has_text("Order history"), wide);
        wide
    };
    assert!(is_wide(&cx));
    cx.simulate_resize(1099., 760.);
    cx.run_until_parked();
    assert!(!is_wide(&cx));
    // the column shows one of the three at a time
    shows(&cx, &["Order", "Book", "History", "Available"]);
    assert!(cx.find("trade-book").is_none());
    cx.simulate_click("trade-tab-book");
    cx.run_until_parked();
    assert!(cx.find("trade-book").is_some());
    assert!(!cx.has_text("Available"));
    cx.simulate_click("trade-tab-trades");
    cx.run_until_parked();
    shows(&cx, &["14:02:41"]);
    view.read(|trade| assert_eq!(trade.column, Column::Trades));
    cx.simulate_resize(1100., 760.);
    cx.run_until_parked();
    assert!(is_wide(&cx));
}

#[test]
fn the_panes_menu_switches_what_the_chart_draws() {
    let (mut cx, view, _) = opened(sample::board(), PANE, Some(READER));
    cx.simulate_click("trade-indicators");
    cx.run_until_parked();
    shows(&cx, &["MA 7", "MA 25", "Volume", "Line"]);
    cx.simulate_click("trade-indicators-menu-0");
    cx.simulate_click("trade-indicators-menu-3");
    cx.run_until_parked();
    view.read(|trade| {
        let chart = &trade.chart;
        assert_eq!(
            (chart.ma7, chart.ma25, chart.volume, chart.line),
            (false, true, true, true)
        );
    });
    assert!(
        cx.find("trade-indicators-menu").is_some(),
        "a set of switches stays open"
    );
    cx.simulate_dismiss("trade-indicators-layer");
    cx.run_until_parked();
    assert!(cx.find("trade-indicators-menu").is_none());
}

/// What the reader set outlives a redeploy; the rows are read again.
#[test]
fn a_snapshot_keeps_the_market_the_span_the_zoom_and_the_switches() {
    let (mut cx, ..) = wide();
    chart::wheel(&mut cx, 1.);
    assert_eq!(chart::slots(&cx), 79, "one notch closer");
    cx.simulate_click("trade-ma25");
    cx.simulate_click("Line");
    cx.simulate_click("trade-balances");
    cx.simulate_click("trade-market");
    cx.simulate_click("market-ETH/USD");
    cx.simulate_click("4h");
    let bytes = cx.snapshot().expect("a snapshot");
    let before = asked(&cx).len();
    let view = cx.restore::<Trade>(&bytes).expect("restored");
    cx.run_until_parked();
    view.read(|trade| {
        let chart = &trade.chart;
        assert_eq!((trade.market, chart.span), (1, 4));
        assert_eq!(
            (chart.ma7, chart.ma25, chart.volume, chart.line),
            (true, false, true, true)
        );
        assert_eq!(trade.panel, Panel::Balances);
        assert!((chart.candle - 8. / 0.9).abs() < 0.01, "{}", chart.candle);
    });
    let again = &asked(&cx)[before..];
    assert!(again.contains(&Query::Stats), "{again:?}");
    assert!(again.iter().any(|query| matches!(
        query,
        Query::Candles {
            market: 1,
            span: 4,
            ..
        }
    )));
}

/// A reader the program refuses reads why where the rows would be.
#[test]
fn a_refused_read_says_why_in_its_place() {
    let mut cx = TestAppContext::new();
    cx.simulate_resize(WIDE.0, WIDE.1);
    cx.host()
        .refuse::<Ask<trade::Trade>>("unavailable", "the node is catching up");
    let sessions = cx
        .host()
        .stream::<ducktape_view_guest::methods::HostSession>();
    cx.open::<Trade>();
    sessions.send(sample::session(Some(READER)));
    cx.run_until_parked();
    let said = cx.texts();
    let said = said
        .iter()
        .filter(|text| *text == "the node is catching up");
    // the bar, the chart, the book, the tape and the open orders
    assert_eq!(said.count(), 5, "{:?}", cx.texts());
}

#[test]
fn a_market_nobody_traded_in_is_calm() {
    let (mut cx, ..) = opened(sample::empty(), WIDE, Some(READER));
    shows(
        &cx,
        &["No trades yet", "No orders in the book", "No open orders"],
    );
    shows(&cx, &["0.00 USD", "Buy BTC"]);
    assert!(!cx.has_text("Cancel all"));
    for (tab, nothing) in [
        ("trade-history", "No orders yet"),
        ("trade-fills", "No fills yet"),
        (
            "trade-balances",
            "No funds yet. Get practice funds to place an order.",
        ),
    ] {
        cx.simulate_click(tab);
        cx.run_until_parked();
        shows(&cx, &[nothing]);
    }
}

#[test]
fn a_number_is_written_with_its_steps_decimals_and_grouped() {
    let [btc, eth, duck] = trade::MARKETS.map(|market| format::decimals(market.tick));
    assert_eq!((btc, eth, duck), (1, 2, 4));
    assert_eq!(format::decimals(100 * UNIT), 0);
    assert_eq!(format::decimals(1), 8);
    assert_eq!(format::fixed(6_358_910_000_000, btc), "63,589.1");
    assert_eq!(format::fixed(316_420_000_000, eth), "3,164.20");
    assert_eq!(format::fixed(126_480_000, duck), "1.2648");
    assert_eq!(format::fixed(1_234_567 * UNIT, 0), "1,234,567");
    // cut, not rounded: a balance never reads as more than it is
    assert_eq!(format::amount(UNIT / 4 + 9_999), "0.2500");
    assert_eq!(format::quote(2_528_000_000_000), "25,280.00");
    assert_eq!(format::balance(0, 2_528_000_000_000), "25,280.00");
    assert_eq!(format::balance(1, 65_000_000), "0.6500");
    // a sum an order comes to, to the nearest cent
    assert_eq!(format::total(2_066_633_856_000), "20,666.34");
    assert_eq!(format::volume(232 * UNIT), "232");
    assert_eq!(format::volume(UNIT / 2), "0.5000");
}

#[test]
fn a_typed_number_is_read_back_exactly_or_not_at_all() {
    assert_eq!(format::typed("63,200.0"), Some(63_200 * UNIT));
    assert_eq!(format::typed(" 0.1 "), Some(UNIT / 10));
    assert_eq!(format::typed(".5"), Some(UNIT / 2));
    assert_eq!(format::typed("0.00000001"), Some(1));
    assert_eq!(format::typed("0.000000001"), None, "finer than a count");
    assert_eq!(format::typed("1e3"), None);
    assert_eq!(format::typed("-1"), None);
    assert_eq!(format::typed(""), None);
    assert_eq!(format::typed("."), None);
    assert_eq!(format::typed("999999999999999"), None, "more than fits");
    assert_eq!(format::cost(63_200 * UNIT, UNIT / 2), 31_600 * UNIT);
    assert_eq!(format::cost(u64::MAX, u64::MAX), u64::MAX);
}

#[test]
fn a_change_and_a_time_are_written_as_the_boards_write_them() {
    assert_eq!(
        format::change(635_891, 629_595)
            .map(format::percent)
            .unwrap(),
        "+1.00%"
    );
    assert_eq!(
        format::change(316_420, 317_755)
            .map(format::percent)
            .unwrap(),
        "-0.42%"
    );
    assert_eq!(format::change(5, 0), None, "no price a day before");
    assert_eq!(format::percent(0), "+0.00%");
    // 2026-10-07 14:02:41 UTC
    let at = sample::NOW + 161_000;
    assert_eq!(format::time(at), "14:02:41");
    assert_eq!(format::day(at), "Oct 7");
    assert_eq!(format::day_time(at), "Oct 7 14:02");
    assert_eq!(format::axis(sample::NOW), "14:00");
    assert_eq!(format::axis(sample::NOW - 14 * 3_600_000), "Oct 7");
    // a leap day, and the epoch
    assert_eq!(format::day_time(1_709_164_800_000), "Feb 29 00:00");
    assert_eq!(format::day_time(0), "Jan 1 00:00");
}
