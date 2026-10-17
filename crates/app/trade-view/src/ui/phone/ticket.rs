//! The order ticket's sheet on the phone (SPEC §2.5, boards 23-25): the
//! side and the kind, the price and the amount in their boxes, the shares
//! of what is free, the total, what is available or what stands in the
//! way, and the side's button. Its head stays while the body scrolls under
//! it, so the sheet may be shorter than what it holds with the keyboard
//! up. It reads the ticket's model (`ticket.rs`) and presses its verbs.
use trade::Side;
use view::accesskit::{Invalid, Live};
use view::design::{Orientation, SHEET_INSET, space};
use view::prelude::*;
use view::{AnyElement, Div, FontWeight, Stateful, wire};

use super::LADDER;
use super::parts::{Ready, side_button, waiting};
use crate::look::sided;
use crate::ticket::{Draft, Problem, SHARES};
use crate::{Trade, format, words};

/// The side's pair and a segment of it: the pair's 36 less its two
/// hairlines.
const PAIR_H: Pixels = px(36.);
const SEGMENT_H: Pixels = px(34.);
/// The unit at a box's right end, and the room the figure leaves it.
const UNIT_W: Pixels = px(36.);
const UNIT_ROOM: Pixels = px(46.);
/// The place button.
const PLACE_H: Pixels = px(48.);

pub(super) fn sheet(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let draft = view.draft();
    let (base, _) = view.assets();
    let last = view.market_stats().filter(|stats| stats.last > 0);
    let last = last.map_or("—".into(), |stats| view.price(stats.last));
    let head = design::sheet_title(
        "trade-ticket-head",
        words::place(draft.side, None, base),
        LADDER,
        theme,
    )
    .flex_none()
    .child(
        div()
            .text_type(LADDER.caption)
            .text_color(theme.muted)
            .child(words::ticket_line(view.market().name, &last)),
    );
    let dismiss = cx.listener(|view: &mut Trade, _: &(), _, cx| view.close_sheet(cx));
    design::sheet("trade-ticket", words::ORDER_TICKET, LADDER, theme)
        .on_dismiss(dismiss)
        .child(
            div()
                .flex()
                .flex_col()
                .max_h_full()
                .child(head)
                .child(body(view, &draft, cx, theme)),
        )
        .into_any_element()
}

/// What scrolls under the head: the order's fields, what it comes to, and
/// its button.
fn body(view: &Trade, draft: &Draft, cx: &mut Context<Trade>, theme: &Theme) -> Stateful<Div> {
    let ticket = &view.ticket;
    let (base, quote) = view.assets();
    let barrier = view.why_not_trading();
    let trades = barrier.is_none();
    let typed = || cx.listener(|view: &mut Trade, _: &wire::TextChange, _, _| view.ticket.edited());
    let entered = || cx.listener(|view: &mut Trade, _: &(), _, cx| view.place(cx));
    // a field that holds no such number says so to who cannot see it
    let input = |id: &'static str, field: &TextField, name: &'static str, wrong: Option<String>| {
        let look = match (trades, &wrong) {
            (false, _) => Look::Off,
            (true, Some(_)) => Look::Bad,
            (true, None) => Look::Plain,
        };
        let input = boxed(look, theme, Input::new(id, field, name))
            .disabled(!trades)
            .on_change(typed())
            .on_submit(entered());
        match wrong {
            Some(wrong) => input.invalid(Invalid::True).description(wrong),
            None => input,
        }
    };
    let price = match ticket.market {
        true => shown(words::BEST_PRICE.into(), Look::Off, theme).into_any_element(),
        false => input(
            "trade-price",
            &ticket.price,
            words::PRICE,
            view.price_problem(draft),
        )
        .into_any_element(),
    };
    // the amount is what the free funds fall short of, too
    let amount = input(
        "trade-amount",
        &ticket.amount,
        words::AMOUNT,
        view.amount_problem(draft).or(view.funds_problem(draft)),
    );
    let kinds = design::segmented(
        "trade-kind",
        words::ORDER_TYPE,
        LADDER,
        theme,
        Some(usize::from(ticket.market)),
        [
            ("trade-limit".into(), words::LIMIT.into()),
            ("trade-at-market".into(), words::MARKET.into()),
        ],
        cx.processor(|view: &mut Trade, kind: usize, _, cx| view.pick_kind(kind == 1, cx)),
    );
    let body = div()
        .id("trade-ticket-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .p(SHEET_INSET)
        .flex()
        .flex_col()
        .gap(space::LG)
        .child(sides(view, cx, theme))
        .child(kinds)
        .child(labelled(words::PRICE, price, quote, theme))
        .child(labelled(words::AMOUNT, amount, base, theme));
    let ready = match (trades, ticket.placing, view.placeable(draft)) {
        (true, true, _) => Ready::Busy,
        (true, false, true) => Ready::On,
        _ => Ready::Off,
    };
    let label = match ready {
        Ready::Busy => waiting(words::SENDING).into_any_element(),
        _ => view.place_label(draft).into_any_element(),
    };
    let place = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.place(cx));
    let place = side_button(
        "trade-submit",
        draft.side,
        label,
        PLACE_H,
        ready,
        theme,
        place,
    );
    if let Some(note) = barrier {
        return body
            .child(strip(note, theme.surface, theme.muted))
            .child(place);
    }
    let shares = SHARES.map(|share| {
        let amount = view.affords(draft, share);
        let picked = amount.is_some() && amount == draft.amount;
        let take =
            cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| view.take_share(share, cx));
        let button = design::button(
            format!("trade-share-{share}"),
            format!("{share}%"),
            LADDER,
            theme,
            take,
        )
        .small()
        .selected(picked);
        div().flex_1().flex().flex_col().child(button)
    });
    let short = matches!(draft.problem, Some(Problem::Funds(_)));
    let total = draft.total.map(format::total).unwrap_or_default();
    // the total is not the reader's to type, and is what the funds fall
    // short of
    let total =
        shown(total, Look::Off, theme).when(short, |total| total.border_color(theme.danger));
    let total_name = match ticket.market {
        true => words::ESTIMATED_TOTAL,
        false => words::TOTAL,
    };
    // what is wrong takes the place of what is available
    let standing = match view.ticket_problem(draft) {
        Some(wrong) => strip(wrong, theme.danger_soft, theme.danger)
            .id("trade-ticket-problem")
            .role(Role::Alert)
            .aria_live(Live::Assertive)
            .into_any_element(),
        None => div()
            .flex()
            .items_baseline()
            .justify_between()
            .text_type(LADDER.secondary)
            .text_color(theme.muted)
            .child(words::AVAILABLE)
            .child(
                div()
                    .font_family(design::fonts::FAMILY_MONO)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(draft.funds(draft.free)),
            )
            .into_any_element(),
    };
    body.child(div().flex().gap(space::SM).children(shares))
        .child(labelled(total_name, total, quote, theme))
        .child(standing)
        .child(place)
}

