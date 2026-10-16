//! The chart's model: the market's candles held of one span, by bucket,
//! and where each part of the plot sits at the zoom and the pan on screen.
//! A candle is one of its span's buckets of time; a bucket nothing traded
//! in has no row and is drawn flat at the close before it. [`Plot`] lays
//! the buckets on screen out: each slot, the price scale and its rules,
//! the volume pane under it, the two moving averages. The model also
//! finds the candle and the row under a pointer, the candle a finger
//! holds, and the reader's resting orders on their lines, and moves the
//! pan and the zoom for a wheel, a drag, a swipe or a pinch.
//!
//! The trees draw from it: the desktop's `ui/chart.rs` and the phone's
//! `ui/phone/chart.rs`, each over the canvas and the labels that
//! `look.rs` lays for both alike.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use trade::{Candle, MAX_ROWS, Side};
use view::prelude::*;
use view::{Point, Task};

use crate::{Trade, words};

/// A resting order on its line: the row it is drawn on, its side, what is
/// left of it and its limit.
pub(crate) type OrderLine = (f32, Side, u64, u64);

/// The least one pinch event shrinks what is drawn to: a delta under -1
/// is no size at all.
const PINCH_FLOOR: f32 = 0.1;
/// The narrowest and the widest a candle's slot is zoomed to, in pixels:
/// a candle is at least the one column its wick is drawn in.
const NARROWEST: f32 = 1.;
const WIDEST: f32 = 40.;
/// The most candles on screen, whatever the zoom: three quads and two
/// lines a candle, with the rules and the orders' lines, stay inside the
/// 4096 commands a canvas carries. A week of 15m candles is 672.
const MOST: usize = 720;
/// The gutters the labels sit in: prices on the right, times underneath.
pub(crate) const PRICES: f32 = 64.;
const TIMES: f32 = 24.;
/// The room left over the highest price, which the readout's line is in,
/// and under the lowest.
const HEADROOM: f32 = 20.;
const FOOTROOM: f32 = 36.;
/// The share of the plot the volume pane takes, under the prices, the
/// most it grows to, and the room over its tallest bar.
const VOLUME: f32 = 0.2;
const VOLUME_MOST: f32 = 90.;
pub(crate) const VOLUME_ROOM: f32 = 12.;
/// Two price rules are no closer than this.
const RULED: f32 = 32.;
/// The smallest plot there is a chart in.
const SMALLEST: (f32, f32) = (48., 96.);
/// The closes each moving average is of.
const FAST: usize = 7;
const SLOW: usize = 25;
/// What the chart draws besides its candles, as a list of switches names
/// them: the two averages, the volume, and the closes joined by a line.
pub(crate) const SWITCHES: [&str; 4] = [words::MA7, words::MA25, words::VOLUME, words::LINE];
/// The open orders drawn on their lines, the newest first.
// ponytail: a cap, not a choice of the nearest; an account with more
// resting orders than this in one market wants them merged by price.
const ORDER_LINES: usize = 16;
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
    pub(crate) origin: Point<Pixels>,
    #[serde(skip)]
    pub(crate) size: (f32, f32),
    /// The pointer while it is over the plot, in the window's pixels. It is
    /// the plot's: a frame drawn with no plot drops it.
    #[serde(skip)]
    pub(crate) pointer: Option<Point<Pixels>>,
    /// Where a drag was last accounted for, in the window's pixels.
    #[serde(skip)]
    pub(crate) grabbed: Option<f32>,
    /// The bucket a finger's tap holds told, in place of a pointer's
    /// candle: it stays on its candle as the chart pans.
    #[serde(skip)]
    pub(crate) held: Option<u64>,
    /// What a finger's pan moved short of a whole candle, carried to the
    /// next, so a slow pan adds up as a slow drag does.
    #[serde(skip)]
    pub(crate) pan_rest: f32,
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
            held: None,
            pan_rest: 0.,
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

    /// Whether the program holds no candle of the market and span.
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The bucket of the newest candle held.
    pub(crate) fn tip(&self) -> Option<u64> {
        self.rows.keys().next_back().copied()
    }

    pub(crate) fn last(&self) -> Option<&Candle> {
        self.rows.values().next_back()
    }
}

