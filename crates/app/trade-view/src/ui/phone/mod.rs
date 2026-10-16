//! The phone's Trade, the view's second design (`wt/briefs/lab-phone/
//! design/SPEC.md` §2). The view's root picks it once, when the window's
//! pointer is a finger; nothing under here asks again.
//!
//! The markets are the root (`markets.rs`). A market's page goes over
//! them (`market.rs`, its chart `chart.rs`), with a sheet over it: the
//! order ticket (`ticket.rs`), what the chart draws or the book's price
//! step (`sheets.rs`). The reader's orders and balances are two more
//! pages over the markets (`orders.rs`, `balances.rs`). `parts.rs` holds
//! the pieces two or more of them draw.
//!
//! It reads the model (`lib.rs`, `data.rs`, `ticket.rs`, `chart.rs`,
//! `book.rs`, `orders.rs`), the chart's overlay (`look.rs`) and the words
//! both designs show (`words.rs`), and writes only through the model's
//! verbs; it never reaches into the desktop's `ui/` files.
use view::AnyElement;
use view::design::{Ladder, Nav};
use view::prelude::*;

use crate::{Route, Sheet, Trade};

mod balances;
mod chart;
mod market;
mod markets;
mod orders;
mod parts;
mod sheets;
mod ticket;

/// Every size and type under here: the touch ones.
const LADDER: &Ladder = &design::TOUCH;

/// The page on top of the phone's stack, the sheet over it, and the
/// phone's Back while there is somewhere to go back to.
pub(crate) fn render(view: &Trade, cx: &mut Context<Trade>) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let page = match view.pages.top() {
        Route::Markets => markets::page(view, cx, &theme),
        Route::Market => market::page(view, cx, &theme),
        Route::Orders => orders::page(view, cx, &theme),
        Route::Balances => balances::page(view, cx, &theme),
    };
    // a sheet is the market page's, and goes with it
    let sheet = match (view.pages.top(), view.sheet) {
        (Route::Market, Some(Sheet::Ticket)) => Some(ticket::sheet(view, cx, &theme)),
        (Route::Market, Some(Sheet::ChartSettings)) => Some(sheets::chart(view, cx, &theme)),
        (Route::Market, Some(Sheet::Step)) => Some(sheets::step(view, cx, &theme)),
        _ => None,
    };
    // Back undoes what is drawn: the sheet, else the page over the
    // markets; with neither, the host leaves the view
    let back = (sheet.is_some() || view.pages.depth() > 0)
        .then(|| cx.listener(|view: &mut Trade, _: &BackEvent, _, cx| view.back(cx)));
    div()
        .id("trade")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_type(LADDER.body)
        .child(page)
        .children(sheet)
        .when_some(back, |root, back| root.on_back(back))
        .into_any_element()
}

/// A page over the markets, and its Back, which runs the phone's.
fn back(view: &Trade, cx: &mut Context<Trade>) -> Nav {
    let back = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.back(cx));
    Nav::back(view.pages.depth(), back)
}
