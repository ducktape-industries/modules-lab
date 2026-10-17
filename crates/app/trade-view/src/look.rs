//! What both designs draw alike, once: the colour of a side, of a candle
//! and of a move, and the chart over the plot the model lays out
//! (`chart.rs`). Its canvas carries quads and lines, which the host paints
//! natively and in order: the rules and the plot's edges, each candle's
//! wick, body and volume, the averages, the reader's orders' lines and the
//! last price's. Over it each label is an element: a price beside each
//! rule, the orders and the last price tagged on their lines, the times
//! under the candles, and the readout of the candle told. Nothing here
//! follows a pointer or a finger: a tree says which candle is told and
//! adds its own gestures, so a hover or a tap sends none of the canvas
//! again.
use std::cmp::Ordering;

use gpui::{point, size};
use trade::{Candle, SPANS, Side, Span};
use view::prelude::*;
use view::{Canvas, Div, FontWeight};

use crate::chart::{Candles, OrderLine, PRICES, Plot, VOLUME_ROOM};
use crate::{Trade, format, words};

/// A label's height on an axis: a rule's price under a tag is left out.
const TAG_H: f32 = 18.;
/// The strip along the plot's top that the readout is told in.
const READOUT: f32 = 28.;
/// How far apart two times under the axis are at least, and how wide the
/// told candle's time is, which keeps that whole inside the plot.
const TIME_W: f32 = 80.;
const TOLD_W: f32 = 88.;
/// From this wide a plot's readout also tells the candle's time and its
/// averages.
const ROOMY: f32 = 600.;
/// Under this wide a plot (a phone's; the desktop's narrowest is 416) has
/// its readout told in a caption's size, its figures closer, and wrapped
/// onto a second line of the strip where one does not hold them.
const NARROW: f32 = 400.;
/// A narrow readout's line: two of them under its 4 px top fill the
/// strip.
const NARROW_LINE: f32 = 12.;
/// The minutes between two time labels: what a clock or a calendar reads
/// easily, up to a month.
const AXIS: [u64; 13] = [
    1, 5, 15, 30, 60, 120, 240, 720, 1440, 2880, 10_080, 20_160, 43_200,
];

/// The chart's parts over a plot, for a tree to lay in its order over
/// its own gestures.
pub(crate) struct Overlay {
    pub(crate) canvas: Canvas,
    /// The line on the candle told. It is always in the tree, drawn or
    /// not, so a told candle moves one element and the labels after it
    /// keep their places.
    pub(crate) line: Div,
    /// The rule prices, in a layer of their own as the times are: a tag
    /// over the axis takes the one under it away and puts it back, and the
    /// plot's other children keep their places for it.
    pub(crate) prices: Div,
    pub(crate) order_tags: Vec<Div>,
    pub(crate) last_tag: Option<Div>,
    /// The time axis, with the told candle's time on it.
    pub(crate) times: Div,
    pub(crate) readout: Option<Div>,
    /// The plot as a screen reader names it.
    pub(crate) named: String,
}

