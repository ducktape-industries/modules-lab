//! The book's rows and the panel under the chart: the reader's orders,
//! fills and balances, and the ops a press there sends.
use trade::{Order, OrderState, Side};

use super::*;

#[test]
fn a_book_rows_price_lands_in_the_ticket() {
    let (mut cx, view, _) = wide();
    cx.simulate_click("trade-at-market");
    // the third bid from the best
    cx.simulate_click("book-bid-2");
    view.read(|trade| {
        assert_eq!(trade.ticket.price.text(), "63,587.3");
        assert!(!trade.ticket.market, "a price is a limit order's");
    });
    cx.simulate_input("trade-amount", "0.1");
    shows(&cx, &["6,358.73"]);
    // the arrows walk the rows from the one pressed, and Enter takes
    // the one they are on
    cx.simulate_focus("trade-book");
    cx.simulate_key_down("trade-book", "down");
    cx.simulate_key_down("trade-book", "enter");
    view.read(|trade| assert_eq!(trade.ticket.price.text(), "63,586.8"));
}

/// The book's rows: fifteen a side where they fit, each side's running
/// total from its best price out, and the sides' shares at its foot.
#[test]
fn the_book_runs_each_sides_total_from_its_best_price() {
    let (cx, ..) = wide();
    assert!(cx.find("book-ask-14").is_some() && cx.find("book-bid-14").is_some());
    // the two best asks: 0.4504, then 0.5998 more
    shows(&cx, &["0.4504", "0.5998", "1.0502"]);
    // the spread, and each side's whole: 8.1640 asked, 7.6547 bid
    shows(&cx, &["0.9", "8.1640", "7.6547", "Bids 48%", "52% Asks"]);
}

#[test]
fn the_books_price_step_is_asked_of_the_program() {
    let (mut cx, view, _) = wide();
    cx.simulate_click("trade-step");
    shows(&cx, &["0.1", "1", "10"]);
    let before = reads(&cx);
    cx.simulate_click("trade-step-menu-2");
    assert!(
        cx.find("trade-step-menu").is_none(),
        "one is picked: the menu closes"
    );
    view.read(|trade| assert_eq!(trade.step, 2));
    assert_eq!(
        asked(&cx).last(),
        Some(&Query::Book {
            market: 0,
            step: 100,
            depth: 15
        })
    );
    let mut after = before;
    after[2] += 1;
    assert_eq!(reads(&cx), after);
}

/// A pane too short for fifteen prices a side shows the best that fit.
#[test]
fn a_short_book_shows_the_best_prices_that_fit() {
    let (mut cx, ..) = opened(sample::board(), PANE, Some(READER));
    cx.simulate_click("trade-tab-book");
    // 500 less the bar, the tabs and the book's own lines, 20 px a row
    assert!(cx.find("book-ask-6").is_some() && cx.find("book-bid-6").is_some());
    assert!(cx.find("book-ask-7").is_none() && cx.find("book-bid-7").is_none());
    shows(&cx, &["63,589.7", "63,588.8"]);
    // a press there turns the column back to the ticket, the price in it
    cx.simulate_click("book-ask-0");
    shows(&cx, &["63,589.7", "Available"]);
}

#[test]
fn a_cancel_a_cancel_all_and_a_grant_send_their_ops() {
    let (mut cx, ..) = wide();
    let before = reads(&cx);
    cx.simulate_click("cancel-6");
    cx.simulate_click("trade-cancel-all");
    cx.simulate_click("trade-balances");
    cx.simulate_click("trade-fund");
    assert_eq!(ops(&cx), [Op::Cancel { order: 6 }, Op::CancelAll, Op::Fund]);
    let after = reads(&cx);
    assert!(
        after
            .iter()
            .zip(before)
            .all(|(after, before)| *after >= before + 3),
        "everything is read again after each: {after:?} after {before:?}"
    );
}

