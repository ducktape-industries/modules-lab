//! Trade: a spot trading terminal over the `trade` program. The market's
//! bar, its chart, the order book and the tape, the order ticket, and the
//! reader's own orders, fills and balances.
//!
//! This file is the model: the view's state, one struct, and the presses
//! that turn it to another market or another price step. Rows come from
//! the program through its own types, and ops go to it (`data.rs`); the
//! ticket reads the order it would place and what stands in its way
//! (`ticket.rs`); the chart holds its candles and lays out where each is
//! drawn (`chart.rs`). `format.rs` writes the numbers, `words.rs` the
//! sentences.
//!
//! What is drawn is `ui`, the desktop's tree (three columns from 1100 px of
//! the view's own width, and under that the desk's pane: the chart over the
//! panel and one tabbed column beside them), or `ui::phone`, the phone's,
//! picked once at the root by whether the window's pointer is a finger.
//! What both draw alike is `look.rs`.
mod chart;
mod data;
mod format;
mod look;
mod ticket;
mod ui;
mod words;

use std::rc::Rc;

use serde::{Deserialize, Serialize};
use trade::{ASSETS, Balance, Book, Fill, MARKETS, Order, SPANS, Stats, Traded};
use view::Paged;
use view::prelude::*;

use chart::Chart;
use data::STEPS;
use ticket::Ticket;

#[derive(Default, Serialize, Deserialize)]
pub struct Trade {
    pub(crate) session: Session,
    /// The market on screen: its place in [`MARKETS`].
    pub(crate) market: usize,
    pub(crate) chart: Chart,
    /// The book's price step: its place in [`STEPS`].
    pub(crate) step: usize,
    pub(crate) ticket: Ticket,
    /// The open tab of the panel under the chart.
    pub(crate) panel: Panel,
    /// The open tab of the pane layout's column.
    pub(crate) column: Column,
    /// The pane's size, read from the window each render.
    #[serde(skip)]
    pub(crate) viewport: (f32, f32),
    #[serde(skip)]
    pub(crate) menu: Option<Menu>,
    /// The item the arrows are on in the open menu.
    #[serde(skip)]
    pub(crate) menu_cursor: usize,
    /// What the markets list is searched for.
    #[serde(skip)]
    pub(crate) search: TextField,
    // What the program said: never saved, read again on every mount.
    #[serde(skip)]
    pub(crate) stats: Loadable<Vec<Stats>>,
    #[serde(skip)]
    pub(crate) book: Loadable<Book>,
    /// The row the arrows are on in the book.
    #[serde(skip)]
    pub(crate) book_cursor: usize,
    #[serde(skip)]
    pub(crate) trades: Loadable<Vec<Traded>>,
    #[serde(skip)]
    pub(crate) balances: Loadable<Vec<Balance>>,
    /// The reader's open orders, every one: the chart draws them and their
    /// tab counts them.
    #[serde(skip)]
    pub(crate) open: Loadable<Rc<[Order]>>,
    #[serde(skip)]
    pub(crate) history: Option<Entity<Paged<Order>>>,
    #[serde(skip)]
    pub(crate) fills: Option<Entity<Paged<Fill>>>,
    /// Why the program refused the last cancel or grant; the ticket keeps
    /// its own.
    #[serde(skip)]
    pub(crate) refusal: String,
}

/// A tab of the panel under the chart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Panel {
    #[default]
    Open,
    History,
    Fills,
    Balances,
}

/// A tab of the pane layout's column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Column {
    #[default]
    Order,
    Book,
    Trades,
}

/// What hangs open under its control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Markets,
    Indicators,
    Step,
}

impl View for Trade {
    const NAME: &'static str = "Trade";
    const DESCRIPTION: &'static str =
        "Spot markets for practice funds: the chart, the book, your orders and balances.";
    const ICON: &'static str = "icons/chart-candlestick.svg";
    const CAPABILITIES: &'static [Capability] =
        &[Capability::Program, Capability::Op, Capability::Host];
    const TARGETS: &'static [&'static str] = &[trade::PROGRAM];
    // The pane layout's 256 px column, and beside it the 480 px a chart
    // needs for its tools and the panel for an order's columns.
    const MIN_WINDOW_WIDTH: u32 = 736;

    /// Follows the host and reads what the screen shows: on a first mount,
    /// and again after a snapshot, which carries none of the rows.
    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        // a snapshot of a build that listed more markets, spans or steps
        self.market = self.market.min(MARKETS.len() - 1);
        self.chart.span = self.chart.span.min(SPANS.len() - 1);
        self.step = self.step.min(STEPS.len() - 1);
        self.watch(cx);
        self.refresh(cx);
    }
}

impl Trade {
    /// The market on screen.
    pub(crate) fn market(&self) -> &'static trade::Market {
        &MARKETS[self.market]
    }

    /// The names of what the market trades and what it is paid in.
    pub(crate) fn assets(&self) -> (&'static str, &'static str) {
        let market = self.market();
        (
            ASSETS[market.base as usize].name,
            ASSETS[market.quote as usize].name,
        )
    }

    /// A price of the market on screen, with the decimals of its tick.
    pub(crate) fn price(&self, price: u64) -> String {
        format::fixed(price, format::decimals(self.market().tick))
    }

    /// The market's last price and last 24 hours, once read.
    pub(crate) fn market_stats(&self) -> Option<&Stats> {
        self.stats.ready()?.get(self.market)
    }

    /// Another market on screen: its chart, book and tape are read, and
    /// the ticket starts over, since a price is one market's.
    pub(crate) fn pick_market(&mut self, market: usize, cx: &mut Context<Self>) {
        if market == self.market {
            return;
        }
        self.market = market;
        self.ticket.clear();
        self.book_cursor = 0;
        self.read_market(cx);
        cx.notify();
    }

    /// Another price step for the book, its place in [`STEPS`]: the book
    /// is read again in it.
    pub(crate) fn pick_step(&mut self, step: usize, cx: &mut Context<Self>) {
        if step < STEPS.len() && step != self.step {
            self.step = step;
            self.book_cursor = 0;
            // the old step's rows would be written with this one's
            // decimals, and a click would take that price
            self.book = Loadable::Idle;
            self.read_book(cx);
            cx.notify();
        }
    }
}

impl Render for Trade {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // the pane's size, as the host lays the frame out: a layout is
        // chosen from it before the frame that shows it
        let size = window.viewport_size();
        self.viewport = (size.width.into(), size.height.into());
        // one wasm, two designs, the model shared: a finger gets the
        // phone's screens, a pointer the desktop's
        if window.touch() {
            ui::phone::render(self, cx)
        } else {
            ui::render(self, cx)
        }
    }
}

// Writes the wasm exports the host calls and the manifest it reads.
export_view!(Trade);

#[cfg(test)]
mod tests;
