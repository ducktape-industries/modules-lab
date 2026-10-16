//! The phone's design (`ui/phone/`): under a finger the view draws the
//! phone's tree, picked once at the root. Each test drives it as a finger
//! would: a tap, a pan, a pinch, the phone's Back.
use gpui::{PinchEvent, ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};
use view::methods::{HostSession, Session};
use view::testing::sweep;
use view::wire;

use super::*;
use crate::{OrderList, Route, Segment, Sheet};

/// The common phone, as the sweep lays it out.
const PHONE: (f32, f32) = (390., 844.);
/// Where the host lays the market page's chart out in the window: under
/// the title row, the price head and the tools, the phone's width across.
const CHART_AT: (f32, f32) = (0., 216.);
const CHART: (f32, f32) = (390., 240.);

/// `world` under a finger on the markets, `account` seated.
fn finger(world: World, account: Option<u64>) -> Opened {
    let (mut cx, view, world) = opened(world, PHONE, account);
    cx.simulate_touch(true);
    cx.run_until_parked();
    (cx, view, world)
}

/// The boards' markets under a finger, the reader seated.
fn phone() -> Opened {
    finger(sample::board(), Some(READER))
}

/// The market page of BTC/USD, its chart measured.
fn btc(cx: &mut TestAppContext) {
    cx.simulate_click("market-BTC/USD");
    cx.run_until_parked();
    cx.simulate_bounds("trade-chart", CHART_AT, CHART);
    cx.run_until_parked();
}

/// The session said again, with or without a link.
fn linked(cx: &mut TestAppContext, connected: bool) {
    cx.host().stream::<HostSession>().send(Session {
        connected,
        ..sample::session(Some(READER))
    });
    cx.run_until_parked();
}

/// A finger lifted at once from the plot, `at` in the chart's pixels.
fn tap_plot(cx: &mut TestAppContext, at: (f32, f32)) {
    let handler = cx
        .interactivity("trade-plot")
        .on_click
        .expect("a tap route");
    cx.tick(vec![wire::Event::Click {
        handler,
        event: wire::click::Click::Touch {
            position: point(px(CHART_AT.0 + at.0), px(CHART_AT.1 + at.1)),
            tap_count: 1,
            long_press: false,
        },
    }]);
}

/// Where the line on the held candle is drawn, from the plot's left
/// edge: `None` while no candle is held.
fn held_line(cx: &TestAppContext) -> Option<gpui::Length> {
    let line = &cx.node("trade-plot").children()[1];
    let style = &cx.styles()[line.style().expect("the line's style")];
    style.background.as_ref().and(style.inset.left)
}

/// Whether the control `key` is said to be waiting on its answer.
fn busy(cx: &TestAppContext, key: &str) -> bool {
    cx.interactivity(key).aria.busy
}

#[test]
fn a_finger_gets_the_phones_tree_and_a_pointer_the_desktops() {
    let (mut cx, ..) = phone();
    assert!(cx.find("trade-markets").is_some());
    assert!(cx.find("trade-panel-tabs").is_none(), "no desktop panel");
    shows(
        &cx,
        &[
            "Trade",
            "BTC/USD",
            "63,589.1",
            "+1.00%",
            "Practice \u{b7} spot",
        ],
    );
    cx.simulate_touch(false);
    cx.simulate_resize(WIDE.0, WIDE.1);
    cx.run_until_parked();
    assert!(cx.find("trade-panel-tabs").is_some());
    assert!(cx.find("trade-markets").is_none());
}

#[test]
fn a_market_row_opens_its_page_and_reads_its_book_tape_and_candles() {
    let (mut cx, view, _) = phone();
    cx.simulate_click("market-ETH/USD");
    cx.run_until_parked();
    view.read(|trade| {
        assert_eq!(trade.pages.top(), &Route::Market);
        assert_eq!(trade.market, 1);
    });
    let asked = asked(&cx);
    assert!(asked.contains(&Query::Book {
        market: 1,
        step: 1,
        depth: 15
    }));
    assert!(
        asked
            .iter()
            .any(|query| matches!(query, Query::Trades { market: 1, .. }))
    );
    assert!(
        asked
            .iter()
            .any(|query| matches!(query, Query::Candles { market: 1, .. }))
    );
    shows(
        &cx,
        &["ETH/USD", "3,164.20", "24h high", "3,198.00", "Book"],
    );
    assert!(
        cx.find("trade-market-back").is_some(),
        "a Back over the markets"
    );
}

