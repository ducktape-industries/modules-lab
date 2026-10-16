//! The view on a fake host: each test drives the frame the way a person
//! would and reads what the host would get. Every frame it sends is held
//! to the host's sanitizer and accessibility audit, and to the view's
//! capabilities (a piece the manifest leaves out fails the test).
use view::Entity;
use view::methods::{
    AudioDevice, AudioDevices, AudioKind, CameraDevice, CameraDevices, ClockTicks, Facing, Float,
    HostFloat, HostPlacement, HostShow, HostTitle, HostUnfloat, MicrophoneChime, MicrophoneDevices,
    Place, Placement,
};
use view::testing::{StreamSender, TestAppContext};
use view::wire::{ImageObjectFit, MediaState, MediaStatus, MicCommand, Node, Playback};

use crate::{FLOAT, Huddle, RECORD_SECONDS};

fn camera(id: &str, name: &str, default: bool, facing: Facing) -> CameraDevice {
    CameraDevice {
        id: id.into(),
        name: name.into(),
        default,
        facing,
    }
}

fn audio(id: &str, name: &str, default: bool, kind: AudioKind) -> AudioDevice {
    AudioDevice {
        id: id.into(),
        name: name.into(),
        default,
        kind,
    }
}

/// A host listing two cameras, a microphone and two speakers, a pane of
/// the desk's default size; its clock.
fn hosted() -> (TestAppContext, StreamSender<ClockTicks>) {
    let mut cx = TestAppContext::new();
    cx.host().handle::<CameraDevices>(|()| {
        Ok(vec![
            camera("0", "Front Camera", true, Facing::Front),
            camera("1", "Desk Camera", false, Facing::External),
        ])
    });
    cx.host().handle::<MicrophoneDevices>(|()| {
        Ok(AudioDevices {
            inputs: vec![audio(
                "alsa:default",
                "Built-in Microphone",
                true,
                AudioKind::Other,
            )],
            outputs: vec![
                audio("alsa:speakers", "Speakers", true, AudioKind::Speaker),
                audio(
                    "alsa:headphones",
                    "Headphones",
                    false,
                    AudioKind::Headphones,
                ),
            ],
        })
    });
    // a host that floats and keeps on top as asked
    cx.host().handle::<HostFloat>(|float| {
        Ok(Placement {
            floating: true,
            on_top: float.on_top,
        })
    });
    let ticks = cx.host().stream::<ClockTicks>();
    cx.simulate_resize(768., 500.);
    (cx, ticks)
}

/// The view open on [`hosted`]'s host.
fn opened() -> (TestAppContext, Entity<Huddle>, StreamSender<ClockTicks>) {
    let (mut cx, ticks) = hosted();
    let view = cx.open::<Huddle>();
    cx.run_until_parked();
    (cx, view, ticks)
}

/// The one camera piece in the tree: its key, device, mirror and fit.
fn camera_piece(cx: &TestAppContext) -> Option<(String, Option<String>, bool, ImageObjectFit)> {
    let mut found = None;
    walk(cx.root(), &mut |node| {
        if let Node::Camera {
            id,
            device,
            mirror,
            object_fit,
            ..
        } = node
        {
            found = Some((
                id.name().unwrap_or_default().to_owned(),
                device.clone(),
                *mirror,
                *object_fit,
            ));
        }
    });
    found
}

/// The one microphone piece: its key, device, output, monitor, commands.
type MicPiece = (
    String,
    Option<String>,
    Option<String>,
    bool,
    Vec<MicCommand>,
);
fn mic_piece(cx: &TestAppContext) -> Option<MicPiece> {
    let mut found = None;
    walk(cx.root(), &mut |node| {
        if let Node::Microphone {
            id,
            device,
            output,
            monitor,
            commands,
            ..
        } = node
        {
            found = Some((
                id.name().unwrap_or_default().to_owned(),
                device.clone(),
                output.clone(),
                *monitor,
                commands.clone(),
            ));
        }
    });
    found
}

fn walk(node: &Node, visit: &mut impl FnMut(&Node)) {
    visit(node);
    for child in node.children() {
        walk(child, visit);
    }
}

fn mic_key(cx: &TestAppContext) -> String {
    mic_piece(cx).expect("a microphone piece").0
}

fn live(device: &str) -> MediaState {
    MediaState {
        status: MediaStatus::Live,
        device: device.into(),
        ..Default::default()
    }
}

fn status(status: MediaStatus) -> MediaState {
    MediaState {
        status,
        ..Default::default()
    }
}

fn disabled(cx: &TestAppContext, id: &str) -> bool {
    cx.interactivity(id).aria.disabled == Some(true)
}

