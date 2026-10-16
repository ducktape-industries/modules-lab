//! The room (phone board 30, `design-phone/SPEC.md` §2; no Expo source: a
//! new screen in the Expo kit's look). Its title row holds the clock and
//! who is here; your tile fills the stage, the picture covering it, with
//! the chip, a ring while you speak, and the line that says calls with
//! others are not connected yet; under it, in the thumb's reach, the bar:
//! the microphone, the camera, the flip, the Sound route and Leave. The
//! view stays where it is (a phone keeps one window): no pin, no float.
//!
//! Reads the picks and switches, what the pieces told and the room's
//! clock. Writes the toggles, the flip, a retry, the sound sheet's opening
//! and the check (`leave_room`).
use view::design::Ladder;
use view::prelude::*;
use view::{Div, FontWeight, Stateful};

use super::SOUND;
use super::parts::{self, FLIP};
use crate::look::{self, LEAVE, MIC, MIC_OFF, SPEAKER, VIDEO, VIDEO_OFF, black, glass, white};
use crate::words::{self, Device};
use crate::{Huddle, Kind};

/// Your face while the camera is off (SPEC §2.1).
const FACE: Pixels = px(72.);

/// The bar's buttons, square in the thumb's reach (SPEC §2).
const BAR_BUTTON: Pixels = px(56.);

/// How far over the tile's foot the solo line sits, and how wide it runs
/// (SPEC §2.1).
const SOLO_ABOVE: Pixels = px(68.);
const SOLO_WIDTH: Pixels = px(320.);

pub(super) fn screen(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    // the check's page, in the room: the host's capsule closes it
    design::page(
        "huddle-room",
        words::HUDDLE,
        design::Nav::Child,
        ladder,
        theme,
    )
    .caption(words::alone(&view.clock()))
    .child(stage(view, cx, ladder, theme))
    .child(bar(view, cx, ladder, theme))
}

/// The stage: a microphone in trouble said over it with Try again (the
/// room has no Sound row to say it in), and your tile.
fn stage(view: &Huddle, cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    let mic_trouble = view.troubled(Device::Microphone).then(|| {
        design::refused(
            "huddle-microphone",
            words::phone_line(Device::Microphone, &view.mic_state),
            ladder,
            theme,
            cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.retry(cx)),
        )
    });
    div()
        .id("huddle-stage")
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .children(mic_trouble)
        .child(tile(view, cx, ladder, theme))
}

/// Your tile: the picture covering it, mirrored as the model says (or
/// your face, the camera off), the camera's card, a ring while you speak,
/// the chip, and the solo line.
fn tile(view: &Huddle, cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    let off = view.camera_off;
    let speaking = view.mic_placed() && view.mic_state.speaking;
    div()
        .id("huddle-tile-you")
        .relative()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .role(Role::Group)
        .aria_label(words::YOU)
        .bg(match off || view.troubled(Device::Camera) {
            true => theme.surface_raised,
            false => black(),
        })
        .when(view.camera_placed(), |tile| {
            tile.child(parts::camera_piece(view, cx).size_full())
        })
        // the microphone has nothing to draw in the room: it is there to
        // be heard, and to say who is speaking
        .when(view.mic_placed(), |tile| {
            tile.child(parts::mic_piece(view, cx).absolute().size(px(0.)))
        })
        .when(off, |tile| tile.child(look::face(FACE, theme)))
        .children(parts::state_card(view, cx, ladder, theme))
        .when(speaking, |tile| tile.child(ring(theme)))
        .child(parts::chip(view, speaking, ladder, theme))
        .child(solo(ladder))
}

// SDK gap: the speaking ring, swap when feat/phone-kit lands
/// The ring round the tile of whoever speaks.
fn ring(theme: &Theme) -> Div {
    div()
        .absolute()
        .inset_0()
        .border_2()
        .border_color(theme.success)
}

/// The room says who is in it: only you, for now.
fn solo(ladder: &Ladder) -> Div {
    div()
        .absolute()
        .left(design::space::LG)
        .right(design::space::LG)
        .bottom(SOLO_ABOVE)
        .flex()
        .justify_center()
        .child(
            div()
                .id("huddle-solo")
                .max_w(SOLO_WIDTH)
                .px(design::space::SM)
                .py(design::space::XXS)
                .bg(glass())
                .text_color(white())
                .text_type(ladder.caption)
                .text_center()
                .child(words::SOLO),
        )
}

