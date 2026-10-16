//! Huddle: check your camera and microphone, then huddle. The device check
//! is a big picture of yourself (mirrored, as a mirror shows you, unless
//! you turn that off) with the microphone and camera toggles over it, and
//! a column of the devices, the microphone's level, a speaker test, "record
//! five seconds and play them back", "hear myself live" with its warning,
//! and Join. The room is your tile on a stage and a bar of the same
//! toggles, each with its device menu, and Leave. Calls with other people
//! are not connected yet: the room says so.
//!
//! Join floats the room in the same press (`host.float`): the pane leaves
//! the desk into a small window of its own, kept above other apps' windows,
//! so the call stays in sight while the person works elsewhere; Leave puts
//! it back (`host.unfloat`). Below 480 by 320 the room is one tile with its
//! controls over it (pin: keep on top or not; back: to the full window);
//! the layout follows the window's size alone, and only the pin's state and
//! back read where the pane is (`host.placement`). A host that cannot float
//! a pane refuses that subscription `unknown_request`: the room then has no
//! pin, and Join keeps the view where it is.
//!
//! The view never touches a device: it places the SDK's `camera` and
//! `microphone` pieces, which the host captures, draws and plays, and hears
//! how their devices are. Turning the camera off or muting removes the
//! piece, which releases the device (the camera's light and the system's
//! microphone mark go off). The menus list what `camera.devices` and
//! `microphone.devices` answer, read again whenever a menu opens (no host
//! tells the view of a device plugged in).
use serde::{Deserialize, Serialize};
use view::prelude::*;
use view::{Bounds, accesskit};

mod ui;

/// How long "Record and play back" records, in seconds.
pub const RECORD_SECONDS: u8 = 5;

/// The float Join asks for: the room laid out 320 by 200, never under 240
/// by 150, kept above other windows.
pub const FLOAT: Float = Float {
    width: 320,
    height: 200,
    min_width: 240,
    min_height: 150,
    on_top: true,
};

/// Below this the room is one tile with its controls over it.
pub(crate) const COMPACT: (f32, f32) = (480., 320.);

/// Which screen is up: the device check, or the room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Screen {
    #[default]
    Check,
    Room,
}

/// Which device menu: one device kind, or the room's microphone caret,
/// which lists the microphones and then the speakers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Which {
    Camera,
    Microphone,
    Speaker,
    Sound,
}

impl Which {
    /// The control that opened it, which takes the keys back.
    pub(crate) fn opener(self) -> &'static str {
        match self {
            Which::Camera => "huddle-pick-camera",
            Which::Microphone => "huddle-pick-microphone",
            Which::Speaker => "huddle-pick-speaker",
            Which::Sound => "huddle-mic-menu",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Which::Camera => "Camera",
            Which::Microphone => "Microphone",
            Which::Speaker => "Speaker",
            Which::Sound => "Microphone and speaker",
        }
    }
}

