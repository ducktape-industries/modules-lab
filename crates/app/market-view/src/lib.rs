//! Market: a candlestick chart over a made-up market. It answers one
//! question: can a trading view be written against the view SDK as it is.
//!
//! The view draws the chart itself. A `canvas()` carries quads the host
//! paints in order, and the labels are elements laid over it. A candle is
//! drawn as gpui-kit's `CandlestickChart` paints one (a 1 px quad for the
//! wick, a quad for the body); that chart paints into the host's window,
//! which a view does not hold, so its layout is repeated here, not called.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Canvas, CursorStyle};
use gpui::{Bounds, point, size};
use serde::{Deserialize, Serialize};

mod series;
use series::{Candle, FRAMES, Frame, SYMBOLS, Series};

/// The fewest and the most candles a zoom shows. The most keeps a frame's
/// quads, three a candle, well inside the 4096 the wire takes.
const FEWEST: usize = 20;
const MOST: usize = 240;
/// The gutters the labels sit in: prices on the right, times underneath.
const PRICES: f32 = 76.;
const TIMES: f32 = 24.;
/// The room left over the highest price, as gpui-kit's chart leaves.
const HEADROOM: f32 = 10.;
/// The share of the plot the volume bars take, under the prices.
const VOLUME: f32 = 0.2;
/// The price pane is ruled into this many bands.
const BANDS: usize = 4;

#[derive(Serialize, Deserialize)]
pub struct Market {
    symbol: usize,
    frame: usize,
    /// The candles on screen: the zoom.
    visible: usize,
    /// How many candles the right edge sits behind the live one: the pan.
    back: usize,
    // Never saved: a series is made again from its seed on every mount.
    #[serde(skip)]
    series: Series,
    /// The chart's size, as the host last measured it.
    #[serde(skip)]
    size: (f32, f32),
    /// The slot under the pointer, counted from the left.
    #[serde(skip)]
    hovered: Option<usize>,
    /// Where a drag was last accounted for, in the window's pixels.
    #[serde(skip)]
    grabbed: Option<f32>,
}

impl Default for Market {
    fn default() -> Self {
        Self {
            symbol: 0,
            frame: 2,
            visible: 120,
            back: 0,
            series: Series::default(),
            size: (0., 0.),
            hovered: None,
            grabbed: None,
        }
    }
}

impl View for Market {
    const NAME: &'static str = "Market";
    const DESCRIPTION: &'static str = "A candlestick chart over a made-up market.";
    const ICON: &'static str = "icons/chart-candlestick.svg";
    // `clock.ticks` moves the market; `host.log` says why it stopped.
    const CAPABILITIES: &'static [Capability] = &[Capability::Clock, Capability::Host];

    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.series = Series::new(&SYMBOLS[self.symbol], &FRAMES[self.frame]);
        cx.follow::<ClockTicks>(1000, |view, tick, cx| match tick {
            Ok(()) => {
                view.series.tick();
                cx.notify();
            }
            Err(refusal) => cx.log_refused("the clock", &refusal),
        })
        .detach();
    }
}

/// Where the chart's parts sit, in the canvas's pixels, for the candles on
/// screen.
struct Plot<'a> {
    candles: &'a [Candle],
    /// Where the first candle on screen is in the series.
    first: usize,
    /// The plot, left of the price gutter and over the time gutter.
    width: f32,
    height: f32,
    /// The price pane's share of the height; the volume pane is under it.
    prices: f32,
    /// One candle's slot.
    step: f32,
    low: f64,
    high: f64,
    /// The largest volume on screen.
    peak: f64,
}

impl Plot<'_> {
    fn y(&self, price: f64) -> f32 {
        let span = (self.high - self.low).max(f64::MIN_POSITIVE);
        HEADROOM + ((self.high - price) / span) as f32 * (self.prices - HEADROOM)
    }

    /// The left edge of a slot: the last candle ends at the plot's edge.
    fn x(&self, slot: usize) -> f32 {
        self.width - (self.candles.len() - slot) as f32 * self.step
    }

    /// The price on rule `rule` of the price pane, the highest first.
    fn ruled(&self, rule: usize) -> (f32, f64) {
        let share = rule as f64 / BANDS as f64;
        let price = self.high - (self.high - self.low) * share;
        (self.y(price).round(), price)
    }
}