/// Opened: a mirrored camera on the default device, whole in its box, the
/// microphone on its default, and each picker naming its default device.
#[test]
fn it_opens_on_the_defaults_mirrored() {
    let (cx, ..) = opened();
    let (_, device, mirror, fit) = camera_piece(&cx).expect("a camera piece");
    assert_eq!((device, mirror, fit), (None, true, ImageObjectFit::Contain));
    let (_, input, output, monitor, commands) = mic_piece(&cx).expect("a microphone piece");
    assert_eq!((input, output, monitor), (None, None, false));
    assert!(commands.is_empty());
    for name in [
        "Front Camera",
        "Built-in Microphone",
        "Speakers",
        "You",
        "Join huddle",
    ] {
        assert!(cx.has_text(name), "{name} shown: {:?}", cx.texts());
    }
    assert!(!cx.has_text("Desk Camera"), "a closed menu lists nothing");
    assert_eq!(
        cx.interactivity("huddle-pick-camera").aria.label.as_deref(),
        Some("Camera: Front Camera")
    );
}

/// Mirror off flips the piece; the camera toggle removes the piece (the
/// host releases the camera) and brings it back.
#[test]
fn mirror_and_camera_toggle_steer_the_piece() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-mirror");
    assert!(!camera_piece(&cx).unwrap().2);
    cx.simulate_click("huddle-camera");
    assert!(camera_piece(&cx).is_none());
    assert!(cx.has_text("Your camera is off"));
    assert!(disabled(&cx, "huddle-mirror"), "nothing to mirror");
    cx.simulate_click("huddle-camera");
    assert!(camera_piece(&cx).is_some());
}

/// A picked camera, microphone or speaker reaches its piece by id; each
/// menu lists its devices, the default first, and closes on a pick.
#[test]
fn picks_reach_the_pieces() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-pick-camera");
    assert!(cx.has_text("Desk Camera"));
    cx.simulate_click("huddle-menu-1");
    assert_eq!(camera_piece(&cx).unwrap().1.as_deref(), Some("1"));
    assert!(
        !cx.has_text("Front Camera"),
        "closed, the picker names the pick"
    );
    assert!(cx.has_text("Desk Camera"));
    cx.simulate_click("huddle-pick-speaker");
    cx.simulate_click("huddle-menu-1");
    let (_, input, output, ..) = mic_piece(&cx).unwrap();
    assert_eq!(input, None);
    assert_eq!(output.as_deref(), Some("alsa:headphones"));
    cx.simulate_click("huddle-pick-microphone");
    cx.simulate_click("huddle-menu-0");
    assert_eq!(mic_piece(&cx).unwrap().1.as_deref(), Some("alsa:default"));
}

/// The host now lists the front camera alone, and the speakers alone.
fn unplugged(cx: &TestAppContext) {
    cx.host()
        .handle::<CameraDevices>(|()| Ok(vec![camera("0", "Front Camera", true, Facing::Front)]));
    cx.host().handle::<MicrophoneDevices>(|()| {
        Ok(AudioDevices {
            inputs: vec![audio(
                "alsa:default",
                "Built-in Microphone",
                true,
                AudioKind::Other,
            )],
            outputs: vec![audio("alsa:speakers", "Speakers", true, AudioKind::Speaker)],
        })
    });
}

/// A pick saved with the view whose device the host no longer lists (it
/// was unplugged since) is let go when the list lands: the pieces and the
/// speaker test ask for the system's default, which the pickers name,
/// not a device the host would not open.
#[test]
fn a_saved_pick_the_host_no_longer_lists_falls_back_to_the_default() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-pick-camera");
    cx.simulate_click("huddle-menu-1");
    cx.simulate_click("huddle-pick-speaker");
    cx.simulate_click("huddle-menu-1");
    let saved = cx.snapshot().expect("a snapshot");
    unplugged(&cx);
    cx.restore::<Huddle>(&saved).expect("restored");
    cx.run_until_parked();
    assert_eq!(camera_piece(&cx).unwrap().1, None);
    assert_eq!(mic_piece(&cx).unwrap().2, None);
    assert_eq!(
        cx.interactivity("huddle-pick-camera").aria.label.as_deref(),
        Some("Camera: Front Camera")
    );
    cx.host().handle::<MicrophoneChime>(|_| Ok(()));
    cx.simulate_click("huddle-speaker-test");
    cx.run_until_parked();
    assert_eq!(cx.host().requests::<MicrophoneChime>(), [None]);
}

/// A list read again without the picked camera (here as its menu opens)
/// lets the pick go, and the trouble its going caused with it: the piece
/// opens the system's default.
#[test]
fn a_list_read_again_without_the_pick_lets_it_and_its_trouble_go() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-pick-camera");
    cx.simulate_click("huddle-menu-1");
    let (key, ..) = camera_piece(&cx).unwrap();
    unplugged(&cx);
    cx.simulate_media_state(&key, status(MediaStatus::NoDevice));
    assert!(cx.has_text("No camera found"));
    cx.simulate_click("huddle-pick-camera");
    cx.run_until_parked();
    cx.simulate_key_down("huddle-menu", "escape");
    assert!(!cx.has_text("No camera found"), "{:?}", cx.texts());
    assert_eq!(camera_piece(&cx).unwrap().1, None);
}