/// Where the chart's parts sit, in the canvas's pixels, for the buckets on
/// screen.
pub(crate) struct Plot {
    /// The plot, left of the price gutter and over the time gutter.
    pub(crate) width: f32,
    pub(crate) height: f32,
    /// The price pane's share of the height; the volume pane is under it.
    pub(crate) prices: f32,
    /// One candle's slot.
    pub(crate) step: f32,
    /// The bucket in the first slot.
    pub(crate) first: u64,
    /// What each slot draws: its bucket's candle, the close before it
    /// drawn flat where nothing traded, nothing before the first candle.
    pub(crate) slots: Vec<Option<Candle>>,
    /// The averages at each slot, of its close and the closes before it.
    pub(crate) fast: Vec<Option<f64>>,
    pub(crate) slow: Vec<Option<f64>>,
    pub(crate) low: f64,
    pub(crate) high: f64,
    /// The largest volume on screen.
    pub(crate) peak: u64,
}

impl Plot {
    pub(crate) fn y(&self, price: f64) -> f32 {
        let share = (self.high - price) / (self.high - self.low);
        HEADROOM + share as f32 * (self.prices - HEADROOM - FOOTROOM)
    }

    /// The price drawn at `y`: [`Plot::y`] the other way.
    pub(crate) fn price(&self, y: f32) -> f64 {
        let share = f64::from(y - HEADROOM) / f64::from(self.prices - HEADROOM - FOOTROOM);
        self.high - share * (self.high - self.low)
    }

    /// The left edge of a slot: the last one ends at the plot's edge.
    fn x(&self, slot: usize) -> f32 {
        self.width - (self.slots.len() - slot) as f32 * self.step
    }

    /// The column of pixels a slot's wick is drawn in.
    pub(crate) fn middle(&self, slot: usize) -> f32 {
        (self.x(slot) + self.step / 2.).floor()
    }

    /// Where `price` is drawn, when that is inside the price pane.
    pub(crate) fn line(&self, price: u64) -> Option<f32> {
        let y = self.y(price as f64).round();
        (y >= 0. && y < self.prices).then_some(y)
    }

