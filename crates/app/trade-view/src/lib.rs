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
//! panel and one tabbed column beside them), or `ui::phone`, the phone's
//! (the markets, a market's page with a sheet over it, the reader's orders
//! and balances), picked once at the root by whether the window's pointer
//! is a finger. The book's rows (`book.rs`) and an order's facts
//! (`orders.rs`) are laid out once for both; what both draw alike is
//! `look.rs`.
mod book;
mod chart;
mod data;
mod format;
mod look;
mod orders;
mod ticket;
mod ui;
mod words;

use std::rc::Rc;

use serde::{Deserialize, Serialize};
use trade::{ASSETS, Balance, Book, Fill, MARKETS, Op, Order, SPANS, Side, Stats, Traded};
use view::Paged;
use view::design::Pages;
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
    /// The reader's last cancel, cancel all or grant that the program
    /// refused, and why; the ticket keeps its own.
    #[serde(skip)]
    pub(crate) refusal: Option<(Op, String)>,
    /// What the reader's last Get practice funds came to, said where it
    /// was pressed until their next write or another account.
    #[serde(skip)]
    pub(crate) granted: Option<&'static str>,
    /// The cancels, cancel alls and grants on their way: each one's
    /// control waits for its answer. The ticket's order is
    /// `ticket.placing`.
    #[serde(skip)]
    pub(crate) sending: Vec<Op>,
    /// The cancels the program took, whose controls wait on until the
    /// open orders are read again without what they cancelled.
    #[serde(skip)]
    pub(crate) settling: Vec<Op>,
    // What the phone's tree shows; the desktop's reads none of it.
    /// The phone's pages: the markets at the root.
    pub(crate) pages: Pages<Route>,
    /// What the phone's market page shows under its chart.
    pub(crate) segment: Segment,
    /// The list the phone's orders page shows.
    pub(crate) order_list: OrderList,
    /// The sheet over the phone's market page.
    #[serde(skip)]
    pub(crate) sheet: Option<Sheet>,
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

/// A page of the phone's stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    /// The root: the markets, and the reader's account under them.
    #[default]
    Markets,
    /// The market on screen.
    Market,
    /// The reader's orders in every market.
    Orders,
    Balances,
}

/// What the phone's market page shows under its chart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Segment {
    #[default]
    Book,
    Trades,
    Orders,
}

/// A list of the phone's orders page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderList {
    #[default]
    Open,
    History,
    Fills,
}

/// A sheet over the phone's market page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    /// The order ticket.
    Ticket,
    /// What the chart draws.
    ChartSettings,
    /// The book's price step.
    Step,
}

impl View for Trade {
    const NAME: &'static str = "Trade";
    const DESCRIPTION: &'static str =
        "Spot markets for practice funds: the chart, the book, your orders and balances.";
    const ICON: &'static str = "icons/chart-candlestick.svg";
    const TINT: Tint = Tint::Green;
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

    /// An asset's name; one past those this build lists reads `?`.
    pub(crate) fn asset_name(asset: u8) -> &'static str {
        ASSETS.get(asset as usize).map_or("?", |asset| asset.name)
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

    /// A market opened on the phone: its page over the markets.
    pub(crate) fn open_market(&mut self, market: usize, cx: &mut Context<Self>) {
        self.pages.push(Route::Market);
        self.pick_market(market, cx);
        cx.notify();
    }

    /// The reader's orders or balances, opened over the markets.
    pub(crate) fn open_page(&mut self, route: Route, cx: &mut Context<Self>) {
        self.pages.push(route);
        cx.notify();
    }

    /// The sheet the phone draws: the one over the market's page, while
    /// that page is on top.
    pub(crate) fn drawn_sheet(&self) -> Option<Sheet> {
        self.sheet.filter(|_| self.pages.top() == &Route::Market)
    }

    /// Whether the drawn sheet stays: the ticket does while its order is
    /// on its way, so the answer is said where the order was placed.
    fn sheet_held(&self) -> bool {
        self.drawn_sheet() == Some(Sheet::Ticket) && self.ticket.placing
    }

    /// The phone's Back: it closes the sheet drawn, unless it is held,
    /// else the page on top.
    pub(crate) fn back(&mut self, cx: &mut Context<Self>) {
        match self.drawn_sheet() {
            Some(_) if self.sheet_held() => return,
            Some(_) => self.sheet = None,
            None => {
                self.pages.pop();
                self.sheet = None;
            }
        }
        cx.notify();
    }

    pub(crate) fn open_sheet(&mut self, sheet: Sheet, cx: &mut Context<Self>) {
        self.sheet = Some(sheet);
        cx.notify();
    }

    /// The sheet drawn, closed, unless it is held.
    pub(crate) fn close_sheet(&mut self, cx: &mut Context<Self>) {
        if !self.sheet_held() {
            self.sheet = None;
            cx.notify();
        }
    }

    /// The ticket's sheet, on the side pressed.
    pub(crate) fn open_ticket(&mut self, sell: bool, cx: &mut Context<Self>) {
        self.open_sheet(Sheet::Ticket, cx);
        self.pick_side(sell, cx);
    }

    /// A level of the book pressed on the phone: the ticket opens at its
    /// price, on the side that trades against it (an ask is bought, a bid
    /// sold).
    pub(crate) fn take_level(&mut self, price: u64, rests: Side, cx: &mut Context<Self>) {
        self.take_price(price, cx);
        self.open_ticket(rests == Side::Buy, cx);
    }

    /// An order the program took: the phone's ticket, which waited on it,
    /// closes, and its market page shows the reader's orders, where it
    /// rests.
    pub(crate) fn placed(&mut self) {
        if self.drawn_sheet() == Some(Sheet::Ticket) {
            self.sheet = None;
            self.segment = Segment::Orders;
        }
    }

    pub(crate) fn pick_segment(&mut self, segment: Segment, cx: &mut Context<Self>) {
        self.segment = segment;
        cx.notify();
    }

    pub(crate) fn pick_order_list(&mut self, list: OrderList, cx: &mut Context<Self>) {
        self.order_list = list;
        cx.notify();
    }

    /// Whether `op` is on its way, or a cancel the open orders do not yet
    /// show taken.
    pub(crate) fn is_sending(&self, op: &Op) -> bool {
        self.sending.contains(op) || self.settling.contains(op)
    }

    /// Why the program refused the reader's last write that `about` picks
    /// out: the phone says a cancel's over the open orders, a grant's over
    /// the balances.
    pub(crate) fn refused(&self, about: fn(&Op) -> bool) -> Option<&str> {
        let refusal = self.refusal.as_ref().filter(|(op, _)| about(op));
        refusal.map(|(_, said)| said.as_str())
    }

    /// The market's volume over its last 24 hours, with what it trades,
    /// once it has a last price.
    pub(crate) fn day_volume(&self) -> Option<String> {
        let (base, _) = self.assets();
        let stats = self.market_stats().filter(|stats| stats.last > 0);
        stats.map(|stats| format!("{} {base}", format::quote(stats.volume)))
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
