//! What the view says that more than one control says, once: the trouble a
//! piece reports, the names of the microphone and camera toggles, and what
//! a device list and a picker say.
use view::prelude::*;

use crate::{Kind, Which};

/// Which piece a trouble line speaks of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Device {
    Camera,
    Microphone,
}

/// A picker naming no device the host listed: the system picks.
pub(crate) const SYSTEM_DEFAULT: &str = "System default";

/// A device list with nothing in it.
pub(crate) const NONE_FOUND: &str = "None found";

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

/// The title and the line under it for a device's trouble: the platform's
/// fix line is the host's (`detail`); with none, the Linux one.
pub(crate) fn trouble(device: Device, state: &MediaState) -> (&'static str, String) {
    let camera = device == Device::Camera;
    let noun = if camera { "camera" } else { "microphone" };
    let detail = |fallback: String| match state.detail.is_empty() {
        true => fallback,
        false => state.detail.clone(),
    };
    match state.status {
        MediaStatus::Denied => {
            let fix = detail(format!(
                "Check the device's permissions (or your desktop's {}), then try again.",
                if camera {
                    "camera portal"
                } else {
                    "sound settings"
                }
            ));
            let why = match state.opens_settings {
                true => format!("Ducktape is not allowed to use the {noun}."),
                false => format!("Your system refused this app the {noun}."),
            };
            (
                if camera {
                    "Camera access is off"
                } else {
                    "Microphone access is off"
                },
                format!("{why} {fix}"),
            )
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
            if camera {
                "Another app is using the camera"
            } else {
                "Another app is using the microphone"
            },
            "Close it (a browser tab, Zoom, OBS), then try again.".into(),
        ),
        MediaStatus::Failed | MediaStatus::Unsupported => (
            if camera {
                "The camera did not start"
            } else {
                "The microphone did not start"
            },
            detail(String::new()),
        ),
        MediaStatus::Starting | MediaStatus::Live => ("", String::new()),
    }
}
