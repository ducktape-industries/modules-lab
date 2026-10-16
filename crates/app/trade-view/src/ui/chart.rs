//! The desktop's chart: the market's candles laid out by the stretch of
//! time each one is (`chart.rs`), their volume under them, two moving
//! averages, the last price and the reader's open orders on their lines.
//!
//! The view draws it itself: the canvas and its labels are `look.rs`,
//! laid out alike for the phone's chart. The chart's sensor tells where the
//! host laid it out, so a pointer's position in the window is a place on
//! the plot: one listener finds the candle under it and the row it is in.
//! That candle's line, its time, the readout, and the crosshair's line
//! across the prices with the price at it are elements, drawn again when
//! the pointer enters another slot or another row; a move that changes
//! neither draws nothing, and the canvas is never sent for a hover. The
//! wheel zooms about the pointer, a drag pans by whole candles, and the
//! arrow keys do both for a reader without a pointer.
use view::CursorStyle;
use view::prelude::*;

use crate::chart::Plot;
use crate::look::tag;
use crate::{Trade, words};

impl Trade {
    fn wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let lines = f32::from(event.delta.pixel_delta(px(16.)).y) / 16.;
        if lines != 0. {
            let about = f32::from(event.position.x - self.chart.origin.x);
            self.zoom(0.9_f32.powf(lines), Some(about), cx);
            cx.notify();
        }
    }

    // A drag pans by whole candles. Only the distance between two positions
    // is read, which the window's pixels give as well as the chart's would.
    fn drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(from) = self.chart.grabbed else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.chart.grabbed = None;
            return;
        }
        let step = self.chart.candle;
        let candles = ((f32::from(event.position.x) - from) / step).trunc();
        if candles != 0. {
            // What is left of the distance stays with the grab, so a slow
            // drag adds up.
            self.chart.grabbed = Some(from + candles * step);
            self.pan(candles as i64, cx);
            cx.notify();
        }
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let page = (self.chart.slots() / 10).max(1) as i64;
        match event.keystroke.key.as_str() {
            "left" => self.pan(page, cx),
            "right" => self.pan(-page, cx),
            "up" => self.zoom(0.8, None, cx),
            "down" => self.zoom(1.25, None, cx),
            _ => return,
        }
        cx.notify();
    }

    /// The chart in its sensor: the plot once the host has measured it and
    /// the program has said a candle, and until then what there is to say.
    pub(super) fn chart(
        &mut self,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let said = |text: SharedString| {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(design::quiet(text, theme))
                .into_any_element()
        };
        let plot = self.chart.plot(self.market().tick);
        // The pointer goes with the plot, whatever took the plot away: the
        // host tells that the pointer left through the plot's own element,
        // and this frame has none.
        if plot.is_none() {
            self.chart.pointer = None;
        }
        let candles = &self.chart.candles;
        let chart = match (candles, &plot) {
            (Loadable::Failed(refusal), _) => said(refusal.message.clone().into()),
            (_, Some(plot)) => self.plotted(plot, cx, theme).into_any_element(),
            (Loadable::Ready(candles), None) if candles.is_empty() => said(words::NO_TRADES.into()),
            // nothing to draw on until the candles and the chart's size land
            _ => div().size_full().into_any_element(),
        };
        // The chart is drawn to its size and the pointer is placed by its
        // origin: a chart that only moved is drawn again when the pointer
        // draws something else on it, and not otherwise.
        let measured = cx.listener(|view: &mut Trade, bounds: &Bounds<Pixels>, _, cx| {
            if view.chart.lay(bounds) {
                cx.notify();
            }
        });
        sensor("trade-chart", chart)
            .flex_1()
            .min_h_0()
            .on_bounds(measured)
    }

    /// The plot: the chart's parts (`look.rs`) for the candle under the
    /// pointer, and the crosshair's line across the prices at its row with
    /// the price there on the axis, over every other tag.
    fn plotted(
        &self,
        plot: &Plot,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let market = self.market();
        let crosshair = self.chart.crosshair();
        let overlay = self.overlay(plot, self.chart.hovered(), crosshair, theme);
        // Both kept in the tree, drawn or not, as the candle's line is. The
        // price is the market's tick nearest the row's, as the rules are
        // ticks.
        let across = div().absolute().left_0().w(px(plot.width)).h(px(1.));
        let across = match crosshair {
            Some(y) => across.top(px(y)).bg(theme.muted),
            None => across,
        };
        let pointed = match crosshair {
            Some(y) => {
                let ticks = (plot.price(y) / market.tick as f64).round().max(0.);
                tag(self.price(ticks as u64 * market.tick), plot, y, theme)
                    .border_color(theme.foreground)
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .into_any_element()
            }
            None => div().into_any_element(),
        };
        div()
            .id("trade-plot")
            .relative()
            .size_full()
            .overflow_hidden()
            .cursor(CursorStyle::Crosshair)
            // Arrow keys pan and zoom, for a reader without a pointer.
            .role(Role::Figure)
            .aria_label(overlay.named)
            .focusable()
            .on_key_down(
                cx.listener(|view: &mut Trade, event: &KeyDownEvent, _, cx| view.key(event, cx)),
            )
            .on_scroll_wheel(
                cx.listener(|view: &mut Trade, event: &ScrollWheelEvent, _, cx| {
                    view.wheel(event, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view: &mut Trade, event: &MouseDownEvent, _, _| {
                    view.chart.grabbed = Some(f32::from(event.position.x));
                }),
            )
            // The one listener that hears the pointer: a drag pans, and the
            // candle under the pointer is the one told, at its row's price.
            .on_mouse_move(
                cx.listener(|view: &mut Trade, event: &MouseMoveEvent, _, cx| {
                    view.drag(event, cx);
                    if view.chart.hover(Some(event.position)) {
                        cx.notify();
                    }
                }),
            )
            .on_hover(cx.listener(|view: &mut Trade, over: &bool, _, cx| {
                if !over && view.chart.hover(None) {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view: &mut Trade, _: &MouseUpEvent, _, _| view.chart.grabbed = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view: &mut Trade, _: &MouseUpEvent, _, _| view.chart.grabbed = None),
            )
            .child(overlay.canvas)
            .child(overlay.line)
            .child(across)
            .child(overlay.prices)
            .children(overlay.order_tags)
            .children(overlay.last_tag)
            .child(overlay.times)
            .children(overlay.readout)
            .child(pointed)
    }
}
