//! The chart: the market's candles laid out by the stretch of time each one
//! is, their volume under them, two moving averages, the last price and
//! the reader's open orders on their lines.
//!
//! The view draws it itself. A `canvas()` carries quads and lines, which
//! the host paints natively and in order, and every label is an element
//! laid over it. A candle is one of its span's buckets of time; a bucket
//! nothing traded in has no row and is drawn flat at the close before it.
//! The chart's sensor tells where the host laid it out, so a pointer's
//! position in the window is a place on the plot: one listener finds the
//! candle under it. That candle's line, its time and the readout are
//! elements, drawn again when the pointer enters another slot; a move
//! inside one slot draws nothing, and the canvas is never sent for a hover.
use std::collections::BTreeMap;

use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Canvas, CursorStyle, FontWeight, Point, Task};
use gpui::{point, size};
use serde::{Deserialize, Serialize};
use trade::{Candle, MAX_ROWS, SPANS, Side};

use crate::{Trade, format};

/// The narrowest and the widest a candle's slot is zoomed to, in pixels:
/// a candle is at least the one column its wick is drawn in.
const NARROWEST: f32 = 1.;
const WIDEST: f32 = 40.;
/// The most candles on screen, whatever the zoom: three quads and two
/// lines a candle, with the rules and the orders' lines, stay inside the
/// 4096 commands a canvas carries. A week of 15m candles is 672.
const MOST: usize = 720;
/// The gutters the labels sit in: prices on the right, times underneath.
const PRICES: f32 = 64.;
const TIMES: f32 = 24.;
/// The room left over the highest price, which the readout's line is in,
/// and under the lowest.
const HEADROOM: f32 = 20.;
const FOOTROOM: f32 = 36.;
/// The share of the plot the volume pane takes, under the prices, the
/// most it grows to, and the room over its tallest bar.
const VOLUME: f32 = 0.2;
const VOLUME_MOST: f32 = 90.;
const VOLUME_ROOM: f32 = 12.;
/// Two price rules are no closer than this.
const RULED: f32 = 32.;
/// The smallest plot there is a chart in.
const SMALLEST: (f32, f32) = (48., 96.);
/// The closes each moving average is of.
const FAST: usize = 7;
const SLOW: usize = 25;
/// The open orders drawn on their lines, the newest first.
// ponytail: a cap, not a choice of the nearest; an account with more
// resting orders than this in one market wants them merged by price.
const ORDER_LINES: usize = 16;
/// A label's height on an axis: a rule's price under a tag is left out.
const TAG_H: f32 = 18.;
/// The strip along the plot's top that the readout is told in.
const READOUT: f32 = 28.;

/// The chart's settings, which a snapshot keeps, and what it holds while
/// it is on screen, which is read and measured again on every mount.
#[derive(Serialize, Deserialize)]
pub struct Chart {
    /// The candle span: its place in [`SPANS`].
    pub(crate) span: usize,
    /// The width of one candle's slot, in pixels: the zoom.
    pub(crate) candle: f32,
    pub(crate) ma7: bool,
    pub(crate) ma25: bool,
    pub(crate) volume: bool,
    /// The closes joined by a line, in place of candles.
    pub(crate) line: bool,
    /// How many buckets the right edge sits before the newest candle: the
    /// pan.
    #[serde(skip)]
    pub(crate) back: u64,
    /// Where the host last laid the chart out: its top left corner in the
    /// window's pixels, which a pointer is placed by, and its size.
    #[serde(skip)]
    origin: Point<Pixels>,
    #[serde(skip)]
    pub(crate) size: (f32, f32),
    /// The pointer while it is over the plot, in the window's pixels. It is
    /// the plot's: a frame drawn with no plot drops it.
    #[serde(skip)]
    pub(crate) pointer: Option<Point<Pixels>>,
    /// Where a drag was last accounted for, in the window's pixels.
    #[serde(skip)]
    grabbed: Option<f32>,
    #[serde(skip)]
    pub(crate) candles: Loadable<Candles>,
    /// The newest page read again while the candles on screen stay.
    #[serde(skip)]
    pub(crate) rereading: Option<Task<()>>,
    /// The page before the oldest candle held, on its way.
    #[serde(skip)]
    pub(crate) older: Option<Task<()>>,
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            span: 3,
            candle: 8.,
            ma7: true,
            ma25: true,
            volume: true,
            line: false,
            back: 0,
            origin: Point::default(),
            size: (0., 0.),
            pointer: None,
            grabbed: None,
            candles: Loadable::Idle,
            rereading: None,
            older: None,
        }
    }
}

