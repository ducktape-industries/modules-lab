//! What both designs draw alike, once: the colour of a side and of a
//! candle, and the chart's canvas, the quads and lines of the plot the
//! model lays out (`chart.rs`): the rules and the plot's edges, each
//! candle's wick, body and volume, the averages, the reader's orders'
//! lines and the last price's. Nothing on the canvas follows the pointer,
//! so a hover sends none of it again. A tree draws its labels over it.
use gpui::{point, size};
use trade::{Candle, Side};
use view::Canvas;
use view::prelude::*;

use crate::Trade;
use crate::chart::{PRICES, Plot, VOLUME_ROOM};

impl Trade {
    /// The chart's canvas over `plot`: `rules` across it, the reader's
    /// `orders` and the `last` price on their lines.
    pub(crate) fn drawn(
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
