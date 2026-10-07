//! The panel under the chart: the reader's open orders, every order they
//! placed, their fills and their balances, a tab each. Each is a table of
//! rows the host scrolls; the two that go on (orders, fills) are read a
//! page at a time as their list is scrolled to its end.
use std::ops::Range;
use std::rc::Rc;

use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Div, Paged, Stateful};
use trade::{ASSETS, Balance, Fill, MARKETS, Op, Order, OrderState, Side, UNIT};

use crate::chart::sided;
use crate::{Panel, TABS_H, Trade, format, nothing, tabs};

/// The panel's height, wide and in the pane: its tabs, a table's head and
/// a few rows. The chart takes what a taller window adds.
const PANEL_H: Pixels = px(158.);
const PANEL_PANE_H: Pixels = px(136.);
/// A table's head and one of its rows.
const HEAD_H: Pixels = px(24.);
const ROW_H: Pixels = px(26.);

/// How a column takes its width, wide and in the pane: a width for each,
/// none where the pane drops the column, or a share of what the others
/// leave, its figures ending at its right edge.
#[derive(Clone, Copy)]
enum Fit {
    Left(f32, Option<f32>),
    Right(f32, f32),
    Share,
    /// A share, wide alone.
    ShareWide,
}

/// A table's columns: the name over each and how it fits.
type Columns = &'static [(&'static str, Fit)];

const OPEN: Columns = &[
    ("Time", Fit::Left(76., Some(64.))),
    ("Market", Fit::Left(84., Some(72.))),
    ("Side", Fit::Left(52., Some(36.))),
    ("Type", Fit::Left(60., None)),
    ("Price", Fit::Share),
    ("Amount", Fit::Share),
    ("Filled", Fit::Share),
    ("", Fit::Right(72., 56.)),
];
const HISTORY: Columns = &[
    ("Time", Fit::Left(100., Some(84.))),
    ("Market", Fit::Left(84., Some(72.))),
    ("Side", Fit::Left(52., Some(36.))),
    ("Type", Fit::Left(60., None)),
    ("Price", Fit::Share),
    ("Amount", Fit::Share),
    ("Filled", Fit::Share),
    ("Average", Fit::ShareWide),
    ("State", Fit::Right(76., 60.)),
];
const FILLS: Columns = &[
    ("Time", Fit::Left(100., Some(84.))),
    ("Market", Fit::Left(84., Some(72.))),
    ("Side", Fit::Left(52., Some(36.))),
    ("Price", Fit::Share),
    ("Amount", Fit::Share),
    ("Total", Fit::Share),
    ("", Fit::Right(60., 48.)),
];
const BALANCES: Columns = &[
    ("Asset", Fit::Left(80., Some(80.))),
    ("Available", Fit::Share),
    ("In orders", Fit::Share),
    ("Total", Fit::Share),
];

/// What a press on a row's control does to the row's order.
type Press = Rc<dyn Fn(u64, &mut Window, &mut App)>;

