//! The order book and the tape of the market on screen. The book's rows
//! are one list the arrows walk: the asks down to the best one, the last
//! price between the sides, the bids from the best one down. A press on a
//! row puts its price in the ticket.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Div, FontWeight};
use gpui::relative;
use trade::{Book, Level, Side};

use crate::chart::sided;
use crate::data::DEPTH;
use crate::{Column, Menu, Trade, chevron, chip, format, nothing};

/// The price steps the book is asked in, in ticks of the market.
pub(crate) const STEPS: [u64; 3] = [1, 10, 100];
/// The height of the control that picks one.
pub(crate) const STEP_H: Pixels = px(22.);
/// A row of the book or the tape.
const ROW_H: f32 = 20.;
/// The book's heading, its column names, the row between the sides and
/// its foot: what its rows do not have of the column's height.
const HEAD_H: f32 = 30.;
const NAMES_H: f32 = 22.;
const MIDDLE_H: f32 = 32.;
const FOOT_H: f32 = 28.;

impl Trade {
    /// The book in a column `height` tall: as many prices a side as fit,
    /// the best ones.
    pub(crate) fn book(
        &self,
        height: f32,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let (base, quote) = self.assets();
        let head = div()
            .flex_none()
            .h(px(HEAD_H))
            .px(design::space::LG)
            .flex()
            .items_center()
            .justify_between()
            .text_size(design::text::SECONDARY)
            .font_weight(FontWeight::MEDIUM)
            .child("Order book")
            .child(self.step_picker(cx, theme));
        let names = [
            format!("Price ({quote})"),
            format!("Amount ({base})"),
            "Total".into(),
        ];
        let rows = height - HEAD_H - NAMES_H - MIDDLE_H - FOOT_H;
        let fit = ((rows / (2. * ROW_H)) as usize).clamp(1, DEPTH as usize);
        let body = match (&self.book, self.book.ready()) {
            (Loadable::Failed(refusal), _) => {
                nothing(refusal.message.clone(), theme).into_any_element()
            }
            (_, Some(book)) if book.asks.is_empty() && book.bids.is_empty() => {
                nothing("No orders in the book", theme).into_any_element()
            }
            (_, Some(book)) => self.levels(book, fit, cx, theme).into_any_element(),
            _ => div().into_any_element(),
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(head)
            .child(column_names(names, theme))
            .child(body)
    }

    /// The price step's control, and its menu while that is open.
    fn step_picker(&self, cx: &mut Context<Self>, theme: &Theme) -> impl IntoElement + use<> {
        let open = self.menu == Some(Menu::Step);
        let tick = self.market().tick;
        let named = |step: u64| {
            let step = step.saturating_mul(tick);
            format::fixed(step, format::decimals(step))
        };
        let opener = chip("trade-step", theme)
            .h(STEP_H)
            .aria_expanded(open)
            .aria_label(format!("Price step: {}", named(STEPS[self.step])))
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.open_menu(Menu::Step, window, cx)
            }))
            .child(named(STEPS[self.step]))
            .child(chevron(open, theme));
        let menu = open.then(|| {
            let steps = STEPS.iter().enumerate();
            let steps = steps.map(|(index, step)| (named(*step).into(), index == self.step));
            let role = Role::MenuItemRadio;
            self.menu_list(Menu::Step, "Price step", role, steps.collect(), cx, theme)
        });
        self.popover(Menu::Step, "Price step", opener, menu, true, cx)
    }

    /// The book's rows, `fit` a side, and its foot: each price with what
    /// rests at it and the running total from the best price out, drawn
    /// as a bar against the larger side's whole.
    fn levels(
        &self,
        book: &Book,
        fit: usize,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let theme = *theme;
        let asks = &book.asks[..book.asks.len().min(fit)];
        let bids = &book.bids[..book.bids.len().min(fit)];
        let whole = |levels: &[Level]| levels.iter().map(|level| level.amount).sum::<u64>();
        let (asked, bid) = (whole(asks), whole(bids));
        let most = asked.max(bid).max(1);
        let count = asks.len() + bids.len();
        let active = self.book_cursor.min(count - 1);
        let step = STEPS[self.step].saturating_mul(self.market().tick);
        let places = format::decimals(step);
        // the asks are listed from the worst down to the best, so the row
        // of the best ask's level `at` is counted from their end
        let row = |side: Side, at: usize, level: &Level, running: u64| {
            let (index, name, soft) = match side {
                Side::Sell => (asks.len() - 1 - at, "ask", theme.danger_soft),
                Side::Buy => (asks.len() + at, "bid", theme.success_soft),
            };
            let price = level.price;
            let depth = div()
                .absolute()
                .right_0()
                .top(px(1.))
                .bottom(px(1.))
                .w(relative(running as f32 / most as f32))
                .bg(soft);
            let row = level_row(&theme)
                .id(format!("book-{name}-{at}"))
                .relative()
                .hover(move |style| style.bg(theme.surface))
                .aria_selected(index == active)
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.book_cursor = index;
                    view.take_price(price, cx)
                }))
                .child(depth)
                .child(
                    div()
                        .flex_1()
                        .text_color(sided(side, &theme))
                        .child(format::fixed(price, places)),
                )
                .child(right().child(format::amount(level.amount)))
                .child(right().child(format::amount(running)));
            design::item(row, Role::ListBoxOption, index == active)
        };
        let running = |levels: &[Level]| {
            let mut running = 0u64;
            let totals = levels.iter().map(move |level| {
                running += level.amount;
                running
            });
            totals.collect::<Vec<_>>()
        };
        let mut ask_rows: Vec<_> = (asks.iter().zip(running(asks)).enumerate())
            .map(|(at, (level, running))| row(Side::Sell, at, level, running))
            .collect();
        ask_rows.reverse();
        let bid_rows = (bids.iter().zip(running(bids)).enumerate())
            .map(|(at, (level, running))| row(Side::Buy, at, level, running));
        let prices: Vec<u64> = (asks.iter().rev().chain(bids))
            .map(|level| level.price)
            .collect();
        let spread = match (asks.first(), bids.first()) {
            (Some(ask), Some(bid)) => self.price(ask.price.saturating_sub(bid.price)),
            _ => "—".into(),
        };
        let rows = design::composite("trade-book", Role::ListBox, "Order book")
            .active(active, count)
            .on_move(cx.processor(|view: &mut Trade, index: usize, _, cx| {
                view.book_cursor = index;
                cx.notify();
            }))
            .on_press(cx.processor(move |view: &mut Trade, index: usize, _, cx| {
                if let Some(price) = prices.get(index) {
                    view.take_price(*price, cx);
                }
            }))
            .build()
            .flex_none()
            .flex()
            .flex_col()
            .children(ask_rows)
            .child(self.middle(spread, &theme))
            .children(bid_rows);
        let share = (u128::from(bid) * 100 / u128::from(asked + bid).max(1)) as u32;
        let split = |text: String, color: Hsla| {
            design::mono(text)
                .text_size(design::text::CAPTION)
                .text_color(color)
        };
        let foot = div()
            .flex_none()
            .h(px(FOOT_H))
            .px(design::space::LG)
            .flex()
            .items_center()
            .gap(design::space::SM)
            .border_t_1()
            .border_color(theme.border)
            .child(split(format!("Bids {share}%"), theme.success))
            .child(
                div()
                    .flex_1()
                    .h(px(4.))
                    .flex()
                    .child(div().w(relative(share as f32 / 100.)).bg(theme.success))
                    .child(div().flex_1().bg(theme.danger)),
            )
            .child(split(format!("{}% Asks", 100 - share), theme.danger));
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(rows)
            .child(div().flex_1())
            .child(foot)
    }

    /// The row between the book's sides: the last price, which way it
    /// moved from the trade before, and the spread.
    fn middle(&self, spread: String, theme: &Theme) -> Div {
        let trades = self.trades.ready().map(Vec::as_slice).unwrap_or_default();
        let last = trades.first().map(|trade| trade.price);
        let last = last.or(self
            .market_stats()
            .map(|stats| stats.last)
            .filter(|last| *last > 0));
        let before = trades.get(1).map(|trade| trade.price);
        let (tone, arrow) = match (last, before) {
            (Some(last), Some(before)) if last > before => (theme.success, "↑"),
            (Some(last), Some(before)) if last < before => (theme.danger, "↓"),
            _ => (theme.foreground, ""),
        };
        div()
            .flex_none()
            .h(px(MIDDLE_H))
            .px(design::space::LG)
            .flex()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_b_1()
            .border_color(theme.border)
            .text_size(design::text::CAPTION)
            .text_color(theme.muted)
            .child(
                design::mono(last.map_or("—".into(), |last| self.price(last)))
                    .flex()
                    .gap(design::space::XXS)
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tone)
                    .child(arrow),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(design::space::XXS)
                    .child("Spread")
                    .child(design::mono(spread).text_size(design::text::CAPTION)),
            )
    }

    /// A book row's price, pressed: the ticket takes it as a limit, and
    /// the pane's column turns to the ticket.
    fn take_price(&mut self, price: u64, cx: &mut Context<Self>) {
        let price = self.price(price);
        self.ticket.price.reset(price);
        self.ticket.market = false;
        self.ticket.refusal.clear();
        self.column = Column::Order;
        cx.notify();
    }

    /// The market's last trades, newest first, each price in the colour of
    /// the side that took it. The wide layout heads it; the pane's tab
    /// already does.
    pub(crate) fn tape(&self, headed: bool, theme: &Theme) -> impl IntoElement + use<> {
        let theme = *theme;
        let (base, quote) = self.assets();
        let head = headed.then(|| {
            div()
                .flex_none()
                .h(px(HEAD_H))
                .px(design::space::LG)
                .flex()
                .items_center()
                .border_t_1()
                .border_color(theme.border)
                .text_size(design::text::SECONDARY)
                .font_weight(FontWeight::MEDIUM)
                .child("Trades")
        });
        let names = [
            format!("Price ({quote})"),
            format!("Amount ({base})"),
            "Time".into(),
        ];
        let rows = match (&self.trades, self.trades.ready()) {
            (Loadable::Failed(refusal), _) => vec![nothing(refusal.message.clone(), &theme)],
            (_, Some(trades)) if trades.is_empty() => vec![nothing("No trades yet", &theme)],
            (_, Some(trades)) => (trades.iter())
                .map(|trade| {
                    level_row(&theme)
                        .hover(move |style| style.bg(theme.surface))
                        .child(
                            div()
                                .flex_1()
                                .text_color(sided(trade.taker, &theme))
                                .child(self.price(trade.price)),
                        )
                        .child(right().child(format::amount(trade.amount)))
                        .child(
                            right()
                                .text_color(theme.muted)
                                .child(format::time(trade.time)),
                        )
                })
                .collect(),
            _ => Vec::new(),
        };
        div()
            .id("trade-tape")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .children(head)
            .child(column_names(names, &theme))
            .children(rows)
    }
}

/// One line of the book or the tape: three columns of mono figures.
fn level_row(theme: &Theme) -> Div {
    div()
        .flex_none()
        .h(px(ROW_H))
        .px(design::space::LG)
        .flex()
        .items_center()
        .font_family(design::fonts::FAMILY_MONO)
        .text_size(design::text::MONO)
        .text_color(theme.foreground)
        .whitespace_nowrap()
}

/// A column of figures: its text ends at its right edge.
fn right() -> Div {
    div().flex_1().min_w_0().flex().justify_end()
}

/// What the three columns hold, quiet, over the rows.
fn column_names(names: [String; 3], theme: &Theme) -> Div {
    let [first, second, third] = names;
    div()
        .flex_none()
        .h(px(NAMES_H))
        .px(design::space::LG)
        .flex()
        .items_center()
        .text_size(design::text::CAPTION)
        .text_color(theme.muted)
        .whitespace_nowrap()
        .child(div().flex_1().child(first))
        .child(right().child(second))
        .child(right().child(third))
}