impl Trade {
    /// The chart's parts over `plot`. `told` is the slot of the candle a
    /// pointer is on or a finger holds, whose line and time are drawn and
    /// which the readout tells; with none, the readout tells the newest.
    /// `crosshair` is a pointer's row over the prices, whose price its
    /// tree tags: the rule price under it is left out.
    pub(crate) fn overlay(
        &self,
        plot: &Plot,
        told: Option<usize>,
        crosshair: Option<f32>,
        theme: &Theme,
    ) -> Overlay {
        let market = self.market();
        let span = &SPANS[self.chart.span];
        let orders = self.order_lines(plot);
        let last = self.chart.candles.ready().and_then(Candles::last);
        let last = last.and_then(|last| Some((*last, plot.line(last.close)?)));
        let (tagged, tags) = tagged(last.map(|(_, y)| y), &orders);
        let (step, rules) = plot.rules(market.tick);
        let rules: Vec<(u64, f32)> = rules
            .filter_map(|rule| Some((rule, plot.line(rule)?)))
            .collect();
        // a tag covers a rule's price that is under it, the pointer's too
        let clear = |y: &f32| {
            let mut tags = tagged.iter().chain(&crosshair);
            tags.all(|tag| (tag - y).abs() > TAG_H - 4.)
        };
        let places = format::decimals(step).min(format::decimals(market.tick));
        let prices = rules.iter().filter(|(_, y)| clear(y)).map(|(rule, y)| {
            label(format::fixed(*rule, places), theme)
                .left(px(plot.width + 8.))
                .top(px(y - 7.))
        });
        let order_tags = tags
            .iter()
            .flat_map(|line| self.order_tag(plot, line, theme))
            .collect();
        let last_tag = last.map(|(last, y)| {
            let color = rising(&last, theme);
            tag(self.price(last.close), plot, y, theme)
                .border_color(color)
                .bg(color)
                .text_color(theme.primary_foreground)
        });
        let line = div().absolute().top_0().w(px(1.)).h(px(plot.height));
        let line = match told {
            Some(slot) => line.left(px(plot.middle(slot))).bg(theme.muted),
            None => line,
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
        let last = last.map(|(last, y)| (y, rising(&last, theme)));
        // the readout tells the candle told, or the newest on screen
        let readout = self.readout(plot, told.unwrap_or(plot.slots.len() - 1), span, theme);
        Overlay {
            canvas: self.drawn(plot, &rules, &orders, last, theme),
            line,
            prices: div().absolute().inset_0().children(prices),
            order_tags,
            last_tag,
            times: times(plot, span, told, theme),
            readout,
            named,
        }
    }

    /// An order's tag on its line, and left of the plot its side and what
    /// is left of it, where that is clear of the readout.
    fn order_tag(
        &self,
        plot: &Plot,
        line: &OrderLine,
        theme: &Theme,
    ) -> impl Iterator<Item = Div> + use<> {
        let (y, side, left, limit) = *line;
        let color = sided(side, theme);
        // the readout is told along the plot's top: no name over it
        let named = (y - TAG_H / 2. >= READOUT).then(|| {
            label(
                format!("{} {}", words::side(side), format::amount(left)),
                theme,
            )
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
        let priced = tag(self.price(limit), plot, y, theme)
            .border_color(color)
            .bg(theme.background)
            .text_color(color);
        [named, Some(priced)].into_iter().flatten()
    }

    /// The readout along the plot's top: the candle in `slot`, its open,
    /// high, low, close and volume, and on a roomy plot its time and its
    /// averages. A narrow plot's ends at the plot's edge, clear of the
    /// tags on the axis.
    fn readout(&self, plot: &Plot, slot: usize, span: &Span, theme: &Theme) -> Option<Div> {
        let candle = plot.slots[slot]?;
        let roomy = plot.width >= ROOMY;
        let narrow = plot.width < NARROW;
        let (inside, between) = match narrow {
            true => (design::space::HAIR, design::space::XXS),
            false => (px(5.), design::space::LG),
        };
        let pair = |key: &'static str, value: String, tone: Hsla| {
            div()
                .flex()
                .gap(inside)
                .child(key)
                .child(div().text_color(tone).child(value))
        };
        let average = |key: &'static str, averages: &[Option<f64>], tone: Hsla| {
            let average = averages.get(slot).copied().flatten().filter(|_| roomy)?;
            Some(pair(key, self.price(average.round() as u64), tone).text_color(tone))
        };
        let readout = div()
            .absolute()
            .left_0()
            .top_0()
            .pl(design::space::LG)
            .pt(design::space::SM)
            .flex()
            .gap(between)
            .font_family(design::fonts::FAMILY_MONO)
            .text_size(design::text::MONO)
            .text_color(theme.muted)
            .whitespace_nowrap()
            .when(narrow, |readout| {
                readout
                    .w(px(plot.width))
                    .flex_wrap()
                    .pl(design::space::SM)
                    .pt(design::space::XXS)
                    .text_size(design::text::CAPTION)
                    .line_height(px(NARROW_LINE))
            })
            .when(roomy, |readout| readout.child(stamp(&candle, span)))
            .child(pair("O", self.price(candle.open), theme.foreground))
            .child(pair("H", self.price(candle.high), theme.foreground))
            .child(pair("L", self.price(candle.low), theme.foreground))
            .child(pair("C", self.price(candle.close), theme.foreground))
            .child(pair("V", format::volume(candle.volume), theme.foreground))
            .children(average("MA7", &plot.fast, theme.muted))
            .children(average("MA25", &plot.slow, theme.warning));
        Some(readout)
    }

    /// The chart's canvas over `plot`: `rules` across it, the reader's
    /// `orders` and the `last` price on their lines.
    fn drawn(
        &self,
        plot: &Plot,
        rules: &[f32],
        orders: &[OrderLine],
        last: Option<(f32, Hsla)>,
        theme: &Theme,
    ) -> Canvas {
        let quad = |x: f32, y: f32, width: f32, height: f32| {
            Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
        };
        let chart = &self.chart;
        let mut drawn = canvas().size_full();
        for rule in rules {
            drawn = drawn.rect(quad(0., *rule, plot.width, 1.), theme.border);
        }
        if chart.volume {
            drawn = drawn.rect(quad(0., plot.prices, plot.width, 1.), theme.border);
        }
        drawn = drawn
            .rect(quad(plot.width, 0., 1., plot.height), theme.border)
            .rect(quad(0., plot.height, plot.width + PRICES, 1.), theme.border);
        // An odd width, so the 1 px wick is the body's middle.
        let body = ((plot.step * 0.7) as i32 | 1) as f32;
        let bars = plot.height - plot.prices - VOLUME_ROOM;
        for (slot, candle) in plot.slots.iter().enumerate() {
            let Some(candle) = candle else { continue };
            let color = rising(candle, theme);
            let middle = plot.middle(slot);
            let left = middle - (body - 1.) / 2.;
            if !chart.line {
                let (high, low) = (plot.y(candle.high as f64), plot.y(candle.low as f64));
                let top = plot.y(candle.open.max(candle.close) as f64);
                let bottom = plot.y(candle.open.min(candle.close) as f64);
                drawn = drawn
                    .rect(quad(middle, high, 1., (low - high).max(1.)), color)
                    .rect(quad(left, top, body, (bottom - top).max(1.)), color);
            }
            if chart.volume && candle.volume > 0 {
                let volume = (candle.volume as f64 / plot.peak as f64) as f32 * bars;
                let bar = quad(left, plot.height - volume, body, volume);
                drawn = drawn.rect(bar, color.opacity(0.4));
            }
        }
        if chart.line {
            let closes = plot.slots.iter();
            let closes = closes.map(|candle| candle.map(|candle| candle.close as f64));
            drawn = joined(drawn, plot, closes, theme.foreground);
        }
        drawn = joined(drawn, plot, plot.fast.iter().copied(), theme.muted);
        drawn = joined(drawn, plot, plot.slow.iter().copied(), theme.warning);
        for (y, side, ..) in orders {
            let line = quad(0., *y, plot.width, 1.);
            drawn = drawn.rect(line, sided(*side, theme).opacity(0.5));
        }
        match last {
            Some((y, color)) => drawn.rect(quad(0., y, plot.width, 1.), color),
            None => drawn,
        }
    }
}

/// The tags on the price axis, each where it covers no other: the last
/// price's (on row `last`) first, then the reader's orders' from the
/// nearest to it outwards. The rows tagged, and the orders that have a
/// tag; an order with no tag keeps its line.
fn tagged(last: Option<f32>, orders: &[OrderLine]) -> (Vec<f32>, Vec<&OrderLine>) {
    let mut tagged: Vec<f32> = last.into_iter().collect();
    let near = last.unwrap_or(0.);
    let mut tags: Vec<_> = orders.iter().collect();
    tags.sort_by(|(a, ..), (b, ..)| (a - near).abs().total_cmp(&(b - near).abs()));
    tags.retain(|(y, ..)| {
        let clear = tagged.iter().all(|tag| (tag - y).abs() >= TAG_H);
        if clear {
            tagged.push(*y);
        }
        clear
    });
    (tagged, tags)
}

/// The time axis: a time under every candle whose bucket is a multiple of
/// `every`, so the labels keep [`TIME_W`] apart, on the first span a
/// clock reads easily that is whole candles, and each stays under its
/// candle as the chart pans; and the `told` candle's time under it, kept
/// whole inside the plot, over the time it covers.
fn times(plot: &Plot, span: &Span, told: Option<usize>, theme: &Theme) -> Div {
    let minutes = span.millis / 60_000;
    let needed = (TIME_W / plot.step).ceil().max(1.) as u64 * minutes;
    let every = AXIS
        .into_iter()
        .find(|axis| *axis >= needed && axis.is_multiple_of(minutes))
        .unwrap_or(needed)
        / minutes;
    let edge = TOLD_W / 2.;
    let told_at = told.map(|slot| plot.middle(slot).clamp(edge, (plot.width - edge).max(edge)));
    let times = (0..plot.slots.len())
        .filter(|slot| (plot.first + *slot as u64).is_multiple_of(every))
        .map(|slot| (slot, plot.middle(slot)))
        .filter(|(_, middle)| *middle >= 30. && *middle <= plot.width - 30.)
        // the told time covers a label that is under it
        .filter(|(_, middle)| told_at.is_none_or(|told| (told - middle).abs() >= 64.))
        .map(|(slot, middle)| {
            label(
                format::axis((plot.first + slot as u64) * span.millis),
                theme,
            )
            .left(px(middle - TIME_W / 2.))
            .top(px(plot.height + 5.))
            .w(px(TIME_W))
            .text_center()
        });
    let told_time = told.zip(told_at).and_then(|(slot, middle)| {
        Some(
            label(stamp(&plot.slots[slot]?, span), theme)
                .left(px(middle - edge))
                .top(px(plot.height + 3.))
                .w(px(TOLD_W))
                .h(px(TAG_H))
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.foreground)
                .text_color(theme.background)
                .font_weight(FontWeight::MEDIUM),
        )
    });
    div()
        .absolute()
        .inset_0()
        .children(times)
        .children(told_time)
}

