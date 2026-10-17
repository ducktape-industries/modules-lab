//! The phone's root (SPEC §2.1, board 20): the markets, each with its
//! last price and its day's move, and under them the reader's account,
//! their open orders and their balances, each a page of its own. A
//! reader with no account reads why there is none.
use trade::MARKETS;
use view::AnyElement;
use view::design::Nav;
use view::prelude::*;
use view::{Div, Stateful};

use super::LADDER;
use super::parts::{no_link, quiet, refused, scroller};
use crate::{Route, Trade, format, look, words};

pub(super) fn page(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let rows: Vec<_> = (0..MARKETS.len())
        .map(|market| market_row(view, market, cx, theme))
        .collect();
    let stats = view.stats.failed().map(|refusal| {
        let retry = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.read_stats(cx));
        refused("trade-stats", refusal, theme, retry)
    });
    let account = design::section_head("trade-account-head", words::YOUR_ACCOUNT, LADDER, theme);
    design::page("trade-markets", words::TRADE, Nav::Child, LADDER, theme)
        .caption(words::SPOT_MARKETS)
        .child(
            scroller("trade-markets-page")
                .children(no_link(view, theme))
                .child(design::section_head(
                    "trade-markets-head",
                    words::MARKETS,
                    LADDER,
                    theme,
                ))
                .children(stats)
                .children(rows)
                .child(account)
                .children(account_rows(view, cx, theme)),
        )
        .into_any_element()
}

/// A market's row: its name, its last price and how far that moved in a
/// day, or a dash for each while they are read; a press opens its page.
fn market_row(
    view: &Trade,
    market: usize,
    cx: &mut Context<Trade>,
    theme: &Theme,
) -> Stateful<Div> {
    let name = MARKETS[market].name;
    let stats = view.stats.ready().and_then(|stats| stats.get(market));
    let stats = stats.filter(|stats| stats.last > 0);
    let moved = stats.and_then(|stats| format::change(stats.last, stats.before));
    let decimals = format::decimals(MARKETS[market].tick);
    let last = stats.map_or("—".into(), |stats| format::fixed(stats.last, decimals));
    let figures = div()
        .flex_none()
        .flex()
        .flex_col()
        .items_end()
        .font_family(design::fonts::FAMILY_MONO)
        .child(
            div()
                .text_type(LADDER.body)
                .text_color(stats.map_or(theme.muted, |_| theme.foreground))
                .child(last),
        )
        .child(
            div()
                .text_type(LADDER.secondary)
                .text_color(look::moved(moved, theme.muted, theme))
                .child(moved.map_or("—".into(), format::percent)),
        );
    let open =
        cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| view.open_market(market, cx));
    design::pressed(design::row(format!("market-{name}"), LADDER, theme), LADDER)
        .min_h(LADDER.row_tall)
        .role(Role::Button)
        .focusable()
        .on_click(open)
        .child(
            design::row_words(design::row_title(name, LADDER, theme)).child(design::row_caption(
                words::PRACTICE_SPOT,
                LADDER,
                theme,
            )),
        )
        .child(figures)
        .child(design::chevron(LADDER, theme))
}

/// The reader's open orders and balances, each a row that opens its
/// page; with no account, why there is none.
fn account_rows(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> Vec<AnyElement> {
    if view.session.account.is_none() {
        return vec![quiet("trade-no-account", words::NO_ACCOUNT, theme).into_any_element()];
    }
    let resting = view.open.ready().map(|open| words::resting(open.len()));
    let balances = view.balances.ready().map(|balances| {
        let free = balances.iter().map(|balance| {
            let name = Trade::asset_name(balance.asset);
            format!("{} {name}", format::brief(balance.asset, balance.free))
        });
        words::joined(&free.collect::<Vec<_>>())
    });
    let open = |route: Route| {
        cx.listener(move |view: &mut Trade, _: &ClickEvent, _, cx| view.open_page(route, cx))
    };
    vec![
        opening_row(
            "trade-open-orders",
            "list",
            words::OPEN_ORDERS,
            resting,
            open(Route::Orders),
            theme,
        ),
        opening_row(
            "trade-balances",
            "wallet",
            words::BALANCES,
            balances,
            open(Route::Balances),
            theme,
        ),
    ]
}

/// A row that opens a page: its glyph, its title over what it holds
/// (once read), the chevron.
fn opening_row(
    id: &'static str,
    glyph: &'static str,
    title: &'static str,
    caption: Option<String>,
    press: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    theme: &Theme,
) -> AnyElement {
    let words = design::row_words(design::row_title(title, LADDER, theme))
        .children(caption.map(|caption| design::row_caption(caption, LADDER, theme)));
    design::pressed(design::row(id, LADDER, theme), LADDER)
        .min_h(LADDER.row_tall)
        .role(Role::Button)
        .focusable()
        .on_click(press)
        .child(design::lead(
            design::icon(glyph, LADDER.icon_row).text_color(theme.muted),
        ))
        .child(words)
        .child(design::chevron(LADDER, theme))
        .into_any_element()
}