impl Trade {
    pub(crate) fn panel(
        &self,
        wide: bool,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        const TABS: [Panel; 4] = [Panel::Open, Panel::History, Panel::Fills, Panel::Balances];
        let open = self.open.ready().map(|orders| orders.len());
        let history = if wide { "Order history" } else { "History" };
        let picked = TABS.iter().position(|tab| *tab == self.panel);
        let bar = tabs(
            "trade-panel-tabs",
            "Orders, fills and balances",
            theme,
            picked.unwrap_or_default(),
            [
                ("trade-open".into(), "Open orders".into(), open),
                ("trade-history".into(), history.into(), None),
                ("trade-fills".into(), "Fills".into(), None),
                ("trade-balances".into(), "Balances".into(), None),
            ],
            cx.processor(|view: &mut Trade, tab: usize, _, cx| {
                view.panel = TABS[tab];
                cx.notify();
            }),
        )
        .h_full();
        // the one thing a tab does to all of its rows, beside its tabs
        let trades = self.may_trade();
        let action = match self.panel {
            Panel::Open if open.is_some_and(|open| open > 0) => Some(link(
                "trade-cancel-all",
                "Cancel all",
                trades,
                theme,
                cx.listener(|view, _: &ClickEvent, _, cx| view.submit(Op::CancelAll, cx)),
            )),
            Panel::Balances => Some(link(
                "trade-fund",
                "Get practice funds",
                trades,
                theme,
                cx.listener(|view, _: &ClickEvent, _, cx| view.submit(Op::Fund, cx)),
            )),
            _ => None,
        };
        let bar = div()
            .flex_none()
            .h(TABS_H)
            .px(design::space::LG)
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(bar)
            .child(div().flex_1())
            .children(action);
        let refused = (!self.refusal.is_empty()).then(|| {
            nothing(self.refusal.clone(), theme)
                .bg(theme.danger_soft)
                .text_color(theme.danger)
        });
        let body = match self.session.account {
            None => {
                nothing("Orders, fills and balances are an account's.", theme).into_any_element()
            }
            Some(_) => match self.panel {
                Panel::Open => self.open_orders(wide, cx, theme),
                Panel::History => self.history(wide, theme),
                Panel::Fills => self.fills(wide, theme),
                Panel::Balances => self.balances(wide, theme),
            },
        };
        div()
            .flex_none()
            .h(if wide { PANEL_H } else { PANEL_PANE_H })
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(theme.border)
            .child(bar)
            .children(refused)
            .child(body)
    }

    fn open_orders(&self, wide: bool, cx: &mut Context<Self>, theme: &Theme) -> AnyElement {
        let orders = match (&self.open, self.open.ready()) {
            (Loadable::Failed(refusal), _) => return said(refusal.message.clone(), theme),
            (_, Some(orders)) if orders.is_empty() => return said("No open orders", theme),
            (_, Some(orders)) => orders.clone(),
            _ => return div().into_any_element(),
        };
        let theme = *theme;
        let trades = self.may_trade();
        let cancel: Press = Rc::new(cx.processor(|view: &mut Trade, order: u64, _, cx| {
            view.submit(Op::Cancel { order }, cx)
        }));
        let count = orders.len();
        table(
            "trade-open-rows",
            OPEN,
            wide,
            count,
            &theme,
            move |rows, _| {
                let rows = rows.filter_map(|row| orders.get(row));
                let rows = rows.map(|order| {
                    let (id, cancel) = (order.id, cancel.clone());
                    let press = move |_: &ClickEvent, window: &mut Window, app: &mut App| {
                        cancel(id, window, app)
                    };
                    let cancel = link(format!("cancel-{id}"), "Cancel", trades, &theme, press);
                    let mut cells = order_cells(order, format::time(order.placed), &theme);
                    cells.push(cancel.into_any_element());
                    cells
                });
                rows.collect()
            },
        )
    }

    fn history(&self, wide: bool, theme: &Theme) -> AnyElement {
        let theme = *theme;
        paged(
            "trade-history-rows",
            HISTORY,
            wide,
            self.history.as_ref(),
            "No orders yet",
            &theme,
            move |order: &Order| {
                let market = &MARKETS[order.market as usize % MARKETS.len()];
                let places = format::decimals(market.tick);
                // what its fills came to, a unit: the price it traded at
                let average = u128::from(order.quote) * u128::from(UNIT);
                let average = average.checked_div(u128::from(order.filled));
                let average = average.map(|average| format::fixed(average as u64, places));
                let state = match order.state {
                    OrderState::Open => "Open",
                    OrderState::Filled => "Filled",
                    OrderState::Cancelled => "Cancelled",
                };
                let mut cells = order_cells(order, format::day_time(order.placed), &theme);
                cells.push(figure(average.unwrap_or("—".into())));
                cells.push(words(state, theme.muted));
                cells
            },
        )
    }