/// A candle's time: its day on a span of a day or more, else its day and
/// minute.
fn stamp(candle: &Candle, span: &Span) -> String {
    match span.millis >= 86_400_000 {
        true => format::day(candle.bucket * span.millis),
        false => format::day_time(candle.bucket * span.millis),
    }
}

/// A figure laid over the plot: mono, a caption, muted.
fn label(text: String, theme: &Theme) -> Div {
    design::mono(text)
        .absolute()
        .text_size(design::text::CAPTION)
        .text_color(theme.muted)
}

/// A tag on the price axis, on row `y` of `plot`: the caller colours it.
pub(crate) fn tag(text: String, plot: &Plot, y: f32, theme: &Theme) -> Div {
    label(text, theme)
        .left(px(plot.width))
        .top(px(y - TAG_H / 2.))
        .w(px(PRICES))
        .h(px(TAG_H))
        .pl(px(7.))
        .flex()
        .items_center()
        .border_1()
        .font_weight(FontWeight::MEDIUM)
}

/// The colour of a move: down in the fall's, up or level in the rise's,
/// and `none` with no move to tell.
pub(crate) fn moved(change: Option<i64>, none: Hsla, theme: &Theme) -> Hsla {
    match change {
        Some(change) if change < 0 => theme.danger,
        Some(_) => theme.success,
        None => none,
    }
}

