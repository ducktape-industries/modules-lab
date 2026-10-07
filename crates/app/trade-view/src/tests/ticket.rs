//! The ticket: the order it sends, what it works out from the funds and
//! the book, and what it holds back.
use ducktape_view_guest::methods::HostSession;
use ducktape_view_guest::wire::Node;
use trade::{LOT, Side};

use super::*;

fn typed(cx: &mut TestAppContext, price: &str, amount: &str) {
    cx.simulate_input("trade-price", price);
    cx.simulate_input("trade-amount", amount);
}

/// What the amount field holds.
fn amount(view: &Entity<Trade>) -> String {
    view.read(|trade| trade.ticket.amount.text())
}

/// Whether the field `key` is one the reader may not type in.
fn disabled(cx: &TestAppContext, key: &str) -> bool {
    let Node::Field { options, .. } = cx.node(key) else {
        panic!("{key:?} is no field");
    };
    options.disabled
}

/// The ticket holds an order back: its button hears no press and says so,
/// and Enter in a field sends nothing.
#[track_caller]
fn held_back(cx: &mut TestAppContext) {
    assert!(!pressable(cx, "trade-submit"));
    assert_eq!(cx.interactivity("trade-submit").aria.disabled, Some(true));
    cx.simulate_submit("trade-amount");
    assert_eq!(ops(cx), []);
}

/// The first states board: 0.1 BTC at 63,200.0.
#[test]
fn a_limit_buy_is_placed_in_the_programs_units() {
    let (mut cx, ..) = wide();
    typed(&mut cx, "63,200.0", "0.1");
    shows(&cx, &["6,320.00", "25,280.00 USD", "Buy 0.1000 BTC"]);
    let before = reads(&cx);
    cx.simulate_click("trade-submit");
    assert_eq!(
        ops(&cx),
        [Op::Place {
            market: 0,
            side: Side::Buy,
            amount: 1000 * LOT,
            limit: Some(63_200 * UNIT),
        }]
    );
    // the program took it: what it changed is read again
    let after = reads(&cx);
    assert!(
        after
            .iter()
            .zip(before)
            .all(|(after, before)| *after > before),
        "{after:?} after {before:?}"
    );
}

/// The second: half of the 0.65 BTC free, at the book's bids.
#[test]
fn a_market_sell_is_estimated_from_the_book_and_placed_with_no_limit() {
    let (mut cx, view, _) = wide();
    cx.simulate_click("trade-sell");
    cx.simulate_click("trade-at-market");
    shows(
        &cx,
        &["Best price in the book", "Estimated total", "0.6500 BTC"],
    );
    assert!(cx.find("trade-price").is_none());
    cx.simulate_click("trade-share-50");
    assert_eq!(amount(&view), "0.3250");
    // 0.1106 at 63,588.8 and the rest at 63,588.7
    shows(&cx, &["20,666.34", "Sell 0.3250 BTC"]);
    cx.simulate_submit("trade-amount");
    assert_eq!(
        ops(&cx),
        [Op::Place {
            market: 0,
            side: Side::Sell,
            amount: 3250 * LOT,
            limit: None,
        }]
    );
}

/// A share of what the free funds afford, down to a whole lot.
#[test]
fn a_share_button_fills_in_what_the_free_funds_afford() {
    let (mut cx, view, _) = wide();
    // no price yet: nothing to work a buy out from
    cx.simulate_click("trade-share-50");
    assert_eq!(amount(&view), "");
    cx.simulate_input("trade-price", "63,200.0");
    cx.simulate_click("trade-share-25");
    assert_eq!(amount(&view), "0.1000");
    let picked = |cx: &TestAppContext, share: u32| {
        let share = cx.interactivity(&format!("trade-share-{share}"));
        share
            .aria
            .toggled
            .is_some_and(|toggled| toggled == true.into())
    };
    assert!(picked(&cx, 25) && !picked(&cx, 50));
    cx.simulate_click("trade-share-100");
    assert_eq!(amount(&view), "0.4000");
    // 25,280.00 / 63,589.1 = 0.39755…, and three quarters of it 0.29816…
    cx.simulate_input("trade-price", "63,589.1");
    cx.simulate_click("trade-share-100");
    assert_eq!(amount(&view), "0.3975");
    cx.simulate_click("trade-share-75");
    assert_eq!(amount(&view), "0.2981");
    shows(&cx, &["Buy 0.2981 BTC"]);
    // at the market: up the asks, 0.4504 at 63,589.7 being more than all of it
    cx.simulate_click("trade-at-market");
    cx.simulate_click("trade-share-100");
    assert_eq!(amount(&view), "0.3975");
    // a sale is of the base
    cx.simulate_click("trade-sell");
    cx.simulate_click("trade-share-100");
    assert_eq!(amount(&view), "0.6500");
}

