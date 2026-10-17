//! The phone's chart (SPEC §2.3, board 21): the desktop's canvas and
//! labels (`look.rs`) in a box 240 tall, under a finger's gestures. A tap
//! holds the candle under it, whose line, time and readout are told until
//! a second tap on it or a tap off the candles; a pan across follows the
//! finger, whole candles at a time; a pinch zooms about its centre. A pan
//! up or down is the page's scroll, and nothing follows a hover.
use view::AnyElement;
use view::prelude::*;
use view::{Div, Stateful};

use super::LADDER;
use super::parts::refused;
use crate::chart::Plot;
use crate::{Trade, words};

/// The chart's box: its plot and the time axis under it.
const CHART_H: Pixels = px(240.);
/// What one line of a wheel is in pixels, for a pan the host says in
/// lines.
const LINE: Pixels = px(16.);

/// The chart in its sensor, which tells the model where the host laid it
/// out; a refusal in its place.
pub(super) fn chart(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let candles = &view.chart.candles;
    if let Loadable::Failed(refusal) = candles {
        let retry = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.read_candles(cx));
        return refused("trade-candles", refusal, theme, retry).into_any_element();
    }
    let plot = view.chart.plot(view.market().tick);
    let body = match (candles, &plot) {
        (_, Some(plot)) => plotted(view, plot, cx, theme).into_any_element(),
        (Loadable::Ready(candles), None) if candles.is_empty() => empty(theme).into_any_element(),
        // nothing to draw on until the candles and the chart's size land
        _ => div().size_full().into_any_element(),
    };
    let measured = cx.listener(|view: &mut Trade, bounds: &Bounds<Pixels>, _, cx| {
        if view.chart.lay(bounds) {
            cx.notify();
        }
    });
    sensor("trade-chart", body)
        .flex_none()
        .w_full()
        .h(CHART_H)
        .on_bounds(measured)
        .into_any_element()
}

/// A market nobody has traded in yet.
fn empty(theme: &Theme) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_type(LADDER.secondary)
        .text_color(theme.muted)
        .child(words::NO_TRADES)
}

/// The plot under the finger's gestures, the held candle told.
fn plotted(view: &Trade, plot: &Plot, cx: &mut Context<Trade>, theme: &Theme) -> Stateful<Div> {
    let overlay = view.overlay(plot, view.chart.held_slot(), None, theme);
    let tap = cx.listener(|view: &mut Trade, event: &ClickEvent, _, cx| {
        view.chart.hold(event.position());
        cx.notify();
    });
    let pan = cx.listener(|view: &mut Trade, event: &ScrollWheelEvent, _, cx| {
        let across = f32::from(event.delta.pixel_delta(LINE).x);
        if across != 0. {
            view.swipe(across, cx);
        }
    });
    // the pinch is the chart's before the page hears it, so the page
    // stays where it is while the chart zooms
    let pinch = cx.listener(|view: &mut Trade, event: &PinchEvent, _, cx| {
        view.pinch(event.delta, event.position, cx);
    });
    div()
        .id("trade-plot")
        .relative()
        .size_full()
        .overflow_hidden()
        .role(Role::Figure)
        .aria_label(overlay.named)
        .focusable()
        .on_click(tap)
        .on_scroll_wheel(pan)
        .capture_pinch(pinch)
        .child(overlay.canvas)
        .child(overlay.line)
        .child(overlay.prices)
        .children(overlay.order_tags)
        .children(overlay.last_tag)
        .child(overlay.times)
        .children(overlay.readout)
}