/// Try again after a picked camera went away opens the system's default:
/// the lists, read again, no longer hold the pick, so the camera that went
/// away is not asked for again.
#[test]
fn try_again_does_not_ask_again_for_a_camera_that_went_away() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-pick-camera");
    cx.simulate_click("huddle-menu-1");
    let (key, ..) = camera_piece(&cx).unwrap();
    unplugged(&cx);
    cx.simulate_media_state(&key, status(MediaStatus::Gone));
    assert!(cx.has_text("The camera went away"));
    cx.simulate_click("huddle-camera-retry");
    cx.run_until_parked();
    assert_eq!(camera_piece(&cx).unwrap().1, None);
    assert!(!cx.has_text("The camera went away"));
}

/// Picking a microphone or a speaker ends the live loop, as muting does:
/// a speaker picked mid-loop could be the one the microphone hears.
#[test]
fn picking_a_microphone_or_speaker_ends_the_live_loop() {
    let (mut cx, ..) = opened();
    for picker in ["huddle-pick-speaker", "huddle-pick-microphone"] {
        let key = mic_key(&cx);
        cx.simulate_media_state(&key, live("Built-in Microphone"));
        cx.simulate_click("huddle-monitor");
        assert!(mic_piece(&cx).unwrap().3, "looping");
        cx.simulate_click(picker);
        cx.simulate_click("huddle-menu-0");
        assert!(!mic_piece(&cx).unwrap().3, "{picker}: the loop ends");
    }
}

/// The menu by keys: ↓ on a picker opens it on the picked row, ↓ moves,
/// Enter picks and closes; Escape closes without a pick and gives the keys
/// back to the picker.
#[test]
fn a_menu_opens_picks_and_closes_by_keys() {
    let (mut cx, ..) = opened();
    cx.simulate_focus("huddle-pick-camera");
    cx.simulate_key_down("huddle-pick-camera", "down");
    assert!(cx.has_text("Desk Camera"), "open: {:?}", cx.texts());
    cx.simulate_key_down("huddle-menu", "down");
    cx.simulate_key_down("huddle-menu", "enter");
    assert_eq!(camera_piece(&cx).unwrap().1.as_deref(), Some("1"));
    assert!(!cx.has_text("Front Camera"), "closed");
    cx.simulate_click("huddle-pick-camera");
    assert!(cx.has_text("Front Camera"));
    cx.simulate_key_down("huddle-menu", "escape");
    assert!(!cx.has_text("Front Camera"), "closed by Escape");
    assert_eq!(camera_piece(&cx).unwrap().1.as_deref(), Some("1"));
}

/// The id of the element that has the keys.
fn focused(cx: &TestAppContext) -> Option<String> {
    cx.focused()
        .and_then(|node| node.identity())
        .and_then(|id| id.name().map(str::to_owned))
}

/// In the room the camera's caret opens the camera menu over the bar, and
/// the keys come back to that caret when the menu closes, by Escape or by
/// a pick: the check's picker is not there to take them.
#[test]
fn the_room_camera_menu_gives_the_keys_back_to_its_caret() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-join");
    cx.run_until_parked();
    cx.simulate_click("huddle-camera-menu");
    assert_eq!(focused(&cx).as_deref(), Some("huddle-menu"));
    assert!(cx.has_text("Desk Camera"));
    cx.simulate_key_down("huddle-menu", "escape");
    assert_eq!(focused(&cx).as_deref(), Some("huddle-camera-menu"));
    cx.simulate_click("huddle-camera-menu");
    cx.simulate_key_down("huddle-menu", "down");
    cx.simulate_key_down("huddle-menu", "enter");
    assert_eq!(camera_piece(&cx).unwrap().1.as_deref(), Some("1"));
    assert_eq!(focused(&cx).as_deref(), Some("huddle-camera-menu"));
}

/// The room's microphone caret opens one menu of two groups, the
/// microphones and then the speakers, each headed; a speaker picked there
/// reaches the microphone piece's output, and the keys return to the caret.
#[test]
fn the_room_sound_menu_lists_microphones_then_speakers() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-join");
    cx.run_until_parked();
    cx.simulate_click("huddle-mic-menu");
    let texts = cx.texts();
    let at = |text: &str| texts.iter().position(|shown| shown == text);
    let order = [
        "Microphone",
        "Built-in Microphone",
        "Speaker",
        "Speakers",
        "Headphones",
    ]
    .map(|text| at(text).unwrap_or_else(|| panic!("{text} in {texts:?}")));
    assert!(order.is_sorted(), "{order:?}");
    // the rows: the microphone, then the two speakers
    cx.simulate_click("huddle-menu-2");
    assert_eq!(
        mic_piece(&cx).unwrap().2.as_deref(),
        Some("alsa:headphones")
    );
    assert_eq!(focused(&cx).as_deref(), Some("huddle-mic-menu"));
}

