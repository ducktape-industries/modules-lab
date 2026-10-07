//! The view on a fake host, natively. Every frame it sends is held to the
//! host's sanitizer and the accessibility audit.
use super::*;
use ducktape_view_guest::testing::TestAppContext;
use ducktape_view_guest::wire::{CanvasCommand, Node};
use gpui::ScrollDelta;

/// The view open, its chart measured at 900 by 480.
fn measured() -> TestAppContext {
    let mut cx = TestAppContext::new();
    cx.open::<Market>();
    cx.simulate_measure("market-chart", 900., 480.);
    cx.run_until_parked();
    cx
}

/// The quads the chart's canvas carries.
fn quads(cx: &TestAppContext) -> usize {
    let canvas = cx
        .node("market-plot")
        .children()
        .iter()
        .find_map(|node| match node {
            Node::Canvas { commands, .. } => Some(commands),
            _ => None,
        });
    canvas
        .expect("a canvas")
        .iter()
        .filter(|command| matches!(command, CanvasCommand::Draw { .. }))
        .count()
}

/// The rules, the plot's two edges and the last price's line.
const FIXED: usize = BANDS + 1 + 2 + 1;

/// The wheel over the chart, `lines` away from the reader.
fn wheel(cx: &mut TestAppContext, lines: f32) {
    cx.simulate_scroll_wheel(
        "market-plot",
        ScrollWheelEvent {
            delta: ScrollDelta::Lines(point(0., lines)),
            ..Default::default()
        },
    );
    cx.run_until_parked();
}

#[test]
fn a_measured_chart_draws_three_quads_a_candle() {
    let cx = measured();
    assert_eq!(quads(&cx), FIXED + 3 * 120);
    assert!(cx.has_text("BTC/USD"), "{:?}", cx.texts());
    assert_eq!(cx.host().requests::<ClockTicks>(), [1000]);
}

#[test]
fn the_wheel_zooms_between_the_fewest_and_the_most() {
    let mut cx = measured();
    wheel(&mut cx, 1.);
    assert_eq!(quads(&cx), FIXED + 3 * 108);
    for _ in 0..40 {
        wheel(&mut cx, 1.);
    }
    assert_eq!(quads(&cx), FIXED + 3 * FEWEST);
    for _ in 0..40 {
        wheel(&mut cx, -1.);
    }
    assert_eq!(quads(&cx), FIXED + 3 * MOST);
}

#[test]
fn a_hovered_candle_is_told_and_the_last_one_is_when_none_is() {
    let mut cx = measured();
    let told_last = cx.texts();
    cx.simulate_hover("candle-0", true);
    cx.run_until_parked();
    assert_eq!(quads(&cx), FIXED + 3 * 120 + 1, "the hovered slot's band");
    assert_ne!(cx.texts(), told_last);
    // The pointer is in the next slot before it has left this one.
    cx.simulate_hover("candle-1", true);
    cx.simulate_hover("candle-0", false);
    cx.run_until_parked();
    assert_eq!(quads(&cx), FIXED + 3 * 120 + 1);
    cx.simulate_hover("candle-1", false);
    cx.run_until_parked();
    assert_eq!(cx.texts(), told_last);
}

#[test]
fn the_arrow_keys_pan_and_the_series_ends_the_pan() {
    let mut cx = measured();
    let live = cx.texts();
    cx.simulate_focus("market-plot");
    cx.simulate_key_down("market-plot", "left");
    cx.run_until_parked();
    assert_ne!(cx.texts(), live, "twelve candles older");
    cx.simulate_key_down("market-plot", "right");
    cx.simulate_key_down("market-plot", "right");
    cx.run_until_parked();
    assert_eq!(cx.texts(), live, "no newer than the live candle");
}

#[test]
fn a_tick_moves_the_live_candle() {
    let mut cx = measured();
    let before = cx.texts();
    cx.host().stream::<ClockTicks>().send(());
    cx.run_until_parked();
    assert_ne!(cx.texts(), before);
    assert_eq!(quads(&cx), FIXED + 3 * 120);
}

#[test]
fn a_picked_span_and_a_zoom_outlive_a_redeploy() {
    let mut cx = measured();
    cx.simulate_click("1D");
    wheel(&mut cx, 1.);
    let bytes = cx.snapshot().expect("a snapshot");
    let mut cx = TestAppContext::new();
    cx.restore::<Market>(&bytes).expect("restored");
    cx.simulate_measure("market-chart", 900., 480.);
    cx.run_until_parked();
    assert_eq!(quads(&cx), FIXED + 3 * 108);
    // A day's candles open at midnight, so the axis names days.
    let names_a_day = |text: &String| {
        text.split_once(' ')
            .is_some_and(|(month, day)| MONTHS.contains(&month) && day.parse::<u32>().is_ok())
    };
    assert!(cx.texts().iter().any(names_a_day), "{:?}", cx.texts());
}

#[test]
fn a_price_is_grouped_and_keeps_the_decimals_its_size_calls_for() {
    assert_eq!(price(64_123.456), "64,123.46");
    assert_eq!(price(1_000_000.), "1,000,000.00");
    assert_eq!(price(999.5), "999.50");
    assert_eq!(price(1.25), "1.2500");
    assert_eq!(price(0.01234567), "0.012346");
}
