//! The pieces two or more phone screens draw: what scrolls under a title
//! row, a quiet line, a refused read, the note that there is no link, a
//! section head's action, a row of the book or the tape, an order's row,
//! a button filled in a side's colour, and the line of a write on its
//! way. Each draws what it is handed.
use trade::{ASSETS, Side};
use view::accesskit::Live;
use view::design::space;
use view::prelude::*;
use view::{Div, FontWeight, Stateful};

use super::LADDER;
use crate::look::sided;
use crate::orders::market_of;
use crate::{Trade, format, words};

/// A row of the book or the tape.
const LEVEL_H: Pixels = px(36.);
/// The row naming a list's three columns.
const NAMES_H: Pixels = px(28.);
/// A glyph that rides a line of text: the note's, a write's spinner.
const NOTE_GLYPH: Pixels = px(18.);
const SPINNER: Pixels = px(14.);

/// What scrolls under a page's title row.
// SDK gap: a touch scroller with no bar gutter: the SDK keeps 16 on the
// right of every vertical scroller for a pointer's bar, so a phone page's
// content stops 16 short of the screen's edge
pub(super) fn scroller(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
}

/// One quiet line in a list's place: nothing in it, or why.
pub(super) fn quiet(
    id: impl Into<ElementId>,
    words: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .px(space::BLOCK)
        .py(space::LG)
        .text_type(LADDER.secondary)
        .text_color(theme.muted)
        .child(words.into())
}

/// A refused read, said as the node said it, and Retry.
pub(super) fn refused(
    name: &str,
    refusal: &Error,
    theme: &Theme,
    retry: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    design::refused(name, design::sentence(refusal), LADDER, theme, retry)
}

/// A refused write, the program's own words, over what it was about.
pub(super) fn refusal(view: &Trade, theme: &Theme) -> Option<Stateful<Div>> {
    (!view.refusal.is_empty()).then(|| {
        div()
            .id("trade-refusal")
            .px(space::BLOCK)
            .py(space::MD)
            .bg(theme.danger_soft)
            .text_type(LADDER.secondary)
            .text_color(theme.danger)
            .role(Role::Alert)
            .aria_live(Live::Assertive)
            .child(view.refusal.clone())
    })
}

/// The note that there is no link to a node: what is shown is the last
/// read, and nothing is sent.
// SDK gap: a note strip (a ground, a glyph and a sentence)
pub(super) fn no_link(view: &Trade, theme: &Theme) -> Option<Stateful<Div>> {
    (!view.session.connected).then(|| {
        div()
            .id("trade-no-link")
            .flex()
            .items_center()
            .gap(space::MD)
            .px(space::BLOCK)
            .py(space::MD)
            .bg(theme.warning_soft)
            .text_type(LADDER.secondary)
            .text_color(theme.warning)
            .role(Role::Status)
            .aria_live(Live::Polite)
            .child(design::icon("wifi-off", NOTE_GLYPH).flex_none())
            .child(div().flex_1().min_w_0().child(words::NO_LINK))
    })
}

/// A section head's one action, at its right: medium in the foreground,
/// or muted and saying so while it may not run or is on its way. Its
/// press reaches a finger's height over and under its line.
// SDK gap: a section head with a trailing action
pub(super) fn head_action(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    busy: bool,
    theme: &Theme,
    press: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let line = LADDER.caption.line.unwrap_or(LADDER.caption.size);
    let reach = (LADDER.hit - line) / 2.;
    let action = div()
        .id(id)
        .flex_none()
        .h(LADDER.hit)
        .my(-reach)
        .flex()
        .items_center()
        .font_weight(FontWeight::MEDIUM)
        .role(Role::Button)
        .child(label);
    match enabled && !busy {
        true => design::pressed(action, LADDER)
            .text_color(theme.foreground)
            .focusable()
            .on_click(press),
        false => action
            .text_color(theme.muted)
            .aria_disabled(true)
            .when(busy, |action| action.aria_busy(true)),
    }
}