/// The colour and the arrow of a last trade beside the one before it: up
/// in the rise's, down in the fall's, level in the foreground with none.
pub(crate) fn ticked(change: Option<Ordering>, theme: &Theme) -> (Hsla, &'static str) {
    match change {
        Some(Ordering::Greater) => (theme.success, "\u{2191}"),
        Some(Ordering::Less) => (theme.danger, "\u{2193}"),
        _ => (theme.foreground, ""),
    }
}

/// A candle that closed at or over its open is drawn as a rise.
pub(crate) fn rising(candle: &Candle, theme: &Theme) -> Hsla {
    if candle.close >= candle.open {
        theme.success
    } else {
        theme.danger
    }
}

/// The colour of a side: a buy is the rise's, a sell the fall's.
pub(crate) fn sided(side: Side, theme: &Theme) -> Hsla {
    match side {
        Side::Buy => theme.success,
        Side::Sell => theme.danger,
    }
}

/// A line through each slot's value, broken where a slot has none.
fn joined(
    mut drawn: Canvas,
    plot: &Plot,
    values: impl Iterator<Item = Option<f64>>,
    color: Hsla,
) -> Canvas {
    let mut from = None;
    for (slot, value) in values.enumerate() {
        let to = value.map(|value| point(px(plot.middle(slot) + 0.5), px(plot.y(value))));
        if let (Some(from), Some(to)) = (from, to) {
            drawn = drawn.line(from, to, px(1.25), color);
        }
        from = to;
    }
    drawn
}
