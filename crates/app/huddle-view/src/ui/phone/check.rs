//! The device check (phone boards 10, 11, 12, 20 and 21, `design-phone/
//! SPEC.md` §1; no Expo source: a new screen in the Expo kit's look). Your
//! picture, square, with the microphone, the camera and the flip over it;
//! then the Sound route over the microphone's level with the speaker test,
//! Record and play back, Hear myself live, Mirror my picture; and Join at
//! the foot, never scrolled away. A phone's route sets the microphone and
//! the speaker together, so one Sound row stands for both, and its press
//! opens the sound sheet; the flip stands for a camera menu.
//!
//! Reads the picks and switches, the device lists and what the pieces
//! told. Writes the toggles, the flip, the mirror, the live loop, a
//! recording, the speaker test, a retry, the sound sheet's opening and the
//! room (`enter_room`: a phone keeps one window, so Join floats nothing).
use view::design::{Kind as ButtonKind, Ladder, Nav};
use view::prelude::*;
use view::{AnyElement, Div, Stateful};

use super::SOUND;
use super::parts::{self, FLIP, MIC, MIC_OFF, SPEAKER, VIDEO, VIDEO_OFF};
use crate::words::{self, Device};
use crate::{Huddle, Kind, RECORD_SECONDS};

/// Your face while the camera is off (SPEC §1.1).
const FACE: Pixels = px(56.);

/// A glass button over the picture, and its glyph (SPEC §1.1).
const GLASS: Pixels = px(44.);
const GLASS_GLYPH: Pixels = px(20.);

/// How tall the microphone's level is (SPEC §1.2).
const METER: Pixels = px(4.);

const PLAY: &str = "play";
const STOP: &str = "square";

pub(super) fn screen(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    // the view's one page: the host's capsule closes it, so it has no
    // Back of its own, and it draws no account avatar (none reaches the
    // view)
    design::page("huddle-check", words::HUDDLE, Nav::Child, ladder, theme)
        .caption(words::CHECK_CAPTION)
        .child(
            div()
                .id("huddle-check-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(
                    div()
                        .p(design::space::BLOCK)
                        .child(preview(view, cx, ladder, theme)),
                )
                .child(design::section_head(
                    "huddle-sound-head",
                    words::SOUND,
                    ladder,
                    theme,
                ))
                .child(sound(view, cx, ladder, theme))
                .child(record(view, cx, ladder, theme))
                .child(monitor(view, cx, ladder, theme))
                .child(design::section_head(
                    "huddle-picture-head",
                    words::PICTURE,
                    ladder,
                    theme,
                ))
                .child(mirror(view, cx, ladder, theme)),
        )
        .child(join(cx, ladder, theme))
}

/// Your picture, square across the body less its insets (a phone held
/// upright gives a tall picture; cut square it leaves the Sound rows above
/// the fold), the chip, the camera's card, and the buttons over its foot.
fn preview(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let side = px((view.viewport.0 - 2. * f32::from(design::space::BLOCK)).max(0.));
    let off = view.camera_off;
    let card = parts::state_card(view, cx, ladder, theme);
    // the flip stands down while a card says what is wrong
    let flip = card
        .is_none()
        .then(|| flip(view, cx, ladder, theme))
        .flatten();
    div()
        .id("huddle-preview")
        .relative()
        .flex_none()
        .size(side)
        .overflow_hidden()
        .bg(match off || view.troubled(Device::Camera) {
            true => theme.surface_raised,
            false => parts::black(),
        })
        .when(view.camera_placed(), |preview| {
            preview.child(parts::camera_piece(view, cx).size_full())
        })
        .when(off, |preview| {
            preview.child(parts::face(FACE, Some(words::CAMERA_IS_OFF), ladder, theme))
        })
        .children(card)
        .child(parts::chip(view, false, ladder, theme))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(design::space::LG)
                .flex()
                .justify_center()
                .gap(design::space::SM)
                .child(mic_toggle(view, cx, ladder, theme))
                .child(camera_toggle(view, cx, ladder, theme))
                .children(flip),
        )
}

