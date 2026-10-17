//! A market's page on the phone (SPEC §2.2, §2.4; boards 21, 22, 28): its
//! last price and day over the chart's spans and the chart, then the book,
//! the tape or the reader's open orders, one at a time; under it all, held
//! at the page's foot, Buy and Sell, which open the ticket's sheet. The
//! page is one scroller, the chart inside it.
use gpui::relative;
use trade::{SPANS, Side};
use view::AnyElement;
use view::design::space;
use view::prelude::*;
use view::{Div, FontWeight, Stateful};

use super::parts::{Ready, column_names, level_row, quiet, refused, scroller, side_button};
use super::{LADDER, chart, orders};
use crate::book::{Depth, Rung};
use crate::data::STEPS;
use crate::look::{moved, sided, ticked};
use crate::{Segment, Sheet, Trade, format, words};

/// The last price, the figure the page leads with.
const LAST: Pixels = px(28.);
/// The middle row's last price.
const MIDDLE_LAST: Pixels = px(17.);
/// The prices a side of the book the phone draws, of the
/// [`DEPTH`](crate::data::DEPTH) read.
const PHONE_LEVELS: usize = 6;
/// The row between the book's sides, and the book's foot.
const MIDDLE_H: Pixels = px(44.);
const FOOT_H: Pixels = px(32.);
/// The bar of the bids' share in the book's foot.
const SHARE_BAR: Pixels = px(4.);
/// The Buy and Sell buttons at the page's foot.
const SIDE_H: Pixels = px(48.);
/// A small button's face under a finger, as the kit's `button().small()`
/// draws it.
const SMALL: Pixels = px(36.);
/// The step opener's chevron.
const CHEVRON: Pixels = px(14.);

pub(super) fn page(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let segment = match view.segment {
        Segment::Book => book(view, cx, theme),
        Segment::Trades => tape(view, cx, theme),
        Segment::Orders => orders::open(view, cx, theme),
    };
    design::page(
        "trade-market",
        view.market().name,
        super::back(view, cx),
        LADDER,
        theme,
    )
    .caption(words::PRACTICE_SPOT)
    .child(
        scroller("trade-market-page")
            .child(price_head(view, theme))
            .child(tools(view, cx, theme))
            .child(chart::chart(view, cx, theme))
            .child(segments(view, cx, theme))
            .child(segment),
    )
    .child(foot(view, cx, theme))
    .into_any_element()
}

/// The market's last price and how far it moved in a day, in the move's
/// colour, over the day's high, low and volume.
fn price_head(view: &Trade, theme: &Theme) -> Stateful<Div> {
    let stats = view.market_stats().filter(|stats| stats.last > 0);
    let change = stats.and_then(|stats| format::change(stats.last, stats.before));
    let tone = moved(change, theme.foreground, theme);
    // a day nobody traded in has no high and no low to tell
    let traded = stats.filter(|stats| stats.high > 0);
    let fact = |label: &'static str, value: Option<String>| {
        div()
            .flex()
            .flex_col()
            .text_type(LADDER.caption)
            .text_color(theme.muted)
            .child(label)
            .child(
                div()
                    .font_family(design::fonts::FAMILY_MONO)
                    .text_type(LADDER.secondary)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(value.unwrap_or("—".into())),
            )
    };
    let figures = div()
        .flex()
        .items_baseline()
        .gap(space::SM)
        .font_family(design::fonts::FAMILY_MONO)
        .font_weight(FontWeight::MEDIUM)
        .text_color(tone)
        .child(
            div()
                .text_size(LAST)
                .child(stats.map_or("—".into(), |stats| view.price(stats.last))),
        )
        .child(
            div()
                .text_type(LADDER.body)
                .child(change.map_or("—".into(), format::percent)),
        );
    div()
        .id("trade-price-head")
        .pt(space::LG)
        .px(space::BLOCK)
        .pb(space::SM)
        .flex()
        .flex_col()
        .gap(space::XS)
        .child(figures)
        .child(
            div()
                .flex()
                .gap(space::BLOCK)
                .child(fact(
                    words::HIGH_24H,
                    traded.map(|stats| view.price(stats.high)),
                ))
                .child(fact(
                    words::LOW_24H,
                    traded.map(|stats| view.price(stats.low)),
                ))
                .child(fact(words::VOLUME_24H, view.day_volume())),
        )
}