/// The candles held of one market and span, by bucket.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Candles {
    rows: BTreeMap<u64, Candle>,
    /// The program holds no candle older than the oldest here.
    pub(crate) whole: bool,
}

impl Candles {
    /// The newest page, as the program answers it.
    pub(crate) fn newest(page: Vec<Candle>) -> Self {
        let mut candles = Self::default();
        candles.older(page);
        candles
    }

    /// Lays the newest page, read again, over the candles held, and says
    /// whether one moved.
    pub(crate) fn lay(&mut self, page: Vec<Candle>) -> bool {
        let mut moved = false;
        for candle in page {
            moved |= self.rows.insert(candle.bucket, candle) != Some(candle);
        }
        moved
    }

    /// Adds the page before the oldest candle held. A short page is the
    /// last one.
    pub(crate) fn older(&mut self, page: Vec<Candle>) {
        self.whole = page.len() < MAX_ROWS as usize;
        self.rows
            .extend(page.into_iter().map(|candle| (candle.bucket, candle)));
    }

    pub(crate) fn oldest(&self) -> Option<u64> {
        self.rows.keys().next().copied()
    }

    /// The bucket of the newest candle held.
    pub(crate) fn tip(&self) -> Option<u64> {
        self.rows.keys().next_back().copied()
    }

    fn last(&self) -> Option<&Candle> {
        self.rows.values().next_back()
    }
}

/// Where the chart's parts sit, in the canvas's pixels, for the buckets on
/// screen.
struct Plot {
    /// The plot, left of the price gutter and over the time gutter.
    width: f32,
    height: f32,
    /// The price pane's share of the height; the volume pane is under it.
    prices: f32,
    /// One candle's slot.
    step: f32,
    /// The bucket in the first slot.
    first: u64,
    /// What each slot draws: its bucket's candle, the close before it
    /// drawn flat where nothing traded, nothing before the first candle.
    slots: Vec<Option<Candle>>,
    /// The averages at each slot, of its close and the closes before it.
    fast: Vec<Option<f64>>,
    slow: Vec<Option<f64>>,
    low: f64,
    high: f64,
    /// The largest volume on screen.
    peak: u64,
}

impl Plot {
    fn y(&self, price: f64) -> f32 {
        let share = (self.high - price) / (self.high - self.low);
        HEADROOM + share as f32 * (self.prices - HEADROOM - FOOTROOM)
    }

    /// The left edge of a slot: the last one ends at the plot's edge.
    fn x(&self, slot: usize) -> f32 {
        self.width - (self.slots.len() - slot) as f32 * self.step
    }

    /// The column of pixels a slot's wick is drawn in.
    fn middle(&self, slot: usize) -> f32 {
        (self.x(slot) + self.step / 2.).floor()
    }

    /// Where `price` is drawn, when that is inside the price pane.
    fn line(&self, price: u64) -> Option<f32> {
        let y = self.y(price as f64).round();
        (y >= 0. && y < self.prices).then_some(y)
    }

    /// The prices the pane is ruled at, and the step between them: round
    /// numbers from the lowest price on screen to the highest, no closer
    /// than [`RULED`], no finer than the market's tick.
    fn rules(&self, tick: u64) -> (u64, impl Iterator<Item = u64> + use<>) {
        let a_pixel = (self.high - self.low) / f64::from(self.prices - HEADROOM - FOOTROOM);
        let step = round_step((a_pixel * f64::from(RULED)).max(tick as f64));
        let lowest = (self.low / step as f64).ceil() as u64;
        let highest = (self.high / step as f64).floor() as u64;
        (step, (lowest..=highest).map(move |rule| rule * step))
    }
}

/// The first of 1, 2 or 5 times a power of ten that is no smaller than
/// `least`.
fn round_step(least: f64) -> u64 {
    let mut unit = 1u64;
    loop {
        for times in [1, 2, 5] {
            let step = unit.saturating_mul(times);
            if step as f64 >= least || step == u64::MAX {
                return step;
            }
        }
        unit = unit.saturating_mul(10);
    }
}

impl Chart {
    /// The slots the plot holds at this zoom.
    fn slots(&self) -> usize {
        (((self.size.0 - PRICES) / self.candle) as usize).clamp(1, MOST)
    }

    /// The buckets in the first slot and in the last: `None` until the
    /// program has said a candle.
    fn buckets(&self) -> Option<(u64, u64)> {
        let last = self.candles.ready()?.last()?;
        let last = last.bucket.saturating_sub(self.back);
        Some((last.saturating_sub(self.slots() as u64 - 1), last))
    }