// SDK gap: the glass button over a picture, swap when feat/phone-kit lands
/// A button over the picture: the dark glass and a white glyph, the danger
/// fill while `off`; over no picture (the camera off), the window's ground
/// in a strong hairline. The caller makes it press.
fn glass_button(
    (id, glyph): (&'static str, &'static str),
    name: &'static str,
    off: bool,
    view: &Huddle,
    theme: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(GLASS)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .map(|button| match (off, view.camera_off) {
            (true, _) => button.bg(theme.danger).text_color(parts::white()),
            (false, false) => button.bg(parts::glass()).text_color(parts::white()),
            (false, true) => button
                .bg(theme.background)
                .border_1()
                .border_color(theme.border_strong)
                .text_color(theme.foreground),
        })
        .role(Role::Button)
        .aria_label(name)
        .child(design::icon(glyph, GLASS_GLYPH))
}

fn mic_toggle(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let muted = view.muted;
    let glyph = if muted { MIC_OFF } else { MIC };
    let button = glass_button(
        ("huddle-mic", glyph),
        words::mute_microphone(muted),
        muted,
        view,
        theme,
    );
    design::pressed(button, ladder)
        .aria_toggled(muted.into())
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
    let button = glass_button(
        ("huddle-camera", glyph),
        words::camera_toggle(off),
        off,
        view,
        theme,
    );
    design::pressed(button, ladder)
        .aria_toggled(off.into())
        .focusable()
        .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_camera(cx)))
}

/// The flip to the camera on the other side, where the host lists a front
/// and a back one; standing still while the camera is off.
fn flip(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Option<Stateful<Div>> {
    let other = view.other_camera()?;
    let button = glass_button(
        ("huddle-flip", FLIP),
        words::flip(other.facing),
        false,
        view,
        theme,
    );
    Some(match view.camera_off {
        true => button.opacity(0.5).aria_disabled(true),
        false => design::pressed(button, ladder)
            .focusable()
            .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.flip(cx))),
    })
}

/// The Sound route (SPEC §1.2): its name over the microphone's level, or
/// over the microphone's trouble; a press opens the sound sheet. Then the
/// speaker test, or Try again for the microphone in trouble.
fn sound(view: &Huddle, cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    let route = view.shown_device(Kind::Speaker);
    let troubled = view.troubled(Device::Microphone);
    let under: AnyElement = match troubled {
        true => div()
            .id("huddle-mic-trouble")
            .text_type(ladder.caption)
            .text_color(theme.danger)
            .child(words::phone_line(Device::Microphone, &view.mic_state))
            .into_any_element(),
        false => meter(view, cx, theme).into_any_element(),
    };
    let open = cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
        view.open_menu(SOUND, "huddle-sound", cx)
    });
    // the route's words and the chevron are the press; the test beside them
    // is a press of its own, never inside it
    let press = div()
        .id("huddle-sound")
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(design::space::LG)
        .role(Role::Button)
        .aria_label(words::sound(&route))
        .focusable()
        .on_click(open)
        .child(design::lead(
            design::icon(SPEAKER, ladder.icon_row).text_color(theme.muted),
        ))
        .child(
            design::row_words(design::row_title(route, ladder, theme))
                .child(under)
                .children(
                    view.speaker_fault
                        .as_deref()
                        .map(|why| design::row_caption(words::test_failed(why), ladder, theme)),
                ),
        )
        .child(design::chevron(ladder, theme));
    let tail = match troubled {
        true => design::button(
            "huddle-microphone-retry",
            words::TRY_AGAIN,
            ladder,
            theme,
            cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.retry(cx)),
        )
        .small()
        .into_any_element(),
        false => design::button(
            "huddle-sound-test",
            if view.chiming {
                words::PLAYING
            } else {
                words::TEST
            },
            ladder,
            theme,
            cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.chime(cx)),
        )
        .small()
        .enabled(view.can_test())
        .into_any_element(),
    };
    design::row("huddle-sound-row", ladder, theme)
        .min_h(ladder.row_tall)
        .child(design::pressed(press, ladder))
        .child(tail)
}