/// Record asks the host once, for five seconds; the host's recording and
/// playback show, with the countdown on the view's clock, and Stop ends
/// it.
#[test]
fn record_then_play_back() {
    let (mut cx, _, ticks) = opened();
    let key = mic_key(&cx);
    cx.simulate_media_state(&key, live("Built-in Microphone"));
    assert!(cx.has_text("Hear how you sound to others."));
    cx.simulate_click("huddle-record");
    assert_eq!(
        mic_piece(&cx).unwrap().4,
        [MicCommand::Record {
            seconds: RECORD_SECONDS
        }]
    );
    cx.simulate_media_state(
        &key,
        MediaState {
            playback: Playback::Recording,
            ..live("Built-in Microphone")
        },
    );
    // the command went with the frame that carried it
    assert!(mic_piece(&cx).unwrap().4.is_empty());
    assert!(cx.has_text("0:05"), "{:?}", cx.texts());
    assert!(cx.has_text("Stop"));
    ticks.send(());
    cx.run_until_parked();
    assert!(cx.has_text("0:04"), "{:?}", cx.texts());
    cx.simulate_media_state(
        &key,
        MediaState {
            playback: Playback::Playing,
            ..live("Built-in Microphone")
        },
    );
    assert!(cx.has_text("Playing it back…"));
    cx.simulate_click("huddle-record");
    assert_eq!(mic_piece(&cx).unwrap().4, [MicCommand::Stop]);
}

/// Test asks the host for its chime on the picked speaker
/// (`microphone.chime`), apart from the microphone piece, and says
/// "Playing…" until the host answers that it sounded.
#[test]
fn the_speaker_test_chimes() {
    let (mut cx, view, _) = opened();
    cx.host().never::<MicrophoneChime>();
    assert!(
        !disabled(&cx, "huddle-speaker-test"),
        "no microphone needed"
    );
    cx.update(&view, |view, _, cx| {
        view.speaker = Some("alsa:headphones".into());
        cx.notify();
    });
    cx.simulate_click("huddle-speaker-test");
    cx.run_until_parked();
    assert_eq!(
        cx.host().requests::<MicrophoneChime>(),
        [Some("alsa:headphones".to_owned())]
    );
    assert!(
        mic_piece(&cx).unwrap().4.is_empty(),
        "the piece is not asked"
    );
    assert!(cx.has_text("Playing…"));
    assert!(disabled(&cx, "huddle-speaker-test"), "one chime at a time");
    assert!(disabled(&cx, "huddle-record"), "one sound at a time");
}

/// Test works while muted (the microphone let go) and while the
/// microphone is in trouble: the host's answer ends "Playing…", and its
/// refusal is said under the speaker.
#[test]
fn test_works_while_muted() {
    let (mut cx, ..) = opened();
    cx.host().handle::<MicrophoneChime>(|_| Ok(()));
    cx.simulate_click("huddle-mic");
    assert!(mic_piece(&cx).is_none(), "muted: no microphone piece");
    assert!(!disabled(&cx, "huddle-speaker-test"));
    cx.simulate_click("huddle-speaker-test");
    cx.run_until_parked();
    assert_eq!(cx.host().requests::<MicrophoneChime>(), [None]);
    assert!(!cx.has_text("Playing…"), "answered: the chime has sounded");
    assert!(!disabled(&cx, "huddle-speaker-test"));
    // unmuted, the microphone refused: the speaker still tests, and says
    // when it does not play
    cx.simulate_click("huddle-mic");
    let key = mic_key(&cx);
    cx.simulate_media_state(&key, status(MediaStatus::Denied));
    cx.host()
        .refuse::<MicrophoneChime>("host_fault", "no such speaker");
    assert!(!disabled(&cx, "huddle-speaker-test"));
    cx.simulate_click("huddle-speaker-test");
    cx.run_until_parked();
    assert_eq!(cx.host().requests::<MicrophoneChime>().len(), 2);
    assert!(cx.has_text("Test failed: no such speaker."));
}

/// The live loop is off until switched on, only while the microphone is
/// live, and its warning about headphones is always beside it. Mute on
/// the check lets the microphone go (the piece leaves), and says what
/// waits for it.
#[test]
fn the_live_loop_waits_for_its_switch_and_mute_lets_the_microphone_go() {
    let (mut cx, ..) = opened();
    assert!(cx.has_text(
        "Wear headphones: through speakers your voice loops back into the microphone and howls."
    ));
    assert!(
        disabled(&cx, "huddle-monitor"),
        "no loop before the microphone is live"
    );
    let key = mic_key(&cx);
    cx.simulate_media_state(&key, live("Built-in Microphone"));
    cx.simulate_click("huddle-monitor");
    assert!(mic_piece(&cx).unwrap().3);
    cx.simulate_click("huddle-mic");
    assert!(mic_piece(&cx).is_none(), "muted: the microphone is let go");
    assert!(cx.has_text("Unmute to test your microphone."));
    for control in ["huddle-record", "huddle-monitor"] {
        assert!(disabled(&cx, control), "{control} waits for the microphone");
    }
    assert!(
        !disabled(&cx, "huddle-speaker-test"),
        "the speaker is apart"
    );
    cx.simulate_click("huddle-mic");
    assert!(!mic_piece(&cx).unwrap().3, "the loop stays off");
}