/// An open device menu: which, and the row the arrows are on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Menu {
    pub(crate) which: Which,
    pub(crate) cursor: usize,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Huddle {
    /// What the host lists: cameras, and microphones with speakers.
    #[serde(skip)]
    pub(crate) cameras: Loadable<Vec<CameraDevice>>,
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
    /// The live loop, on only while the person keeps it on, on the check.
    #[serde(skip)]
    pub(crate) monitor: bool,
    /// The view always opens on the check.
    #[serde(skip)]
    pub(crate) screen: Screen,
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
    /// The speaker test asked, until the host answers that it sounded.
    #[serde(skip)]
    pub(crate) chiming: bool,
    /// Why the last speaker test did not sound, as the host said.
    #[serde(skip)]
    pub(crate) speaker_fault: Option<String>,
    #[serde(skip)]
    pub(crate) menu: Option<Menu>,
    /// Where layout put each menu's opener, in window pixels: a menu hangs
    /// under its picker, or over the room's caret.
    #[serde(skip)]
    pub(crate) anchors: Vec<(Which, Bounds<Pixels>)>,
    /// Seconds on the view's clock, and when the room was entered, a
    /// recording started and the camera was last placed, by it.
    #[serde(skip)]
    pub(crate) ticks: u64,
    #[serde(skip)]
    pub(crate) entered: u64,
    #[serde(skip)]
    pub(crate) recording_since: Option<u64>,
    #[serde(skip)]
    pub(crate) camera_since: u64,
    /// The pane's size as the host lays the frame out.
    #[serde(skip)]
    pub(crate) viewport: (f32, f32),
    /// Where the pane is, as the host last said (`host.placement`).
    #[serde(skip)]
    pub(crate) placement: Placement,
    /// The host answered a float kept on top with none: this platform
    /// keeps no window on top (Wayland), so there is no pin.
    #[serde(skip)]
    pub(crate) no_keep_on_top: bool,
    /// The host cannot float a pane (it refused `host.placement`
    /// `unknown_request`): no pin, and Join stays in place.
    #[serde(skip)]
    pub(crate) no_float: bool,
    /// The line that says so, once per float, until closed or the next
    /// press in the float.
    #[serde(skip)]
    pub(crate) no_keep_on_top_line: bool,
}

impl View for Huddle {
    const NAME: &'static str = "Huddle";
    const DESCRIPTION: &'static str = "Check your camera and microphone, then huddle.";
    const ICON: &'static str = "icons/video.svg";
    // `camera` and `microphone`: the pieces and their device lists; `clock`:
    // the room's clock and the recording's countdown; `host`: the window's
    // title, the float and where the pane is, the system's page a refused
    // device opens, and the log a refusal nothing waits on goes to
    const CAPABILITIES: &'static [Capability] = &[
        Capability::Camera,
        Capability::Microphone,
        Capability::Clock,
        Capability::Host,
    ];
    /// The check's two columns: the picture and the 272 px settings.
    const MIN_WINDOW_WIDTH: u32 = 560;

    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.list(cx);
        cx.host().notify::<HostTitle>("Huddle".into());
        cx.follow::<ClockTicks>(1000, |view, tick, cx| match tick {
            Ok(()) => view.tick(cx),
            Err(refusal) => cx.log_refused("the clock", &refusal),
        })
        .detach();
        cx.follow::<HostPlacement>((), |view, placement, cx| match placement {
            Ok(placement) => {
                if view.placement != placement {
                    view.placement = placement;
                    cx.notify();
                }
            }
            Err(refusal) if unknown(&refusal) => {
                view.no_float = true;
                cx.notify();
            }
            Err(refusal) => cx.log_refused("where the pane is", &refusal),
        })
        .detach();
    }
}

/// Whether the host does not answer that kind of request at all: a host
/// without camera and microphone, or one that cannot float a pane.
fn unknown(error: &Error) -> bool {
    error.code == refusal::UNKNOWN_REQUEST
}

/// A device that is not starting or live, said as the view says it.
pub(crate) fn trouble(state: &MediaState) -> bool {
    !matches!(state.status, MediaStatus::Starting | MediaStatus::Live)
}

impl Huddle {
    /// The device lists, read again (the values on screen stay meanwhile).
    pub(crate) fn list(&mut self, cx: &mut Context<Self>) {
        let cameras = cx.host().ask::<CameraDevices>(());
        cx.load(self, cameras, |view| &mut view.cameras);
        let audio = cx.host().ask::<MicrophoneDevices>(());
        cx.load(self, audio, |view| &mut view.audio);
    }

    /// Both devices opened again, the lists read again: after a device
    /// went away, was refused or was busy.
    pub(crate) fn retry(&mut self, cx: &mut Context<Self>) {
        self.attempt += 1;
        self.camera_state = MediaState::default();
        self.mic_state = MediaState::default();
        self.camera_since = self.ticks;
        self.recording_since = None;
        self.list(cx);
        cx.notify();
    }