impl Market {
    /// `None` until the host has measured a chart with room for a plot.
    fn plot(&self) -> Option<Plot<'_>> {
        let (width, height) = (self.size.0 - PRICES, self.size.1 - TIMES);
        let end = self.series.candles.len().checked_sub(self.back)?;
        let first = end.saturating_sub(self.visible);
        let candles = &self.series.candles[first..end];
        if candles.is_empty() || width < 1. || height < 1. {
            return None;
        }
        let (mut low, mut high, mut peak) = (f64::MAX, f64::MIN, f64::MIN_POSITIVE);
        for candle in candles {
            low = low.min(candle.low);
            high = high.max(candle.high);
            peak = peak.max(candle.volume);
        }
        Some(Plot {
            candles,
            first,
            width,
            height,
            prices: height * (1. - VOLUME),
            step: width / self.visible as f32,
            low,
            high,
            peak,
        })
    }

    /// Moves the right edge `candles` older, or newer below zero, as far as
    /// the series goes.
    fn pan(&mut self, candles: i64) {
        let oldest = self.series.candles.len().saturating_sub(self.visible) as i64;
        self.back = (self.back as i64 + candles).clamp(0, oldest) as usize;
    }

    /// Shows `factor` times the candles, by at least one, so a trackpad's
    /// small deltas still move a tight zoom.
    fn zoom(&mut self, factor: f32) {
        let scaled = self.visible as f32 * factor;
        let next = if factor < 1. {
            scaled.floor()
        } else {
            scaled.ceil()
        };
        self.visible = (next as usize).clamp(FEWEST, MOST);
        self.pan(0);
    }

    fn pick(&mut self, symbol: usize, frame: usize) {
        self.symbol = symbol;
        self.frame = frame;
        self.series = Series::new(&SYMBOLS[symbol], &FRAMES[frame]);
        self.back = 0;
        self.hovered = None;
    }

    // The wheel zooms about the right edge. A pointer's position arrives in
    // the window's pixels and a view is not told where its elements sit in
    // the window, so the zoom cannot be about the pointer.
    fn wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let lines = f32::from(event.delta.pixel_delta(px(16.)).y) / 16.;
        if lines != 0. {
            self.zoom(0.9_f32.powf(lines));
            cx.notify();
        }
    }

    // A drag pans by whole candles. Only the distance between two positions
    // is read, which the window's pixels give as well as the chart's would.
    fn drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let (Some(from), Some(step)) = (self.grabbed, self.plot().map(|plot| plot.step)) else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.grabbed = None;
            return;
        }
        let candles = ((f32::from(event.position.x) - from) / step).trunc();
        if candles != 0. {
            // What is left of the distance stays with the grab, so a slow
            // drag adds up.
            self.grabbed = Some(from + candles * step);
            self.pan(candles as i64);
            cx.notify();
        }
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let page = (self.visible / 10).max(1) as i64;
        match event.keystroke.key.as_str() {
            "left" => self.pan(page),
            "right" => self.pan(-page),
            "up" => self.zoom(0.8),
            "down" => self.zoom(1.25),
            _ => return,
        }
        cx.notify();
    }

    // The pointer may enter the next slot before it leaves this one.
    fn hover(&mut self, slot: usize, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.hovered = Some(slot);
        } else if self.hovered == Some(slot) {
            self.hovered = None;
        } else {
            return;
        }
        cx.notify();
    }

    /// The quads of the chart: the rules, the plot's edges, then each
    /// candle's wick, body and volume, and the last price. Nothing here
    /// follows the pointer, so a hover sends none of it again.
    fn drawn(&self, plot: &Plot<'_>, theme: &Theme) -> Canvas {
        let quad = |x: f32, y: f32, width: f32, height: f32| {
            Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
        };
        let mut drawn = canvas().size_full();
        for rule in 0..=BANDS {
            drawn = drawn.rect(quad(0., plot.ruled(rule).0, plot.width, 1.), theme.border);
        }
        drawn = drawn
            .rect(quad(plot.width, 0., 1., plot.height), theme.border_strong)
            .rect(
                quad(0., plot.height, plot.width + 1., 1.),
                theme.border_strong,
            );
        // An odd width, so the 1 px wick is the body's middle.
        let body = ((plot.step * 0.7) as i32 | 1) as f32;
        for (slot, candle) in plot.candles.iter().enumerate() {
            let color = rising(candle, theme);
            let middle = (plot.x(slot) + plot.step / 2.).floor();
            let left = middle - (body - 1.) / 2.;
            let (high, low) = (plot.y(candle.high), plot.y(candle.low));
            let top = plot.y(candle.open.max(candle.close));
            let bottom = plot.y(candle.open.min(candle.close));
            let volume = (candle.volume / plot.peak) as f32 * (plot.height - plot.prices);
            drawn = drawn
                .rect(quad(middle, high, 1., (low - high).max(1.)), color)
                .rect(quad(left, top, body, (bottom - top).max(1.)), color)
                .rect(
                    quad(left, plot.height - volume, body, volume),
                    color.opacity(0.4),
                );
        }
        match plot.candles.last() {
            Some(last) => drawn.rect(
                quad(0., plot.y(last.close).round(), plot.width, 1.),
                rising(last, theme).opacity(0.6),
            ),
            None => drawn,
        }
    }
}