/// A refused or vanished device says so in its card, and Try again opens
/// a new piece; a missing camera offers joining without it.
#[test]
fn trouble_is_said_and_retried() {
    let (mut cx, ..) = opened();
    let (first, ..) = camera_piece(&cx).unwrap();
    cx.simulate_media_state(&first, status(MediaStatus::Denied));
    assert!(cx.has_text("Camera access is off"));
    assert!(cx.has_text(
        "Your system refused this app the camera. Check the device's permissions (or your \
         desktop's camera portal), then try again."
    ));
    assert!(camera_piece(&cx).is_none(), "a refused camera is let go");
    cx.simulate_click("huddle-camera-retry");
    cx.run_until_parked();
    assert_ne!(camera_piece(&cx).unwrap().0, first, "a new piece");
    let key = mic_key(&cx);
    cx.simulate_media_state(&key, status(MediaStatus::Gone));
    assert!(cx.has_text("The microphone went away"));
    cx.simulate_click("huddle-microphone-retry");
    let key = mic_key(&cx);
    cx.simulate_media_state(&key, status(MediaStatus::NoDevice));
    assert!(cx.has_text("No microphone found"));
    let (camera, ..) = camera_piece(&cx).unwrap();
    cx.simulate_media_state(&camera, status(MediaStatus::NoDevice));
    assert!(cx.has_text("No camera found"));
    cx.simulate_click("huddle-camera-without");
    assert!(cx.has_text("Only you are here. Calls with others are not connected yet."));
    assert!(camera_piece(&cx).is_none(), "joined without the camera");
}

/// In the room a camera that is not found says so, with Try again, and
/// offers no "Join without camera": the person has joined already.
#[test]
fn a_missing_camera_in_the_room_offers_no_join() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-join");
    cx.run_until_parked();
    let (key, ..) = camera_piece(&cx).unwrap();
    cx.simulate_media_state(&key, status(MediaStatus::NoDevice));
    assert!(cx.has_text("No camera found"));
    assert!(cx.find("huddle-camera-retry").is_some());
    assert!(cx.find("huddle-camera-without").is_none());
}

/// A host that can open the system's settings for a refused camera says
/// so: Open Settings asks for exactly that page.
#[test]
fn a_refusal_opens_the_settings_where_the_host_can() {
    let (mut cx, ..) = opened();
    let (key, ..) = camera_piece(&cx).unwrap();
    cx.simulate_media_state(
        &key,
        MediaState {
            status: MediaStatus::Denied,
            detail: "Open System Settings → Privacy & Security → Camera, turn Ducktape on, \
                     then try again."
                .into(),
            opens_settings: true,
            ..Default::default()
        },
    );
    assert!(cx.has_text(
        "Ducktape is not allowed to use the camera. Open System Settings → Privacy & Security \
         → Camera, turn Ducktape on, then try again."
    ));
    cx.simulate_click("huddle-camera-settings");
    assert_eq!(cx.host().requests::<HostShow>(), [Place::CameraSettings]);
}

/// Join enters the room with every pick kept, the tile covering its box;
/// Leave comes back to the check. The room is honest about being alone.
#[test]
fn join_and_leave_switch_screens_keeping_picks() {
    let (mut cx, _, ticks) = opened();
    cx.simulate_click("huddle-pick-camera");
    cx.simulate_click("huddle-menu-1");
    cx.simulate_click("huddle-mirror");
    cx.simulate_click("huddle-join");
    assert!(cx.has_text("Only you are here. Calls with others are not connected yet."));
    assert!(cx.has_text("00:00 · only you"), "{:?}", cx.texts());
    let (_, device, mirror, fit) = camera_piece(&cx).unwrap();
    assert_eq!(
        (device.as_deref(), mirror, fit),
        (Some("1"), false, ImageObjectFit::Cover)
    );
    assert!(
        mic_piece(&cx).is_some(),
        "the microphone is open in the room"
    );
    ticks.send(());
    cx.run_until_parked();
    assert!(cx.has_text("00:01 · only you"));
    cx.simulate_click("huddle-leave");
    assert!(cx.has_text("Join huddle"));
    let (_, device, mirror, fit) = camera_piece(&cx).unwrap();
    assert_eq!(
        (device.as_deref(), mirror, fit),
        (Some("1"), false, ImageObjectFit::Contain)
    );
}