// SDK gap: the meter, swap when feat/phone-kit lands
/// The microphone's level, which the host draws as a bar in the text's
/// colour over the track; the track alone while the microphone is muted.
fn meter(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    div()
        .id("huddle-meter")
        .mt(design::space::XS)
        .h(METER)
        .w_full()
        .bg(theme.surface_raised)
        .role(Role::Meter)
        .aria_label(words::MICROPHONE_LEVEL)
        .when(view.mic_placed(), |meter| {
            meter.child(
                parts::mic_piece(view, cx)
                    .size_full()
                    .bg(theme.surface_raised)
                    .text_color(theme.foreground),
            )
        })
}

/// Record five seconds and play them back (SPEC §1.3): the whole row the
/// press while the microphone is live, or to stop what is under way.
fn record(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let playback = view.mic_state.playback.clone();
    let under_way = playback != Playback::Idle;
    let line = words::record_line(view.muted, &playback);
    let under = match playback {
        // the countdown in the data face
        Playback::Recording => design::row_caption(line, ladder, theme).child(
            div()
                .font_family(design::fonts::FAMILY_MONO)
                .child(format!("0:{:02}", view.seconds_left())),
        ),
        _ => design::row_caption(line, ladder, theme),
    };
    let row = design::row("huddle-record", ladder, theme)
        .min_h(ladder.row_tall)
        .role(Role::Button)
        .child(design::lead(
            design::icon(if under_way { STOP } else { PLAY }, ladder.icon_row)
                .text_color(theme.muted),
        ))
        .child(
            design::row_words(design::row_title(words::record(&playback), ladder, theme))
                .child(under.id("huddle-record-line").flex()),
        );
    match view.can_record() {
        true => design::pressed(row, ladder)
            .focusable()
            .on_click(
                cx.listener(move |view: &mut Huddle, _: &ClickEvent, _, cx| {
                    match under_way {
                        true => view.recorder.stop(),
                        false => view.recorder.record(RECORD_SECONDS),
                    }
                    cx.notify();
                }),
            ),
        false => row.opacity(0.5).aria_disabled(true),
    }
}

/// Hear myself live (SPEC §1.4): refused where the sound loops back into
/// the microphone (the phone's speaker is a hand's width from it), and the
/// line under it says so, or what the loop does.
fn monitor(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let loops = view.loops_back();
    let route = view.speaker_in_use().map(|speaker| speaker.kind);
    let switch = design::switch(
        "huddle-monitor",
        words::HEAR_MYSELF,
        view.monitor && !loops,
        view.mic_live() && !loops,
        ladder,
        theme,
        cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
            view.monitor = !view.monitor;
            cx.notify();
        }),
    );
    let tone = if loops { theme.warning } else { theme.muted };
    switch_row(
        ("huddle-monitor-row", words::HEAR_MYSELF),
        (words::hear_myself_line(route), tone),
        switch,
        ladder,
        theme,
    )
}

/// Mirror my picture (SPEC §1.5): on until switched off, and standing
/// still while the back camera is in use, which is never mirrored.
fn mirror(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let switch = design::switch(
        "huddle-mirror",
        words::MIRROR,
        !view.unmirrored,
        !view.facing_away(),
        ladder,
        theme,
        cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
            view.unmirrored = !view.unmirrored;
            cx.notify();
        }),
    );
    switch_row(
        ("huddle-mirror-row", words::MIRROR),
        (words::MIRROR_LINE, theme.muted),
        switch,
        ladder,
        theme,
    )
}

/// A row a switch ends: its title over a line in `tone`, which wraps.
fn switch_row(
    (id, title): (&'static str, &'static str),
    (line, tone): (&'static str, Hsla),
    switch: Stateful<Div>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    design::row(id, ladder, theme)
        .min_h(ladder.row_tall)
        .child(
            design::row_words(design::row_title(title, ladder, theme))
                .child(div().text_type(ladder.caption).text_color(tone).child(line)),
        )
        .child(switch)
}

/// Join, at the foot under a hairline, never scrolled away.
fn join(cx: &mut Context<Huddle>, ladder: &Ladder, theme: &Theme) -> Div {
    let join = cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.enter_room(cx));
    div()
        .flex_none()
        // a column stretches the button it holds across its width
        .flex()
        .flex_col()
        .p(design::space::BLOCK)
        .border_t_1()
        .border_color(theme.border)
        .child(
            design::button("huddle-join", words::JOIN, ladder, theme, join)
                .kind(ButtonKind::Primary),
        )
}