#[test]
fn the_markets_tell_the_readers_orders_and_balances_and_open_their_pages() {
    let (mut cx, view, _) = phone();
    shows(
        &cx,
        &[
            "Your account",
            "3 resting \u{b7} Cancel all from the list",
            "25,280.00 USD \u{b7} 0.65 BTC \u{b7} 0.7 ETH \u{b7} 0 DUCK",
        ],
    );
    cx.simulate_click("trade-open-orders");
    view.read(|trade| assert_eq!(trade.pages.top(), &Route::Orders));
    shows(
        &cx,
        &[
            "Orders",
            "Every market",
            "Open orders \u{b7} 3",
            "Cancel all",
        ],
    );
    shows(&cx, &["Limit 62,635.0 \u{b7} 0% filled \u{b7} 14:02:10"]);
    cx.simulate_click("trade-list-history");
    view.read(|trade| assert_eq!(trade.order_list, OrderList::History));
    shows(
        &cx,
        &[
            "Order history",
            "Limit 63,670.0 \u{b7} Filled \u{b7} avg 63,670.0 \u{b7} Oct 7 09:30",
        ],
    );
    cx.simulate_click("trade-list-fills");
    shows(
        &cx,
        &["63,670.0 \u{b7} Maker \u{b7} 6,367.00 USD \u{b7} Oct 7 09:41"],
    );
    cx.simulate_back();
    cx.simulate_click("trade-balances");
    shows(
        &cx,
        &[
            "Assets",
            "In orders 19,753.75",
            "25,280.00",
            "available",
            "Get practice funds",
        ],
    );
    shows(
        &cx,
        &[
            "Tops each asset up to its grant: 50,000 USD, 1 BTC, 10 ETH, 10,000 DUCK. \
                  Never lowers a balance.",
        ],
    );
    cx.simulate_click("trade-fund");
    assert_eq!(ops(&cx), [Op::Fund]);
}

#[test]
fn an_account_with_nothing_is_told_how_to_fill_it() {
    let (mut cx, ..) = finger(sample::empty(), Some(READER));
    cx.simulate_click("trade-balances");
    assert!(cx.find("trade-no-funds").is_some());
    shows(
        &cx,
        &["No funds yet", "Get practice funds to place an order."],
    );
    cx.simulate_click("trade-fund");
    assert_eq!(ops(&cx), [Op::Fund]);
}

#[test]
fn buy_and_sell_open_the_ticket_on_their_side_and_a_level_at_its_price() {
    let (mut cx, view, _) = phone();
    btc(&mut cx);
    cx.simulate_click("trade-buy");
    assert!(cx.find("trade-ticket").is_some());
    shows(
        &cx,
        &[
            "Buy BTC",
            "BTC/USD \u{b7} last 63,589.1 \u{b7} practice funds",
        ],
    );
    cx.simulate_back();
    assert!(cx.find("trade-ticket").is_none());
    cx.simulate_click("trade-sell");
    shows(&cx, &["Sell BTC"]);
    cx.simulate_back();
    // an ask is bought, a bid sold, each at its price
    cx.simulate_click("book-ask-0");
    view.read(|trade| {
        assert_eq!(trade.sheet, Some(Sheet::Ticket));
        assert!(!trade.ticket.sell);
        assert_eq!(trade.ticket.price.text(), "63,589.7");
    });
    cx.simulate_back();
    cx.simulate_click("book-bid-0");
    view.read(|trade| {
        assert!(trade.ticket.sell);
        assert_eq!(trade.ticket.price.text(), "63,588.8");
    });
}

#[test]
fn the_ticket_says_what_stands_in_the_orders_way() {
    let (mut cx, ..) = phone();
    btc(&mut cx);
    cx.simulate_click("trade-buy");
    cx.simulate_input("trade-price", "63,200.0");
    cx.simulate_input("trade-amount", "0.00001");
    shows(&cx, &["An amount is a number in steps of 0.0001 BTC."]);
    cx.simulate_input("trade-amount", "0.5");
    shows(
        &cx,
        &["This order needs 31,600.00 USD. 25,280.00 USD is available."],
    );
    assert!(!pressable(&cx, "trade-submit"));
    assert_eq!(cx.interactivity("trade-submit").aria.disabled, Some(true));
    cx.simulate_submit("trade-amount");
    assert_eq!(ops(&cx), []);
    cx.simulate_input("trade-amount", "0.1");
    shows(
        &cx,
        &["Available", "25,280.00 USD", "6,320.00", "Buy 0.1000 BTC"],
    );
}

/// The ticket's sheet on BTC/USD, a limit buy of 0.1 at 63,200.0 typed.
fn typed_buy(cx: &mut TestAppContext) {
    btc(cx);
    cx.simulate_click("trade-buy");
    cx.simulate_input("trade-price", "63,200.0");
    cx.simulate_input("trade-amount", "0.1");
}

