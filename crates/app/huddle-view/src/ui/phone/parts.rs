//! What the check and the room both draw (the phone boards, `design-phone/
//! SPEC.md` §0: no Expo source, the Expo kit's look): the two pieces, the
//! chip that says it is you, and the card a camera in trouble or slow to
//! open shows over its picture. They draw what the model says; the card's
//! buttons write through its verbs. What the desktop draws alike (the
//! glass, the shared glyphs, your face) is `look`'s. The ones marked "SDK
//! gap" are pieces the kit (#417) lacks, drawn here until it has them.
use view::design::{Kind as ButtonKind, Ladder};
use view::prelude::*;
use view::{Div, FontWeight, Stateful};

use crate::look::{self, MIC_OFF, SPEAKING, VIDEO, glass, muted_mark, white};
use crate::words::{self, Device};
use crate::{Huddle, Screen};

// the phone's own glyphs, by the Lucide names `design::icon` draws
pub(super) const FLIP: &str = "switch-camera";
pub(super) const SMARTPHONE: &str = "smartphone";

/// How tall the chip is, and its mark (SPEC §1.1).
const CHIP: Pixels = px(24.);
const MARK: Pixels = px(12.);

/// A camera opening says so in the kit's empty state's measures: its
/// glyph, and how wide its line runs (SPEC §3).
const CARD_GLYPH: Pixels = px(28.);
const CARD_TEXT: Pixels = px(280.);

/// The camera's piece, covering its box, mirrored as the model says, on
/// the camera picked (the flip's), else the system's default.
pub(super) fn camera_piece(view: &Huddle, cx: &mut Context<Huddle>) -> view::Camera {
    camera(view.camera_key())
        .mirror(view.mirrored())
        .object_fit(ObjectFit::Cover)
        .on_state(cx.listener(|view: &mut Huddle, state: &MediaState, _, cx| {
            view.camera_state = state.clone();
            cx.notify();
        }))
        .when_some(view.camera.clone(), |piece, id| piece.device(id))
}

/// The microphone's piece. It names no microphone: a phone's follows the
/// route, which the speaker picked sets (`output`). The live loop runs on
/// the check only, and never where the sound loops back.
pub(super) fn mic_piece(view: &Huddle, cx: &mut Context<Huddle>) -> view::Microphone {
    microphone(view.mic_key())
        .monitor(view.monitor && view.screen == Screen::Check && !view.loops_back())
        .track(&view.recorder)
        .on_state(cx.listener(|view: &mut Huddle, state: &MediaState, _, cx| {
            view.heard_mic(state);
            cx.notify();
        }))
        .when_some(view.speaker.clone(), |piece, id| piece.output(id))
}

// SDK gap: #417's badge holds a label alone, and the chip leads with a
// mark and trails the mirror's tag; swap when the kit has a chip over a
// picture
/// "You" at the picture's top left: the muted mark before it (or, while
/// `speaking`, the speaking one), "· mirrored" after it while the picture
/// is (not while the camera is still opening: there is no picture yet).
/// On the glass over the picture; with the camera off, on the window's
/// ground in a hairline.
pub(super) fn chip(view: &Huddle, speaking: bool, ladder: &Ladder, theme: &Theme) -> Div {
    let picture = !view.camera_off;
    let (danger, tone) = match picture {
        true => (muted_mark(), white()),
        false => (theme.danger, theme.foreground),
    };
    let mark = match (view.muted, speaking) {
        (true, _) => Some(("huddle-mark-muted", MIC_OFF, words::MUTED, danger)),
        (false, true) => Some(("huddle-mark-speaking", SPEAKING, words::SPEAKING, tone)),
        (false, false) => None,
    };
    let tagged = view.camera_placed() && !view.camera_slow() && view.mirrored();
    div()
        .absolute()
        .top(design::space::LG)
        .left(design::space::LG)
        .h(CHIP)
        .px(design::space::XS)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .text_type(ladder.caption)
        .text_color(tone)
        .map(|chip| match picture {
            true => chip.bg(glass()),
            false => chip
                .bg(theme.background)
                .border_1()
                .border_color(theme.border),
        })
        .children(mark.map(|(id, glyph, name, color)| {
            div()
                .id(id)
                .flex()
                .role(Role::Image)
                .aria_label(name)
                .child(design::icon(glyph, MARK).text_color(color))
        }))
        .child(words::YOU)
        .when(tagged, |chip| {
            chip.child(
                div()
                    .font_family(design::fonts::FAMILY_MONO)
                    .opacity(0.7)
                    .child(words::MIRRORED),
            )
        })
}

