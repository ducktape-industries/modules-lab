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
//! it back (`host.unfloat`). The pin keeps the float on top or not, and back
//! returns it to the full window; only they read where the pane is
//! (`host.placement`). A host that cannot float a pane refuses that
//! subscription `unknown_request`: the room then has no pin, and Join keeps
//! the view where it is.
//!
//! The view never touches a device: it places the SDK's `camera` and
//! `microphone` pieces, which the host captures, draws and plays, and hears
//! how their devices are. Turning the camera off or muting removes the
//! piece, which releases the device (the camera's light and the system's
//! microphone mark go off). The menus list what `camera.devices` and
//! `microphone.devices` answer, read again whenever a menu opens (no host
//! tells the view of a device plugged in).
//!
//! A phone keeps one full-screen window and cannot float a pane: under a
//! finger the view draws the phone's own screens (`ui::phone`), picked once
//! at the root, where Join keeps the view where it is, one Sound route
//! stands for the microphone and the speaker, and a flip turns to the
//! camera on the other side.
//!
//! This file is the model: what the person picked and switched, what the
//! host listed and the pieces told, and what each press changes. What is
//! drawn is `ui` (the desktop's tree) or `ui::phone` (the phone's), in the
//! words of `words`, with what both draw alike in `look`.
use serde::{Deserialize, Serialize};
use view::prelude::*;

use crate::words::Device;

mod look;
mod ui;
mod words;

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

/// Which screen is up: the device check, or the room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Screen {
    #[default]
    Check,
    Room,
}

/// What a pick picks: a camera, a microphone or a speaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Camera,
    Microphone,
    Speaker,
}

/// Which device menu: one kind's, the room's sound menu, which lists the
/// microphones and then the speakers, or a phone's routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Which {
    Kind(Kind),
    Sound,
    /// A phone's sound sheet: the speakers, which on a phone are the
    /// routes the microphone follows.
    Routes,
}

/// How a menu orders its devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    /// The system's default first, the rest as the host listed them: a
    /// desktop's picker, whose default stays where it is while one is
    /// picked.
    DefaultFirst,
    /// As the host listed them: a phone's routes, whose default is the
    /// route playing, which moves to the one picked; first, its row would
    /// jump under the finger.
    Listed,
}

impl Which {
    /// The kinds it lists, in order.
    pub(crate) fn kinds(&self) -> &[Kind] {
        match self {
            Which::Kind(kind) => std::slice::from_ref(kind),
            Which::Sound => &[Kind::Microphone, Kind::Speaker],
            Which::Routes => &[Kind::Speaker],
        }
    }

    fn order(&self) -> Order {
        match self {
            Which::Kind(_) | Which::Sound => Order::DefaultFirst,
            Which::Routes => Order::Listed,
        }
    }
}

