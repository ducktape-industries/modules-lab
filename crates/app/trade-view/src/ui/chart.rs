//! The desktop's chart: the market's candles laid out by the stretch of
//! time each one is (`chart.rs`), their volume under them, two moving
//! averages, the last price and the reader's open orders on their lines.
//!
//! The view draws it itself. A `canvas()` carries quads and lines
//! (`look.rs`), which the host paints natively and in order, and every
//! label is an element laid over it. The chart's sensor tells where the
//! host laid it out, so a pointer's position in the window is a place on
//! the plot: one listener finds the candle under it and the row it is in.
//! That candle's line, its time, the readout, and the crosshair's line
//! across the prices with the price at it are elements, drawn again when
//! the pointer enters another slot or another row; a move that changes
//! neither draws nothing, and the canvas is never sent for a hover. The
//! wheel zooms about the pointer, a drag pans by whole candles, and the
//! arrow keys do both for a reader without a pointer.
use trade::{Candle, SPANS};
use view::prelude::*;
use view::{CursorStyle, FontWeight};

use crate::chart::{Candles, PRICES, Plot};
use crate::look::{rising, sided};
use crate::{Trade, format, words};

/// A label's height on an axis: a rule's price under a tag is left out.
const TAG_H: f32 = 18.;
/// The strip along the plot's top that the readout is told in.
const READOUT: f32 = 28.;

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
            let chart = &mut view.chart;
            let size = (bounds.size.width.into(), bounds.size.height.into());
            let pointed = chart.pointed();
            chart.origin = bounds.origin;
            if std::mem::replace(&mut chart.size, size) != size || pointed != chart.pointed() {
                cx.notify();
            }
        });
        sensor("trade-chart", chart)
            .flex_1()
            .min_h_0()
            .on_bounds(measured)
    }

    /// The plot: the canvas, the crosshair's lines, and laid over them a
    /// price beside each rule, the orders and the last price on their
    /// lines, the times, the readout and the price at the pointer.
    fn plotted(
        &self,
        plot: &Plot,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let theme = *theme;
        let market = self.market();
        let span = &SPANS[self.chart.span];
        let label = |text: String| {
            design::mono(text)
                .absolute()
                .text_size(design::text::CAPTION)
                .text_color(theme.muted)
        };
        // a tag on the price axis, on its line
        let tag = |price: u64, y: f32| {
            label(self.price(price))
                .left(px(plot.width))
                .top(px(y - TAG_H / 2.))
                .w(px(PRICES))
                .h(px(TAG_H))
                .pl(px(7.))
                .flex()
                .items_center()
                .border_1()
                .font_weight(FontWeight::MEDIUM)
        };
        let orders = self.order_lines(plot);
        let last = self.chart.candles.ready().and_then(Candles::last);
        let last = last.and_then(|last| Some((last, plot.line(last.close)?)));
        // A tag is drawn only where it covers no other: the last price's
        // first, then the orders' from the nearest to it outwards. An
        // order with no tag keeps its line.
        let mut tagged: Vec<f32> = last.iter().map(|(_, y)| *y).collect();
        let near = last.map_or(0., |(_, y)| y);
        let mut tags: Vec<_> = orders.iter().collect();
        tags.sort_by(|(a, ..), (b, ..)| (a - near).abs().total_cmp(&(b - near).abs()));
        tags.retain(|(y, ..)| {
            let clear = tagged.iter().all(|tag| (tag - y).abs() >= TAG_H);
            if clear {
                tagged.push(*y);
            }
            clear
        });
        let (step, rules) = plot.rules(market.tick);
        let rules: Vec<(u64, f32)> = rules
            .filter_map(|rule| Some((rule, plot.line(rule)?)))
            .collect();
        let crosshair = self.chart.crosshair();
        // a tag covers a rule's price that is under it, the pointer's too
        let clear = |y: &f32| {
            let mut tags = tagged.iter().chain(&crosshair);
            tags.all(|tag| (tag - y).abs() > TAG_H - 4.)
        };
        let places = format::decimals(step).min(format::decimals(market.tick));
        let prices = rules.iter().filter(|(_, y)| clear(y)).map(|(rule, y)| {
            label(format::fixed(*rule, places))
                .left(px(plot.width + 8.))
                .top(px(y - 7.))
        });
        // The rule prices in a layer of their own, as the times are: the
        // pointer's tag takes the one under it away and puts it back, and
        // the plot's other children keep their places for it.
        let prices = div().absolute().inset_0().children(prices);
        let order_tags = tags.iter().flat_map(|(y, side, left, limit)| {
            let color = sided(*side, &theme);
            let word = words::side(*side);
            // the readout is told along the plot's top: no name over it
            let named = (y - TAG_H / 2. >= READOUT).then(|| {
                label(format!("{word} {}", format::amount(*left)))
                    .left(design::space::SM)
                    .top(px(y - TAG_H / 2.))
                    .h(px(TAG_H))
                    .px(design::space::XS)
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(color)
                    .bg(theme.background)
                    .text_color(color)
            });
            let priced = tag(*limit, *y)
                .border_color(color)
                .bg(theme.background)
                .text_color(color);
            [named, Some(priced)].into_iter().flatten()
        });
        let last_tag = last.map(|(last, y)| {
            let color = rising(last, &theme);
            tag(last.close, y)
                .border_color(color)
                .bg(color)
                .text_color(theme.primary_foreground)
        });
        // A time under every candle whose bucket is a multiple of `every`:
        // the labels keep 80 px apart, on the first span a clock reads
        // easily that is whole candles, and each stays under its candle as
        // the chart pans.
        let minutes = span.millis / 60_000;
        let needed = (80. / plot.step).ceil().max(1.) as u64 * minutes;
        let every = AXIS
            .into_iter()
            .find(|axis| *axis >= needed && axis.is_multiple_of(minutes))
            .unwrap_or(needed)
            / minutes;
        // the candle under the pointer, and where its time is told: under
        // it, kept whole inside the plot
        let hovered = self.chart.hovered();
        let told_at = hovered.map(|slot| plot.middle(slot).clamp(44., (plot.width - 44.).max(44.)));
        let times = (0..plot.slots.len())
            .filter(|slot| (plot.first + *slot as u64).is_multiple_of(every))
            .map(|slot| (slot, plot.middle(slot)))
            .filter(|(_, middle)| *middle >= 30. && *middle <= plot.width - 30.)
            // that time covers a label that is under it
            .filter(|(_, middle)| told_at.is_none_or(|told| (told - middle).abs() >= 64.))
            .map(|(slot, middle)| {
                label(format::axis((plot.first + slot as u64) * span.millis))
                    .left(px(middle - 40.))
                    .top(px(plot.height + 5.))
                    .w(px(80.))
                    .text_center()
            });
        // the readout tells that candle, or the newest one on screen
        let told = hovered.unwrap_or(plot.slots.len() - 1);
        let told = plot.slots[told].map(|candle| (told, candle));
        let stamp = |candle: &Candle| match span.millis >= 86_400_000 {
            true => format::day(candle.bucket * span.millis),
            false => format::day_time(candle.bucket * span.millis),
        };
        let roomy = plot.width >= 600.;
        let readout = told.map(|(slot, candle)| {
            let pair = |key: &'static str, value: String, tone: Hsla| {
                div()
                    .flex()
                    .gap(px(5.))
                    .child(key)
                    .child(div().text_color(tone).child(value))
            };
            let average = |key: &'static str, averages: &[Option<f64>], tone: Hsla| {
                let average = averages.get(slot).copied().flatten().filter(|_| roomy)?;
                Some(pair(key, self.price(average.round() as u64), tone).text_color(tone))
            };
            div()
                .absolute()
                .left_0()
                .top_0()
                .pl(design::space::LG)
                .pt(design::space::SM)
                .flex()
                .gap(design::space::LG)
                .font_family(design::fonts::FAMILY_MONO)
                .text_size(design::text::MONO)
                .text_color(theme.muted)
                .whitespace_nowrap()
                .when(roomy, |readout| readout.child(stamp(&candle)))
                .child(pair("O", self.price(candle.open), theme.foreground))
                .child(pair("H", self.price(candle.high), theme.foreground))
                .child(pair("L", self.price(candle.low), theme.foreground))
                .child(pair("C", self.price(candle.close), theme.foreground))
                .child(pair("V", format::volume(candle.volume), theme.foreground))
                .children(average("MA7", &plot.fast, theme.muted))
                .children(average("MA25", &plot.slow, theme.warning))
        });
        // the hovered candle's time, on the time axis under it
        let hovered_time = hovered.zip(told_at).and_then(|(slot, middle)| {
            Some(
                label(stamp(&plot.slots[slot]?))
                    .left(px(middle - 44.))
                    .top(px(plot.height + 3.))
                    .w(px(88.))
                    .h(px(TAG_H))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .font_weight(FontWeight::MEDIUM),
            )
        });
        // The time axis in a layer of its own: a hover takes the label
        // under the candle's time away and puts it back, and the plot's
        // other children keep their places for it.
        let times = div()
            .absolute()
            .inset_0()
            .children(times)
            .children(hovered_time);
        // The line on the candle under the pointer. It is always in the
        // tree, drawn or not, so a hover moves one element and the labels
        // after it keep their places.
        let line = div().absolute().top_0().w(px(1.)).h(px(plot.height));
        let line = match hovered {
            Some(slot) => line.left(px(plot.middle(slot))).bg(theme.muted),
            None => line,
        };
        // The crosshair's other line, across the prices at the pointer's
        // row, and the price there on the axis, over every other tag: both
        // kept in the tree, drawn or not, as the candle's line is. The
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
                tag(ticks as u64 * market.tick, y)
                    .border_color(theme.foreground)
                    .bg(theme.foreground)
                    .text_color(theme.background)
                    .into_any_element()
            }
            None => div().into_any_element(),
        };
        let named = format!(
            "{} {} candles, {} on screen, last {}",
            market.name,
            span.name,
            plot.slots.iter().flatten().count(),
            last.map(|(last, _)| self.price(last.close))
                .unwrap_or_default(),
        );
        let rules: Vec<f32> = rules.iter().map(|(_, y)| *y).collect();
        let last = last.map(|(last, y)| (y, rising(last, &theme)));
        div()
            .id("trade-plot")
            .relative()
            .size_full()
            .overflow_hidden()
            .cursor(CursorStyle::Crosshair)
            // Arrow keys pan and zoom, for a reader without a pointer.
            .role(Role::Figure)
            .aria_label(named)
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
            .child(self.drawn(plot, &rules, &orders, last, &theme))
            .child(line)
            .child(across)
            .child(prices)
            .children(order_tags)
            .children(last_tag)
            .child(times)
            .children(readout)
            .child(pointed)
    }
}

/// The minutes between two time labels: what a clock or a calendar reads
/// easily, up to a month.
const AXIS: [u64; 13] = [
    1, 5, 15, 30, 60, 120, 240, 720, 1440, 2880, 10_080, 20_160, 43_200,
];