/// The chart's spans, and what opens the sheet of what it draws.
fn tools(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> Div {
    let spans = SPANS.iter().map(|span| {
        let id = ElementId::Name(format!("trade-span-{}", span.name).into());
        (id, span.name.into())
    });
    let spans = design::segmented(
        "trade-spans",
        words::CANDLE_SPAN,
        LADDER,
        theme,
        Some(view.chart.span),
        spans,
        cx.processor(|view: &mut Trade, span: usize, _, cx| view.pick_span(span, cx)),
    );
    let settings = design::icon_button(
        "trade-chart-settings",
        design::icon("sliders-horizontal", LADDER.icon),
        words::CHART_SETTINGS,
        LADDER,
        theme,
        cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| {
            view.open_sheet(Sheet::ChartSettings, cx)
        }),
    );
    // the settings' finger box reaches into the gutter by its glyph's
    // inset, so the glyph ends on the content's edge and the spans take
    // that room: at 360 each span's label then fits whole
    let inset = (LADDER.hit - LADDER.icon) / 2.;
    div()
        .py(space::SM)
        .px(space::BLOCK)
        .flex()
        .items_center()
        .child(spans.flex_1().min_w_0())
        .child(settings.mr(-inset))
}

/// Book, trades or orders, under the chart.
fn segments(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> Div {
    const SEGMENTS: [(Segment, &str, &str); 3] = [
        (Segment::Book, "trade-segment-book", words::BOOK),
        (Segment::Trades, "trade-segment-trades", words::TRADES),
        (Segment::Orders, "trade-segment-orders", words::ORDERS),
    ];
    let picked = SEGMENTS
        .iter()
        .position(|(segment, ..)| *segment == view.segment);
    let choices = SEGMENTS.map(|(_, id, name)| (id.into(), name.into()));
    let segmented = design::segmented(
        "trade-segment",
        words::BOOK_TRADES_ORDERS,
        LADDER,
        theme,
        picked,
        choices,
        cx.processor(|view: &mut Trade, segment: usize, _, cx| {
            view.pick_segment(SEGMENTS[segment].0, cx)
        }),
    );
    div()
        .pt(space::XXS)
        .px(space::BLOCK)
        .flex()
        .flex_col()
        .child(segmented)
}

/// The book: its head with the price step, the column names, six asks
/// over the last price and the spread over six bids, and the bids' share.
fn book(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let (base, quote) = view.assets();
    let step = view.step_name(STEPS[view.step]);
    let opener = step_opener(step, cx, theme);
    let head =
        design::section_head("trade-book-head", words::ORDER_BOOK, LADDER, theme).child(opener);
    let names = [
        words::price_in(quote),
        words::amount_in(base),
        words::TOTAL.into(),
    ];
    let body = match (&view.book, view.book.ready()) {
        (Loadable::Failed(refusal), _) => {
            let retry = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.read_book(cx));
            refused("trade-book", refusal, theme, retry).into_any_element()
        }
        (_, Some(book)) if book.asks.is_empty() && book.bids.is_empty() => {
            quiet("trade-book-empty", words::EMPTY_BOOK, theme).into_any_element()
        }
        (_, Some(book)) => levels(view, &view.depth(book, PHONE_LEVELS), cx, theme),
        _ => div().into_any_element(),
    };
    div()
        .flex()
        .flex_col()
        .child(head)
        .child(column_names(names, theme))
        .child(body)
        .into_any_element()
}

/// The control that opens the price step's sheet: the step, and a
/// chevron that says it opens.
// SDK gap: a small button with a trailing glyph
fn step_opener(step: String, cx: &mut Context<Trade>, theme: &Theme) -> Stateful<Div> {
    let reach = (LADDER.hit - SMALL) / 2.;
    let face = div()
        .h(SMALL)
        .px(space::LG)
        .flex()
        .items_center()
        .gap(space::XS)
        .border_1()
        .border_color(theme.border_strong)
        .text_type(LADDER.secondary)
        .text_color(theme.foreground)
        .child(step.clone())
        .child(design::icon("chevron-down", CHEVRON));
    let press = div()
        .id("trade-step")
        .flex_none()
        .h(LADDER.hit)
        .my(-reach)
        .flex()
        .items_center()
        .role(Role::Button)
        .aria_label(words::step_named(&step))
        .focusable()
        .on_click(
            cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.open_sheet(Sheet::Step, cx)),
        )
        .child(face);
    design::pressed(press, LADDER)
}