/// Buy | Sell, the picked side filled in its colour.
// SDK gap: a side-coloured radio pair (`segmented` fills in ink)
fn sides(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> Stateful<Div> {
    let sell = view.ticket.sell;
    let side = |side: Side, cx: &mut Context<Trade>| {
        let (picked, id) = match side {
            Side::Buy => (!sell, "trade-side-buy"),
            Side::Sell => (sell, "trade-side-sell"),
        };
        let pick = cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| {
            view.pick_side(side == Side::Sell, cx)
        });
        let press = design::hit(id, LADDER.hit, SEGMENT_H, px(0.), LADDER)
            .aria_label(words::side(side))
            .aria_toggled(picked.into())
            .on_click(pick);
        div()
            .relative()
            .flex_1()
            .h(SEGMENT_H)
            .flex()
            .items_center()
            .justify_center()
            .text_type(LADDER.secondary)
            .map(|segment| match picked {
                true => segment
                    .bg(sided(side, theme))
                    .text_color(theme.primary_foreground)
                    .font_weight(FontWeight::MEDIUM),
                false => segment.text_color(theme.muted),
            })
            .child(words::side(side))
            .child(design::item(press, Role::RadioButton, picked))
    };
    design::composite("trade-side", Role::RadioGroup, words::SIDE)
        .orientation(Orientation::Horizontal)
        .wrap()
        .active(usize::from(sell), 2)
        .on_move(cx.processor(|view: &mut Trade, side: usize, _, cx| view.pick_side(side == 1, cx)))
        .build()
        .flex_none()
        .h(PAIR_H)
        .flex()
        .border_1()
        .border_color(theme.border_strong)
        .child(side(Side::Buy, cx))
        .child(side(Side::Sell, cx))
}

/// How a box is drawn.
#[derive(Clone, Copy)]
enum Look {
    Plain,
    /// Not the reader's to fill: a figure the ticket works out, a market
    /// order's price, a ticket that places nothing.
    Off,
    /// The figure in it is what is wrong.
    Bad,
}

/// A box around `element`, as the kit's field draws one: a control's
/// height, a hairline, 12 in, the figure in mono ending short of the unit
/// at its right end.
// SDK gap: a field with a trailing unit, and a read-only figure box
fn boxed<E: Styled>(look: Look, theme: &Theme, element: E) -> E {
    let (edge, ground, ink) = match look {
        Look::Plain => (theme.border_strong, theme.background, theme.foreground),
        Look::Off => (theme.border, theme.surface, theme.muted),
        Look::Bad => (theme.danger, theme.background, theme.foreground),
    };
    element
        .h(LADDER.control)
        .w_full()
        .pl(space::LG)
        .pr(UNIT_ROOM)
        .border_1()
        .border_color(edge)
        .bg(ground)
        .text_color(ink)
        .font_family(design::fonts::FAMILY_MONO)
        .text_type(LADDER.body)
        .text_right()
}

/// A figure the ticket works out, in a box: not the reader's to type.
fn shown(text: String, look: Look, theme: &Theme) -> Div {
    boxed(look, theme, div())
        .flex()
        .items_center()
        .justify_end()
        .whitespace_nowrap()
        .child(text)
}

/// A box under its name, its figure's unit at its right end.
fn labelled(name: &'static str, field: impl IntoElement, unit: &'static str, theme: &Theme) -> Div {
    let unit = div()
        .absolute()
        .right(space::LG)
        .top_0()
        .h_full()
        .w(UNIT_W)
        .flex()
        .items_center()
        .justify_end()
        .text_type(LADDER.caption)
        .text_color(theme.muted)
        .child(unit);
    div()
        .flex()
        .flex_col()
        .gap(space::XS)
        .text_type(LADDER.caption)
        .text_color(theme.muted)
        .child(name)
        .child(div().relative().child(field).child(unit))
}

/// A few words on their own ground: a note, or what is wrong with the
/// order.
fn strip(text: String, ground: Hsla, ink: Hsla) -> Div {
    div()
        .px(space::LG)
        .py(space::MD)
        .bg(ground)
        .text_color(ink)
        .text_type(LADDER.secondary)
        .child(text)
}
