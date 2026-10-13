//! Huddle: check yourself before a call. Your camera as others would see
//! it (mirrored, as a mirror shows you, unless you turn that off), your
//! microphone's level as you speak, and your own voice: record five
//! seconds and hear them played back, or, with headphones on, a live loop.
//!
//! The view never touches a device: it places the SDK's `camera` and
//! `microphone` pieces, which the host captures, draws and plays, and hears
//! how their devices are. Turning the camera off or muting removes the
//! piece, which releases the device (the camera's light goes off). The
//! pickers list what `camera.devices` and `microphone.devices` answer; a
//! device that goes away, or an answer of none, offers a retry.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Div, FontWeight, Stateful};
use serde::{Deserialize, Serialize};

/// How long "Record and play back" records, in seconds.
pub const RECORD_SECONDS: u8 = 5;

/// The widest the column grows.
const COLUMN: Pixels = px(640.);

#[derive(Default, Serialize, Deserialize)]
pub struct Huddle {
    /// What the host lists: cameras, and microphones with speakers.
    #[serde(skip)]
    pub(crate) cameras: Loadable<Vec<Device>>,
    #[serde(skip)]
    pub(crate) audio: Loadable<AudioDevices>,
    /// The picks, by id; `None`: the system's default.
    pub(crate) camera: Option<String>,
    pub(crate) microphone: Option<String>,
    pub(crate) speaker: Option<String>,
    pub(crate) camera_off: bool,
    pub(crate) muted: bool,
    /// Kept on: a person checking themselves expects a mirror.
    pub(crate) unmirrored: bool,
    /// The live loop, on only while the person keeps it on.
    #[serde(skip)]
    pub(crate) monitor: bool,
    /// What the pieces last told.
    #[serde(skip)]
    pub(crate) camera_state: MediaState,
    #[serde(skip)]
    pub(crate) mic_state: MediaState,
    /// Bumped by a retry: a piece under a new id is a new one, so the host
    /// opens its device again.
    #[serde(skip)]
    attempt: u32,
    #[serde(skip)]
    pub(crate) recorder: MicHandle,
}

impl View for Huddle {
    const NAME: &'static str = "Huddle";
    const DESCRIPTION: &'static str = "Check your camera and microphone.";
    const ICON: &'static str = "icons/video.svg";
    // `camera` and `microphone`: the pieces and their device lists; `host`:
    // the log a refusal nothing waits on goes to
    const CAPABILITIES: &'static [Capability] =
        &[Capability::Camera, Capability::Microphone, Capability::Host];
    const MIN_WINDOW_WIDTH: u32 = 360;

    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.list(cx);
    }
}

/// Whether a refusal says this host has no camera or microphone at all.
fn unsupported(error: &Error) -> bool {
    error.code == refusal::MEDIA_UNSUPPORTED
}

impl Huddle {
    /// The device lists, read again.
    pub(crate) fn list(&mut self, cx: &mut Context<Self>) {
        let cameras = cx.host().ask::<CameraDevices>(());
        cx.load(self, cameras, |view| &mut view.cameras);
        let audio = cx.host().ask::<MicrophoneDevices>(());
        cx.load(self, audio, |view| &mut view.audio);
    }

    /// Both devices opened again, the lists read again: after a device
    /// went away, was refused or was busy.
    fn retry(&mut self, cx: &mut Context<Self>) {
        self.attempt += 1;
        self.camera_state = MediaState::default();
        self.mic_state = MediaState::default();
        self.list(cx);
        cx.notify();
    }

    /// The host has no camera or microphone: its lists were refused so.
    pub(crate) fn no_support(&self) -> bool {
        matches!(&self.cameras, Loadable::Failed(error) if unsupported(error))
            || matches!(&self.audio, Loadable::Failed(error) if unsupported(error))
    }

    fn recording(&self) -> Playback {
        self.mic_state.playback.clone()
    }
}

impl Render for Huddle {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.global::<Theme>();
        let column = div()
            .w_full()
            .max_w(COLUMN)
            .flex()
            .flex_col()
            .gap(design::space::LG)
            .child(design::heading("huddle-title", "Check yourself", 1, &theme));
        let column = match self.no_support() {
            true => column.child(design::empty_state(
                "huddle-unsupported",
                "No camera or microphone here",
                "This build of the app has no camera or microphone support.",
                &theme,
            )),
            false => column
                .child(self.camera_section(cx, &theme))
                .child(self.microphone_section(cx, &theme)),
        };
        div()
            .id("huddle")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .p(design::space::XL)
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(design::text::BODY)
            .child(column)
    }
}

