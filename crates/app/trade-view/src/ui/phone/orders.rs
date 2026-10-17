//! The reader's orders on the phone (SPEC §2.4c, §2.6; boards 22, 25,
//! 26): their open orders in every market, each with its Cancel, and
//! Cancel all over them, which a market's page shows under its chart and
//! the orders page as its first list; and on that page every order they
//! placed and their fills, read a page at a time as each list is
//! scrolled to its end.
use std::ops::Range;

use trade::{Fill, Op, Order};
use view::AnyElement;
use view::accesskit::Live;
use view::design::space;
use view::prelude::*;
use view::{Div, Paged, Stateful};

use super::parts::{head_action, order_row, quiet, refusal, refused, scroller, traded, waiting};
use super::{LADDER, back};
use crate::data::cancels;
use crate::orders::{average, filled, limit, market_of, priced};
use crate::{OrderList, Trade, format, words};

/// The orders page: which list, and that list.
pub(super) fn page(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    const LISTS: [(OrderList, &str, &str); 3] = [
        (OrderList::Open, "trade-list-open", words::OPEN),
        (OrderList::History, "trade-list-history", words::HISTORY),
        (OrderList::Fills, "trade-list-fills", words::FILLS),
    ];
    let picked = LISTS.iter().position(|(list, ..)| *list == view.order_list);
    let lists = design::segmented(
        "trade-account-tab",
        words::ORDER_LISTS,
        LADDER,
        theme,
        picked,
        LISTS.map(|(_, id, name)| (id.into(), name.into())),
        cx.processor(|view: &mut Trade, list: usize, _, cx| {
            view.pick_order_list(LISTS[list].0, cx)
        }),
    );
    let lists = div()
        .flex_none()
        .pt(space::SM)
        .px(space::BLOCK)
        .pb(space::XXS)
        .flex()
        .flex_col()
        .child(lists);
    let list = match (view.session.account, view.order_list) {
        (None, _) => quiet("trade-signed-out", words::SIGNED_OUT, theme).into_any_element(),
        (Some(_), OrderList::Open) => scroller("trade-orders-page")
            .child(open(view, cx, theme))
            .into_any_element(),
        (Some(_), OrderList::History) => history(view, cx, theme),
        (Some(_), OrderList::Fills) => fills(view, cx, theme),
    };
    design::page("trade-orders", words::ORDERS, back(view, cx), LADDER, theme)
        .caption(words::EVERY_MARKET)
        .child(lists)
        .child(list)
        .into_any_element()
}

/// The reader's open orders in every market, under Cancel all, and what
/// the program said of the last cancel; with no account, why there are
/// none.
pub(super) fn open(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    if view.session.account.is_none() {
        return quiet("trade-signed-out", words::SIGNED_OUT, theme).into_any_element();
    }
    let trades = view.may_trade();
    let orders = view.open.ready();
    let count = orders.map_or(0, |orders| orders.len());
    let cancel_all = (count > 0).then(|| {
        let cancel =
            cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.submit(Op::CancelAll, cx));
        let busy = view.is_sending(&Op::CancelAll);
        head_action(
            "trade-cancel-all",
            words::CANCEL_ALL,
            trades,
            busy,
            theme,
            cancel,
        )
    });
    let head = design::section_head("trade-open-head", words::open_orders(count), LADDER, theme)
        .children(cancel_all);
    let rows = match (&view.open, orders) {
        (Loadable::Failed(failed), _) => {
            let retry = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.refresh(cx));
            vec![refused("trade-open", failed, theme, retry).into_any_element()]
        }
        (_, Some(orders)) if orders.is_empty() => {
            vec![quiet("trade-no-open", words::NO_OPEN_ORDERS, theme).into_any_element()]
        }
        (_, Some(orders)) => (orders.iter())
            .map(|order| open_row(view, order, trades, cx, theme).into_any_element())
            .collect(),
        _ => Vec::new(),
    };
    div()
        .flex()
        .flex_col()
        .children(refusal(view.refused(cancels), theme))
        .child(head)
        .children(rows)
        .into_any_element()
}

