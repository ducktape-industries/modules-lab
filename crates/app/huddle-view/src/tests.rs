//! The view on a fake host: each test drives the frame the way a person
//! would and reads what the host would get. Every frame it sends is held
//! to the host's sanitizer and accessibility audit, and to the view's
//! capabilities (a piece the manifest leaves out fails the test).
use ducktape_view_guest::Entity;
use ducktape_view_guest::methods::{
    AudioDevices, CameraDevices, ClockTicks, Device, LinkOpen, MicrophoneChime, MicrophoneDevices,
};
use ducktape_view_guest::testing::{StreamSender, TestAppContext};
use ducktape_view_guest::wire::{Fit, MediaState, MediaStatus, MicCommand, Node, Playback};

use crate::{Huddle, RECORD_SECONDS};

fn device(id: &str, name: &str, default: bool) -> Device {
    Device {
        id: id.into(),
        name: name.into(),
        default,
    }
}

/// The view on a host listing two cameras, a microphone and two speakers,
/// in a pane of the desk's default size; its clock.
fn opened() -> (TestAppContext, Entity<Huddle>, StreamSender<ClockTicks>) {
    let mut cx = TestAppContext::new();
    cx.host().handle::<CameraDevices>(|()| {
        Ok(vec![
            device("0", "Front Camera", true),
            device("1", "Desk Camera", false),
        ])
    });
    cx.host().handle::<MicrophoneDevices>(|()| {
        Ok(AudioDevices {
            inputs: vec![device("alsa:default", "Built-in Microphone", true)],
            outputs: vec![
                device("alsa:speakers", "Speakers", true),
                device("alsa:headphones", "Headphones", false),
            ],
        })
    });
    let ticks = cx.host().stream::<ClockTicks>();
    cx.simulate_resize(768., 500.);
    let view = cx.open::<Huddle>();
    cx.run_until_parked();
    (cx, view, ticks)
}

/// The one camera piece in the tree: its key, device, mirror and fit.
fn camera_piece(cx: &TestAppContext) -> Option<(String, Option<String>, bool, Fit)> {
    let mut found = None;
    walk(cx.root(), &mut |node| {
        if let Node::Camera {
            id,
            device,
            mirror,
            fit,
            ..
        } = node
        {
            found = Some((
                id.name().unwrap_or_default().to_owned(),
                device.clone(),
                *mirror,
                *fit,
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
    assert_eq!((device, mirror, fit), (None, true, Fit::Contain));
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
            settings: true,
            ..Default::default()
        },
    );
    assert!(cx.has_text(
        "Ducktape is not allowed to use the camera. Open System Settings → Privacy & Security \
         → Camera, turn Ducktape on, then try again."
    ));
    cx.simulate_click("huddle-camera-settings");
    assert_eq!(
        cx.host().requests::<LinkOpen>(),
        [ducktape_view_guest::wire::media::SETTINGS_CAMERA.to_owned()]
    );
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
        (Some("1"), false, Fit::Cover)
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
        (Some("1"), false, Fit::Contain)
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
        ducktape_view_guest::methods::refusal::MEDIA_UNSUPPORTED,
        "this build has no camera or microphone",
    );
    cx.host().refuse::<MicrophoneDevices>(
        ducktape_view_guest::methods::refusal::MEDIA_UNSUPPORTED,
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
        device("alsa:plughw:CARD=Loopback,DEV=0", "Loopback PCM", false),
        device("alsa:plughw:CARD=Loopback,DEV=1", "Loopback PCM", false),
        device("alsa:default", "System default", true),
    ];
    assert_eq!(
        crate::twin(&devices, &devices[1]).as_deref(),
        Some("CARD=Loopback,DEV=1")
    );
    assert_eq!(crate::twin(&devices, &devices[2]), None);
}