impl Huddle {
    /// The preview, its switches and the camera picker.
    fn camera_section(&self, cx: &mut Context<Self>, theme: &Theme) -> Div {
        let theme = *theme;
        let preview = div()
            .id("huddle-preview")
            .w_full()
            .h(px(300.))
            .relative()
            .bg(theme.surface_raised)
            .border_1()
            .border_color(theme.border);
        let preview = match self.camera_off {
            true => preview.child(notice("huddle-camera-off", "Your camera is off.", &theme)),
            false => {
                let piece = camera(SharedString::from(format!(
                    "huddle-camera-{}",
                    self.attempt
                )))
                .mirror(!self.unmirrored)
                .on_state(cx.listener(|view: &mut Huddle, state: &MediaState, _, cx| {
                    view.camera_state = state.clone();
                    cx.notify();
                }))
                .size_full()
                .bg(hsla(0., 0., 0., 1.));
                let piece = match &self.camera {
                    Some(id) => piece.device(id.clone()),
                    None => piece,
                };
                let trouble = trouble("camera", &self.camera_state).map(|line| {
                    notice("huddle-camera-trouble", line, &theme)
                        .absolute()
                        .inset_0()
                        .bg(theme.surface_raised)
                        .child(design::button(
                            "huddle-camera-retry",
                            "Try again",
                            &theme,
                            {
                                cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                                    view.retry(cx)
                                })
                            },
                        ))
                });
                preview.child(piece).children(trouble)
            }
        };
        let switches = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(design::space::LG)
            .child(labelled(
                design::switch(
                    "huddle-camera-on",
                    "Camera",
                    !self.camera_off,
                    true,
                    &theme,
                    cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                        view.camera_off = !view.camera_off;
                        view.camera_state = MediaState::default();
                        cx.notify();
                    }),
                ),
                "Camera",
            ))
            .child(labelled(
                design::switch(
                    "huddle-mirror",
                    "Mirror",
                    !self.unmirrored,
                    !self.camera_off,
                    &theme,
                    cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                        view.unmirrored = !view.unmirrored;
                        cx.notify();
                    }),
                ),
                "Mirror",
            ));
        let cameras = self.cameras.ready().cloned().unwrap_or_default();
        let listed = cameras.clone();
        let picker = picker(
            "huddle-cameras",
            "Camera",
            &cameras,
            failure(&self.cameras),
            self.camera.as_deref(),
            &theme,
            cx.listener(move |view: &mut Huddle, index: &usize, _, cx| {
                if let Some(device) = listed.get(*index) {
                    view.camera = Some(device.id.clone());
                    view.camera_state = MediaState::default();
                    cx.notify();
                }
            }),
        );
        div()
            .flex()
            .flex_col()
            .gap(design::space::MD)
            .child(preview)
            .child(switches)
            .child(picker)
    }

    /// The meter, mute, the pickers, the recording and the live loop.
    fn microphone_section(&self, cx: &mut Context<Self>, theme: &Theme) -> Div {
        let theme = *theme;
        let meter = match self.muted {
            true => div()
                .id("huddle-muted")
                .h(px(8.))
                .w_full()
                .bg(theme.surface_raised),
            false => div().id("huddle-meter-box").w_full().child({
                let piece = microphone(SharedString::from(format!(
                    "huddle-microphone-{}",
                    self.attempt
                )))
                .monitor(self.monitor)
                .track(&self.recorder)
                .on_state(cx.listener(|view: &mut Huddle, state: &MediaState, _, cx| {
                    view.mic_state = state.clone();
                    cx.notify();
                }))
                .h(px(8.))
                .w_full()
                .bg(theme.surface_raised)
                .text_color(theme.primary);
                let piece = match &self.microphone {
                    Some(id) => piece.device(id.clone()),
                    None => piece,
                };
                match &self.speaker {
                    Some(id) => piece.output(id.clone()),
                    None => piece,
                }
            }),
        };
        let heard = match (self.muted, trouble("microphone", &self.mic_state)) {
            (true, _) => "Muted.".to_owned(),
            (false, Some(line)) => line.to_owned(),
            (false, None) if self.mic_state.status == MediaStatus::Live => {
                format!("Listening on {}.", self.mic_state.device)
            }
            (false, None) => "Opening the microphone…".to_owned(),
        };
        let audio = self.audio.ready().cloned().unwrap_or_default();
        let (inputs, outputs) = (audio.inputs.clone(), audio.outputs.clone());
        let (picked_input, picked_output) = (inputs.clone(), outputs.clone());
        let live = !self.muted && self.mic_state.status == MediaStatus::Live;
        let idle = self.recording() == Playback::Idle;
        let record = design::button(
            "huddle-record",
            match idle {
                true => format!("Record {RECORD_SECONDS} s and play back"),
                false => "Stop".to_owned(),
            },
            &theme,
            cx.listener(move |view: &mut Huddle, _: &ClickEvent, _, cx| {
                match idle {
                    true => view.recorder.record(RECORD_SECONDS),
                    false => view.recorder.stop(),
                }
                cx.notify();
            }),
        )
        .enabled(live || !idle);
        let phase = match self.recording() {
            Playback::Idle => "",
            Playback::Recording => "Recording… say something.",
            Playback::Playing => "Playing it back…",
        };
        div()
            .flex()
            .flex_col()
            .gap(design::space::MD)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(design::space::LG)
                    .child(labelled(
                        design::switch(
                            "huddle-mute",
                            "Microphone",
                            !self.muted,
                            true,
                            &theme,
                            cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                                view.muted = !view.muted;
                                view.monitor = false;
                                view.mic_state = MediaState::default();
                                cx.notify();
                            }),
                        ),
                        "Microphone",
                    ))
                    .child(div().flex_1().child(meter)),
            )
            .child(
                div()
                    .id("huddle-heard")
                    .flex()
                    .items_center()
                    .gap(design::space::MD)
                    .text_size(design::text::SECONDARY)
                    .text_color(theme.muted)
                    .child(heard)
                    .when(
                        !self.muted && trouble("microphone", &self.mic_state).is_some(),
                        |line| {
                            line.child(design::button(
                                "huddle-microphone-retry",
                                "Try again",
                                &theme,
                                cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                                    view.retry(cx)
                                }),
                            ))
                        },
                    ),
            )
            .child(picker(
                "huddle-microphones",
                "Microphone",
                &inputs,
                failure(&self.audio),
                self.microphone.as_deref(),
                &theme,
                cx.listener(move |view: &mut Huddle, index: &usize, _, cx| {
                    if let Some(device) = picked_input.get(*index) {
                        view.microphone = Some(device.id.clone());
                        view.mic_state = MediaState::default();
                        cx.notify();
                    }
                }),
            ))
            .child(picker(
                "huddle-speakers",
                "Speaker",
                &outputs,
                failure(&self.audio),
                self.speaker.as_deref(),
                &theme,
                cx.listener(move |view: &mut Huddle, index: &usize, _, cx| {
                    if let Some(device) = picked_output.get(*index) {
                        view.speaker = Some(device.id.clone());
                        view.mic_state = MediaState::default();
                        cx.notify();
                    }
                }),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(design::space::MD)
                    .child(record)
                    .child(
                        div()
                            .id("huddle-phase")
                            .text_size(design::text::SECONDARY)
                            .text_color(theme.muted)
                            .child(phase),
                    ),
            )
            .child(design::setting_row(
                "huddle-monitor-row",
                "Hear myself live",
                "Wear headphones: through speakers your voice loops back into \
                 the microphone and howls.",
                design::switch(
                    "huddle-monitor",
                    "Hear myself live",
                    self.monitor,
                    live,
                    &theme,
                    cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                        view.monitor = !view.monitor;
                        cx.notify();
                    }),
                ),
                &theme,
            ))
    }
}

