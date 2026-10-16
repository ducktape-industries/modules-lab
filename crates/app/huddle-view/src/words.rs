//! What the view says, once: the trouble a piece reports, the names of the
//! toggles, what a device list and a picker say, and the words of the
//! check and the room a second design would say too. A sentence only the
//! desktop's tree says stays in it.
use view::prelude::*;

use crate::{Kind, RECORD_SECONDS, Which};

/// Which piece a trouble line speaks of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Device {
    Camera,
    Microphone,
}

/// The view's name, and the window's title on the check.
pub(crate) const HUDDLE: &str = "Huddle";

/// The window's title in the room.
pub(crate) const IN_THE_ROOM: &str = "Huddle \u{b7} just you";

/// A picker naming no device the host listed: the system picks.
pub(crate) const SYSTEM_DEFAULT: &str = "System default";

/// A device list with nothing in it.
pub(crate) const NONE_FOUND: &str = "None found";

/// A device list the host has not answered yet.
pub(crate) const LISTING: &str = "Looking for devices\u{2026}";

/// Your tile's and your chip's name.
pub(crate) const YOU: &str = "You";

/// After your name while the picture is mirrored.
pub(crate) const MIRRORED: &str = "\u{b7} mirrored";

/// The marks a chip carries, as a reader names them.
pub(crate) const MUTED: &str = "Muted";
pub(crate) const SPEAKING: &str = "Speaking";

/// Under your face while the camera is off.
pub(crate) const CAMERA_IS_OFF: &str = "Your camera is off";

/// A camera slow to open.
pub(crate) const OPENING_CAMERA: &str = "Opening the camera\u{2026}";

/// The microphone's level, as a reader names the meter.
pub(crate) const MICROPHONE_LEVEL: &str = "Microphone level";

/// The speaker test, and the test while it sounds.
pub(crate) const TEST: &str = "Test";
pub(crate) const PLAYING: &str = "Playing\u{2026}";

/// Why the last speaker test did not sound, as the host said.
pub(crate) fn test_failed(why: &str) -> String {
    format!("Test failed: {why}.")
}

/// The live loop's control.
pub(crate) const HEAR_MYSELF: &str = "Hear myself live";

/// The mirror's control.
pub(crate) const MIRROR: &str = "Mirror my picture";

/// Into the room.
pub(crate) const JOIN: &str = "Join huddle";

/// Out of the room.
pub(crate) const LEAVE: &str = "Leave";

/// The room is honest about who is in it.
pub(crate) const SOLO: &str = "Only you are here. Calls with others are not connected yet.";

/// What fixes a device's trouble.
pub(crate) const OPEN_SETTINGS: &str = "Open Settings";
pub(crate) const TRY_AGAIN: &str = "Try again";

/// A build without camera and microphone.
pub(crate) const UNSUPPORTED: &str = "No camera or microphone in this build";
pub(crate) const UNSUPPORTED_DETAIL: &str = "This copy of Ducktape was built without camera and \
     microphone support, so a huddle has nothing to carry. Install a full build.";

/// The room's clock, and who is in the room.
pub(crate) fn alone(clock: &str) -> String {
    format!("{clock} \u{b7} only you")
}

/// A kind of device, as its picker's label and its group in a menu say it.
pub(crate) fn kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Camera => "Camera",
        Kind::Microphone => "Microphone",
        Kind::Speaker => "Speaker",
    }
}

/// A device menu's name.
pub(crate) fn menu(which: Which) -> &'static str {
    match which {
        Which::Kind(of) => kind(of),
        Which::Sound => "Microphone and speaker",
    }
}

/// The microphone toggle over the check's picture, which names the device.
pub(crate) fn mute_microphone(muted: bool) -> &'static str {
    match muted {
        true => "Unmute microphone",
        false => "Mute microphone",
    }
}

/// The microphone toggle in the room and the float.
pub(crate) fn mute(muted: bool) -> &'static str {
    match muted {
        true => "Unmute",
        false => "Mute",
    }
}