    fn fills(&self, wide: bool, theme: &Theme) -> AnyElement {
        let theme = *theme;
        paged(
            "trade-fills-rows",
            FILLS,
            wide,
            self.fills.as_ref(),
            "No fills yet",
            &theme,
            move |fill: &Fill| {
                let market = &MARKETS[fill.market as usize % MARKETS.len()];
                let places = format::decimals(market.tick);
                vec![
                    stamp(format::day_time(fill.time), &theme),
                    words(market.name, theme.foreground),
                    side(fill.side, &theme),
                    figure(format::fixed(fill.price, places)),
                    figure(format::amount(fill.amount)),
                    figure(format::total(format::cost(fill.price, fill.amount))),
                    words(if fill.maker { "Maker" } else { "Taker" }, theme.muted),
                ]
            },
        )
    }

    /// What the account has of every asset: free, held by its resting
    /// orders, and both.
    fn balances(&self, wide: bool, theme: &Theme) -> AnyElement {
        let balances = match (&self.balances, self.balances.ready()) {
            (Loadable::Failed(refusal), _) => return said(refusal.message.clone(), theme),
            (_, Some(balances)) => balances,
            _ => return div().into_any_element(),
        };
        if balances
            .iter()
            .all(|balance| balance.free == 0 && balance.held == 0)
        {
            return said("No funds yet. Get practice funds to place an order.", theme);
        }
        let theme = *theme;
        let balances: Rc<[Balance]> = balances.as_slice().into();
        let count = balances.len();
        table(
            "trade-balance-rows",
            BALANCES,
            wide,
            count,
            &theme,
            move |rows, _| {
                let rows = rows.filter_map(|row| balances.get(row));
                let rows = rows.map(|balance| {
                    let asset = ASSETS.get(balance.asset as usize);
                    let amount = |value: u64| format::balance(balance.asset, value);
                    vec![
                        words(asset.map_or("?", |asset| asset.name), theme.foreground),
                        figure(amount(balance.free)),
                        design::mono(amount(balance.held))
                            .text_color(theme.muted)
                            .into_any_element(),
                        figure(amount(balance.free.saturating_add(balance.held))),
                    ]
                });
                rows.collect()
            },
        )
    }
}

/// The cells every order's row starts with: when it was placed, its
/// market, side and type, its price, its amount and how much of it filled.
fn order_cells(order: &Order, placed: String, theme: &Theme) -> Vec<AnyElement> {
    let market = &MARKETS[order.market as usize % MARKETS.len()];
    let places = format::decimals(market.tick);
    let filled = u128::from(order.filled) * 100 / u128::from(order.amount.max(1));
    vec![
        stamp(placed, theme),
        words(market.name, theme.foreground),
        side(order.side, theme),
        words(
            if order.limit.is_some() {
                "Limit"
            } else {
                "Market"
            },
            theme.foreground,
        ),
        figure(
            order
                .limit
                .map_or("Market".into(), |limit| format::fixed(limit, places)),
        ),
        figure(format::amount(order.amount)),
        figure(format!("{filled}%")),
    ]
}

fn words(text: &'static str, color: Hsla) -> AnyElement {
    div().text_color(color).child(text).into_any_element()
}

fn figure(text: String) -> AnyElement {
    design::mono(text).into_any_element()
}

fn stamp(text: String, theme: &Theme) -> AnyElement {
    design::mono(text)
        .text_color(theme.muted)
        .into_any_element()
}

fn side(side: Side, theme: &Theme) -> AnyElement {
    let name = match side {
        Side::Buy => "Buy",
        Side::Sell => "Sell",
    };
    words(name, sided(side, theme))
}

/// One calm line in a table's place.
fn said(text: impl Into<SharedString>, theme: &Theme) -> AnyElement {
    nothing(text, theme).into_any_element()
}