#[test]
fn a_grant_says_it_is_on_its_way_then_what_it_did() {
    let (mut cx, ..) = wide();
    cx.simulate_click("trade-balances");
    cx.host().never::<Submit<trade::Trade>>();
    cx.simulate_click("trade-fund");
    shows(&cx, &["Getting practice funds\u{2026}"]);
    assert!(!pressable(&cx, "trade-fund"), "one grant at a time");

    let (mut cx, ..) = wide();
    cx.simulate_click("trade-balances");
    cx.simulate_click("trade-fund");
    cx.run_until_parked();
    shows(&cx, &["Practice funds topped up to the grant."]);
    assert!(pressable(&cx, "trade-fund"));
    // the reader's next write takes the note away
    cx.simulate_click("trade-open");
    cx.simulate_click("trade-cancel-all");
    assert!(cx.find("trade-granted").is_none());
}

#[test]
fn what_the_program_refused_a_cancel_for_is_said_over_the_rows() {
    let (mut cx, _, world) = wide();
    let refusal = "order 6 is not open";
    world.borrow_mut().refusal = Some(refusal.into());
    cx.simulate_click("cancel-6");
    shows(&cx, &[refusal, "14:02:10"]);
    // the next op that goes through takes it back
    world.borrow_mut().refusal = None;
    cx.simulate_click("trade-cancel-all");
    assert!(!cx.has_text(refusal));
}

#[test]
fn each_tab_lists_its_rows() {
    let (mut cx, view, _) = wide();
    // the open orders: a resting buy, nothing of it filled
    shows(
        &cx,
        &["BTC/USD", "Limit", "62,635.0", "0.2500", "0%", "Cancel"],
    );
    cx.simulate_click("trade-history");
    view.read(|trade| assert_eq!(trade.panel, Panel::History));
    // an order's average is what its fills came to, a unit
    shows(
        &cx,
        &["Oct 7 09:30", "63,670.0", "100%", "Filled", "Cancelled"],
    );
    shows(
        &cx,
        &["Average", "State", "Open", "DUCK/USD", "1.2000", "500.0000"],
    );
    assert!(!cx.has_text("Cancel all"));
    cx.simulate_click("trade-fills");
    shows(
        &cx,
        &["Oct 7 09:41", "6,367.00", "3,183.50", "Maker", "Taker"],
    );
    shows(&cx, &["63,805.0", "3,190.25"]);
    cx.simulate_click("trade-balances");
    shows(
        &cx,
        &["Asset", "Available", "In orders", "Get practice funds"],
    );
    shows(&cx, &["USD", "25,280.00", "19,753.75", "45,033.75"]);
    shows(
        &cx,
        &["BTC", "0.6500", "0.1000", "0.7500", "DUCK", "0.0000"],
    );
}

/// The history goes on past what one page holds: the page after is read
/// when its list is scrolled to the end of the rows held.
#[test]
fn the_history_reads_on_as_its_list_is_scrolled() {
    let mut world = sample::board();
    world.orders = (1..=150u64)
        .rev()
        .map(|id| Order {
            id,
            owner: READER,
            market: 0,
            side: Side::Buy,
            limit: Some(60_000 * UNIT + id * UNIT),
            amount: UNIT,
            filled: 0,
            quote: 0,
            placed: sample::NOW,
            state: OrderState::Cancelled,
        })
        .collect();
    let (mut cx, ..) = opened(world, WIDE, Some(READER));
    let pages = |cx: &TestAppContext| {
        let asked = asked(cx).into_iter();
        let pages = asked.filter_map(|query| match query {
            Query::Orders {
                open: false, page, ..
            } => Some(page.after.is_some()),
            _ => None,
        });
        pages.collect::<Vec<_>>()
    };
    cx.simulate_click("trade-history");
    shows(&cx, &["60,150.0", "No open orders"][..1]);
    assert_eq!(pages(&cx), [false], "the first page alone");
    cx.simulate_range("trade-history-rows", 60..65);
    assert_eq!(pages(&cx), [false, true]);
    cx.simulate_range("trade-history-rows", 64..70);
    shows(&cx, &["60,086.0"]);
    assert_eq!(pages(&cx).len(), 2, "the rows are held");
}