/// The bar: the microphone, the camera, the flip and the Sound route, each
/// a square, and Leave taking the rest.
fn bar(view: &Huddle, cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    // the host lays the view out above the home bar: nothing under the bar
    div()
        .id("huddle-bar")
        .flex_none()
        .flex()
        .gap(design::space::SM)
        .pt(design::space::LG)
        .px(design::space::BLOCK)
        .border_t_1()
        .border_color(theme.border)
        .bg(theme.background)
        .child(mic_toggle(view, cx, ladder, theme))
        .child(camera_toggle(view, cx, ladder, theme))
        .child(flip(view, cx, ladder, theme))
        .child(sound(view, cx, ladder, theme))
        .child(leave(cx, ladder, theme))
}

// SDK gap: the room's square bar button, swap when feat/phone-kit lands
/// A square of the bar: its glyph in a strong hairline, the danger fill
/// while `off`. The caller makes it press.
fn square(
    (id, glyph): (&'static str, &'static str),
    name: impl Into<SharedString>,
    off: bool,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(BAR_BUTTON)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .map(|button| match off {
            true => button.bg(theme.danger).text_color(white()),
            false => button
                .border_1()
                .border_color(theme.border_strong)
                .text_color(theme.foreground),
        })
        .role(Role::Button)
        .aria_label(name)
        .child(design::icon(glyph, ladder.icon))
}

/// The microphone: slashed on the danger fill while muted or in trouble,
/// its trouble told to assistive technology.
fn mic_toggle(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let troubled = view.troubled(Device::Microphone);
    let off = view.muted || troubled;
    let glyph = if off { MIC_OFF } else { MIC };
    let trouble = troubled.then(|| words::phone_trouble(Device::Microphone, &view.mic_state).0);
    let button = square(
        ("huddle-mic", glyph),
        words::mute(view.muted),
        off,
        ladder,
        theme,
    );
    design::pressed(button, ladder)
        .aria_toggled(view.muted.into())
        .when_some(trouble, |button, trouble| button.aria_description(trouble))
        .focusable()
        .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_mute(cx)))
}

fn camera_toggle(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let off = view.camera_off;
    let glyph = if off { VIDEO_OFF } else { VIDEO };
    let button = square(
        ("huddle-camera", glyph),
        words::camera_toggle(off),
        off,
        ladder,
        theme,
    );
    design::pressed(button, ladder)
        .aria_toggled(off.into())
        .focusable()
        .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_camera(cx)))
}

/// The flip to the camera on the other side; standing still while the
/// camera is off, or where the host lists no camera on the other side.
fn flip(view: &Huddle, cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    let other = view.other_camera().map(|camera| camera.facing);
    let to = other.unwrap_or(Facing::Back);
    let button = square(("huddle-flip", FLIP), words::flip(to), false, ladder, theme);
    match other.is_some() && !view.camera_off {
        true => design::pressed(button, ladder)
            .focusable()
            .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.flip(cx))),
        false => button.opacity(0.5).aria_disabled(true),
    }
}

/// The Sound route, named with it: it opens the sound sheet.
fn sound(view: &Huddle, cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    let route = view.shown_device(Kind::Speaker);
    let button = square(
        ("huddle-sound", SPEAKER),
        words::sound(&route),
        false,
        ladder,
        theme,
    );
    design::pressed(button, ladder)
        .focusable()
        .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
            view.open_menu(SOUND, "huddle-sound", cx)
        }))
}

/// Leave: back to the check.
fn leave(cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    let leave = div()
        .id("huddle-leave")
        .flex_1()
        .min_w_0()
        .h(BAR_BUTTON)
        .flex()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .bg(theme.danger)
        .text_color(white())
        .text_type(ladder.body)
        .font_weight(FontWeight::MEDIUM)
        .role(Role::Button)
        .child(design::icon(LEAVE, ladder.icon_row))
        .child(words::LEAVE);
    design::pressed(leave, ladder)
        .focusable()
        .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.leave_room(cx)))
}