/// An underlined control in a table: what it does, pressed; quiet and
/// saying so when the reader may not.
fn link(
    id: impl Into<ElementId>,
    label: &'static str,
    enabled: bool,
    theme: &Theme,
    press: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let link = div()
        .id(id)
        .text_size(design::text::SECONDARY)
        .whitespace_nowrap()
        .underline()
        .role(Role::Button)
        .child(label);
    match enabled {
        true => link.cursor_pointer().focusable().on_click(press),
        false => link.text_color(theme.muted).aria_disabled(true),
    }
}

/// A line of a table, its cells laid in `columns`.
fn line(
    columns: Columns,
    wide: bool,
    cells: impl IntoIterator<Item = AnyElement>,
    theme: &Theme,
) -> Div {
    let cells = columns.iter().zip(cells).filter_map(|((_, fit), cell)| {
        let fixed = |width: f32| div().w(px(width)).flex_none().overflow_hidden();
        let share = || div().flex_1().min_w_0().flex().justify_end();
        Some(match (fit, wide) {
            (Fit::Left(width, _), true) => fixed(*width).child(cell),
            (Fit::Left(_, pane), false) => fixed((*pane)?).child(cell),
            (Fit::Right(width, _), true) | (Fit::Right(_, width), false) => {
                fixed(*width).flex().justify_end().child(cell)
            }
            (Fit::Share, _) | (Fit::ShareWide, true) => share().child(cell),
            (Fit::ShareWide, false) => return None,
        })
    });
    div()
        .px(design::space::LG)
        .flex()
        .items_center()
        .border_b_1()
        .border_color(theme.border)
        .whitespace_nowrap()
        .children(cells)
}

/// A table: its head over `count` rows the host scrolls, each row the
/// cells `paint` gives for it, in the columns' order.
fn table(
    id: &'static str,
    columns: Columns,
    wide: bool,
    count: usize,
    theme: &Theme,
    paint: impl Fn(Range<usize>, &mut App) -> Vec<Vec<AnyElement>> + 'static,
) -> AnyElement {
    let theme = *theme;
    let names = columns.iter().map(|(name, _)| (*name).into_any_element());
    let head = line(columns, wide, names, &theme)
        .flex_none()
        .h(HEAD_H)
        .text_size(design::text::CAPTION)
        .text_color(theme.muted);
    let rows = uniform_list(id, count, move |rows: Range<usize>, _, app| {
        let rows = paint(rows, app).into_iter();
        let rows = rows.map(|cells| {
            line(columns, wide, cells, &theme)
                .h(ROW_H)
                .text_size(design::text::SECONDARY)
                .hover(move |style| style.bg(theme.surface))
        });
        rows.collect::<Vec<_>>()
    });
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(head)
        .child(rows.flex_1().min_h_0())
        .into_any_element()
}

/// A table of a list that pages: the rows read so far, and while the list
/// goes on a row that reads on as it comes into the window.
fn paged<T: PartialEq + 'static>(
    id: &'static str,
    columns: Columns,
    wide: bool,
    list: Option<&Entity<Paged<T>>>,
    empty: &'static str,
    theme: &Theme,
    cells: impl Fn(&T) -> Vec<AnyElement> + 'static,
) -> AnyElement {
    let Some(list) = list.cloned() else {
        return div().into_any_element();
    };
    if let Some(refusal) = list.read(|list| list.failed().cloned()) {
        return said(refusal.message, theme);
    }
    let (held, count) = list.read(|list| (list.rows().len(), list.count()));
    if held == 0 {
        // the first page is on its way, or the list has no rows
        let nothing = if list.read(Paged::is_loading) {
            ""
        } else {
            empty
        };
        return said(nothing, theme);
    }
    let quiet = theme.muted;
    table(id, columns, wide, count, theme, move |rows, app| {
        list.update(app, |list, cx| list.show(rows.clone(), cx));
        list.read(|list| {
            let rows = rows.map(|row| match list.rows().get(row) {
                Some(row) => cells(row),
                None => vec![words("Reading…", quiet)],
            });
            rows.collect()
        })
    })
}
