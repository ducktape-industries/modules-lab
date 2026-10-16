//! What both designs draw alike, once. Over the camera's picture the
//! picture is the ground, so what sits on it is dark glass with white
//! words, not the theme's; the glyphs both trees draw for the same thing;
//! the glyph a camera's trouble shows; and your face while the camera is
//! off. The desktop's tree (`ui`) and the phone's (`ui::phone`) both read
//! these; neither draws them its own way.
use view::Div;
use view::prelude::*;

use crate::words::Device;

// the glyphs both trees draw, by the Lucide names `design::icon` draws
pub(crate) const MIC: &str = "mic";
pub(crate) const MIC_OFF: &str = "mic-off";
pub(crate) const VIDEO: &str = "video";
pub(crate) const VIDEO_OFF: &str = "video-off";
pub(crate) const SPEAKER: &str = "volume-2";
pub(crate) const HEADPHONES: &str = "headphones";
pub(crate) const SPEAKING: &str = "audio-lines";
pub(crate) const ALERT: &str = "circle-alert";
pub(crate) const PLAY: &str = "play";
pub(crate) const STOP: &str = "square";
pub(crate) const LEAVE: &str = "phone-off";
pub(crate) const CHECK: &str = "check";
const CAMERA_OFF: &str = "camera-off";
const PERSON: &str = "user-round";

/// How big the person's glyph is in their face, of the face's side.
const FACE_GLYPH: f32 = 0.4;

/// The glass over the picture.
pub(crate) fn glass() -> Hsla {
    hsla(0., 0., 0., 0.55)
}

/// The words and glyphs on the glass.
pub(crate) fn white() -> Hsla {
    hsla(0., 0., 1., 1.)
}

/// The ground under a picture not drawn yet.
pub(crate) fn black() -> Hsla {
    hsla(0., 0., 0., 1.)
}

/// The muted mark on the glass: the dark theme's danger, legible on black
/// in either theme.
pub(crate) fn muted_mark() -> Hsla {
    Theme::dark().danger
}

/// What the ids of `device`'s fixes begin with, in both designs
/// (`huddle-camera-retry`, `huddle-microphone-settings`).
pub(crate) fn fixes_id(device: Device) -> &'static str {
    match device {
        Device::Camera => "huddle-camera",
        Device::Microphone => "huddle-microphone",
    }
}

/// The glyph a camera's trouble shows.
pub(crate) fn trouble_glyph(status: &MediaStatus) -> &'static str {
    match status {
        MediaStatus::NoDevice | MediaStatus::Gone => CAMERA_OFF,
        MediaStatus::Busy => VIDEO_OFF,
        _ => ALERT,
    }
}

// SDK gap: the kit's avatar draws a name's initial, and no account name
// reaches the view; local until the kit draws a face without a name
/// Your face while the camera is off, `side` across, in the middle of the
/// raised ground: the person's glyph in a ring on the window's ground. A
/// line under it is the caller's child, in its design's type.
pub(crate) fn face(side: Pixels, theme: &Theme) -> Div {
    let glyph = px((f32::from(side) * FACE_GLYPH).round());
    let face = div()
        .size(side)
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .text_color(theme.muted)
        .child(design::icon(PERSON, glyph));
    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .child(face)
}
