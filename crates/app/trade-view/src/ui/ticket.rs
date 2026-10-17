//! The desktop's order ticket: Buy | Sell, Limit | Market, the price and
//! the amount in their boxes, the shares of what is free, the total, and
//! what is available or what stands in the order's way over the button.
//! It reads the ticket's model (`ticket.rs`) and presses its verbs.
use trade::Side;
use view::accesskit::Invalid;
use view::design::{Ladder, Orientation};
use view::prelude::*;
use view::{Div, FontWeight, Stateful, wire};

use super::tabs;
use crate::look::sided;
use crate::ticket::{Problem, SHARES};
use crate::{Trade, format, words};

impl Trade {
    pub(super) fn ticket(
        &self,
        cx: &mut Context<Self>,
        ladder: &Ladder,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let theme = *theme;
        let ticket = &self.ticket;
        let (base, quote) = self.assets();
        let draft = self.draft();
        let barrier = self.why_not_trading();
        let trades = barrier.is_none();
        let color = sided(draft.side, &theme);
        // Buy | Sell, the picked side filled in its colour
        let side = |id: &'static str, name: &'static str, sell: bool| {
            let picked = ticket.sell == sell;
            let fill = sided(if sell { Side::Sell } else { Side::Buy }, &theme);
            let segment = div()
                .id(id)
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .font_weight(FontWeight::MEDIUM)
                .map(|segment| match picked {
                    true => segment.bg(fill).text_color(theme.primary_foreground),
                    false => segment
                        .text_color(theme.muted)
                        .hover(move |style| style.text_color(theme.foreground)),
                })
                .aria_toggled(picked.into())
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| view.pick_side(sell, cx)))
                .child(name);
            design::item(segment, Role::RadioButton, picked)
        };
        let sides = design::composite("trade-side", Role::RadioGroup, words::SIDE)
            .orientation(Orientation::Horizontal)
            .wrap()
            .active(usize::from(ticket.sell), 2)
            .on_move(
                cx.processor(|view: &mut Trade, side: usize, _, cx| view.pick_side(side == 1, cx)),
            )
            .build()
            .flex_none()
            .h(px(30.))
            .flex()
            .border_1()
            .border_color(theme.border_strong)
            .child(side("trade-buy", words::BUY, false))
            .child(side("trade-sell", words::SELL, true));
        let kinds = tabs(
            "trade-kind",
            words::ORDER_TYPE,
            ladder,
            &theme,
            usize::from(ticket.market),
            [
                ("trade-limit".into(), words::LIMIT.into(), None),
                ("trade-at-market".into(), words::MARKET.into(), None),
            ],
            cx.processor(|view: &mut Trade, kind: usize, _, cx| view.pick_kind(kind == 1, cx)),
        )
        .h_full();
        let kinds = div()
            .flex_none()
            .h(design::size::CONTROL)
            .border_b_1()
            .border_color(theme.border)
            .child(kinds);
        let typed =
            || cx.listener(|view: &mut Trade, _: &wire::TextChange, _, _| view.ticket.edited());
        let entered = || cx.listener(|view: &mut Trade, _: &(), _, cx| view.place(cx));
        // a field that holds no such number says so to who cannot see it
        let input =
            |id: &'static str, field: &TextField, name: &'static str, wrong: Option<String>| {
                let look = match (trades, &wrong) {
                    (false, _) => Look::Off,
                    (true, Some(_)) => Look::Bad,
                    (true, None) => Look::Plain,
                };
                let input = boxed(look, &theme, Input::new(id, field, name))
                    .disabled(!trades)
                    .on_change(typed())
                    .on_submit(entered());
                match wrong {
                    Some(wrong) => input.invalid(Invalid::True).description(wrong),
                    None => input,
                }
            };
        let price = match ticket.market {
            true => shown(words::BEST_PRICE.into(), Look::Off, &theme).into_any_element(),
            false => input(
                "trade-price",
                &ticket.price,
                words::PRICE,
                self.price_problem(&draft),
            )
            .into_any_element(),
        };
        let amount = input(
            "trade-amount",
            &ticket.amount,
            words::AMOUNT,
            self.amount_problem(&draft),
        );
        let mut column = div()
            .flex_none()
            .p(design::space::LG)
            .flex()
            .flex_col()
            .gap(design::space::MD)
            .child(sides)
            .child(kinds)
            .child(labelled(
                words::PRICE,
                price,
                (!ticket.market).then_some(quote),
                &theme,
            ))
            .child(labelled(words::AMOUNT, amount, Some(base), &theme));
        let label = self.place_label(&draft);
        if let Some(note) = barrier {
            return column
                .child(message(note, theme.surface, theme.muted))
                .child(go("trade-submit", label, None, &theme));
        }
        let shares = SHARES.map(|share| {
            let amount = self.affords(&draft, share);
            let picked = amount.is_some() && amount == draft.amount;
            div()
                .id(format!("trade-share-{share}"))
                .flex_1()
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .text_size(design::text::CAPTION)
                .map(|button| match picked {
                    true => button
                        .border_color(theme.foreground)
                        .text_color(theme.foreground),
                    false => button
                        .border_color(theme.border)
                        .text_color(theme.muted)
                        .hover(move |style| style.text_color(theme.foreground)),
                })
                .role(Role::Button)
                .aria_label(words::share(share))
                .aria_toggled(picked.into())
                .focusable()
                .on_click(
                    cx.listener(move |view, _: &ClickEvent, _, cx| view.take_share(share, cx)),
                )
                .child(format!("{share}%"))
        });
        let short = matches!(draft.problem, Some(Problem::Funds(_)));
        let total = draft.total.map(format::total).unwrap_or_default();
        let total = match (ticket.market, short) {
            (_, true) => shown(total, Look::Bad, &theme),
            (true, _) => shown(total, Look::Off, &theme),
            (false, _) => shown(total, Look::Plain, &theme),
        };
        let total_name = if ticket.market {
            words::ESTIMATED_TOTAL
        } else {
            words::TOTAL
        };
        column = column
            .child(div().flex().gap(design::space::XXS).children(shares))
            .child(labelled(total_name, total, Some(quote), &theme));
        // what is wrong takes the place of what is available
        column = match self.ticket_problem(&draft) {
            Some(wrong) => column.child(message(wrong, theme.danger_soft, theme.danger)),
            None => column.child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .text_size(design::text::SECONDARY)
                    .text_color(theme.muted)
                    .child(words::AVAILABLE)
                    .child(
                        design::mono(draft.funds(draft.free))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground),
                    ),
            ),
        };
        let ready = self.placeable(&draft);
        let place: Press =
            Box::new(cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.place(cx)));
        column.child(go(
            "trade-submit",
            label,
            ready.then_some((color, place)),
            &theme,
        ))
    }
}