/// The book's rungs: the asks from the worst down to the best, the last
/// price between the sides, the bids from the best down, and the bids'
/// share of it all. A press on a rung opens the ticket at its price, on
/// the side that trades against it.
fn levels(view: &Trade, depth: &Depth, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let rung = |side: Side, at: usize, rung: &Rung, cx: &mut Context<Trade>| {
        let (name, soft) = match side {
            Side::Sell => ("ask", theme.danger_soft),
            Side::Buy => ("bid", theme.success_soft),
        };
        let price = rung.price;
        let bar = div()
            .absolute()
            .right_0()
            .top(px(1.))
            .bottom(px(1.))
            .w(relative(depth.reach(rung)))
            .bg(soft);
        let row = level_row(
            Some(bar),
            format::fixed(price, depth.places),
            sided(side, theme),
            format::fixed(rung.amount, depth.sized),
            format::fixed(rung.running, depth.sized),
            theme,
        );
        // the book's rows are the tape's height: a dense list, each row
        // the press
        let row = row
            .id(format!("book-{name}-{at}"))
            .role(Role::Button)
            .focusable()
            .on_click(cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| {
                view.take_level(price, side, cx)
            }));
        design::dense(design::pressed(row, LADDER))
    };
    let mut asks: Vec<_> = (depth.asks.iter().enumerate())
        .map(|(at, level)| rung(Side::Sell, at, level, cx))
        .collect();
    asks.reverse();
    let bids: Vec<_> = (depth.bids.iter().enumerate())
        .map(|(at, level)| rung(Side::Buy, at, level, cx))
        .collect();
    let share = depth.bid_share();
    let split = |text: String, tone: Hsla| {
        div()
            .font_family(design::fonts::FAMILY_MONO)
            .text_type(LADDER.caption)
            .text_color(tone)
            .child(text)
    };
    let foot = div()
        .h(FOOT_H)
        .px(space::BLOCK)
        .flex()
        .items_center()
        .gap(space::SM)
        .border_t_1()
        .border_color(theme.border)
        .child(split(words::bids_share(share), theme.success))
        .child(
            div()
                .flex_1()
                .h(SHARE_BAR)
                .flex()
                .child(div().w(relative(share as f32 / 100.)).bg(theme.success))
                .child(div().flex_1().bg(theme.danger)),
        )
        .child(split(words::asks_share(100 - share), theme.danger));
    div()
        .flex()
        .flex_col()
        .children(asks)
        .child(middle(view, depth, theme))
        .children(bids)
        .child(foot)
        .into_any_element()
}

/// The row between the book's sides: the last price, which way it moved
/// from the trade before, and the spread.
fn middle(view: &Trade, depth: &Depth, theme: &Theme) -> Div {
    let (last, change) = view.last_trade();
    let (tone, arrow) = ticked(change, theme);
    let spread = depth.spread.map_or("—".into(), |spread| view.price(spread));
    div()
        .h(MIDDLE_H)
        .px(space::BLOCK)
        .flex()
        .items_center()
        .justify_between()
        .border_t_1()
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .gap(space::XS)
                .font_family(design::fonts::FAMILY_MONO)
                .text_size(MIDDLE_LAST)
                .font_weight(FontWeight::MEDIUM)
                .text_color(tone)
                .child(last.map_or("—".into(), |last| view.price(last)))
                .child(arrow),
        )
        .child(
            div()
                .flex()
                .gap(space::XS)
                .text_type(LADDER.caption)
                .text_color(theme.muted)
                .child(words::SPREAD)
                .child(div().font_family(design::fonts::FAMILY_MONO).child(spread)),
        )
}

/// The market's last trades, newest first, each price in the colour of
/// the side that took it.
fn tape(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let (base, quote) = view.assets();
    let names = [
        words::price_in(quote),
        words::amount_in(base),
        words::TIME.into(),
    ];
    let trades = view.trades.ready();
    let sized = format::places(trades.into_iter().flatten().map(|trade| trade.amount));
    let rows = match (&view.trades, trades) {
        (Loadable::Failed(refusal), _) => {
            let retry = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.read_trades(cx));
            vec![refused("trade-tape", refusal, theme, retry).into_any_element()]
        }
        (_, Some(trades)) if trades.is_empty() => {
            vec![quiet("trade-tape-empty", words::NO_TRADES, theme).into_any_element()]
        }
        (_, Some(trades)) => (trades.iter())
            .map(|trade| {
                level_row(
                    None,
                    view.price(trade.price),
                    sided(trade.taker, theme),
                    format::fixed(trade.amount, sized),
                    format::time(trade.time),
                    theme,
                )
                .into_any_element()
            })
            .collect(),
        _ => Vec::new(),
    };
    div()
        .id("trade-tape")
        .flex()
        .flex_col()
        .child(design::section_head(
            "trade-tape-head",
            words::TRADES,
            LADDER,
            theme,
        ))
        .child(column_names(names, theme))
        .children(rows)
        .into_any_element()
}

/// Buy and Sell, held at the page's foot; while the reader may not trade,
/// both off under the one line that says why.
fn foot(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> Div {
    let why = view.why_not_trading();
    let ready = match why {
        Some(_) => Ready::Off,
        None => Ready::On,
    };
    let side = |id: &'static str, side: Side, cx: &mut Context<Trade>| {
        let sell = side == Side::Sell;
        let open =
            cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| view.open_ticket(sell, cx));
        side_button(id, side, words::side(side), SIDE_H, ready, theme, open)
    };
    div()
        .flex_none()
        .pt(space::LG)
        .px(space::BLOCK)
        .flex()
        .flex_col()
        .gap(space::SM)
        .border_t_1()
        .border_color(theme.border)
        .children(why.map(|why| {
            div()
                .text_type(LADDER.secondary)
                .text_color(theme.muted)
                .child(why)
        }))
        .child(
            div()
                .flex()
                .gap(space::SM)
                .child(side("trade-buy", Side::Buy, cx))
                .child(side("trade-sell", Side::Sell, cx)),
        )
}