/// A candle that closed at or over its open is drawn as a rise.
fn rising(candle: &Candle, theme: &Theme) -> Hsla {
    if candle.close >= candle.open {
        theme.success
    } else {
        theme.danger
    }
}

/// A price as a person reads it: thousands grouped, and the decimals its
/// size calls for.
fn price(value: f64) -> String {
    let decimals = match value {
        v if v >= 100. => 2,
        v if v >= 1. => 4,
        _ => 6,
    };
    let fixed = format!("{value:.decimals$}");
    let (whole, fraction) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let mut grouped = String::new();
    for (place, digit) in whole.chars().enumerate() {
        if place > 0 && (whole.len() - place).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("{grouped}.{fraction}")
}

/// The minutes between two time labels: what a clock or a calendar reads
/// easily, up to a month.
const SPANS: [u64; 13] = [
    1, 5, 15, 30, 60, 120, 240, 720, 1440, 2880, 10_080, 20_160, 43_200,
];

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A candle's time on the axis: the day where a day starts, the time of
/// day inside one.
fn axis_time(index: usize, frame: &Frame) -> String {
    let (_, month, day, hour, minute) = series::opened(index, frame);
    if hour == 0 && minute == 0 {
        format!("{} {day}", MONTHS[month as usize - 1])
    } else {
        format!("{hour:02}:{minute:02}")
    }
}

/// One candle in words: when it opened, its four prices and its volume.
fn told(index: usize, candle: &Candle, frame: &Frame) -> String {
    let (year, month, day, hour, minute) = series::opened(index, frame);
    format!(
        "{year}-{month:02}-{day:02} {hour:02}:{minute:02}   O {}   H {}   L {}   C {}   V {:.0}",
        price(candle.open),
        price(candle.high),
        price(candle.low),
        price(candle.close),
        candle.volume,
    )
}

impl Render for Market {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.global::<Theme>();
        let frame = &FRAMES[self.frame];
        let symbols = design::segmented(
            "market-symbols",
            "Symbol",
            &theme,
            self.symbol,
            SYMBOLS.iter().map(|s| (s.name.into(), s.name.into())),
            cx.processor(|view: &mut Market, picked: usize, _, cx| {
                view.pick(picked, view.frame);
                cx.notify();
            }),
        );
        let frames = design::segmented(
            "market-frames",
            "Candle span",
            &theme,
            self.frame,
            FRAMES.iter().map(|f| (f.name.into(), f.name.into())),
            cx.processor(|view: &mut Market, picked: usize, _, cx| {
                view.pick(view.symbol, picked);
                cx.notify();
            }),
        );
        let plot = self.plot();
        // The last price, and how far it is from the first open on screen.
        let quote = plot.as_ref().and_then(|plot| {
            let (first, last) = (plot.candles.first()?, plot.candles.last()?);
            let change = (last.close / first.open - 1.) * 100.;
            let color = if change >= 0. {
                theme.success
            } else {
                theme.danger
            };
            Some(
                div()
                    .flex()
                    .items_baseline()
                    .gap(design::space::SM)
                    .child(
                        div()
                            .text_size(design::text::TITLE)
                            .child(price(last.close)),
                    )
                    .child(div().text_color(color).child(format!("{change:+.2}%"))),
            )
        });
        // The candle under the pointer, or the last one on screen.
        let told = plot.as_ref().and_then(|plot| {
            let slot = self
                .hovered
                .filter(|slot| *slot < plot.candles.len())
                .or(plot.candles.len().checked_sub(1))?;
            Some(told(plot.first + slot, &plot.candles[slot], frame))
        });
        let chart = match &plot {
            // Nothing to draw on until the host has measured the chart.
            None => div().size_full().into_any_element(),
            Some(plot) => self.chart(plot, &theme, cx).into_any_element(),
        };
        // The host tells a first sight of the chart as a show and every
        // later size as a resize; the chart is drawn from either.
        let measured = || {
            cx.listener(|view: &mut Market, size: &(Pixels, Pixels), _, cx| {
                view.size = (f32::from(size.0), f32::from(size.1));
                cx.notify();
            })
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(design::space::SM)
            .p(design::space::LG)
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(design::text::BODY)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(design::space::MD)
                    .child(symbols)
                    .child(frames)
                    .child(div().flex_1())
                    .children(quote),
            )
            .child(
                design::mono(told.unwrap_or_default())
                    .h(design::size::ROW)
                    .text_color(theme.muted),
            )
            .child(
                sensor("market-chart", chart)
                    .flex_1()
                    .min_h_0()
                    .on_show(measured())
                    .on_resize(measured()),
            )
    }
}