/// An open order: what it is, what it rests at, how much filled and when
/// it was placed, and its Cancel, or the cancel on its way.
fn open_row(
    view: &Trade,
    order: &Order,
    trades: bool,
    cx: &mut Context<Trade>,
    theme: &Theme,
) -> Stateful<Div> {
    let id = order.id;
    let caption = words::joined(&[
        words::limit(limit(order)),
        words::filled(filled(order)),
        format::time(order.placed),
    ]);
    let tail = match view.is_sending(&Op::Cancel { order: id }) {
        true => waiting(words::CANCELLING)
            .id(("trade-cancelling", id))
            .flex_none()
            .text_type(LADDER.secondary)
            .text_color(theme.muted)
            .role(Role::Status)
            .aria_live(Live::Polite)
            .aria_busy(true)
            .into_any_element(),
        false => {
            let cancel = cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| {
                view.submit(Op::Cancel { order: id }, cx)
            });
            design::button(format!("cancel-{id}"), words::CANCEL, LADDER, theme, cancel)
                .small()
                .enabled(trades)
                .into_any_element()
        }
    };
    let row = traded(order.side, order.amount, order.market, theme);
    order_row(("trade-order", id), row, caption, theme).child(tail)
}

/// Every order the reader placed, the newest first.
fn history(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let theme = *theme;
    let head = design::section_head("trade-history-head", words::ORDER_HISTORY, LADDER, &theme);
    let list = paged(
        "trade-history",
        view.history.as_ref(),
        words::NO_ORDERS,
        cx,
        &theme,
        move |at, order: &Order| {
            let caption = words::joined(&[
                words::limit(limit(order)),
                words::state(order.state).into(),
                words::average(average(order)),
                format::day_time(order.placed),
            ]);
            let row = traded(order.side, order.amount, order.market, &theme);
            order_row(("trade-placed", at), row, caption, &theme)
        },
    );
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(head)
        .child(list)
        .into_any_element()
}

/// The reader's fills, the newest first.
fn fills(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let theme = *theme;
    let list = paged(
        "trade-fills",
        view.fills.as_ref(),
        words::NO_FILLS,
        cx,
        &theme,
        move |at, fill: &Fill| {
            let market = market_of(fill.market);
            let quote = Trade::asset_name(market.quote);
            let total = format::total(format::cost(fill.price, fill.amount));
            let caption = words::joined(&[
                priced(market, fill.price),
                words::fill_role(fill.maker).into(),
                format!("{total} {quote}"),
                format::day_time(fill.time),
            ]);
            let row = traded(fill.side, fill.amount, fill.market, &theme);
            order_row(("trade-fill", at), row, caption, &theme)
        },
    );
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(list)
        .into_any_element()
}

/// A list that pages: the rows read so far, each drawn by `row` from its
/// place and itself, and while the list goes on a row that reads on as it
/// comes into the window; until then a quiet line, or why it was refused.
fn paged<T: PartialEq + 'static>(
    id: &'static str,
    list: Option<&Entity<Paged<T>>>,
    empty: &'static str,
    cx: &mut Context<Trade>,
    theme: &Theme,
    row: impl Fn(usize, &T) -> Stateful<Div> + 'static,
) -> AnyElement {
    let Some(list) = list.cloned() else {
        return div().into_any_element();
    };
    if let Some(failed) = list.read(|list| list.failed().cloned()) {
        let retry = {
            let list = list.clone();
            cx.listener(move |_: &mut Trade, _: &ClickEvent, _, cx| {
                list.update(cx, |list, cx| list.reread(cx))
            })
        };
        return refused(id, &failed, theme, retry).into_any_element();
    }
    let (held, count) = list.read(|list| (list.rows().len(), list.count()));
    if held == 0 {
        // the first page is on its way, or the list has no rows
        let said = match list.read(Paged::is_loading) {
            true => words::READING,
            false => empty,
        };
        return quiet(format!("{id}-empty"), said, theme).into_any_element();
    }
    let theme = *theme;
    uniform_list(id, count, move |rows: Range<usize>, _, app| {
        list.update(app, |list, cx| list.show(rows.clone(), cx));
        list.read(|list| {
            let rows = rows.map(|at| match list.rows().get(at) {
                Some(found) => row(at, found).into_any_element(),
                None => reading(at, &theme).into_any_element(),
            });
            rows.collect::<Vec<_>>()
        })
    })
    .flex_1()
    .min_h_0()
    .into_any_element()
}

/// The row at `at` of a list whose page is on its way.
fn reading(at: usize, theme: &Theme) -> Stateful<Div> {
    div()
        .id(("trade-reading", at))
        .h(LADDER.row_tall)
        .px(space::BLOCK)
        .flex()
        .items_center()
        .text_type(LADDER.secondary)
        .text_color(theme.muted)
        .child(words::READING)
}