/// In the room M mutes and V turns the camera off, each removing its
/// piece; speaking rings the tile and marks the chip.
#[test]
fn the_room_mutes_by_key_and_shows_who_speaks() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-join");
    let key = mic_key(&cx);
    cx.simulate_media_state(
        &key,
        MediaState {
            speaking: true,
            ..live("Built-in Microphone")
        },
    );
    assert_eq!(
        cx.interactivity("huddle-mark-speaking")
            .aria
            .label
            .as_deref(),
        Some("Speaking")
    );
    cx.simulate_key_down("huddle-room", "m");
    assert!(
        mic_piece(&cx).is_none(),
        "muted in the room: the microphone is let go"
    );
    assert_eq!(
        cx.interactivity("huddle-mark-muted").aria.label.as_deref(),
        Some("Muted")
    );
    cx.simulate_key_down("huddle-room", "v");
    assert!(camera_piece(&cx).is_none());
    cx.simulate_click("huddle-mic");
    assert!(mic_piece(&cx).is_some());
}

/// A host without camera and microphone support: said once, no piece.
#[test]
fn a_host_without_media_says_so() {
    let mut cx = TestAppContext::new();
    cx.host().refuse::<CameraDevices>(
        view::methods::refusal::UNKNOWN_REQUEST,
        "this build has no camera or microphone",
    );
    cx.host().refuse::<MicrophoneDevices>(
        view::methods::refusal::UNKNOWN_REQUEST,
        "this build has no camera or microphone",
    );
    let _ticks = cx.host().stream::<ClockTicks>();
    cx.open::<Huddle>();
    cx.run_until_parked();
    assert!(cx.has_text("No camera or microphone in this build"));
    assert!(camera_piece(&cx).is_none() && mic_piece(&cx).is_none());
    assert!(!cx.has_text("Join huddle"));
}

/// Two devices of one name are told apart by their ids' ends.
#[test]
fn a_shared_name_shows_its_id() {
    let devices = [
        audio(
            "alsa:plughw:CARD=Loopback,DEV=0",
            "Loopback PCM",
            false,
            AudioKind::Other,
        ),
        audio(
            "alsa:plughw:CARD=Loopback,DEV=1",
            "Loopback PCM",
            false,
            AudioKind::Other,
        ),
        audio("alsa:default", "System default", true, AudioKind::Other),
    ];
    assert_eq!(
        crate::twin(&devices, &devices[1]).as_deref(),
        Some("CARD=Loopback,DEV=1")
    );
    assert_eq!(crate::twin(&devices, &devices[2]), None);
}

// ---------- the float ----------