    /// How many slots `x` of the chart is from the plot's right edge, the
    /// last slot being the first: `None` off the plot.
    fn slots_to_edge(&self, x: f32) -> Option<u64> {
        let width = self.size.0 - PRICES;
        (0. ..width)
            .contains(&x)
            .then(|| ((width - x) / self.candle).ceil() as u64)
    }

    /// The slot under the pointer, counted from the left, when the pointer
    /// is over the plot and a candle is drawn there.
    fn hovered(&self) -> Option<usize> {
        let at = self.pointer? - self.origin;
        if !(0. ..self.size.1 - TIMES).contains(&f32::from(at.y)) {
            return None;
        }
        let (first, last) = self.buckets()?;
        let bucket = (last + 1).checked_sub(self.slots_to_edge(at.x.into())?)?;
        // nothing is drawn before the oldest candle held
        let drawn = bucket >= first.max(self.candles.ready()?.oldest()?);
        drawn.then(|| (bucket - first) as usize)
    }

    /// Puts the pointer at `at`, in the window's pixels, or takes it off
    /// the plot, and says whether another candle is under it now: a move
    /// inside one slot draws nothing.
    fn hover(&mut self, at: Option<Point<Pixels>>) -> bool {
        let before = self.hovered();
        self.pointer = at;
        before != self.hovered()
    }

    /// `None` until the host has measured a chart with room for a plot and
    /// the program has said a candle.
    fn plot(&self, tick: u64) -> Option<Plot> {
        let candles = self.candles.ready()?;
        let (width, height) = (self.size.0 - PRICES, self.size.1 - TIMES);
        if width < SMALLEST.0 || height < SMALLEST.1 {
            return None;
        }
        let (first, last) = self.buckets()?;
        // the slow average of the first slot reaches this far back
        let start = first.saturating_sub(SLOW as u64 - 1);
        let before = candles.rows.range(..start).next_back();
        let mut carried = before.map(|(_, candle)| candle.close);
        let mut rows = candles.rows.range(start..=last).peekable();
        let (mut slots, mut closes) = (Vec::new(), Vec::new());
        for bucket in start..=last {
            let row = rows.next_if(|(at, _)| **at == bucket).map(|(_, row)| *row);
            carried = row.map(|row| row.close).or(carried);
            closes.push(carried);
            if bucket >= first {
                let flat = carried.map(|close| Candle {
                    bucket,
                    open: close,
                    high: close,
                    low: close,
                    close,
                    volume: 0,
                });
                slots.push(row.or(flat));
            }
        }
        let lead = (first - start) as usize;
        let average = |of: usize| -> Vec<Option<f64>> {
            (lead + 1..=closes.len())
                .map(|end| {
                    let closes = closes.get(end.checked_sub(of)?..end)?.iter();
                    let sum: Option<f64> = closes.map(|close| close.map(|c| c as f64)).sum();
                    Some(sum? / of as f64)
                })
                .collect()
        };
        let fast = if self.ma7 { average(FAST) } else { Vec::new() };
        let slow = if self.ma25 { average(SLOW) } else { Vec::new() };
        let (mut low, mut high, mut peak) = (f64::MAX, f64::MIN, 1);
        for candle in slots.iter().flatten() {
            low = low.min(candle.low as f64);
            high = high.max(candle.high as f64);
            peak = peak.max(candle.volume);
        }
        for average in fast.iter().chain(&slow).flatten() {
            low = low.min(*average);
            high = high.max(*average);
        }
        if low > high {
            return None;
        }
        // a market that has not moved still has a height to be drawn in
        if high - low < tick as f64 {
            let room = tick as f64 * 5.;
            (low, high) = ((low - room).max(0.), high + room);
        }
        let volume = match self.volume {
            true => (height * VOLUME).min(VOLUME_MOST).floor(),
            false => 0.,
        };
        Some(Plot {
            width,
            height,
            prices: height - volume,
            step: self.candle,
            first,
            slots,
            fast,
            slow,
            low,
            high,
            peak,
        })
    }
}

impl Trade {
    /// Moves the chart's right edge `buckets` older, or newer below zero,
    /// as far as the candles held go. Where that leaves the chart is what
    /// asks for the page before them: with the oldest candle held in the
    /// first slot, or room left of it, whichever way the chart was asked
    /// to go. With no buckets to move, that is the page a screen with room
    /// left of the oldest candle is waiting for.
    pub(crate) fn pan(&mut self, buckets: i64, cx: &mut Context<Self>) {
        let chart = &mut self.chart;
        let Some(candles) = chart.candles.ready() else {
            return;
        };
        let (Some(oldest), Some(newest)) = (candles.oldest(), candles.last()) else {
            return;
        };
        let held = newest.bucket - oldest;
        let furthest = held.saturating_sub(chart.slots() as u64 - 1) as i64;
        let wanted = chart.back as i64 + buckets;
        chart.back = wanted.clamp(0, furthest) as u64;
        if chart.back as i64 == furthest {
            self.read_older(cx);
        }
    }

