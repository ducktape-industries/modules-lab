//! The desktop's call (`call.rs` is its model, `people.rs` what both trees
//! draw of the people): the grid the stage lays them in, the network row,
//! your thumbnail, the bar's Invite, and the rings on the check. `mod.rs`
//! places them in the room and on the check.
use view::prelude::*;
use view::{Div, FontWeight, Stateful};

use super::{CARET, MENU_POPUP, anchor, f, open};
use crate::call::STAGE;
use crate::look::{glass, white};
use crate::words;
use crate::{Huddle, Which};

const INVITE: &str = "user-plus";
const NETWORK: &str = "wifi-off";

/// The grid `n` tiles take on a stage `w` by `h`: columns and rows.
pub(super) fn grid(n: usize, w: f32, h: f32) -> (usize, usize) {
    match n {
        0 | 1 => (1, 1),
        2 if w >= h * 1.2 => (2, 1),
        2 => (1, 2),
        3 | 4 => (2, 2),
        _ => (3, 2),
    }
}

/// How many other people a stage of the room shows beside you.
pub(super) fn shown(others: usize) -> usize {
    others.min(STAGE - 1)
}

/// What the network row says, if anything: reconnecting, or the camera
/// sending less picture (the host's words).
pub(super) fn network(view: &Huddle) -> Option<String> {
    if view.reconnecting() {
        return Some(words::RECONNECTING.into());
    }
    let camera = &view.camera_state;
    (view.call.is_some()
        && view.camera_placed()
        && camera.status == MediaStatus::Live
        && !camera.detail.is_empty())
    .then(|| camera.detail.clone())
}

/// The network row under the stage: one line, no numbers.
pub(super) fn network_row(line: String, theme: &Theme) -> Stateful<Div> {
    div()
        .id("huddle-network")
        .h(px(22.))
        .flex_none()
        .px(design::space::LG)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .bg(theme.background)
        .text_size(design::text::CAPTION)
        .text_color(theme.warning)
        .role(Role::Status)
        .child(design::icon(NETWORK, px(12.)))
        .child(div().min_w(px(0.)).truncate().child(line))
}

/// The same line on the glass over the compact room's strip.
pub(super) fn network_glass(line: String) -> Stateful<Div> {
    div()
        .id("huddle-network")
        .absolute()
        .left(design::space::SM)
        .right(design::space::SM)
        .bottom(design::space::SM + design::size::CONTROL + design::space::XS)
        .h(px(22.))
        .px(design::space::XS)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .bg(glass())
        .text_color(white())
        .text_size(design::text::CAPTION)
        .role(Role::Status)
        .child(design::icon(NETWORK, px(12.)))
        .child(div().min_w(px(0.)).truncate().child(line))
}

/// Your thumbnail in the compact room with others: your tile a third of
/// the window wide, top right.
pub(super) fn thumbnail(you: Stateful<Div>, width: f32) -> Stateful<Div> {
    you.absolute()
        .top(design::space::SM)
        .right(design::space::SM)
        .w(px(width))
        .h(px(width * 9. / 16.))
        .border_1()
        .border_color(hsla(0., 0., 1., 0.25))
}

/// Invite in the room's bar: the people to ring, in a menu over it.
pub(super) fn invite(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> view::Sensor {
    let theme = *theme;
    let id = "huddle-invite";
    let expanded = view.menu.is_some_and(|menu| menu.which == Which::Invite);
    let button = div()
        .id(id)
        .flex_none()
        .h(px(32.))
        .px(design::space::MD)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .border_1()
        .border_color(theme.border_strong)
        .text_color(theme.foreground)
        .cursor_pointer()
        .hover(move |style| style.bg(theme.surface))
        .role(Role::Button)
        .aria_has_popup(MENU_POPUP)
        .aria_expanded(expanded)
        .focusable()
        .on_click(
            cx.listener(move |view: &mut Huddle, _: &ClickEvent, window, cx| {
                open(view, Which::Invite, id, window, cx)
            }),
        )
        .child(design::icon(INVITE, px(14.)))
        .child(words::INVITE)
        .child(design::icon(CARET, px(12.)).text_color(theme.muted));
    anchor(Which::Invite, id, button, cx)
}

/// The rings over the check's settings, newest first, each with Answer
/// and Decline; and why the last call did not go on.
pub(super) fn rings(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Option<Div> {
    let rings = view.rings();
    if rings.is_empty() && view.call_note.is_none() {
        return None;
    }
    let cards = rings.into_iter().map(|listed| {
        let id = listed.id.clone();
        let caller = view.name(listed.call.caller);
        let with = words::huddle_with(listed.call.people().count());
        let (answer, decline) = (id.clone(), id.clone());
        let answer = cx.listener(move |view: &mut Huddle, _: &ClickEvent, window, cx| {
            view.answer(answer.clone(), cx);
            window.focus(super::ROOM);
        });
        let decline = cx.listener(move |view: &mut Huddle, _: &ClickEvent, _, cx| {
            view.decline(decline.clone(), cx)
        });
        div()
            .id(SharedString::from(format!("huddle-ring-{id}")))
            .flex()
            .flex_col()
            .gap(design::space::SM)
            .p(design::space::MD)
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.surface)
            .role(Role::Alert)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(design::space::SM)
                    .child(design::avatar(&caller, px(28.), theme))
                    .child(
                        div()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(design::text::BODY)
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(words::calling(&caller)),
                            )
                            .when(!with.is_empty(), |text| {
                                text.child(
                                    div()
                                        .text_size(design::text::CAPTION)
                                        .text_color(theme.muted)
                                        .child(with),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(design::space::SM)
                    .child(
                        design::button(
                            format!("huddle-answer-{id}"),
                            words::ANSWER,
                            &design::POINTER,
                            theme,
                            answer,
                        )
                        .kind(design::Kind::Primary),
                    )
                    .child(
                        design::button(
                            format!("huddle-decline-{id}"),
                            words::DECLINE,
                            &design::POINTER,
                            theme,
                            decline,
                        )
                        .kind(design::Kind::Outline),
                    ),
            )
    });
    let note = view.call_note.clone().map(|note| {
        div()
            .id("huddle-call-note")
            .text_size(design::text::CAPTION)
            .text_color(theme.muted)
            .child(note)
    });
    Some(
        div()
            .flex()
            .flex_col()
            .gap(design::space::SM)
            .children(note)
            .children(cards),
    )
}

/// One of `count` cells across `span`, the grid's gap between them.
pub(super) fn cell(span: f32, count: usize) -> f32 {
    let gap = f(design::space::SM);
    (span - gap * (count as f32 - 1.)) / count as f32
}