/// The ids from the root down to the one node of `kind`, each named
/// ancestor's: the path the host keys a piece's device by.
fn path_to(cx: &TestAppContext, piece: fn(&Node) -> bool) -> Option<Vec<String>> {
    fn down(node: &Node, piece: fn(&Node) -> bool, path: &mut Vec<String>) -> bool {
        let named = node.identity().and_then(|id| id.name().map(str::to_owned));
        if let Some(name) = &named {
            path.push(name.clone());
        }
        if piece(node) || node.children().iter().any(|child| down(child, piece, path)) {
            return true;
        }
        if named.is_some() {
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    down(cx.root(), piece, &mut path).then_some(path)
}

fn is_camera(node: &Node) -> bool {
    matches!(node, Node::Camera { .. })
}

fn is_microphone(node: &Node) -> bool {
    matches!(node, Node::Microphone { .. })
}

fn toggled(cx: &TestAppContext, id: &str) -> Option<bool> {
    cx.interactivity(id)
        .aria
        .toggled
        .map(|toggled| toggled == view::accesskit::Toggled::True)
}

/// Joined, the host saying the pane floats on top, the float's size.
fn floated(
    cx: &mut TestAppContext,
    (width, height): (f32, f32),
) -> view::testing::StreamSender<HostPlacement> {
    let placement = cx.host().stream::<HostPlacement>();
    cx.simulate_click("huddle-join");
    cx.run_until_parked();
    placement.send(Placement {
        floating: true,
        on_top: true,
    });
    cx.simulate_resize(width, height);
    cx.run_until_parked();
    placement
}

/// THE HEADLINE: Join floats the room in the same press (`host.float`,
/// 320 by 200, never under 240 by 150, kept on top), and Leave puts it
/// back (`host.unfloat`). The window says who is in it.
#[test]
fn join_floats_and_leave_puts_it_back() {
    let (mut cx, ..) = opened();
    cx.simulate_click("huddle-join");
    cx.run_until_parked();
    assert_eq!(cx.host().requests::<HostFloat>(), [FLOAT]);
    assert_eq!(
        FLOAT,
        Float {
            width: 320,
            height: 200,
            min_width: 240,
            min_height: 150,
            on_top: true
        }
    );
    assert_eq!(
        cx.host().requests::<HostTitle>().last().map(String::as_str),
        Some("Huddle · just you")
    );
    assert!(cx.host().requests::<HostUnfloat>().is_empty());
    cx.simulate_click("huddle-leave");
    assert_eq!(cx.host().requests::<HostUnfloat>().len(), 1);
    assert!(cx.has_text("Join huddle"));
}

/// Below 480 by 320 the room is one tile: the picture covering it,
/// mirrored, and one row of controls over its foot, always shown, each
/// named; the room's bar and its solo line are gone. The pieces keep the
/// paths they had in the room, so the float's move opens no device again
/// (the microphone stays open; the camera does not blink).
#[test]
fn the_float_is_one_tile_with_its_controls_and_the_pieces_stay() {
    let (mut cx, ..) = opened();
    let placement = cx.host().stream::<HostPlacement>();
    cx.simulate_click("huddle-join");
    cx.run_until_parked();
    let room = (path_to(&cx, is_camera), path_to(&cx, is_microphone));
    assert!(room.0.is_some() && room.1.is_some());
    assert!(cx.find("huddle-bar").is_some());
    placement.send(Placement {
        floating: true,
        on_top: true,
    });
    for size in [(320., 200.), (240., 150.)] {
        cx.simulate_resize(size.0, size.1);
        cx.run_until_parked();
        assert!(cx.find("huddle-bar").is_none(), "{size:?}: no bar");
        assert!(cx.find("huddle-solo").is_none(), "{size:?}: no solo line");
        assert!(cx.find("huddle-float-strip").is_some());
        for (id, name) in [
            ("huddle-mic", "Mute"),
            ("huddle-camera", "Turn camera off"),
            ("huddle-leave", "Leave"),
            ("huddle-pin", "Keep on top"),
            ("huddle-back", "Back to the full window"),
        ] {
            assert_eq!(
                cx.interactivity(id).aria.label.as_deref(),
                Some(name),
                "{size:?}"
            );
        }
        assert_eq!(toggled(&cx, "huddle-pin"), Some(true));
        let (_, _, mirror, fit) = camera_piece(&cx).unwrap();
        assert_eq!((mirror, fit), (true, ImageObjectFit::Cover));
        assert_eq!(
            (path_to(&cx, is_camera), path_to(&cx, is_microphone)),
            room,
            "{size:?}: the pieces moved"
        );
    }
    // sized up to 480 by 320 and more: the room again, pin and back in its bar
    cx.simulate_resize(480., 320.);
    cx.run_until_parked();
    assert!(cx.find("huddle-bar").is_some());
    assert!(cx.find("huddle-float-strip").is_none());
    assert_eq!(toggled(&cx, "huddle-pin"), Some(true));
    assert!(cx.find("huddle-back").is_some());
    assert_eq!((path_to(&cx, is_camera), path_to(&cx, is_microphone)), room);
}

/// The pin lets the float among the other windows and back (a float while
/// floating: the host applies on-top alone), and follows what the host
/// says, the Window menu's Keep on Top included. Back returns the pane to
/// the desk with the call going on, where the bar's pin floats it again.
#[test]
fn the_pin_and_back() {
    let (mut cx, ..) = opened();
    let placement = floated(&mut cx, (320., 200.));
    cx.simulate_click("huddle-pin");
    cx.run_until_parked();
    assert_eq!(
        cx.host().requests::<HostFloat>().last(),
        Some(&Float {
            on_top: false,
            ..FLOAT
        })
    );
    assert_eq!(
        toggled(&cx, "huddle-pin"),
        Some(true),
        "until the host says"
    );
    placement.send(Placement {
        floating: true,
        on_top: false,
    });
    cx.run_until_parked();
    assert_eq!(toggled(&cx, "huddle-pin"), Some(false));
    cx.simulate_click("huddle-pin");
    cx.run_until_parked();
    assert_eq!(cx.host().requests::<HostFloat>().last(), Some(&FLOAT));
    // the Window menu's Keep on Top
    placement.send(Placement {
        floating: true,
        on_top: true,
    });
    cx.run_until_parked();
    assert_eq!(toggled(&cx, "huddle-pin"), Some(true));

    cx.simulate_click("huddle-back");
    assert_eq!(cx.host().requests::<HostUnfloat>().len(), 1);
    placement.send(Placement::default());
    cx.simulate_resize(768., 500.);
    cx.run_until_parked();
    assert!(cx.has_text("00:00 · only you"), "still in the room");
    assert!(cx.find("huddle-back").is_none());
    assert_eq!(
        cx.interactivity("huddle-pin").aria.label.as_deref(),
        Some("Float this window")
    );
    assert_eq!(toggled(&cx, "huddle-pin"), Some(false));
    let asked = cx.host().requests::<HostFloat>().len();
    cx.simulate_click("huddle-pin");
    cx.run_until_parked();
    let floats = cx.host().requests::<HostFloat>();
    assert_eq!((floats.len(), floats.last()), (asked + 1, Some(&FLOAT)));
}

/// Wayland keeps no window on top: the host floats the pane and says
/// `on_top: false`. The float has no pin then, says so once in a line
/// over the strip, closed by its own close or the next press; back stays.
#[test]
fn wayland_floats_with_no_pin_and_says_so_once() {
    let (mut cx, ..) = opened();
    cx.host().handle::<HostFloat>(|_| {
        Ok(Placement {
            floating: true,
            on_top: false,
        })
    });
    let placement = floated(&mut cx, (240., 150.));
    placement.send(Placement {
        floating: true,
        on_top: false,
    });
    cx.run_until_parked();
    assert!(cx.has_text("Wayland: no keep-on-top here."));
    assert!(cx.find("huddle-pin").is_none(), "no pin");
    assert!(cx.find("huddle-back").is_some());
    cx.simulate_click("huddle-no-keep-on-top-close");
    assert!(!cx.has_text("Wayland: no keep-on-top here."));
    // once per float: back, then float again from the desk's pin
    cx.simulate_click("huddle-back");
    placement.send(Placement::default());
    cx.simulate_resize(768., 500.);
    cx.run_until_parked();
    cx.simulate_click("huddle-pin");
    cx.run_until_parked();
    placement.send(Placement {
        floating: true,
        on_top: false,
    });
    cx.simulate_resize(320., 200.);
    cx.run_until_parked();
    assert!(cx.has_text("Wayland: no keep-on-top here."));
    cx.simulate_click("huddle-mic");
    assert!(
        !cx.has_text("Wayland: no keep-on-top here."),
        "the next press"
    );
}

/// A host that cannot float a pane (a phone) answers none of the float's
/// requests, `host.placement` refused `unknown_request` at subscribe: Join
/// keeps the view where it is, asking no float, the room has no pin or
/// back at any size, and Leave asks no unfloat.
#[test]
fn a_host_that_cannot_float_keeps_the_room_in_place() {
    let (mut cx, _ticks) = hosted();
    let unknown = view::methods::refusal::UNKNOWN_REQUEST;
    cx.host().refuse::<HostPlacement>(unknown, "no float here");
    cx.host().refuse::<HostFloat>(unknown, "no float here");
    cx.host().refuse::<HostUnfloat>(unknown, "no float here");
    cx.open::<Huddle>();
    cx.run_until_parked();
    cx.simulate_click("huddle-join");
    cx.run_until_parked();
    assert!(cx.has_text("00:00 · only you"), "in the room");
    assert!(
        cx.host().requests::<HostFloat>().is_empty(),
        "no float asked"
    );
    for size in [(768., 500.), (320., 200.)] {
        cx.simulate_resize(size.0, size.1);
        cx.run_until_parked();
        assert!(cx.find("huddle-pin").is_none(), "{size:?}: no pin");
        assert!(cx.find("huddle-back").is_none(), "{size:?}: no back");
    }
    cx.simulate_click("huddle-leave");
    assert!(cx.has_text("Join huddle"));
    assert!(
        cx.host().requests::<HostUnfloat>().is_empty(),
        "no unfloat asked"
    );
}

/// The float's states, alone: muted (the strip's microphone off, the
/// chip's mark), speaking (the ring and the chip's mark), the camera off
/// (your avatar, no picture), and a camera that went away (its card as one
/// row, Try again opening a new piece).
#[test]
fn the_float_says_muted_speaking_off_and_gone() {
    let (mut cx, ..) = opened();
    let _placement = floated(&mut cx, (320., 200.));
    let key = mic_key(&cx);
    cx.simulate_media_state(
        &key,
        MediaState {
            speaking: true,
            ..live("Built-in Microphone")
        },
    );
    assert_eq!(
        cx.interactivity("huddle-mark-speaking")
            .aria
            .label
            .as_deref(),
        Some("Speaking")
    );
    cx.simulate_click("huddle-mic");
    assert!(mic_piece(&cx).is_none(), "muted: the microphone let go");
    assert_eq!(toggled(&cx, "huddle-mic"), Some(true));
    assert_eq!(
        cx.interactivity("huddle-mic").aria.label.as_deref(),
        Some("Unmute")
    );
    assert!(cx.find("huddle-mark-muted").is_some());
    let (first, ..) = camera_piece(&cx).unwrap();
    cx.simulate_media_state(&first, status(MediaStatus::Gone));
    assert!(cx.has_text("The camera went away"));
    assert!(camera_piece(&cx).is_none(), "a vanished camera is let go");
    cx.simulate_click("huddle-camera-retry");
    cx.run_until_parked();
    assert_ne!(camera_piece(&cx).unwrap().0, first, "a new piece");
    cx.simulate_click("huddle-camera");
    assert!(camera_piece(&cx).is_none());
    assert_eq!(toggled(&cx, "huddle-camera"), Some(true));
    assert_eq!(
        cx.interactivity("huddle-camera").aria.label.as_deref(),
        Some("Turn camera on")
    );
}