#[test]
fn placing_waits_on_its_answer() {
    let (mut cx, ..) = phone();
    typed_buy(&mut cx);
    cx.host().never::<Submit<trade::Trade>>();
    cx.simulate_click("trade-submit");
    shows(&cx, &["Sending\u{2026}"]);
    assert!(busy(&cx, "trade-submit") && !pressable(&cx, "trade-submit"));
    cx.simulate_submit("trade-amount");
    assert_eq!(ops(&cx).len(), 1, "one order, however often it is pressed");
}

#[test]
fn a_placed_order_closes_the_sheet_on_the_readers_orders() {
    let (mut cx, view, _) = phone();
    typed_buy(&mut cx);
    cx.simulate_click("trade-submit");
    cx.run_until_parked();
    assert_eq!(ops(&cx).len(), 1);
    assert!(cx.find("trade-ticket").is_none());
    view.read(|trade| assert_eq!(trade.segment, Segment::Orders));
    shows(&cx, &["Open orders \u{b7} 3"]);
}

#[test]
fn a_refused_order_is_said_in_the_sheet() {
    let (mut cx, _, world) = phone();
    let refusal = "the market is closed for the day";
    world.borrow_mut().refusal = Some(refusal.into());
    typed_buy(&mut cx);
    cx.simulate_click("trade-submit");
    cx.run_until_parked();
    assert!(cx.find("trade-ticket").is_some());
    shows(&cx, &[refusal]);
    assert!(
        pressable(&cx, "trade-submit"),
        "the order may be sent again"
    );
}

#[test]
fn a_cancel_waits_on_its_answer_in_its_row() {
    let (mut cx, ..) = phone();
    btc(&mut cx);
    cx.simulate_click("trade-segment-orders");
    shows(&cx, &["Open orders \u{b7} 3", "Cancel all"]);
    cx.host().never::<Submit<trade::Trade>>();
    cx.simulate_click("cancel-6");
    assert_eq!(ops(&cx), [Op::Cancel { order: 6 }]);
    shows(&cx, &["Cancelling\u{2026}"]);
    assert!(cx.find("cancel-6").is_none(), "no second cancel");
    assert!(pressable(&cx, "cancel-5"), "the other rows' still go");
}

#[test]
fn a_pan_across_moves_the_chart_and_a_pinch_zooms_it() {
    let (mut cx, view, _) = phone();
    btc(&mut cx);
    let (back, candle) = view.read(|trade| (trade.chart.back, trade.chart.candle));
    assert_eq!(back, 0);
    let pan = |dx: f32, dy: f32| ScrollWheelEvent {
        position: point(px(100.), px(CHART_AT.1 + 100.)),
        delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
        ..Default::default()
    };
    // a finger moving right shows the candles before: five, and a part of
    // one carried to the next pan
    cx.simulate_scroll_wheel("trade-plot", pan(5. * candle + 3., 0.));
    view.read(|trade| assert_eq!(trade.chart.back, 5));
    cx.simulate_scroll_wheel("trade-plot", pan(candle - 3., 0.));
    view.read(|trade| assert_eq!(trade.chart.back, 6));
    // a pan up or down is the page's
    cx.simulate_scroll_wheel("trade-plot", pan(0., 120.));
    view.read(|trade| assert_eq!(trade.chart.back, 6));
    cx.simulate_scroll_wheel("trade-plot", pan(-6. * candle, 0.));
    view.read(|trade| assert_eq!(trade.chart.back, 0));
    cx.simulate_pinch(
        "trade-plot",
        PinchEvent {
            position: point(px(100.), px(CHART_AT.1 + 100.)),
            delta: 0.5,
            modifiers: Default::default(),
            phase: TouchPhase::Moved,
        },
    );
    view.read(|trade| assert_eq!(trade.chart.candle, candle * 1.5));
}

#[test]
fn a_tap_holds_a_candle_and_a_second_tap_lets_it_go() {
    let (mut cx, view, _) = phone();
    btc(&mut cx);
    assert_eq!(held_line(&cx), None);
    tap_plot(&mut cx, (100., 100.));
    let held = view.read(|trade| trade.chart.held);
    assert!(held.is_some());
    assert!(held_line(&cx).is_some(), "the held candle's line");
    // the held candle stays held as the chart pans
    let pan = ScrollWheelEvent {
        delta: ScrollDelta::Pixels(point(px(16.), px(0.))),
        ..Default::default()
    };
    cx.simulate_scroll_wheel("trade-plot", pan);
    view.read(|trade| assert_eq!(trade.chart.held, held));
    tap_plot(&mut cx, (100. + 16., 100.));
    view.read(|trade| assert_eq!(trade.chart.held, None));
    assert_eq!(held_line(&cx), None);
    // a tap under the candles, on the times, holds none
    tap_plot(&mut cx, (100., CHART.1 - 4.));
    view.read(|trade| assert_eq!(trade.chart.held, None));
}