    /// The host has no camera or microphone: it does not answer its lists,
    /// or a piece says so.
    pub(crate) fn no_support(&self) -> bool {
        matches!(&self.cameras, Loadable::Failed(error) if unknown(error))
            || matches!(&self.audio, Loadable::Failed(error) if unknown(error))
            || self.camera_state.status == MediaStatus::Unsupported
            || self.mic_state.status == MediaStatus::Unsupported
    }

    /// The piece ids: a retry makes new ones.
    pub(crate) fn camera_key(&self) -> SharedString {
        format!("huddle-camera-{}", self.attempt).into()
    }

    pub(crate) fn mic_key(&self) -> SharedString {
        format!("huddle-microphone-{}", self.attempt).into()
    }

    /// Whether the camera piece is placed: on, and not in trouble (a
    /// refused or vanished camera is let go until Try again).
    pub(crate) fn camera_placed(&self) -> bool {
        !self.camera_off && !trouble(&self.camera_state)
    }

    /// Whether the microphone piece is placed: unmuted, and not in trouble.
    pub(crate) fn mic_placed(&self) -> bool {
        !self.muted && !trouble(&self.mic_state)
    }

    pub(crate) fn mic_live(&self) -> bool {
        !self.muted && self.mic_state.status == MediaStatus::Live
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.ticks += 1;
        // only what the clock shows is drawn again
        let starting = !self.camera_off && self.camera_state.status == MediaStatus::Starting;
        if self.screen == Screen::Room || self.recording_since.is_some() || starting {
            cx.notify();
        }
    }