/// An open device menu: which, the row the arrows are on, and the id of
/// the control that opened it, which takes the keys back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Menu {
    pub(crate) which: Which,
    pub(crate) cursor: usize,
    pub(crate) opener: &'static str,
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
    /// What the desktop's tree keeps between frames.
    #[serde(skip)]
    pub(crate) desk: ui::Desk,
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
        cx.host().notify::<HostTitle>(words::HUDDLE.into());
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
    /// The device lists, read again (the lists on screen stay meanwhile).
    pub(crate) fn list(&mut self, cx: &mut Context<Self>) {
        let cameras = cx.host().ask::<CameraDevices>(());
        self.read(cameras, |view| &mut view.cameras, cx);
        let audio = cx.host().ask::<MicrophoneDevices>(());
        self.read(audio, |view| &mut view.audio, cx);
    }

    /// Reads one device list into the slot `at` finds, as `cx.load` does,
    /// and drops the picks the list landed without.
    fn read<T: 'static>(
        &mut self,
        list: impl Future<Output = Result<T, Error>> + 'static,
        at: fn(&mut Self) -> &mut Loadable<T>,
        cx: &mut Context<Self>,
    ) {
        let landing = cx.land(list, move |view, listed, cx| {
            *at(view) = listed.into();
            view.forget_unlisted();
            cx.notify();
        });
        let slot = at(self);
        *slot = match slot.take() {
            Loadable::Ready(shown) | Loadable::Reloading(shown, _) => {
                Loadable::Reloading(shown, landing)
            }
            _ => Loadable::Loading(landing),
        };
    }

    /// Drops each pick its list no longer holds (a device unplugged since
    /// it was picked, or since the pick was saved): its piece and the
    /// speaker test go back to the system's default, since the host opens
    /// only a device it lists.
    fn forget_unlisted(&mut self) {
        let audio = self.audio.ready();
        let gone = [
            (
                Kind::Camera,
                self.cameras
                    .ready()
                    .is_some_and(|cameras| !holds(cameras, &self.camera)),
            ),
            (
                Kind::Microphone,
                audio.is_some_and(|audio| !holds(&audio.inputs, &self.microphone)),
            ),
            (
                Kind::Speaker,
                audio.is_some_and(|audio| !holds(&audio.outputs, &self.speaker)),
            ),
        ];
        for (kind, gone) in gone {
            if gone {
                self.set_pick(kind, None);
            }
        }
    }

    /// Both devices opened again, the lists read again: after a device
    /// went away, was refused or was busy. A pick the lists land without
    /// is let go then.
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

    /// Whether `device`'s piece is in trouble: switched on, and neither
    /// starting nor live.
    pub(crate) fn troubled(&self, device: Device) -> bool {
        match device {
            Device::Camera => !self.camera_off && trouble(&self.camera_state),
            Device::Microphone => !self.muted && trouble(&self.mic_state),
        }
    }

    /// What `device`'s piece last told.
    pub(crate) fn state(&self, device: Device) -> &MediaState {
        match device {
            Device::Camera => &self.camera_state,
            Device::Microphone => &self.mic_state,
        }
    }

    pub(crate) fn mic_live(&self) -> bool {
        !self.muted && self.mic_state.status == MediaStatus::Live
    }

    /// The camera on and still opening two seconds on (a system asking
    /// for permission, a slow driver): said, so it is not taken for a dead
    /// one.
    pub(crate) fn camera_slow(&self) -> bool {
        !self.camera_off
            && self.camera_state.status == MediaStatus::Starting
            && self.ticks.saturating_sub(self.camera_since) >= 2
    }

    /// Whether Record can act: on a live microphone with no speaker test
    /// sounding, or to stop what is under way.
    pub(crate) fn can_record(&self) -> bool {
        (self.mic_live() && !self.chiming) || self.mic_state.playback != Playback::Idle
    }

    /// Whether the speaker test can sound: not over another, nor over a
    /// recording's playback. It works muted, and with the microphone in
    /// trouble: an output, apart from the microphone.
    pub(crate) fn can_test(&self) -> bool {
        self.mic_state.playback == Playback::Idle && !self.chiming
    }

    /// A recording's countdown, `0:SS`: the seconds it has left, at least
    /// one (the host's playback takes over at the end).
    pub(crate) fn countdown(&self) -> String {
        let since = self.recording_since.unwrap_or(self.ticks);
        let left = u64::from(RECORD_SECONDS)
            .saturating_sub(self.ticks.saturating_sub(since))
            .max(1);
        format!("0:{left:02}")
    }

    /// The time in the room, `MM:SS`, `H:MM:SS` past an hour.
    pub(crate) fn clock(&self) -> String {
        let elapsed = self.ticks.saturating_sub(self.entered);
        match elapsed / 3600 {
            0 => format!("{:02}:{:02}", elapsed / 60, elapsed % 60),
            hours => format!("{hours}:{:02}:{:02}", elapsed / 60 % 60, elapsed % 60),
        }
    }

    /// The camera the pieces open: the pick, else the system's default;
    /// `None` before the list lands, or with none in it.
    pub(crate) fn camera_in_use(&self) -> Option<&CameraDevice> {
        in_use(self.cameras.ready()?, self.camera.as_deref())
    }

    /// The speaker the microphone piece plays through (on a phone, the
    /// route the microphone follows): the pick, else the system's default.
    pub(crate) fn speaker_in_use(&self) -> Option<&AudioDevice> {
        in_use(&self.audio.ready()?.outputs, self.speaker.as_deref())
    }

    /// The listed speaker `id` names.
    pub(crate) fn speaker(&self, id: &str) -> Option<&AudioDevice> {
        let outputs = &self.audio.ready()?.outputs;
        outputs.iter().find(|output| output.id == id)
    }

    /// Whether the sound plays where the microphone hears it: through a
    /// loudspeaker, or a speaker not listed yet. A phone refuses the live
    /// loop then, its speaker a hand's width from its microphone.
    pub(crate) fn loops_back(&self) -> bool {
        self.speaker_in_use()
            .is_none_or(|speaker| speaker.kind == AudioKind::Speaker)
    }

    /// Whether the camera in use faces away from the person: a phone's
    /// back camera.
    pub(crate) fn facing_away(&self) -> bool {
        self.camera_in_use()
            .is_some_and(|camera| camera.facing == Facing::Back)
    }

    /// Whether the picture is mirrored: as the person set it, and never
    /// from a camera facing away (text in it would read backwards).
    pub(crate) fn mirrored(&self) -> bool {
        !self.unmirrored && !self.facing_away()
    }

    /// The camera on the other side from the one in use, where the host
    /// lists a front and a back one: what the phone's flip turns to.
    pub(crate) fn other_camera(&self) -> Option<&CameraDevice> {
        let cameras = self.cameras.ready()?;
        let facing = |facing| cameras.iter().find(|camera| camera.facing == facing);
        let (front, back) = (facing(Facing::Front)?, facing(Facing::Back)?);
        match self.camera_in_use()?.facing {
            Facing::Back => Some(front),
            Facing::Front | Facing::External => Some(back),
        }
    }

    /// The camera on the other side picked: its piece opens again on it.
    pub(crate) fn flip(&mut self, cx: &mut Context<Self>) {
        if let Some(other) = self.other_camera() {
            let id = other.id.clone();
            self.set_pick(Kind::Camera, Some(id));
            cx.notify();
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.ticks += 1;
        // drawn again only while something counts seconds: the room (its
        // bar's clock; the compact tile, which has none, is the tree's
        // business), a recording's countdown, a camera slow to open
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

    /// Into the room where the view is: the room's clock starts, the live
    /// loop ends. The phone's Join: a phone keeps one window.
    pub(crate) fn enter_room(&mut self, cx: &mut Context<Self>) {
        self.screen = Screen::Room;
        self.entered = self.ticks;
        self.monitor = false;
        self.menu = None;
        // the room's own clock; the pieces open again in their new place
        self.camera_since = self.ticks;
        cx.host().notify::<HostTitle>(words::IN_THE_ROOM.into());
        cx.notify();
    }

    /// Into the room, floated in the same press where the host floats: the
    /// call stays in sight above other windows while the person works
    /// elsewhere.
    pub(crate) fn join(&mut self, cx: &mut Context<Self>) {
        self.enter_room(cx);
        if !self.no_float {
            self.float(true, cx);
        }
    }

    /// Back to the check where the view is. The phone's Leave.
    pub(crate) fn leave_room(&mut self, cx: &mut Context<Self>) {
        self.screen = Screen::Check;
        self.menu = None;
        self.camera_since = self.ticks;
        self.no_keep_on_top_line = false;
        cx.host().notify::<HostTitle>(words::HUDDLE.into());
        cx.notify();
    }

    /// Out of the room, the pane back on the desk (asked whether or not
    /// the host has said it floats yet: nothing happens where it does not).
    pub(crate) fn leave(&mut self, cx: &mut Context<Self>) {
        self.leave_room(cx);
        if !self.no_float {
            cx.host().notify::<HostUnfloat>(());
        }
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

    /// The menu of `which` opened by the control `opener`, its picked row
    /// active, the lists read again (a device plugged in since shows).
    pub(crate) fn open_menu(&mut self, which: Which, opener: &'static str, cx: &mut Context<Self>) {
        let rows = self.rows(which);
        let cursor = rows.iter().position(|row| row.picked).unwrap_or(0);
        self.menu = Some(Menu {
            which,
            cursor,
            opener,
        });
        self.list(cx);
        cx.notify();
    }

    /// The open menu closed, and handed back to the one who closed it.
    pub(crate) fn close_menu(&mut self, cx: &mut Context<Self>) -> Option<Menu> {
        cx.notify();
        self.menu.take()
    }

    /// Row `index` of the open menu picked, its device on its piece; the
    /// menu closed, and handed back.
    pub(crate) fn pick(&mut self, index: usize, cx: &mut Context<Self>) -> Option<Menu> {
        let menu = self.menu?;
        if let Some(row) = self.rows(menu.which).into_iter().nth(index)
            && let Some(id) = row.id
        {
            self.set_pick(row.kind, Some(id));
        }
        self.close_menu(cx)
    }

    /// `kind`'s pick set to `id` (`None`: the system's default): its piece
    /// opens again on it, and a microphone or a speaker picked ends the
    /// live loop and a recording, as muting does.
    fn set_pick(&mut self, kind: Kind, id: Option<String>) {
        match kind {
            Kind::Camera => {
                self.camera = id;
                self.camera_state = MediaState::default();
                self.camera_since = self.ticks;
                return;
            }
            Kind::Microphone => self.microphone = id,
            Kind::Speaker => self.speaker = id,
        }
        self.mic_state = MediaState::default();
        self.monitor = false;
        self.recording_since = None;
    }

    /// The rows of menu `which`: each kind's devices in turn, in its order.
    pub(crate) fn rows(&self, which: Which) -> Vec<Row> {
        which
            .kinds()
            .iter()
            .flat_map(|kind| self.rows_of(*kind, which.order()))
            .collect()
    }

    /// The rows of one kind in `order`; one row saying why when there are
    /// none.
    fn rows_of(&self, kind: Kind, order: Order) -> Vec<Row> {
        let audio = self.audio.ready();
        match kind {
            Kind::Camera => rows(
                (kind, order),
                self.cameras.ready().map_or(&[][..], Vec::as_slice),
                self.camera.as_deref(),
                nothing(&self.cameras),
            ),
            Kind::Microphone => rows(
                (kind, order),
                audio.map_or(&[][..], |audio| &audio.inputs),
                self.microphone.as_deref(),
                nothing(&self.audio),
            ),
            Kind::Speaker => rows(
                (kind, order),
                audio.map_or(&[][..], |audio| &audio.outputs),
                self.speaker.as_deref(),
                nothing(&self.audio),
            ),
        }
    }

    /// The name a picker shows: the picked device's, else the default's.
    pub(crate) fn shown_device(&self, kind: Kind) -> String {
        self.rows_of(kind, Order::DefaultFirst)
            .into_iter()
            .find(|row| row.picked && row.id.is_some())
            .map_or(words::SYSTEM_DEFAULT.into(), |row| row.name)
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
    pub(crate) kind: Kind,
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

/// The rows of `kind`'s `devices` in `order`, the one in use (`picked`,
/// else the default) checked; `nothing` alone where there are none.
fn rows<D: Listed>(
    (kind, order): (Kind, Order),
    devices: &[D],
    picked: Option<&str>,
    nothing: &str,
) -> Vec<Row> {
    if devices.is_empty() {
        return vec![Row {
            kind,
            id: None,
            name: nothing.to_owned(),
            twin: None,
            picked: false,
        }];
    }
    let in_use = in_use(devices, picked).map(Listed::id);
    let mut ordered: Vec<&D> = devices.iter().collect();
    if order == Order::DefaultFirst {
        ordered.sort_by_key(|device| !device.default());
    }
    ordered
        .into_iter()
        .map(|device| Row {
            kind,
            id: Some(device.id().to_owned()),
            name: device.name().to_owned(),
            twin: twin(devices, device),
            picked: in_use == Some(device.id()),
        })
        .collect()
}

/// The device `pick` names, else the system's default.
fn in_use<'a, D: Listed>(devices: &'a [D], pick: Option<&str>) -> Option<&'a D> {
    devices
        .iter()
        .find(|device| Some(device.id()) == pick)
        .or_else(|| devices.iter().find(|device| device.default()))
}

/// Whether `devices` lists the device `pick` names (no pick, the system's
/// default, always is).
fn holds<D: Listed>(devices: &[D], pick: &Option<String>) -> bool {
    pick.as_deref()
        .is_none_or(|id| devices.iter().any(|device| device.id() == id))
}

/// What a menu says where `list` holds no device: that it is still being
/// asked, the host's refusal, or that there is none.
fn nothing<T>(list: &Loadable<T>) -> &str {
    match list {
        Loadable::Idle | Loadable::Loading(_) => words::LISTING,
        Loadable::Failed(error) => &error.message,
        Loadable::Ready(_) | Loadable::Reloading(..) => words::NONE_FOUND,
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

impl Render for Huddle {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = window.viewport_size();
        self.viewport = (size.width.into(), size.height.into());
        // one wasm, two designs, the model shared: a finger gets the
        // phone's screens, a pointer the desktop's
        if window.touch() {
            ui::phone::render(self, cx)
        } else {
            ui::render(self, cx)
        }
    }
}

export_view!(Huddle);

#[cfg(test)]
mod tests;