/// The third board: the order is not sent, and the ticket says what it
/// needs where it said what is available.
#[test]
fn an_order_the_free_funds_do_not_cover_is_not_sent() {
    let (mut cx, ..) = wide();
    typed(&mut cx, "63,200.0", "0.5");
    shows(&cx, &["31,600.00", "Buy 0.5000 BTC"]);
    shows(
        &cx,
        &["This order needs 31,600.00 USD. 25,280.00 USD is available."],
    );
    assert!(!cx.has_text("Available"));
    held_back(&mut cx);
    // a sale of more of the base than is free
    cx.simulate_click("trade-sell");
    cx.simulate_input("trade-amount", "0.7");
    shows(
        &cx,
        &["This order needs 0.7000 BTC. 0.6500 BTC is available."],
    );
    held_back(&mut cx);
    cx.simulate_input("trade-amount", "0.65");
    assert!(pressable(&cx, "trade-submit"));
}

#[test]
fn a_price_off_the_tick_or_an_amount_off_the_lot_is_not_sent() {
    let (mut cx, ..) = wide();
    typed(&mut cx, "63,200.05", "0.1");
    let off_tick = "A price here is a number in steps of 0.1 USD.";
    shows(&cx, &[off_tick]);
    let Node::Field { options, .. } = cx.node("trade-price") else {
        panic!("the price is a field");
    };
    assert!(options.invalid.is_some());
    assert_eq!(options.description.as_deref(), Some(off_tick));
    held_back(&mut cx);
    typed(&mut cx, "63,200.0", "0.00015");
    shows(&cx, &["An amount is a number in steps of 0.0001 BTC."]);
    held_back(&mut cx);
    typed(&mut cx, "a lot", "0.1");
    shows(&cx, &[off_tick]);
    typed(&mut cx, "0", "0.1");
    shows(&cx, &[off_tick]);
    held_back(&mut cx);
    // an empty field is not wrong yet: there is only nothing to send
    typed(&mut cx, "", "0.1");
    assert!(!cx.has_text(off_tick));
    shows(&cx, &["Available"]);
    held_back(&mut cx);
}

#[test]
fn what_the_program_refused_an_order_for_is_said_in_the_ticket() {
    let (mut cx, _, world) = wide();
    let refusal = "the market is closed for the day";
    world.borrow_mut().refusal = Some(refusal.into());
    typed(&mut cx, "63,200.0", "0.1");
    let before = reads(&cx);
    cx.simulate_click("trade-submit");
    assert_eq!(ops(&cx).len(), 1);
    shows(&cx, &[refusal]);
    assert_eq!(reads(&cx), before, "nothing changed: nothing is read again");
    assert!(
        pressable(&cx, "trade-submit"),
        "the order may be sent again"
    );
    // the next thing typed takes it back
    cx.simulate_input("trade-amount", "0.2");
    assert!(!cx.has_text(refusal));
}

/// The fourth board: with no account everything reads, the ticket and the
/// panel say whose they are, and no op leaves.
#[test]
fn a_reader_with_no_account_reads_everything_and_sends_nothing() {
    let (mut cx, ..) = opened(sample::board(), WIDE, None);
    let note = "Placing an order needs an account. You are looking around without one, so \
                prices, the book and trades are all here to read.";
    shows(&cx, &[note, "Buy BTC"]);
    shows(&cx, &["Orders, fills and balances are an account's."]);
    assert!(disabled(&cx, "trade-price") && disabled(&cx, "trade-amount"));
    assert!(cx.find("trade-share-25").is_none() && !cx.has_text("Available"));
    assert!(!pressable(&cx, "trade-submit"));
    assert_eq!(cx.interactivity("trade-submit").aria.disabled, Some(true));
    // the market reads; nothing of an account's is asked for
    shows(&cx, &["63,981.4", "63,589.7", "14:02:41"]);
    assert_eq!(chart_candles(&cx), 88);
    let read = reads(&cx);
    assert!(read[..4].iter().all(|read| *read > 0), "{read:?}");
    assert_eq!(read[4..], [0, 0, 0]);
    // a book row's price still lands in the ticket, where it reads
    cx.simulate_click("book-ask-0");
    shows(&cx, &["63,589.7"]);
    assert_eq!(ops(&cx), []);
    // an account seated: its rows are read and the ticket is live
    let sessions = cx.host().stream::<HostSession>();
    sessions.send(sample::session(Some(READER)));
    cx.run_until_parked();
    shows(&cx, &["Available", "25,280.00 USD", "Cancel all"]);
    assert!(!disabled(&cx, "trade-price"));
    assert!(reads(&cx)[4..].iter().all(|read| *read > 0));
    // the link lost: the ticket says so and places nothing
    sessions.send(ducktape_view_guest::methods::Session {
        connected: false,
        ..sample::session(Some(READER))
    });
    cx.run_until_parked();
    shows(
        &cx,
        &[
            "Placing an order needs a link to the network. There is none now, so \
                  prices, the book and trades are all here to read.",
        ],
    );
    assert!(!pressable(&cx, "trade-submit") && !pressable(&cx, "cancel-6"));
}

/// The candles on screen.
fn chart_candles(cx: &TestAppContext) -> usize {
    chart::slots(cx)
}
