//! The view on a fake host: each test drives the frame the way a person
//! would and reads what the host would get. Every frame it sends is held
//! to the host's sanitizer and accessibility audit, and to the view's
//! capabilities (a piece the manifest leaves out fails the test).
use ducktape_view_guest::Entity;
use ducktape_view_guest::methods::{AudioDevices, CameraDevices, Device, MicrophoneDevices};
use ducktape_view_guest::testing::TestAppContext;
use ducktape_view_guest::wire::{MediaState, MediaStatus, MicCommand, Node, Playback};

use crate::{Huddle, RECORD_SECONDS};

fn device(id: &str, name: &str, default: bool) -> Device {
    Device {
        id: id.into(),
        name: name.into(),
        default,
    }
}

/// The view on a host listing two cameras, a microphone and two speakers.
fn opened() -> (TestAppContext, Entity<Huddle>) {
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
    let view = cx.open::<Huddle>();
    cx.run_until_parked();
    (cx, view)
}

/// The one camera piece in the tree, by its node.
fn camera_piece(cx: &TestAppContext) -> Option<(String, Option<String>, bool)> {
    let mut found = None;
    walk(cx.root(), &mut |node| {
        if let Node::Camera {
            id, device, mirror, ..
        } = node
        {
            found = Some((
                id.name().unwrap_or_default().to_owned(),
                device.clone(),
                *mirror,
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

/// Opened: a mirrored camera on the default device, the microphone on its
/// default, and every device listed by name.
#[test]
fn it_opens_on_the_defaults_mirrored() {
    let (cx, _) = opened();
    let (_, device, mirror) = camera_piece(&cx).expect("a camera piece");
    assert_eq!(device, None);
    assert!(mirror);
    let (_, input, output, monitor, commands) = mic_piece(&cx).expect("a microphone piece");
    assert_eq!((input, output, monitor), (None, None, false));
    assert!(commands.is_empty());
    for name in [
        "Front Camera",
        "Desk Camera",
        "Built-in Microphone",
        "Speakers",
        "Headphones",
    ] {
        assert!(cx.has_text(name), "{name} listed: {:?}", cx.texts());
    }
}

/// Mirror off flips the piece; the camera switch removes the piece (the
/// host releases the camera) and brings it back.
#[test]
fn mirror_and_camera_switch_steer_the_piece() {
    let (mut cx, _) = opened();
    cx.simulate_click("huddle-mirror");
    assert!(!camera_piece(&cx).unwrap().2);
    cx.simulate_click("huddle-camera-on");
    assert!(camera_piece(&cx).is_none());
    assert!(cx.has_text("Your camera is off."));
    cx.simulate_click("huddle-camera-on");
    assert!(camera_piece(&cx).is_some());
}

/// A picked camera, microphone or speaker reaches its piece by id.
#[test]
fn picks_reach_the_pieces() {
    let (mut cx, _) = opened();
    cx.simulate_click("huddle-cameras-1");
    assert_eq!(camera_piece(&cx).unwrap().1.as_deref(), Some("1"));
    cx.simulate_click("huddle-speakers-1");
    let (_, input, output, ..) = mic_piece(&cx).unwrap();
    assert_eq!(input, None);
    assert_eq!(output.as_deref(), Some("alsa:headphones"));
    cx.simulate_click("huddle-microphones-0");
    assert_eq!(mic_piece(&cx).unwrap().1.as_deref(), Some("alsa:default"));
}

/// Record asks the host once, for five seconds; the host's recording and
/// playback show, and Stop ends it.
#[test]
fn record_then_play_back() {
    let (mut cx, _) = opened();
    let key = mic_key(&cx);
    cx.simulate_media_state(&key, live("Built-in Microphone"));
    assert!(cx.has_text("Listening on Built-in Microphone."));
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
    assert!(cx.has_text("Recording… say something."));
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

/// The live loop is off until switched on, only while the microphone is
/// live, and its warning about headphones is always beside it.
#[test]
fn the_live_loop_waits_for_its_switch() {
    let (mut cx, _) = opened();
    assert!(cx.has_text(
        "Wear headphones: through speakers your voice loops back into the microphone and howls."
    ));
    assert_eq!(
        cx.interactivity("huddle-monitor").aria.disabled,
        Some(true),
        "no loop before the microphone is live"
    );
    let key = mic_key(&cx);
    cx.simulate_media_state(&key, live("Built-in Microphone"));
    cx.simulate_click("huddle-monitor");
    assert!(mic_piece(&cx).unwrap().3);
    // mute drops the piece, and the loop with it
    cx.simulate_click("huddle-mute");
    assert!(mic_piece(&cx).is_none());
    assert!(cx.has_text("Muted."));
    cx.simulate_click("huddle-mute");
    assert!(!mic_piece(&cx).unwrap().3);
}

/// A refused or vanished device says so, and Try again opens a new piece.
#[test]
fn trouble_is_said_and_retried() {
    let (mut cx, _) = opened();
    let (first, ..) = camera_piece(&cx).unwrap();
    cx.simulate_media_state(
        &first,
        MediaState {
            status: MediaStatus::Denied,
            ..Default::default()
        },
    );
    assert!(cx.has_text(
        "Camera access is off for this app. Turn it on in your system's privacy settings, then try again."
    ));
    cx.simulate_click("huddle-camera-retry");
    cx.run_until_parked();
    assert_ne!(camera_piece(&cx).unwrap().0, first, "a new piece");
    let key = mic_key(&cx);
    cx.simulate_media_state(
        &key,
        MediaState {
            status: MediaStatus::Gone,
            ..Default::default()
        },
    );
    assert!(cx.has_text("The microphone went away."));
    cx.simulate_media_state(
        &key,
        MediaState {
            status: MediaStatus::NoDevice,
            ..Default::default()
        },
    );
    assert!(cx.has_text("No microphone found."));
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
    cx.open::<Huddle>();
    cx.run_until_parked();
    assert!(cx.has_text("This build of the app has no camera or microphone support."));
    assert!(camera_piece(&cx).is_none() && mic_piece(&cx).is_none());
}