#[test]
fn the_chart_sheet_switches_what_it_draws_and_the_step_sheet_picks_a_step() {
    let (mut cx, view, _) = phone();
    btc(&mut cx);
    cx.simulate_click("trade-chart-settings");
    shows(
        &cx,
        &[
            "Chart",
            "MA 7",
            "MA 25",
            "Volume",
            "Line instead of candles",
        ],
    );
    cx.simulate_click("trade-switch-3");
    view.read(|trade| assert!(trade.chart.line));
    assert!(cx.find("trade-chart-sheet").is_some(), "the sheet stays");
    cx.simulate_back();
    cx.simulate_click("trade-step");
    shows(
        &cx,
        &["Price step", "USD per row of the book", "0.1", "1", "10"],
    );
    cx.simulate_click("trade-step-1");
    cx.run_until_parked();
    view.read(|trade| {
        assert_eq!(trade.step, 1);
        assert_eq!(trade.sheet, None);
    });
    assert!(asked(&cx).contains(&Query::Book {
        market: 0,
        step: 10,
        depth: 15
    }));
}

#[test]
fn back_closes_the_sheet_then_the_page_and_at_the_markets_is_the_hosts() {
    let (mut cx, view, _) = phone();
    assert_eq!(cx.back_handler(), None, "the markets: the host leaves");
    btc(&mut cx);
    cx.simulate_click("trade-buy");
    assert!(cx.back_handler().is_some());
    cx.simulate_back();
    view.read(|trade| {
        assert_eq!(trade.sheet, None);
        assert_eq!(trade.pages.top(), &Route::Market);
    });
    cx.simulate_back();
    view.read(|trade| assert_eq!(trade.pages.top(), &Route::Markets));
    assert_eq!(cx.back_handler(), None);
}

#[test]
fn with_no_link_the_bar_is_off_and_says_why() {
    let (mut cx, ..) = phone();
    linked(&mut cx, false);
    shows(
        &cx,
        &["Not connected to a node: prices are the last read. Orders are not sent."],
    );
    btc(&mut cx);
    shows(
        &cx,
        &[
            "Placing an order needs a link to the network. There is none now, so prices, \
           the book and trades are all here to read.",
        ],
    );
    assert!(!pressable(&cx, "trade-buy") && !pressable(&cx, "trade-sell"));
    assert_eq!(cx.interactivity("trade-buy").aria.disabled, Some(true));
    linked(&mut cx, true);
    assert!(pressable(&cx, "trade-buy"));
}

#[test]
fn a_reader_with_no_account_reads_the_markets_and_no_account_rows() {
    let (mut cx, ..) = finger(sample::board(), None);
    assert!(cx.find("trade-open-orders").is_none() && cx.find("trade-balances").is_none());
    shows(
        &cx,
        &[
            "Orders and balances are an account's. You are looking around without one, so \
           prices, the book and trades are all here to read.",
        ],
    );
    btc(&mut cx);
    assert!(!pressable(&cx, "trade-buy"));
    cx.simulate_click("trade-segment-orders");
    shows(&cx, &["Orders, fills and balances are an account's."]);
    // a level still opens the ticket, which places nothing
    cx.simulate_click("trade-segment-book");
    cx.simulate_click("book-ask-0");
    assert!(!pressable(&cx, "trade-submit"));
    assert_eq!(ops(&cx), []);
}

#[test]
fn every_screen_fits_each_phone_and_presses_at_a_fingers_size() {
    let (mut cx, view, _) = phone();
    sweep(&mut cx, |cx| {
        btc(cx);
        for segment in [
            "trade-segment-trades",
            "trade-segment-orders",
            "trade-segment-book",
        ] {
            cx.simulate_click(segment);
        }
        tap_plot(cx, (100., 100.));
        for sheet in ["trade-chart-settings", "trade-step", "trade-buy"] {
            cx.simulate_click(sheet);
            cx.simulate_back();
        }
        cx.simulate_click("trade-sell");
        cx.simulate_click("trade-at-market");
        cx.simulate_input("trade-amount", "0.1");
        cx.simulate_back();
        cx.simulate_back();
        cx.simulate_click("trade-open-orders");
        for list in ["trade-list-history", "trade-list-fills", "trade-list-open"] {
            cx.simulate_click(list);
        }
        cx.simulate_back();
        cx.simulate_click("trade-balances");
        cx.simulate_back();
        view.read(|trade| assert_eq!(trade.pages.top(), &Route::Markets));
    });
}
