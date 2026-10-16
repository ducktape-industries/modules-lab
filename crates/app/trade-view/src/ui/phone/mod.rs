//! The phone's Trade, the view's second design (`wt/briefs/lab-phone/
//! design/SPEC.md` §2). The view's root picks it once, when the window's
//! pointer is a finger; nothing under here asks again.
//!
//! It reads the model (`lib.rs`, `data.rs`, `ticket.rs`, `chart.rs`) and
//! the words both designs show (`words.rs`), and writes only through the
//! model's verbs; it never reaches into the desktop's `ui/` files.
use view::AnyElement;
use view::prelude::*;

use crate::Trade;

/// The market on screen and its last price.
pub(crate) fn render(view: &Trade, cx: &mut Context<Trade>) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let ladder = &design::TOUCH;
    let last = view.market_stats().filter(|stats| stats.last > 0);
    div()
        .id("trade")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_type(ladder.body)
        .child(view.market().name)
        .children(last.map(|stats| view.price(stats.last)))
        .into_any_element()
}
