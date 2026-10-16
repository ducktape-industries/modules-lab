//! The room (phone board 30, `design-phone/SPEC.md` §2; no Expo source: a
//! new screen in the Expo kit's look). Its title row holds the clock and
//! who is here; your tile fills the stage, the picture covering it, with
//! the chip, a ring while you speak, and the line that says calls with
//! others are not connected yet; under it, in the thumb's reach, the bar:
//! the microphone, the camera, the flip, the Sound route and Leave. The
//! view stays where it is (a phone keeps one window): no pin, no float.
//!
//! Reads the picks and switches, what the pieces told and the room's
//! clock. Writes the toggles, the flip, a retry or the system's settings
//! for a device in trouble, the sound sheet's opening and the check
//! (`leave_room`).
use view::design::{IconButton, Ladder};
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

/// The stage: a microphone in trouble said over it with what fixes it
/// (the room has no Sound row to say it in), and your tile.
fn stage(view: &Huddle, cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    let mic_trouble = view
        .troubled(Device::Microphone)
        .then(|| mic_trouble(view, cx, ladder, theme));
    div()
        .id("huddle-stage")
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .children(mic_trouble)
        .child(tile(view, cx, ladder, theme))
}

/// The microphone's trouble, in the danger tone over the stage: its line
/// (the check's Sound row's), and what fixes it under the line.
fn mic_trouble(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id("huddle-mic-trouble")
        .flex_none()
        .flex()
        .flex_col()
        .gap(design::space::SM)
        .px(design::space::BLOCK)
        .py(design::space::LG)
        .bg(theme.danger_soft)
        .border_b_1()
        .border_color(theme.danger)
        .child(
            div()
                .text_type(ladder.secondary)
                .text_color(theme.danger)
                .child(words::phone_line(Device::Microphone, &view.mic_state)),
        )
        .child(parts::fixes(view, Device::Microphone, cx, ladder, theme))
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

// SDK gap: #417 draws no ring round a tile while its person speaks; swap
// when the kit has one
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

/// A square of the bar (SPEC §2), the kit's glyph button grown to the
/// thumb's reach: its glyph in a strong hairline, the danger fill while
/// `off`.
fn square<F>(button: IconButton<F>, off: bool, theme: &Theme) -> IconButton<F>
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let button = button.size(BAR_BUTTON);
    match off {
        true => button.bg(theme.danger).text_color(white()),
        false => button.border_1().border_color(theme.border_strong),
    }
}

/// The microphone: slashed on the danger fill while muted or in trouble
/// (the line over the stage says what the trouble is).
fn mic_toggle(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    let off = view.muted || view.troubled(Device::Microphone);
    let glyph = design::icon(if off { MIC_OFF } else { MIC }, ladder.icon);
    let toggle = cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_mute(cx));
    let name = words::mute(view.muted);
    let button = design::icon_button("huddle-mic", glyph, name, ladder, theme, toggle);
    square(button.toggled(view.muted), off, theme)
}

fn camera_toggle(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    let off = view.camera_off;
    let glyph = design::icon(if off { VIDEO_OFF } else { VIDEO }, ladder.icon);
    let toggle = cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_camera(cx));
    let name = words::camera_toggle(off);
    let button = design::icon_button("huddle-camera", glyph, name, ladder, theme, toggle);
    square(button.toggled(off), off, theme)
}

/// The flip to the camera on the other side; standing still while the
/// camera is off, or where the host lists no camera on the other side.
fn flip(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    let other = view.other_camera().map(|camera| camera.facing);
    let glyph = design::icon(FLIP, ladder.icon);
    let flip = cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.flip(cx));
    let name = words::flip(other.unwrap_or(Facing::Back));
    let button = design::icon_button("huddle-flip", glyph, name, ladder, theme, flip);
    square(
        button.enabled(other.is_some() && !view.camera_off),
        false,
        theme,
    )
}

/// The Sound route, named with it: it opens the sound sheet.
fn sound(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    let glyph = design::icon(SPEAKER, ladder.icon);
    let open = cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
        view.open_menu(SOUND, "huddle-sound", cx)
    });
    let name = words::sound(&view.shown_device(Kind::Speaker));
    let button = design::icon_button("huddle-sound", glyph, name, ladder, theme, open);
    square(button, false, theme)
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