    pub(crate) fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.no_keep_on_top_line = false;
        self.muted = !self.muted;
        self.monitor = false;
        self.recording_since = None;
        self.mic_state = MediaState::default();
        cx.notify();
    }

    pub(crate) fn toggle_camera(&mut self, cx: &mut Context<Self>) {
        self.no_keep_on_top_line = false;
        self.camera_off = !self.camera_off;
        self.camera_state = MediaState::default();
        self.camera_since = self.ticks;
        cx.notify();
    }

    /// Into the room, floated in the same press where the host floats: the
    /// call stays in sight above other windows while the person works
    /// elsewhere.
    pub(crate) fn join(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Room;
        self.entered = self.ticks;
        self.monitor = false;
        self.menu = None;
        // the room's own clock; the pieces open again in their new place
        self.camera_since = self.ticks;
        cx.host()
            .notify::<HostTitle>("Huddle \u{b7} just you".into());
        if !self.no_float {
            self.float(true, cx);
        }
        window.focus("huddle-room");
        cx.notify();
    }

    /// Out of the room, the pane back on the desk (asked whether or not
    /// the host has said it floats yet: nothing happens where it does not).
    pub(crate) fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Check;
        self.menu = None;
        self.camera_since = self.ticks;
        self.no_keep_on_top_line = false;
        cx.host().notify::<HostTitle>("Huddle".into());
        if !self.no_float {
            cx.host().notify::<HostUnfloat>(());
        }
        window.focus("huddle-join");
        cx.notify();
    }

    /// The pane in a window of its own (or, floating already, kept on top
    /// or not): at a press, as the host takes it. A host that keeps nothing
    /// on top says so in its answer, and the float says so once.
    pub(crate) fn float(&mut self, on_top: bool, cx: &mut Context<Self>) {
        let fresh = !self.placement.floating;
        let floated = cx.host().ask::<HostFloat>(Float { on_top, ..FLOAT });
        cx.land(floated, move |view, floated, cx| match floated {
            Ok(placement) => {
                if on_top && !placement.on_top {
                    view.no_keep_on_top = true;
                    view.no_keep_on_top_line = fresh;
                    cx.notify();
                }
            }
            Err(refusal) => cx.log_refused("the float", &refusal),
        })
        .detach();
    }

    /// Back to the full window: the call goes on, on the desk.
    pub(crate) fn back(&mut self, cx: &mut Context<Self>) {
        self.no_keep_on_top_line = false;
        cx.host().notify::<HostUnfloat>(());
        cx.notify();
    }

    /// The pin: floating, kept on top or let among the other windows; on
    /// the desk, float again.
    pub(crate) fn pin(&mut self, cx: &mut Context<Self>) {
        let on_top = !self.placement.floating || !self.placement.on_top;
        self.float(on_top, cx);
    }

    /// The room is one tile with its controls over it: below 480 by 320.
    pub(crate) fn compact(&self) -> bool {
        let (width, height) = self.viewport;
        width < COMPACT.0 || height < COMPACT.1
    }

    /// The menu of `which` opened, its picked row active, the lists read
    /// again (a device plugged in since shows), the keys in it.
    pub(crate) fn open_menu(&mut self, which: Which, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(which);
        let cursor = rows.iter().position(|row| row.picked).unwrap_or(0);
        self.menu = Some(Menu { which, cursor });
        self.list(cx);
        window.focus("huddle-menu");
        cx.notify();
    }

    pub(crate) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.menu.take() {
            window.focus(menu.which.opener());
        }
        cx.notify();
    }

    /// Row `index` of the open menu picked: its device on its piece.
    pub(crate) fn pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu else { return };
        if let Some(row) = self.rows(menu.which).into_iter().nth(index)
            && let Some(id) = row.id
        {
            match row.kind {
                Which::Camera => {
                    self.camera = Some(id);
                    self.camera_state = MediaState::default();
                    self.camera_since = self.ticks;
                }
                Which::Microphone => {
                    self.microphone = Some(id);
                    self.mic_state = MediaState::default();
                }
                Which::Speaker | Which::Sound => {
                    self.speaker = Some(id);
                    self.mic_state = MediaState::default();
                }
            }
        }
        self.close_menu(window, cx);
    }

    /// The rows of menu `which`: its devices, the system's default first,
    /// the rest in the host's order; one row saying why when there are
    /// none.
    pub(crate) fn rows(&self, which: Which) -> Vec<Row> {
        let audio = self.audio.ready().cloned().unwrap_or_default();
        let (cameras, failed_cameras) = (
            self.cameras.ready().cloned().unwrap_or_default(),
            failure(&self.cameras),
        );
        let failed_audio = failure(&self.audio);
        match which {
            Which::Camera => rows(
                Which::Camera,
                &cameras,
                self.camera.as_deref(),
                failed_cameras,
            ),
            Which::Microphone => rows(
                Which::Microphone,
                &audio.inputs,
                self.microphone.as_deref(),
                failed_audio,
            ),
            Which::Speaker => rows(
                Which::Speaker,
                &audio.outputs,
                self.speaker.as_deref(),
                failed_audio,
            ),
            Which::Sound => {
                let mut both = rows(
                    Which::Microphone,
                    &audio.inputs,
                    self.microphone.as_deref(),
                    failed_audio.clone(),
                );
                both.extend(rows(
                    Which::Speaker,
                    &audio.outputs,
                    self.speaker.as_deref(),
                    failed_audio,
                ));
                both
            }
        }
    }

    /// The name a picker shows: the picked device's, else the default's.
    pub(crate) fn shown_device(&self, which: Which) -> String {
        self.rows(which)
            .into_iter()
            .find(|row| row.picked && row.id.is_some())
            .map_or("System default".into(), |row| row.name)
    }

    /// The menu's opener's bounds, as layout last put it.
    pub(crate) fn anchor(&self, which: Which) -> Option<Bounds<Pixels>> {
        self.anchors
            .iter()
            .find(|(at, _)| *at == which)
            .map(|(_, bounds)| *bounds)
    }

    pub(crate) fn set_anchor(&mut self, which: Which, bounds: Bounds<Pixels>) {
        self.anchors.retain(|(at, _)| *at != which);
        self.anchors.push((which, bounds));
    }

    /// The speaker test: the host's chime on the picked speaker, apart
    /// from the microphone (it works muted), "Playing…" until the host
    /// answers that it sounded.
    pub(crate) fn chime(&mut self, cx: &mut Context<Self>) {
        self.chiming = true;
        self.speaker_fault = None;
        let chimed = cx.host().ask::<MicrophoneChime>(self.speaker.clone());
        cx.land(chimed, |view, chimed, cx| {
            view.chiming = false;
            view.speaker_fault = chimed.err().map(|refused| refused.message);
            cx.notify();
        })
        .detach();
        cx.notify();
    }

    /// The microphone piece told its state: a recording's start starts the
    /// countdown.
    pub(crate) fn heard_mic(&mut self, state: &MediaState) {
        match state.playback {
            Playback::Recording if self.recording_since.is_none() => {
                self.recording_since = Some(self.ticks);
            }
            Playback::Recording => {}
            _ => self.recording_since = None,
        }
        self.mic_state = state.clone();
    }
}