/// The card over the picture (SPEC §3) while the camera is still opening
/// (the system asking first) or in trouble; none while it is neither.
pub(super) fn state_card(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Option<Stateful<Div>> {
    let card = match view.camera_slow() {
        true => opening(ladder),
        false if view.troubled(Device::Camera) => trouble(view, cx, ladder, theme),
        false => return None,
    };
    Some(
        div()
            .id("huddle-camera-card")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(card),
    )
}

// SDK gap: #417's empty_state draws on the window's ground, and this card
// sits white on the black of a picture not drawn yet; swap when the kit
// has a card over a picture
/// A camera still opening: its glyph, what is happening and what the
/// phone is about to ask, white on the black under the picture.
fn opening(ladder: &Ladder) -> Stateful<Div> {
    div()
        .id("huddle-camera-opening")
        .flex()
        .flex_col()
        .items_center()
        .gap(design::space::SM)
        .p(design::space::XL)
        .text_center()
        .text_color(white())
        .child(design::icon(VIDEO, CARD_GLYPH))
        .child(
            div()
                .text_type(ladder.body)
                .font_weight(FontWeight::MEDIUM)
                .child(words::OPENING_CAMERA),
        )
        .child(
            div()
                .max_w(CARD_TEXT)
                .text_type(ladder.secondary)
                .opacity(0.7)
                .child(words::ALLOW_CAMERA),
        )
}

/// A camera in trouble: the kit's empty state on the raised ground behind
/// it (its glyph, title and line), and what fixes it under them. In the
/// room the person has joined already: a missing camera says no word of
/// joining.
fn trouble(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let state = &view.camera_state;
    let (title, text) = words::phone_trouble(Device::Camera, state);
    let text = match (&state.status, view.screen) {
        (MediaStatus::NoDevice, Screen::Room) => String::new(),
        _ => text,
    };
    let glyph = look::trouble_glyph(&state.status);
    design::empty_state("huddle-camera-trouble", glyph, title, text, ladder, theme).child(
        fixes(view, Device::Camera, cx, ladder, theme)
            .mt(design::space::SM)
            .justify_center(),
    )
}

/// What fixes `device`'s trouble, in a row that wraps: Open Settings where
/// the host says its settings can (a refused device), Try again (a grant in
/// Settings does not reopen a piece let go), and on the check, for a phone
/// without the device, joining without it. The caller places the row.
pub(super) fn fixes(
    view: &Huddle,
    device: Device,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Div {
    let state = view.state(device);
    let settings = state.status == MediaStatus::Denied && state.opens_settings;
    let missing = state.status == MediaStatus::NoDevice && view.screen == Screen::Check;
    let (prefix, page) = match device {
        Device::Camera => ("huddle-camera", Place::CameraSettings),
        Device::Microphone => ("huddle-microphone", Place::MicrophoneSettings),
    };
    let id = |what: &str| format!("{prefix}-{what}");
    let open_settings = cx
        .listener(move |_: &mut Huddle, _: &ClickEvent, _, cx| cx.host().notify::<HostShow>(page));
    let try_again = cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.retry(cx));
    let without = cx.listener(move |view: &mut Huddle, _: &ClickEvent, _, cx| {
        match device {
            Device::Camera => view.camera_off = true,
            Device::Microphone => view.muted = true,
        }
        view.enter_room(cx)
    });
    div()
        .flex()
        .flex_wrap()
        .gap(design::space::SM)
        .when(settings, |row| {
            let open = design::button(
                id("settings"),
                words::OPEN_SETTINGS,
                ladder,
                theme,
                open_settings,
            );
            row.child(open.small())
        })
        .child(design::button(id("retry"), words::TRY_AGAIN, ladder, theme, try_again).small())
        .when(missing, |row| {
            let join = design::button(
                id("without"),
                words::join_without(device),
                ladder,
                theme,
                without,
            );
            row.child(join.small().kind(ButtonKind::Quiet))
        })
}