/// The camera toggle.
pub(crate) fn camera_toggle(off: bool) -> &'static str {
    match off {
        true => "Turn camera on",
        false => "Turn camera off",
    }
}

/// Record and play back, as its control says it: to start, or to stop
/// the recording or the playback under way.
pub(crate) fn record(playback: &Playback) -> String {
    match playback {
        Playback::Idle => format!("Record {RECORD_SECONDS} s and play back"),
        Playback::Recording => "Stop".into(),
        Playback::Playing => "Stop playback".into(),
    }
}

/// The line under Record: what waits for the microphone, what the
/// recording does, or what it is for. A recording's countdown follows
/// [`RECORDING`].
pub(crate) fn record_line(muted: bool, playback: &Playback) -> &'static str {
    match playback {
        _ if muted => "Unmute to test your microphone.",
        Playback::Recording => RECORDING,
        Playback::Playing => "Playing it back\u{2026}",
        Playback::Idle => "Hear how you sound to others.",
    }
}

/// A recording under way, before its countdown.
pub(crate) const RECORDING: &str = "Recording\u{2026} say something \u{b7}\u{a0}";

/// Joining with a device that is missing.
pub(crate) fn join_without(device: Device) -> &'static str {
    match device {
        Device::Camera => "Join without camera",
        Device::Microphone => "Join without a microphone",
    }
}

// ---------- troubles ----------

/// What a trouble is called where both designs call it the same: a
/// refused, busy or failed device.
fn called(device: Device, status: &MediaStatus) -> &'static str {
    let camera = device == Device::Camera;
    let either = |of_camera, of_microphone| if camera { of_camera } else { of_microphone };
    match status {
        MediaStatus::Denied => either("Camera access is off", "Microphone access is off"),
        MediaStatus::Busy => either(
            "Another app is using the camera",
            "Another app is using the microphone",
        ),
        _ => either("The camera did not start", "The microphone did not start"),
    }
}

/// The host's words for the trouble (`detail`), else `fallback`.
fn detail_or(state: &MediaState, fallback: &str) -> String {
    match state.detail.is_empty() {
        true => fallback.to_owned(),
        false => state.detail.clone(),
    }
}

/// The title and the line under it for a device's trouble on the desktop:
/// the platform's fix line is the host's (`detail`); with none, the Linux
/// one.
pub(crate) fn trouble(device: Device, state: &MediaState) -> (&'static str, String) {
    let camera = device == Device::Camera;
    let noun = if camera { "camera" } else { "microphone" };
    match state.status {
        MediaStatus::Denied => {
            let fix = detail_or(
                state,
                &format!(
                    "Check the device's permissions (or your desktop's {}), then try again.",
                    if camera {
                        "camera portal"
                    } else {
                        "sound settings"
                    }
                ),
            );
            let why = match state.opens_settings {
                true => format!("Ducktape is not allowed to use the {noun}."),
                false => format!("Your system refused this app the {noun}."),
            };
            (called(device, &state.status), format!("{why} {fix}"))
        }
        MediaStatus::NoDevice => (
            if camera {
                "No camera found"
            } else {
                "No microphone found"
            },
            match camera {
                true => "Plug one in, or join with your microphone only.".into(),
                false => "Plug one in, or join with your camera only.".into(),
            },
        ),
        MediaStatus::Gone => (
            if camera {
                "The camera went away"
            } else {
                "The microphone went away"
            },
            match camera {
                true => "It was unplugged or its driver stopped. The last picture is gone; \
                         plug it back in and try again."
                    .into(),
                false => {
                    "It was unplugged or its driver stopped; plug it back in and try again.".into()
                }
            },
        ),
        MediaStatus::Busy => (
            called(device, &state.status),
            "Close it (a browser tab, Zoom, OBS), then try again.".into(),
        ),
        MediaStatus::Failed | MediaStatus::Unsupported => {
            (called(device, &state.status), detail_or(state, ""))
        }
        MediaStatus::Starting | MediaStatus::Live => ("", String::new()),
    }
}