impl Market {
    /// The plot: a slot per candle that hears the pointer, the canvas over
    /// them, and laid over that a price beside each rule, the last price on
    /// its line and the times.
    fn chart(
        &self,
        plot: &Plot<'_>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let frame = &FRAMES[self.frame];
        let label = |text: String| {
            design::mono(text)
                .absolute()
                .text_size(design::text::CAPTION)
                .text_color(theme.muted)
        };
        // The last price's tag covers a rule's price that is under it.
        let tagged = plot.candles.last().map(|last| plot.y(last.close).round());
        let prices = (0..=BANDS)
            .map(|rule| plot.ruled(rule))
            .filter(|(y, _)| tagged.is_none_or(|tag| (tag - y).abs() > 14.))
            .map(|(y, at)| label(price(at)).left(px(plot.width + 6.)).top(px(y - 7.)));
        let last = plot.candles.last().map(|last| {
            label(price(last.close))
                .left(px(plot.width + 1.))
                .top(px(plot.y(last.close).round() - 8.))
                .w(px(PRICES - 1.))
                .h(px(16.))
                .pl(px(5.))
                .bg(rising(last, theme))
                .text_color(theme.background)
        });
        // A time under every candle whose place in the series is a
        // multiple of `every`: the labels keep 80 px apart, on the first
        // span a clock reads easily that is whole candles, and each stays
        // under its candle as the chart pans.
        let needed = (80. / plot.step).ceil().max(1.) as u64 * frame.minutes;
        let every = SPANS
            .into_iter()
            .find(|span| *span >= needed && span.is_multiple_of(frame.minutes))
            .unwrap_or(needed)
            / frame.minutes;
        let times = (0..plot.candles.len())
            .filter(|slot| ((plot.first + slot) as u64).is_multiple_of(every))
            .map(|slot| (slot, plot.x(slot) + plot.step / 2.))
            .filter(|(_, middle)| *middle >= 30. && *middle <= plot.width - 30.)
            .map(|(slot, middle)| {
                label(axis_time(plot.first + slot, frame))
                    .left(px(middle - 40.))
                    .top(px(plot.height + 6.))
                    .w(px(80.))
                    .text_center()
            });
        // A slot per candle, under the canvas. The band on the hovered one
        // is the slot's own hover style: the host paints it as the pointer
        // moves, with no tick of the view. The listener only moves the
        // readout, which is one text the view sends again.
        let band = theme.hover;
        let slots = (0..plot.candles.len()).map(|slot| {
            div()
                .id(format!("candle-{slot}"))
                .absolute()
                .left(px(plot.x(slot)))
                .top_0()
                .w(px(plot.step))
                .h(px(plot.height))
                .hover(|style| style.bg(band))
                .on_hover(
                    cx.listener(move |view: &mut Market, hovered: &bool, _, cx| {
                        view.hover(slot, *hovered, cx)
                    }),
                )
        });
        let named = format!(
            "{} {} candles, {} on screen, last {}",
            SYMBOLS[self.symbol].name,
            frame.name,
            plot.candles.len(),
            plot.candles
                .last()
                .map(|c| price(c.close))
                .unwrap_or_default(),
        );
        div()
            .id("market-plot")
            .relative()
            .size_full()
            .overflow_hidden()
            .cursor(CursorStyle::Crosshair)
            // Arrow keys pan and zoom, for a reader without a pointer.
            .role(Role::Figure)
            .aria_label(named)
            .focusable()
            .on_key_down(
                cx.listener(|view: &mut Market, event: &KeyDownEvent, _, cx| view.key(event, cx)),
            )
            .on_scroll_wheel(
                cx.listener(|view: &mut Market, event: &ScrollWheelEvent, _, cx| {
                    view.wheel(event, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view: &mut Market, event: &MouseDownEvent, _, _| {
                    view.grabbed = Some(f32::from(event.position.x));
                }),
            )
            .on_mouse_move(
                cx.listener(|view: &mut Market, event: &MouseMoveEvent, _, cx| {
                    view.drag(event, cx)
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view: &mut Market, _: &MouseUpEvent, _, _| view.grabbed = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view: &mut Market, _: &MouseUpEvent, _, _| view.grabbed = None),
            )
            .children(slots)
            .child(self.drawn(plot, theme))
            .children(prices)
            .children(last)
            .children(times)
    }
}

// Writes the wasm exports the host calls and the manifest it reads.
export_view!(Market);

#[cfg(test)]
mod tests;
