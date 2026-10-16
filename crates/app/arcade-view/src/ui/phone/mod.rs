//! The phone's Arcade, the view's second design (`wt/briefs/lab-phone/
//! design/SPEC.md` §1, boards 10 to 13; Expo has no arcade, so these are
//! new screens in the Expo kit's look, `design::TOUCH`). The view's root
//! picks it once, when the window's pointer is a finger; nothing under
//! here asks again.
//!
//! On a phone no game is played: the boards are read. Nothing here deals
//! a game or opens versus, so the clock, the live tracks and the keys are
//! never asked for, and no score is posted.
//!
//! It reads the model (`lib.rs`, `data.rs`) and the words both designs
//! show (`words.rs`), and writes only through the model's verbs; it never
//! reaches into the desktop's files. One file per screen:
//! - `hub.rs`: the games, each with the reader's best;
//! - `board.rs`: a game's board, pushed over the hub.
use view::AnyElement;
use view::prelude::*;

use crate::Arcade;

mod board;
mod hub;

/// The screen the open board picks: the hub, or the board over it. Back
/// closes the board, and is there exactly while one is drawn: on the hub
/// the way out is the host's capsule.
pub(crate) fn render(view: &Arcade, cx: &mut Context<Arcade>) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let ladder = &design::TOUCH;
    // what is drawn, decided once
    let open = view.board.game;
    let screen = match open {
        None => hub::screen(view, cx, ladder, &theme).into_any_element(),
        Some(which) => board::screen(view, which, cx, ladder, &theme).into_any_element(),
    };
    div()
        .id("arcade")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_type(ladder.body)
        .when(open.is_some(), |root| {
            root.on_back(cx.listener(|view: &mut Arcade, _: &BackEvent, _, cx| {
                view.close_board();
                cx.notify();
            }))
        })
        .child(screen)
        .into_any_element()
}
