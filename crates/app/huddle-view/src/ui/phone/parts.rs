//! What the check and the room both draw (the phone boards, `design-phone/
//! SPEC.md` §0: no Expo source, the Expo kit's look): the two pieces, the
//! chip that says it is you, and the card a camera in trouble or slow to
//! open shows over its picture. They draw what the model says; the card's
//! buttons write through its verbs. What the desktop draws alike (the
//! glass, the shared glyphs, your face) is `look`'s. The ones marked "SDK
//! gap" are pieces the kit lacks, drawn here until it has them.
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

/// A state card's glyph, and how wide its text runs (SPEC §3).
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

// SDK gap: the chip over a picture, swap when feat/phone-kit lands
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

// SDK gap: the state card over a picture, swap when feat/phone-kit lands
/// The card over the picture (SPEC §3) while the camera is in trouble,
/// with what fixes it, or still opening (the system asking first): no
/// ground of its own, the picture's box is raised behind a trouble, and
/// black under a camera still opening, so that card is white.
pub(super) fn state_card(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Option<Stateful<Div>> {
    let opening = view.camera_slow();
    if !opening && !view.troubled(Device::Camera) {
        return None;
    }
    let state = &view.camera_state;
    let (glyph, title, text) = match opening {
        true => (VIDEO, words::OPENING_CAMERA, words::ALLOW_CAMERA.to_owned()),
        false => {
            let (title, text) = words::phone_trouble(Device::Camera, state);
            // in the room the person has joined already: no word of joining
            let text = match (&state.status, view.screen) {
                (MediaStatus::NoDevice, Screen::Room) => String::new(),
                _ => text,
            };
            (look::trouble_glyph(&state.status), title, text)
        }
    };
    let (glyph_tone, title_tone, text_tone) = match opening {
        true => (white(), white(), white()),
        false => (theme.faint, theme.foreground, theme.muted),
    };
    let card = div()
        .id("huddle-camera-card")
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .p(design::space::XL)
        .text_center()
        .child(design::icon(glyph, CARD_GLYPH).text_color(glyph_tone))
        .child(
            div()
                .text_type(ladder.body)
                .font_weight(FontWeight::MEDIUM)
                .text_color(title_tone)
                .child(title),
        )
        .when(!text.is_empty(), |card| {
            card.child(
                div()
                    .max_w(CARD_TEXT)
                    .text_type(ladder.secondary)
                    .text_color(text_tone)
                    .when(opening, |text| text.opacity(0.7))
                    .child(text),
            )
        });
    Some(match opening {
        true => card,
        false => card.child(
            fixes(view, Device::Camera, cx, ladder, theme)
                .mt(design::space::SM)
                .justify_center(),
        ),
    })
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
