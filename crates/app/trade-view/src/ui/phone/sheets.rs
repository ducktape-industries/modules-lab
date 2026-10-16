//! The two small sheets over a market's page (SPEC §2.7, board 27): what
//! the chart draws, a switch each, which stays open while they are
//! switched; and the book's price step, a row each, which closes on a
//! pick.
use view::AnyElement;
use view::design::{SHEET_INSET, space};
use view::prelude::*;
use view::{Div, Stateful};

use super::LADDER;
use crate::chart::SWITCHES;
use crate::data::STEPS;
use crate::{Trade, words};

/// The picked step's check.
const CHECK: Pixels = px(20.);
/// The chart's switches as their rows name them, in [`SWITCHES`]' order:
/// the line's stands alone on its row, so it says what it trades.
const ROWS: [&str; SWITCHES.len()] = [words::MA7, words::MA25, words::VOLUME, words::LINE_INSTEAD];

/// What the chart draws besides its candles: the two averages, the
/// volume, and the line in place of the candles.
pub(super) fn chart(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let on = view.chart.switches();
    let rows = ROWS.into_iter().enumerate().map(|(index, name)| {
        let toggle = cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| {
            view.chart.switch(index);
            cx.notify();
        });
        let id = ElementId::Name(format!("trade-switch-{index}").into());
        switch_row(id, name, on[index], toggle, theme)
    });
    let rows: Vec<_> = rows.collect();
    let dismiss = cx.listener(|view: &mut Trade, _: &(), _, cx| view.close_sheet(cx));
    design::sheet("trade-chart-sheet", words::CHART_SETTINGS, LADDER, theme)
        .on_dismiss(dismiss)
        .child(design::sheet_title(
            "trade-chart-settings-head",
            words::CHART,
            LADDER,
            theme,
        ))
        .child(div().flex().flex_col().pb(space::BLOCK).children(rows))
        .into_any_element()
}

/// One of what the chart draws: its name, and its switch.
fn switch_row(
    id: ElementId,
    name: &'static str,
    on: bool,
    toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    theme: &Theme,
) -> Div {
    div()
        .min_h(LADDER.sheet_row)
        .px(SHEET_INSET)
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(theme.border)
        .child(name)
        .child(design::switch(id, name, on, true, LADDER, theme, toggle))
}

/// The book's price steps, the picked one checked.
pub(super) fn step(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let (_, quote) = view.assets();
    let rows = STEPS.iter().enumerate().map(|(index, step)| {
        let picked = index == view.step;
        let pick = cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| {
            view.pick_step(index, cx);
            view.close_sheet(cx);
        });
        let id = ElementId::Name(format!("trade-step-{index}").into());
        let row: Stateful<Div> =
            design::sheet_row(id, view.step_name(*step), LADDER, theme, pick).into_element();
        row.aria_toggled(picked.into())
            .when(picked, |row| row.child(design::icon("check", CHECK)))
    });
    let rows: Vec<_> = rows.collect();
    let head = design::sheet_title("trade-step-head", words::PRICE_STEP, LADDER, theme).child(
        div()
            .text_type(LADDER.caption)
            .text_color(theme.muted)
            .child(words::per_row(quote)),
    );
    let dismiss = cx.listener(|view: &mut Trade, _: &(), _, cx| view.close_sheet(cx));
    design::sheet("trade-step-sheet", words::PRICE_STEP, LADDER, theme)
        .on_dismiss(dismiss)
        .child(head)
        .child(div().flex().flex_col().pb(space::BLOCK).children(rows))
        .into_any_element()
}