/// One row of a device menu.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) kind: Which,
    /// `None`: the row that says there is nothing to pick.
    pub(crate) id: Option<String>,
    pub(crate) name: String,
    /// The id beside the name, where another device shares the name.
    pub(crate) twin: Option<String>,
    pub(crate) picked: bool,
}

/// What a menu reads of a device: a camera and a microphone or speaker
/// alike.
pub(crate) trait Listed {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn default(&self) -> bool;
}

impl Listed for CameraDevice {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn default(&self) -> bool {
        self.default
    }
}

impl Listed for AudioDevice {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn default(&self) -> bool {
        self.default
    }
}

fn rows<D: Listed>(
    kind: Which,
    devices: &[D],
    picked: Option<&str>,
    failed: Option<String>,
) -> Vec<Row> {
    if devices.is_empty() {
        return vec![Row {
            kind,
            id: None,
            name: failed.unwrap_or_else(|| "None found".into()),
            twin: None,
            picked: false,
        }];
    }
    let picked = devices
        .iter()
        .position(|device| Some(device.id()) == picked)
        .or_else(|| devices.iter().position(|device| device.default()));
    let mut ordered: Vec<(usize, &D)> = devices.iter().enumerate().collect();
    // the system's default first, the rest as the host listed them
    ordered.sort_by_key(|(_, device)| !device.default());
    ordered
        .into_iter()
        .map(|(at, device)| Row {
            kind,
            id: Some(device.id().to_owned()),
            name: device.name().to_owned(),
            twin: twin(devices, device),
            picked: picked == Some(at),
        })
        .collect()
}

/// Why a device list is not there, in the host's words.
fn failure<T>(list: &Loadable<T>) -> Option<String> {
    match list {
        Loadable::Failed(error) => Some(error.message.clone()),
        _ => None,
    }
}

/// Where another device shares its name (a sound card's devices often
/// do), the end of its id that tells the two apart: past the last `:`
/// (`alsa:plughw:CARD=Loopback,DEV=1` is `CARD=Loopback,DEV=1`), at most
/// 20 characters.
pub(crate) fn twin<D: Listed>(devices: &[D], device: &D) -> Option<String> {
    let shared = devices
        .iter()
        .filter(|other| other.name() == device.name())
        .count();
    (shared > 1).then(|| {
        let end = device.id().rsplit(':').next().unwrap_or(device.id());
        let chars: Vec<char> = end.chars().collect();
        match chars.len() > 20 {
            true => format!(
                "\u{2026}{}",
                chars[chars.len() - 19..].iter().collect::<String>()
            ),
            false => end.to_owned(),
        }
    })
}

/// The accesskit popup kind a picker and a caret announce.
pub(crate) const MENU_POPUP: accesskit::HasPopup = accesskit::HasPopup::Menu;

impl Render for Huddle {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = window.viewport_size();
        self.viewport = (size.width.into(), size.height.into());
        ui::render(self, cx)
    }
}

export_view!(Huddle);

#[cfg(test)]
mod tests;