    /// Shows `factor` times the candles. About `about`, a place on the
    /// plot in the chart's pixels, the candle there stays there, as far as
    /// the candles held and the live edge let the chart pan; about none,
    /// the right edge stays.
    fn zoom(&mut self, factor: f32, about: Option<f32>, cx: &mut Context<Self>) {
        let chart = &mut self.chart;
        let narrowest = NARROWEST.max((chart.size.0 - PRICES) / MOST as f32);
        let before = about.and_then(|x| chart.slots_to_edge(x));
        chart.candle = (chart.candle / factor).clamp(narrowest, WIDEST.max(narrowest));
        // the candle is as many slots from the edge as before: the edge
        // moves by the slots that are now between them
        let after = about.and_then(|x| chart.slots_to_edge(x));
        let moved = before
            .zip(after)
            .map_or(0, |(before, after)| before as i64 - after as i64);
        self.pan(moved, cx);
    }

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
    pub(crate) fn chart(
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
            (Loadable::Ready(candles), None) if candles.rows.is_empty() => {
                said("No trades yet".into())
            }
            // nothing to draw on until the candles and the chart's size land
            _ => div().size_full().into_any_element(),
        };
        // The chart is drawn to its size and the pointer is placed by its
        // origin: a chart that only moved is drawn again when another
        // candle is under the pointer for it, and not otherwise.
        let measured = cx.listener(|view: &mut Trade, bounds: &Bounds<Pixels>, _, cx| {
            let chart = &mut view.chart;
            let size = (bounds.size.width.into(), bounds.size.height.into());
            let hovered = chart.hovered();
            chart.origin = bounds.origin;
            if std::mem::replace(&mut chart.size, size) != size || hovered != chart.hovered() {
                cx.notify();
            }
        });
        sensor("trade-chart", chart)
            .flex_1()
            .min_h_0()
            .on_bounds(measured)
    }

    /// The reader's resting orders in this market whose price is on
    /// screen: where each line is, its side and what is left of it.
    fn order_lines(&self, plot: &Plot) -> Vec<(f32, Side, u64, u64)> {
        let orders = self
            .open
            .ready()
            .map(|orders| &orders[..])
            .unwrap_or_default();
        let mine = orders
            .iter()
            .filter(|order| order.market as usize == self.market);
        mine.filter_map(|order| {
            let limit = order.limit?;
            let left = order.amount.saturating_sub(order.filled);
            Some((plot.line(limit)?, order.side, left, limit))
        })
        .take(ORDER_LINES)
        .collect()
    }

    /// The quads and lines of the chart: the rules, the plot's edges, each
    /// candle's wick, body and volume, the averages, the orders' lines and
    /// the last price's. Nothing here follows the pointer, so a hover
    /// sends none of it again.
    fn drawn(
        &self,
        plot: &Plot,
        rules: &[f32],
        orders: &[(f32, Side, u64, u64)],
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

    /// The plot: the canvas, the line on the candle under the pointer, and
    /// laid over them a price beside each rule, the orders and the last
    /// price on their lines, the times and the readout.
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
        // a tag covers a rule's price that is under it
        let clear = |y: &f32| tagged.iter().all(|tag| (tag - y).abs() > TAG_H - 4.);
        let places = format::decimals(step).min(format::decimals(market.tick));
        let prices = rules.iter().filter(|(_, y)| clear(y)).map(|(rule, y)| {
            label(format::fixed(*rule, places))
                .left(px(plot.width + 8.))
                .top(px(y - 7.))
        });
        let order_tags = tags.iter().flat_map(|(y, side, left, limit)| {
            let color = sided(*side, &theme);
            let word = match side {
                Side::Buy => "Buy",
                Side::Sell => "Sell",
            };
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
            // candle under the pointer is the one told.
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
            .children(prices)
            .children(order_tags)
            .children(last_tag)
            .child(times)
            .children(readout)
    }
}

/// The minutes between two time labels: what a clock or a calendar reads
/// easily, up to a month.
const AXIS: [u64; 13] = [
    1, 5, 15, 30, 60, 120, 240, 720, 1440, 2880, 10_080, 20_160, 43_200,
];

/// A candle that closed at or over its open is drawn as a rise.
fn rising(candle: &Candle, theme: &Theme) -> Hsla {
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