    /// The prices the pane is ruled at, and the step between them: round
    /// numbers from the lowest price on screen to the highest, no closer
    /// than [`RULED`], no finer than the market's tick.
    pub(crate) fn rules(&self, tick: u64) -> (u64, impl Iterator<Item = u64> + use<>) {
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
    pub(crate) fn slots(&self) -> usize {
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
    pub(crate) fn hovered(&self) -> Option<usize> {
        self.slot_of(self.candle_at(self.pointer?)?)
    }

    /// The bucket of the candle drawn at `at`, in the window's pixels:
    /// `None` off the plot, or where nothing is drawn.
    fn candle_at(&self, at: Point<Pixels>) -> Option<u64> {
        let at = at - self.origin;
        if !(0. ..self.size.1 - TIMES).contains(&f32::from(at.y)) {
            return None;
        }
        let (_, last) = self.buckets()?;
        let bucket = (last + 1).checked_sub(self.slots_to_edge(at.x.into())?)?;
        self.slot_of(bucket).map(|_| bucket)
    }

    /// The slot `bucket` is drawn in, counted from the left: `None` off
    /// screen, and before the oldest candle held, where nothing is drawn.
    fn slot_of(&self, bucket: u64) -> Option<usize> {
        let (first, last) = self.buckets()?;
        let drawn = bucket >= first.max(self.candles.ready()?.oldest()?) && bucket <= last;
        drawn.then(|| (bucket - first) as usize)
    }

    /// The slot of the candle a finger holds, while it is on screen.
    pub(crate) fn held_slot(&self) -> Option<usize> {
        self.slot_of(self.held?)
    }

    /// A finger's tap at `at`, in the window's pixels: the candle under it
    /// is held, until a second tap on it or a tap off the candles.
    pub(crate) fn hold(&mut self, at: Point<Pixels>) {
        self.held = match self.candle_at(at) {
            Some(bucket) if self.held != Some(bucket) => Some(bucket),
            _ => None,
        };
    }

    /// The host laid the chart out at `bounds`: whether that draws
    /// anything else, a new size or the pointer on another candle or row.
    pub(crate) fn lay(&mut self, bounds: &Bounds<Pixels>) -> bool {
        let size = (bounds.size.width.into(), bounds.size.height.into());
        let pointed = self.pointed();
        self.origin = bounds.origin;
        std::mem::replace(&mut self.size, size) != size || pointed != self.pointed()
    }

    /// Whether the chart draws each of what it draws besides its candles,
    /// in the order a list of them names them ([`SWITCHES`]).
    pub(crate) fn switches(&self) -> [bool; 4] {
        [self.ma7, self.ma25, self.volume, self.line]
    }

    /// Switches the `index`th of [`SWITCHES`] on or off.
    pub(crate) fn switch(&mut self, index: usize) {
        let switch = [
            &mut self.ma7,
            &mut self.ma25,
            &mut self.volume,
            &mut self.line,
        ];
        if let Some(on) = switch.into_iter().nth(index) {
            *on = !*on;
        }
    }

    /// The price pane's height: the plot's, less the volume pane under it.
    fn prices(&self) -> f32 {
        let height = self.size.1 - TIMES;
        match self.volume {
            true => height - (height * VOLUME).min(VOLUME_MOST).floor(),
            false => height,
        }
    }

    /// The row of pixels the crosshair's line is in, from the plot's top:
    /// the pointer's, while it is over the price pane. Over the volume pane
    /// or an axis there is none.
    pub(crate) fn crosshair(&self) -> Option<f32> {
        let at = self.pointer? - self.origin;
        let (x, y) = (f32::from(at.x), f32::from(at.y));
        let over = (0. ..self.size.0 - PRICES).contains(&x) && (0. ..self.prices()).contains(&y);
        over.then(|| y.floor())
    }

    /// What the pointer draws: the candle under it and the crosshair's row.
    pub(crate) fn pointed(&self) -> (Option<usize>, Option<f32>) {
        (self.hovered(), self.crosshair())
    }

    /// Puts the pointer at `at`, in the window's pixels, or takes it off
    /// the plot, and says whether it draws anything else now: a move that
    /// stays in one slot and one row draws nothing.
    pub(crate) fn hover(&mut self, at: Option<Point<Pixels>>) -> bool {
        let before = self.pointed();
        self.pointer = at;
        before != self.pointed()
    }

    /// `None` until the host has measured a chart with room for a plot and
    /// the program has said a candle.
    pub(crate) fn plot(&self, tick: u64) -> Option<Plot> {
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
        Some(Plot {
            width,
            height,
            prices: self.prices(),
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

    /// A finger's pan, `dx` pixels across: the chart follows the finger,
    /// whole candles at a time, the rest carried to the next.
    pub(crate) fn swipe(&mut self, dx: f32, cx: &mut Context<Self>) {
        let chart = &mut self.chart;
        let moved = chart.pan_rest + dx;
        let candles = (moved / chart.candle).trunc();
        chart.pan_rest = moved - candles * chart.candle;
        if candles != 0. {
            self.pan(candles as i64, cx);
            cx.notify();
        }
    }

    /// A pinch about `at`, in the window's pixels, growing what is drawn
    /// by `delta` (0.1 a tenth bigger): the candle under it stays there.
    pub(crate) fn pinch(&mut self, delta: f32, at: Point<Pixels>, cx: &mut Context<Self>) {
        let grown = (1. + delta).max(PINCH_FLOOR);
        let about = f32::from(at.x - self.chart.origin.x);
        self.zoom(1. / grown, Some(about), cx);
        cx.notify();
    }

    /// Shows `factor` times the candles. About `about`, a place on the
    /// plot in the chart's pixels, the candle there stays there, as far as
    /// the candles held and the live edge let the chart pan; about none,
    /// the right edge stays.
    pub(crate) fn zoom(&mut self, factor: f32, about: Option<f32>, cx: &mut Context<Self>) {
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

    /// The reader's resting orders in this market whose price is on
    /// screen: where each line is, its side and what is left of it.
    pub(crate) fn order_lines(&self, plot: &Plot) -> Vec<OrderLine> {
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
}
