//! A call ringing you (no board: a new screen in the Expo kit's look, as
//! the call's SPEC §8 names it): the caller's face and name in the middle,
//! and under them, in the thumb's reach, Decline and Answer, each a square.
//! It stands in for the check while the program lists a ring.
//!
//! Reads the ring and the caller's name. Writes the answer (into the room,
//! at once) or the decline.
use huddle::Listed;
use view::design::{Ladder, Nav};
use view::prelude::*;
use view::{Div, FontWeight, Stateful};

use crate::Huddle;
use crate::look::{LEAVE, white};
use crate::words;

/// The caller's face.
const FACE: Pixels = px(72.);

/// Decline and Answer: squares in the thumb's reach, as the room's bar's.
const BUTTON: Pixels = px(56.);

/// Answer's glyph, by the Lucide name `design::icon` draws.
const ANSWER: &str = "phone";

pub(super) fn screen(
    view: &Huddle,
    ring: &Listed,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    let caller = view.name(ring.call.caller);
    let with = words::huddle_with(ring.call.people().count());
    design::page("huddle-incoming", words::HUDDLE, Nav::Child, ladder, theme)
        .child(
            div()
                .id("huddle-incoming-caller")
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(design::space::LG)
                .px(design::space::BLOCK)
                .role(Role::Alert)
                .aria_live(view::accesskit::Live::Assertive)
                .child(design::avatar(&caller, FACE, theme))
                .child(
                    div()
                        .text_type(ladder.title)
                        .font_weight(FontWeight::MEDIUM)
                        .text_center()
                        .child(words::calling(&caller)),
                )
                .when(!with.is_empty(), |caller| {
                    caller.child(
                        div()
                            .text_type(ladder.secondary)
                            .text_color(theme.muted)
                            .child(with),
                    )
                }),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .justify_center()
                .gap(px(56.))
                .py(design::space::BLOCK)
                .child(answer_button(
                    ("huddle-decline", LEAVE, words::DECLINE),
                    theme.danger,
                    ring.id.clone(),
                    |view, id, cx| view.decline(id, cx),
                    cx,
                    ladder,
                ))
                .child(answer_button(
                    ("huddle-answer", ANSWER, words::ANSWER),
                    theme.success,
                    ring.id.clone(),
                    |view, id, cx| view.answer(id, true, cx),
                    cx,
                    ladder,
                )),
        )
}

/// One of the two answers: a filled square with its glyph, its word under
/// it.
fn answer_button(
    (id, glyph, name): (&'static str, &'static str, &'static str),
    fill: Hsla,
    call: String,
    act: fn(&mut Huddle, String, &mut Context<Huddle>),
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
) -> Div {
    let square: Stateful<Div> = div()
        .id(id)
        .size(BUTTON)
        .flex()
        .items_center()
        .justify_center()
        .bg(fill)
        .text_color(white())
        .role(Role::Button)
        .aria_label(name)
        .child(design::icon(glyph, ladder.icon));
    let square = design::pressed(square, ladder).focusable().on_click(
        cx.listener(move |view: &mut Huddle, _: &ClickEvent, _, cx| act(view, call.clone(), cx)),
    );
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(design::space::SM)
        .child(square)
        .child(div().text_type(ladder.caption).child(name))
}