/// How a field's box is drawn.
#[derive(Clone, Copy)]
enum Look {
    Plain,
    /// Not the reader's to fill: a market order's price, a ticket that
    /// places nothing.
    Off,
    /// The figure in it is what is wrong.
    Bad,
}

/// A field's box around `element`: the control's height, figures in mono
/// that end at the unit beside them.
fn boxed<E: Styled>(look: Look, theme: &Theme, element: E) -> E {
    let (edge, ground, ink) = match look {
        Look::Plain => (theme.border_strong, theme.background, theme.foreground),
        Look::Off => (theme.border, theme.surface, theme.muted),
        Look::Bad => (theme.danger, theme.background, theme.foreground),
    };
    element
        .h(design::size::CONTROL)
        .w_full()
        .pl(design::space::SM)
        // the unit sits over the box's right end
        .pr(px(46.))
        .border_1()
        .border_color(edge)
        .bg(ground)
        .text_color(ink)
        .font_family(design::fonts::FAMILY_MONO)
        .text_size(design::text::MONO)
        .text_right()
}

/// A figure the ticket works out, in a field's box: not the reader's to
/// type.
fn shown(text: String, look: Look, theme: &Theme) -> Div {
    boxed(look, theme, div())
        .flex()
        .items_center()
        .justify_end()
        .whitespace_nowrap()
        .child(text)
}

/// A field under its name, the unit of its figure at its right end.
fn labelled(
    name: &'static str,
    field: impl IntoElement,
    unit: Option<&'static str>,
    theme: &Theme,
) -> Div {
    let unit = unit.map(|unit| {
        div()
            .absolute()
            .right(design::space::SM)
            .top_0()
            .h_full()
            .w(px(30.))
            .flex()
            .items_center()
            .child(unit)
    });
    div()
        .flex()
        .flex_col()
        .gap(design::space::XXS)
        .text_size(design::text::CAPTION)
        .text_color(theme.muted)
        .child(name)
        .child(div().relative().child(field).children(unit))
}

/// A few words in the ticket, on their own ground: a note, or what is
/// wrong with the order.
fn message(text: String, ground: Hsla, ink: Hsla) -> Div {
    div()
        .px(design::space::SM)
        .py(px(7.))
        .bg(ground)
        .text_color(ink)
        .text_size(design::text::SECONDARY)
        .child(text)
}

/// What a press on the ticket's button does.
type Press = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// The ticket's button: filled in its side's colour with a press to hear,
/// greyed out and saying so with none.
fn go(
    id: &'static str,
    label: String,
    press: Option<(Hsla, Press)>,
    theme: &Theme,
) -> Stateful<Div> {
    let button = div()
        .id(id)
        .flex_none()
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .font_weight(FontWeight::MEDIUM)
        .role(Role::Button)
        .child(label);
    match press {
        Some((color, press)) => {
            design::focus_shown_on_ink(button.bg(color).text_color(theme.primary_foreground), theme)
                .focusable()
                .on_click(press)
        }
        None => button
            .bg(theme.surface_raised)
            .text_color(theme.muted)
            .aria_disabled(true),
    }
}