/// A row of the book or the tape: three columns of figures, the first in
/// `tone`, the last muted, over the `bar` of a rung's depth.
// SDK gap: a 36-tall mono level row with a depth bar
pub(super) fn level_row(
    bar: Option<Div>,
    first: String,
    tone: Hsla,
    second: String,
    third: String,
    theme: &Theme,
) -> Div {
    div()
        .relative()
        .flex_none()
        .h(LEVEL_H)
        .px(space::BLOCK)
        .flex()
        .items_center()
        .gap(space::SM)
        .font_family(design::fonts::FAMILY_MONO)
        .text_type(LADDER.secondary)
        .whitespace_nowrap()
        .children(bar)
        .child(div().flex_1().text_color(tone).child(first))
        .child(column().text_color(theme.foreground).child(second))
        .child(column().text_color(theme.muted).child(third))
}

/// A column of figures that ends at its right edge.
fn column() -> Div {
    div().flex_1().flex().justify_end()
}

/// What a list's three columns hold, quiet, over its rows.
pub(super) fn column_names(names: [String; 3], theme: &Theme) -> Div {
    let [first, second, third] = names;
    div()
        .flex_none()
        .h(NAMES_H)
        .px(space::BLOCK)
        .flex()
        .items_center()
        .gap(space::SM)
        .text_type(LADDER.caption)
        .text_color(theme.muted)
        .whitespace_nowrap()
        .child(div().flex_1().child(first))
        .child(column().child(second))
        .child(column().child(third))
}

/// What an order or a fill is, on its row's first line: its side in its
/// colour, how much of the market's base, the market.
pub(super) fn traded(side: Side, amount: u64, market: u8, theme: &Theme) -> Div {
    let market = market_of(market);
    let base = ASSETS[market.base as usize].name;
    div()
        .min_w_0()
        .flex()
        .items_baseline()
        .gap(space::XS)
        .whitespace_nowrap()
        .overflow_hidden()
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(sided(side, theme))
                .child(words::side(side)),
        )
        .child(
            div()
                .font_family(design::fonts::FAMILY_MONO)
                .child(format!("{} {base}", format::amount(amount))),
        )
        .child(
            div()
                .text_type(LADDER.secondary)
                .text_color(theme.muted)
                .child(words::in_market(market.name)),
        )
}

/// An order's or a fill's row: what it is over `caption`, and what
/// trails it.
pub(super) fn order_row(
    id: impl Into<ElementId>,
    traded: Div,
    caption: String,
    theme: &Theme,
) -> Stateful<Div> {
    design::row(id, LADDER, theme)
        .h(LADDER.row_tall)
        .child(design::row_words(traded).child(design::row_caption(caption, LADDER, theme)))
}

/// A button filled in `side`'s colour, `height` tall, the whole of its
/// width the press: grey and saying so while it may not run, its fill
/// kept and saying so while what it sent is on its way.
// SDK gap: a button filled in a side's colour
pub(super) fn side_button(
    id: &'static str,
    side: Side,
    label: impl IntoElement,
    height: Pixels,
    state: Ready,
    theme: &Theme,
    press: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let button = div()
        .id(id)
        .flex_1()
        .h(height)
        .flex()
        .items_center()
        .justify_center()
        .gap(space::SM)
        .font_weight(FontWeight::MEDIUM)
        .role(Role::Button)
        .child(label);
    match state {
        Ready::Off => button
            .bg(theme.surface_raised)
            .text_color(theme.faint)
            .aria_disabled(true),
        Ready::Busy => button
            .bg(sided(side, theme))
            .text_color(theme.primary_foreground)
            .aria_disabled(true)
            .aria_busy(true),
        Ready::On => design::pressed(button, LADDER)
            .bg(sided(side, theme))
            .text_color(theme.primary_foreground)
            .focusable()
            .on_click(press),
    }
}

/// Whether a [`side_button`] runs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ready {
    On,
    /// It may not run: nothing to send, or nobody to send it.
    Off,
    /// What it sent is on its way.
    Busy,
}

/// A write on its way, in so many words, after a spinner.
// SDK gap: a loading line (a spinner and quiet words)
pub(super) fn waiting(words: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(space::XS)
        .child(design::icon("loader-circle", SPINNER))
        .child(words)
}