/// What to tell the person about a piece's device, if anything is wrong.
pub(crate) fn trouble(what: &str, state: &MediaState) -> Option<&'static str> {
    let camera = what == "camera";
    Some(match state.status {
        MediaStatus::Starting | MediaStatus::Live => return None,
        MediaStatus::Denied if camera => {
            "Camera access is off for this app. Turn it on in your system's privacy settings, \
             then try again."
        }
        MediaStatus::Denied => {
            "Microphone access is off for this app. Turn it on in your system's privacy \
             settings, then try again."
        }
        MediaStatus::Busy if camera => "Another app is using the camera.",
        MediaStatus::Busy => "Another app is using the microphone.",
        MediaStatus::Gone if camera => "The camera went away.",
        MediaStatus::Gone => "The microphone went away.",
        MediaStatus::NoDevice if camera => "No camera found.",
        MediaStatus::NoDevice => "No microphone found.",
        MediaStatus::Unsupported => "This build has no camera or microphone support.",
        MediaStatus::Failed if camera => "The camera did not start.",
        MediaStatus::Failed => "The microphone did not start.",
    })
}

/// A line in the middle of the preview.
fn notice(id: &'static str, line: &'static str, theme: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(design::space::MD)
        .p(design::space::LG)
        .text_color(theme.muted)
        .child(line)
}

/// A switch with its name beside it.
fn labelled(switch: Stateful<Div>, name: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(design::space::SM)
        .child(switch)
        .child(name)
}

/// Why a device list is not there, in the host's words.
fn failure<T>(list: &Loadable<T>) -> Option<String> {
    match list {
        Loadable::Failed(error) => Some(error.message.clone()),
        _ => None,
    }
}

/// One device picker: the devices side by side, the picked one (else the
/// system's default) filled. Nothing listed: says why, or that there is
/// none.
fn picker(
    id: &'static str,
    label: &'static str,
    devices: &[Device],
    failed: Option<String>,
    picked: Option<&str>,
    theme: &Theme,
    pick: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> Div {
    let row = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(design::space::MD)
        .child(
            div()
                .w(px(88.))
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .child(label),
        );
    if devices.is_empty() {
        return row.child(design::quiet(failed.unwrap_or("None found".into()), theme));
    }
    let at = devices
        .iter()
        .position(|device| Some(device.id.as_str()) == picked)
        .or_else(|| devices.iter().position(|device| device.default));
    let choices = devices.iter().enumerate().map(|(index, device)| {
        (
            ElementId::from(SharedString::from(format!("{id}-{index}"))),
            SharedString::from(device.name.clone()),
        )
    });
    row.child(design::segmented(
        id,
        label,
        theme,
        at,
        choices,
        move |index, window, app| pick(&index, window, app),
    ))
}

export_view!(Huddle);

#[cfg(test)]
mod tests;
