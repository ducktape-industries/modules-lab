//! The desktop's tree: what Huddle draws under a pointer. The device check
//! (the picture with its toggles, the settings column), the room (the
//! stage and its bar, or below 480 by 320 one tile with its controls over
//! it), the device menu, the state cards a refused or missing device shows,
//! and the screen of a build without camera and microphone. Pieces sit
//! first in their parent, so the cards and chips after them draw over the
//! picture. The host keys a piece's device by its path: the root always
//! goes through the one menu overlay, and both room layouts hold the pieces
//! under the same room, stage and tile, so a menu or a float moving between
//! layouts opens no device again.
use view::prelude::*;
use view::{
    Anchor, AnchoredPositionMode, AnyElement, Bounds, Div, Edges, FontWeight, Point, Stateful,
    accesskit,
};

use crate::words::{self, Device};
use crate::{Huddle, Kind, Menu, RECORD_SECONDS, Row, Screen, Which, trouble};

/// The check's settings column, and the menus' width.
const COLUMN: f32 = 272.;

/// Below this the room is one tile with its controls over it.
const COMPACT: (f32, f32) = (480., 320.);

/// The accesskit popup kind a picker and a caret announce.
const MENU_POPUP: accesskit::HasPopup = accesskit::HasPopup::Menu;

/// What takes the keys: the room on Join, Join on Leave, the open menu.
const ROOM: &str = "huddle-room";
const JOIN: &str = "huddle-join";
const MENU: &str = "huddle-menu";

/// What the desktop's tree keeps between frames.
#[derive(Default)]
pub(crate) struct Desk {
    /// Where layout put each menu's opener, in window pixels: a menu hangs
    /// under its picker, or over the room's caret.
    anchors: Vec<(Which, Bounds<Pixels>)>,
}

impl Desk {
    /// The menu's opener's bounds, as layout last put it.
    fn anchor(&self, which: Which) -> Option<Bounds<Pixels>> {
        self.anchors
            .iter()
            .find(|(at, _)| *at == which)
            .map(|(_, bounds)| *bounds)
    }

    fn set_anchor(&mut self, which: Which, bounds: Bounds<Pixels>) {
        self.anchors.retain(|(at, _)| *at != which);
        self.anchors.push((which, bounds));
    }
}

/// The room is one tile with its controls over it: below 480 by 320.
fn compact(view: &Huddle) -> bool {
    let (width, height) = view.viewport;
    width < COMPACT.0 || height < COMPACT.1
}

/// Into the room, the keys in it.
fn join(view: &mut Huddle, window: &mut Window, cx: &mut Context<Huddle>) {
    view.join(cx);
    window.focus(ROOM);
}

/// Out of the room, the keys on Join.
fn leave(view: &mut Huddle, window: &mut Window, cx: &mut Context<Huddle>) {
    view.leave(cx);
    window.focus(JOIN);
}

/// The menu of `which` opened by the control `opener`, the keys in it.
fn open(
    view: &mut Huddle,
    which: Which,
    opener: &'static str,
    window: &mut Window,
    cx: &mut Context<Huddle>,
) {
    view.open_menu(which, opener, cx);
    window.focus(MENU);
}

/// A menu closed (a pick, Escape, a press outside): the keys back on the
/// control that opened it.
fn closed(menu: Option<Menu>, window: &mut Window) {
    if let Some(menu) = menu {
        window.focus(menu.opener);
    }
}

// the glyphs, by the Lucide names `design::icon` draws
const MIC: &str = "mic";
const MIC_OFF: &str = "mic-off";
const VIDEO: &str = "video";
const VIDEO_OFF: &str = "video-off";
const SPEAKER: &str = "volume-2";
const HEADPHONES: &str = "headphones";
const LEAVE: &str = "phone-off";
const CARET: &str = "chevron-down";
const MIRROR: &str = "flip-horizontal-2";
const ALERT: &str = "circle-alert";
const CAMERA_OFF: &str = "camera-off";
const PLAY: &str = "play";
const STOP: &str = "square";
const CHECK: &str = "check";
const PEOPLE: &str = "users";
const SPEAKING: &str = "audio-lines";
const PERSON: &str = "user-round";
const PIN: &str = "pin";
const PIN_OFF: &str = "pin-off";
const BACK: &str = "picture-in-picture-2";
const CLOSE: &str = "x";

fn f(pixels: Pixels) -> f32 {
    pixels.into()
}

/// Over the camera's picture the picture is the ground: controls and chips
/// are this dark glass with a white glyph, not the theme's.
fn glass() -> Hsla {
    hsla(0., 0., 0., 0.55)
}

fn white() -> Hsla {
    hsla(0., 0., 1., 1.)
}

fn black() -> Hsla {
    hsla(0., 0., 0., 1.)
}

/// The muted mark on the glass: the dark theme's danger, legible on black
/// in either theme.
fn muted_mark() -> Hsla {
    Theme::dark().danger
}

pub(crate) fn render(view: &mut Huddle, cx: &mut Context<Huddle>) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let screen = match (view.no_support(), view.screen) {
        (true, _) => unsupported(&theme).into_any_element(),
        (false, Screen::Check) => check(view, cx, &theme).into_any_element(),
        (false, Screen::Room) => room(view, cx, &theme).into_any_element(),
    };
    let base = div()
        .id("huddle")
        .size_full()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_size(design::text::BODY)
        // the boards' leading: a label sits 15 px tall over its control
        .line_height(gpui::relative(1.3))
        .child(screen);
    let open = view.menu.filter(|_| !view.no_support());
    let (label, floating) = match open {
        Some(open) => (words::menu(open.which), Some(menu(view, open, cx, &theme))),
        None => ("", None),
    };
    let mut root = modal_overlay("huddle-menu-overlay", label, base, floating);
    if open.is_some() {
        root = root.on_dismiss(
            cx.listener(|view, _: &(), window, cx| closed(view.close_menu(cx), window)),
        );
    }
    root.into_any_element()
}

// ---------- the device check ----------

fn check(view: &mut Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let (block, column) = (f(design::space::BLOCK), COLUMN);
    // the picture's own shape, 16:9, as wide as the column leaves; a short
    // wide window gives its height first
    let (width, height) = view.viewport;
    let mut w = (width - 3. * block - column).max(0.);
    let mut h = w * 9. / 16.;
    let room = (height - 2. * block).max(0.);
    if h > room {
        h = room;
        w = h * 16. / 9.;
    }
    div()
        .id("huddle-check")
        .size_full()
        .flex()
        .items_start()
        .p(design::space::BLOCK)
        .gap(design::space::BLOCK)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(preview(view, w, h, cx, theme)),
        )
        .child(settings(view, cx, theme))
}

/// Your picture, the mirror, mic and camera toggles over it, the chip that
/// says it is you, and a card when the camera is off or in trouble.
fn preview(
    view: &Huddle,
    w: f32,
    h: f32,
    cx: &mut Context<Huddle>,
    theme: &Theme,
) -> Stateful<Div> {
    let off = view.camera_off;
    let troubled = view.troubled(Device::Camera);
    let mut preview = div()
        .id("huddle-preview")
        .relative()
        .w(px(w))
        .h(px(h))
        .flex_none()
        .overflow_hidden()
        .bg(match off || troubled {
            true => theme.surface_raised,
            false => black(),
        });
    if view.camera_placed() {
        preview = preview.child(camera_piece(view, ObjectFit::Contain, cx).size_full());
    }
    if off {
        preview = preview.child(camera_off(72., Some("Your camera is off"), theme));
    }
    let mirrored = !view.unmirrored;
    preview
        // nothing to mirror with the camera off or in trouble
        .child(mirror_toggle(mirrored, !off && !troubled, cx))
        .child(
            div()
                .absolute()
                .bottom(design::space::SM)
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .gap(design::space::SM)
                .child(mic_toggle(view.muted, theme.danger, cx))
                .child(camera_toggle(off, theme.danger, cx)),
        )
        .children(troubled.then(|| state_card(view, Device::Camera, cx, theme)))
        .child(chip(view, off, theme))
}

fn camera_piece(view: &Huddle, fit: ObjectFit, cx: &mut Context<Huddle>) -> view::Camera {
    let piece = camera(view.camera_key())
        .mirror(!view.unmirrored)
        .object_fit(fit)
        .on_state(cx.listener(|view: &mut Huddle, state: &MediaState, _, cx| {
            view.camera_state = state.clone();
            cx.notify();
        }));
    match &view.camera {
        Some(id) => piece.device(id.clone()),
        None => piece,
    }
}

fn mic_piece(view: &Huddle, cx: &mut Context<Huddle>) -> view::Microphone {
    let piece = microphone(view.mic_key())
        .monitor(view.monitor && view.screen == Screen::Check)
        .track(&view.recorder)
        .on_state(cx.listener(|view: &mut Huddle, state: &MediaState, _, cx| {
            view.heard_mic(state);
            cx.notify();
        }));
    let piece = match &view.microphone {
        Some(id) => piece.device(id.clone()),
        None => piece,
    };
    match &view.speaker {
        Some(id) => piece.output(id.clone()),
        None => piece,
    }
}

/// A 32 px glass toggle over the picture: danger-filled while off.
fn glass_toggle(
    id: &'static str,
    icon: &'static str,
    name: &'static str,
    off: bool,
    danger: Hsla,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(white())
        .bg(glass())
        .when(off, |button| button.bg(danger))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(name)
        .aria_toggled(off.into())
        .focusable()
        .on_click(click)
        .child(design::icon(icon, px(16.)))
}

fn mic_toggle(muted: bool, danger: Hsla, cx: &mut Context<Huddle>) -> Stateful<Div> {
    glass_toggle(
        "huddle-mic",
        if muted { MIC_OFF } else { MIC },
        words::mute_microphone(muted),
        muted,
        danger,
        cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_mute(cx)),
    )
}

fn camera_toggle(off: bool, danger: Hsla, cx: &mut Context<Huddle>) -> Stateful<Div> {
    glass_toggle(
        "huddle-camera",
        if off { VIDEO_OFF } else { VIDEO },
        words::camera_toggle(off),
        off,
        danger,
        cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_camera(cx)),
    )
}

/// The mirror toggle, top right: white while mirroring, glass when not,
/// faded with the camera off.
fn mirror_toggle(mirrored: bool, enabled: bool, cx: &mut Context<Huddle>) -> Stateful<Div> {
    let toggle = div()
        .id("huddle-mirror")
        .absolute()
        .top(design::space::SM)
        .right(design::space::SM)
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .map(|toggle| match mirrored && enabled {
            true => toggle.bg(white()).text_color(rgb(0x111111)),
            false => toggle.bg(glass()).text_color(white()),
        })
        .role(Role::Button)
        .aria_label("Mirror my picture")
        .aria_toggled(mirrored.into())
        .child(design::icon(MIRROR, px(14.)));
    match enabled {
        true => toggle.cursor_pointer().focusable().on_click(cx.listener(
            |view: &mut Huddle, _: &ClickEvent, _, cx| {
                view.unmirrored = !view.unmirrored;
                cx.notify();
            },
        )),
        false => toggle.opacity(0.5).aria_disabled(true),
    }
}

/// "You" in the picture's corner, "· mirrored" while it is; the camera's
/// opening said here once it takes a while.
fn chip(view: &Huddle, off: bool, theme: &Theme) -> Div {
    let opening = !off
        && view.camera_state.status == MediaStatus::Starting
        && view.ticks.saturating_sub(view.camera_since) >= 2;
    let mirrored = !view.unmirrored && !off && !trouble(&view.camera_state);
    div()
        .absolute()
        .left(design::space::SM)
        .bottom(design::space::SM)
        .h(px(22.))
        .px(design::space::XS)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .text_size(design::text::CAPTION)
        .map(|chip| match off {
            // on the raised ground of a camera that is off: the window's
            true => chip
                .bg(theme.background)
                .text_color(theme.foreground)
                .border_1()
                .border_color(theme.border),
            false => chip.bg(glass()).text_color(white()),
        })
        .child(match opening {
            true => "Opening the camera\u{2026}",
            false => "You",
        })
        .when(mirrored && !opening, |chip| {
            chip.child(
                div()
                    .font_family(design::fonts::FAMILY_MONO)
                    .opacity(0.7)
                    .child("\u{b7} mirrored"),
            )
        })
}

/// The camera that is off: your avatar in the middle of the raised ground
/// (no account name reaches the view: the person's glyph), and a line.
fn camera_off(size: f32, line: Option<&'static str>, theme: &Theme) -> Div {
    let avatar = div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .text_color(theme.muted)
        .child(design::icon(PERSON, px((size * 0.4).round())));
    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .child(avatar)
        .children(line.map(|line| design::quiet(line, theme)))
}

/// The settings column: the three devices, the meter, the tests, Join.
fn settings(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let theme = *theme;
    div()
        .id("huddle-settings")
        .w(px(COLUMN))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(field(
            words::kind(Kind::Camera),
            picker(view, Kind::Camera, VIDEO, cx, &theme),
            &theme,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(design::space::XS)
                .child(field(
                    words::kind(Kind::Microphone),
                    picker(view, Kind::Microphone, MIC, cx, &theme),
                    &theme,
                ))
                .child(meter(view, cx, &theme)),
        )
        .child(
            field(
                words::kind(Kind::Speaker),
                div()
                    .flex()
                    .items_center()
                    .gap(design::space::XS)
                    .child(picker(view, Kind::Speaker, SPEAKER, cx, &theme).flex_1())
                    .child(speaker_test(view, cx, &theme)),
                &theme,
            )
            .children(view.speaker_fault.as_ref().map(|why| {
                div()
                    .id("huddle-speaker-fault")
                    .text_size(design::text::CAPTION)
                    .text_color(theme.muted)
                    .child(format!("Test failed: {why}."))
            })),
        )
        .child(div().h(px(1.)).flex_none().bg(theme.border))
        .child(record(view, cx, &theme))
        .child(monitor(view, cx, &theme))
        .child(div().flex_1())
        .child(join_button(cx, &theme))
}

fn field(label: &'static str, control: impl IntoElement, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(design::space::XXS)
        .child(
            div()
                .text_size(design::text::CAPTION)
                .text_color(theme.muted)
                .child(label),
        )
        .child(control)
}

/// One device's picker: its glyph, the device, a caret; it opens the
/// device's menu under itself (press, Enter, Space or ↓).
fn picker(
    view: &Huddle,
    kind: Kind,
    icon: &'static str,
    cx: &mut Context<Huddle>,
    theme: &Theme,
) -> view::Sensor {
    let theme = *theme;
    let which = Which::Kind(kind);
    let id = match kind {
        Kind::Camera => "huddle-pick-camera",
        Kind::Microphone => "huddle-pick-microphone",
        Kind::Speaker => "huddle-pick-speaker",
    };
    let expanded = view.menu.is_some_and(|menu| menu.which == which);
    let name = view.shown_device(kind);
    let button = div()
        .id(id)
        .h(design::size::CONTROL)
        .w_full()
        .px(design::space::SM)
        .flex()
        .items_center()
        .gap(design::space::SM)
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.background)
        .text_size(design::text::SECONDARY)
        .cursor_pointer()
        .hover(move |style| style.bg(theme.surface))
        .role(Role::Button)
        .aria_has_popup(MENU_POPUP)
        .aria_expanded(expanded)
        .aria_label(format!("{}: {name}", words::kind(kind)))
        .focusable()
        .on_click(
            cx.listener(move |view: &mut Huddle, _: &ClickEvent, window, cx| {
                open(view, which, id, window, cx)
            }),
        )
        .on_key_down(
            cx.listener(move |view: &mut Huddle, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "down" && !event.keystroke.modifiers.modified() {
                    open(view, which, id, window, cx);
                }
            }),
        )
        .child(design::icon(icon, px(14.)))
        .child(div().flex_1().min_w(px(0.)).truncate().child(name))
        .child(design::icon(CARET, px(12.)).text_color(theme.muted));
    anchor(which, id, button, cx).w_full()
}

/// A menu's opener `id` in a sensor: where layout puts it is where the
/// menu of `which` hangs.
fn anchor(
    which: Which,
    id: &str,
    control: impl IntoElement,
    cx: &mut Context<Huddle>,
) -> view::Sensor {
    sensor(format!("{id}-at"), control).on_bounds(cx.listener(
        move |view: &mut Huddle, bounds: &Bounds<Pixels>, _, cx| {
            view.desk.set_anchor(which, *bounds);
            if view.menu.is_some_and(|menu| menu.which == which) {
                cx.notify();
            }
        },
    ))
}

/// The microphone's level, drawn by the host in the bar's colour; with
/// the microphone muted or in trouble, its line instead.
fn meter(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> AnyElement {
    if view.troubled(Device::Microphone) {
        return mic_trouble(view, cx, theme).into_any_element();
    }
    div()
        .id("huddle-meter")
        .h(px(6.))
        .w_full()
        .bg(theme.surface_raised)
        .role(Role::Meter)
        .aria_label("Microphone level")
        .when(view.mic_placed(), |meter| {
            meter.child(
                mic_piece(view, cx)
                    .size_full()
                    .bg(theme.surface_raised)
                    .text_color(theme.foreground),
            )
        })
        .into_any_element()
}

/// A microphone that is refused, missing, busy or gone: one line and
/// what fixes it, where the meter was.
fn mic_trouble(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let state = &view.mic_state;
    let (title, text) = words::trouble(Device::Microphone, state);
    let line = match state.status {
        MediaStatus::Denied | MediaStatus::Failed => format!("{title}. {text}"),
        _ => title.to_owned(),
    };
    div()
        .id("huddle-mic-trouble")
        .flex()
        .flex_col()
        .gap(design::space::XS)
        .child(
            div()
                .flex()
                .items_start()
                .gap(design::space::XS)
                .text_size(design::text::CAPTION)
                .text_color(theme.muted)
                .child(design::icon(ALERT, px(12.)).mt(px(1.)))
                .child(div().flex_1().min_w(px(0.)).child(line)),
        )
        .child(actions(view, Device::Microphone, cx, theme))
}

/// Test: the host's chime on the picked speaker; muted or with the
/// microphone in trouble too (an output, apart from the microphone), not
/// over a recording's playback.
fn speaker_test(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let enabled = view.mic_state.playback == Playback::Idle && !view.chiming;
    outline(
        "huddle-speaker-test",
        None,
        if view.chiming {
            "Playing\u{2026}"
        } else {
            "Test"
        },
        enabled,
        theme,
        cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.chime(cx)),
    )
    .flex_none()
}

/// Record five seconds and hear them back, and the line under it.
fn record(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Div {
    let playback = view.mic_state.playback.clone();
    let under_way = playback != Playback::Idle;
    let (icon, label) = match (&playback, under_way) {
        (Playback::Recording, true) => (STOP, "Stop".to_owned()),
        (Playback::Playing, true) => (STOP, "Stop playback".to_owned()),
        _ => (PLAY, format!("Record {RECORD_SECONDS} s and play back")),
    };
    let enabled = (view.mic_live() && !view.chiming) || under_way;
    let button = outline(
        "huddle-record",
        Some(icon),
        label,
        enabled,
        theme,
        cx.listener(move |view: &mut Huddle, _: &ClickEvent, _, cx| {
            match under_way {
                true => view.recorder.stop(),
                false => view.recorder.record(RECORD_SECONDS),
            }
            cx.notify();
        }),
    )
    .w_full()
    .justify_start();
    let line: AnyElement = match (&playback, under_way) {
        _ if view.muted => caption("Unmute to test your microphone.", theme),
        (Playback::Recording, true) => {
            let since = view.recording_since.unwrap_or(view.ticks);
            let left = u64::from(RECORD_SECONDS)
                .saturating_sub(view.ticks.saturating_sub(since))
                .max(1);
            div()
                .id("huddle-record-line")
                .flex()
                .text_size(design::text::CAPTION)
                .text_color(theme.muted)
                .child("Recording\u{2026} say something \u{b7}\u{a0}")
                .child(
                    div()
                        .font_family(design::fonts::FAMILY_MONO)
                        .child(format!("0:0{left}")),
                )
                .into_any_element()
        }
        (Playback::Playing, true) => caption("Playing it back\u{2026}", theme),
        _ => caption("Hear how you sound to others.", theme),
    };
    div()
        .flex()
        .flex_col()
        .gap(design::space::XS)
        .child(button)
        .child(line)
}

fn caption(text: &'static str, theme: &Theme) -> AnyElement {
    div()
        .id("huddle-record-line")
        .text_size(design::text::CAPTION)
        .text_color(theme.muted)
        .child(text)
        .into_any_element()
}

/// Hear myself live: its switch, and the headphones warning always beside
/// it.
fn monitor(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(design::space::XS)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(design::text::SECONDARY)
                        .font_weight(FontWeight::MEDIUM)
                        .child("Hear myself live"),
                )
                .child(design::switch(
                    "huddle-monitor",
                    "Hear myself live",
                    view.monitor,
                    view.mic_live(),
                    // the desktop's sizes: a phone gets a tree of its own
                    &design::POINTER,
                    theme,
                    cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                        view.monitor = !view.monitor;
                        cx.notify();
                    }),
                )),
        )
        .child(
            div()
                .flex()
                .items_start()
                .gap(design::space::XS)
                .text_size(design::text::CAPTION)
                .text_color(theme.warning)
                .child(design::icon(HEADPHONES, px(13.)).mt(px(1.)))
                .child(div().flex_1().min_w(px(0.)).child(
                    "Wear headphones: through speakers your voice loops back into the \
                     microphone and howls.",
                )),
        )
}

fn join_button(cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    design::focus_shown_on_ink(
        div()
            .id(JOIN)
            .h(px(32.))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .text_size(design::text::BODY)
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .role(Role::Button)
            .focusable()
            .on_click(
                cx.listener(|view: &mut Huddle, _: &ClickEvent, window, cx| join(view, window, cx)),
            )
            .child("Join huddle"),
        theme,
    )
}

/// A 28 px control in a hairline box, its glyph before its label; one
/// that cannot act is faint and says so.
fn outline(
    id: &'static str,
    icon: Option<&'static str>,
    label: impl Into<SharedString>,
    enabled: bool,
    theme: &Theme,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let theme = *theme;
    let button = div()
        .id(id)
        .h(design::size::CONTROL)
        .px(design::space::MD)
        .flex()
        .items_center()
        .justify_center()
        .gap(design::space::XS)
        .border_1()
        .bg(theme.background)
        .text_size(design::text::SECONDARY)
        .whitespace_nowrap()
        .role(Role::Button)
        .children(icon.map(|icon| design::icon(icon, px(14.))))
        .child(label.into());
    match enabled {
        true => button
            .border_color(theme.border_strong)
            .cursor_pointer()
            .hover(move |style| style.bg(theme.surface))
            .focusable()
            .on_click(click),
        false => button
            .border_color(theme.border)
            .text_color(theme.faint)
            .aria_disabled(true),
    }
}

// ---------- states ----------

/// The glyph a device's trouble shows.
fn trouble_glyph(status: &MediaStatus) -> &'static str {
    match status {
        MediaStatus::NoDevice | MediaStatus::Gone => CAMERA_OFF,
        MediaStatus::Busy => VIDEO_OFF,
        _ => ALERT,
    }
}

/// The card in the middle of the picture: glyph, title, text, what fixes
/// it. No ground of its own: the picture's box is raised behind it.
fn state_card(
    view: &Huddle,
    device: Device,
    cx: &mut Context<Huddle>,
    theme: &Theme,
) -> Stateful<Div> {
    let state = state(view, device);
    let (title, text) = words::trouble(device, state);
    div()
        .id("huddle-camera-trouble")
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .p(design::space::BLOCK)
        .text_center()
        .child(design::icon(trouble_glyph(&state.status), px(20.)).text_color(theme.muted))
        .child(
            div()
                .text_size(design::text::BODY)
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(title),
        )
        .when(!text.is_empty(), |card| {
            card.child(
                div()
                    .max_w(px(240.))
                    .text_size(design::text::CAPTION)
                    .text_color(theme.muted)
                    .child(text),
            )
        })
        .child(actions(view, device, cx, theme))
}

/// What a device's piece last told.
fn state(view: &Huddle, device: Device) -> &MediaState {
    match device {
        Device::Camera => &view.camera_state,
        Device::Microphone => &view.mic_state,
    }
}

/// What fixes a device's trouble: Open Settings where the host can open
/// them, Try again, and on the check, for a missing device, joining
/// without it.
fn actions(view: &Huddle, device: Device, cx: &mut Context<Huddle>, theme: &Theme) -> Div {
    let state = state(view, device);
    let camera = device == Device::Camera;
    let prefix = if camera {
        "huddle-camera"
    } else {
        "huddle-microphone"
    };
    let settings = state.status == MediaStatus::Denied && state.opens_settings;
    let missing = state.status == MediaStatus::NoDevice && view.screen == Screen::Check;
    let page = match camera {
        true => Place::CameraSettings,
        false => Place::MicrophoneSettings,
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .when(settings, |row| {
            row.child(outline(
                if camera {
                    "huddle-camera-settings"
                } else {
                    "huddle-microphone-settings"
                },
                None,
                "Open Settings",
                true,
                theme,
                cx.listener(move |_: &mut Huddle, _: &ClickEvent, _, cx| {
                    cx.host().notify::<HostShow>(page)
                }),
            ))
        })
        .child(outline(
            if camera {
                "huddle-camera-retry"
            } else {
                "huddle-microphone-retry"
            },
            None,
            "Try again",
            true,
            theme,
            cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.retry(cx)),
        ))
        .when(missing, |row| {
            row.child(
                div()
                    .id(SharedString::from(format!("{prefix}-without")))
                    .h(design::size::CONTROL)
                    .px(design::space::MD)
                    .flex()
                    .items_center()
                    .text_size(design::text::SECONDARY)
                    .text_color(theme.muted)
                    .cursor_pointer()
                    .hover({
                        let theme = *theme;
                        move |style| style.text_color(theme.foreground)
                    })
                    .role(Role::Button)
                    .focusable()
                    .on_click(
                        cx.listener(move |view: &mut Huddle, _: &ClickEvent, window, cx| {
                            match camera {
                                true => view.camera_off = true,
                                false => view.muted = true,
                            }
                            join(view, window, cx)
                        }),
                    )
                    .child(match camera {
                        true => "Join without camera",
                        false => "Join without a microphone",
                    }),
            )
        })
}

// ---------- the room ----------

fn room(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let compact = compact(view);
    div()
        .id(ROOM)
        .size_full()
        .flex()
        .flex_col()
        // M mutes, V turns the camera off: from anywhere in the room
        .role(Role::Group)
        .aria_label("Huddle")
        .focusable()
        .focus_visible(|style| style)
        .consumes_keys(["m", "v"])
        .on_key_down(
            cx.listener(|view: &mut Huddle, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "m" => view.toggle_mute(cx),
                    "v" => view.toggle_camera(cx),
                    _ => {}
                }
            }),
        )
        .child(stage(view, compact, cx, theme))
        .when(!compact, |room| room.child(bar(view, cx, theme)))
}

/// The stage: one tile per person in a grid. Today only you: your tile
/// fills it, edge to edge in the compact room.
fn stage(view: &Huddle, compact: bool, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let (width, _) = view.viewport;
    let tile_width = match compact {
        true => width,
        false => width - 2. * f(design::space::LG),
    };
    div()
        .id("huddle-stage")
        .flex_1()
        .min_h(px(0.))
        .flex()
        .map(|stage| match compact {
            true => stage.bg(black()),
            false => stage.p(design::space::LG).bg(theme.surface),
        })
        .child(tile(view, tile_width, compact, cx, theme))
}

/// Your tile: the picture covering it (mirrored as you set), a ring while
/// you speak, and the chip with your marks; in the room the line that says
/// you are alone, in the compact room the strip of controls instead.
fn tile(
    view: &Huddle,
    width: f32,
    compact: bool,
    cx: &mut Context<Huddle>,
    theme: &Theme,
) -> Stateful<Div> {
    let off = view.camera_off;
    let troubled = view.troubled(Device::Camera);
    let speaking = view.mic_placed() && view.mic_state.speaking;
    let mut tile = div()
        .id("huddle-tile-you")
        .relative()
        .size_full()
        .overflow_hidden()
        .role(Role::Group)
        .aria_label("You")
        .bg(match off || troubled {
            true => theme.surface_raised,
            false => black(),
        });
    if view.camera_placed() {
        tile = tile.child(camera_piece(view, ObjectFit::Cover, cx).size_full());
    }
    // the microphone has nothing to draw in the room: it is there to be
    // heard, and to say who is speaking
    if view.mic_placed() {
        tile = tile.child(mic_piece(view, cx).absolute().size(px(0.)));
    }
    if off {
        let size = match width {
            _ if compact => 48.,
            w if w >= 300. => 72.,
            w if w >= 180. => 48.,
            _ => 32.,
        };
        tile = tile.child(camera_off(size, None, theme));
    }
    let tile = tile.when(speaking, |tile| {
        tile.child(
            div()
                .absolute()
                .inset_0()
                .border_2()
                .border_color(theme.success),
        )
    });
    if compact {
        // the strip first: Try again and the line's close come after its
        // controls in the keys' order
        return tile
            .child(float_strip(view, speaking, cx, theme))
            .children(trouble_rows(view, cx, theme))
            .when(view.no_keep_on_top_line, |tile| {
                tile.child(no_keep_on_top_line(cx))
            });
    }
    // the chip and the line share the tile's foot: the line centred
    // between two equal sides, the chip's never narrower than the chip,
    // so in a float sized up to the room the line moves off it, wrapping
    // if it must
    tile.children(troubled.then(|| state_card(view, Device::Camera, cx, theme)))
        .child(
            div()
                .absolute()
                .left(design::space::SM)
                .right(design::space::SM)
                .bottom(design::space::SM)
                .flex()
                .items_end()
                .gap(design::space::SM)
                .child(div().flex_1().flex().child(room_chip(view, speaking)))
                .child(
                    div()
                        .id("huddle-solo")
                        .min_w(px(0.))
                        .px(design::space::SM)
                        .py(design::space::XXS)
                        .bg(glass())
                        .text_color(white())
                        .text_size(design::text::CAPTION)
                        .child("Only you are here. Calls with others are not connected yet."),
                )
                .child(div().flex_1()),
        )
}

/// The microphone's control draws it off, slashed on the danger fill:
/// muted, or in trouble.
fn mic_off(view: &Huddle) -> bool {
    view.muted || view.troubled(Device::Microphone)
}

/// The muted or the speaking mark a chip carries, named for a reader.
fn mark(view: &Huddle, speaking: bool) -> Option<Stateful<Div>> {
    let (icon, name, color) = match (view.muted, speaking) {
        (true, _) => (MIC_OFF, "Muted", muted_mark()),
        (false, true) => (SPEAKING, "Speaking", white()),
        (false, false) => return None,
    };
    Some(
        div()
            .id(SharedString::from(format!(
                "huddle-mark-{}",
                name.to_lowercase()
            )))
            .flex()
            .role(Role::Image)
            .aria_label(name)
            .child(design::icon(icon, px(12.)).text_color(color)),
    )
}

/// The tile's chip: the muted or speaking mark, the name, "· mirrored".
fn room_chip(view: &Huddle, speaking: bool) -> Div {
    div()
        .flex_none()
        .h(px(22.))
        .px(design::space::XS)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .bg(glass())
        .text_color(white())
        .text_size(design::text::CAPTION)
        .children(mark(view, speaking))
        .child("You")
        .when(view.camera_placed() && !view.unmirrored, |chip| {
            chip.child(
                div()
                    .font_family(design::fonts::FAMILY_MONO)
                    .opacity(0.7)
                    .child("\u{b7} mirrored"),
            )
        })
}

/// The room's bar: its name and clock, the microphone and camera with
/// their menus, Leave, and who is here.
fn bar(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let elapsed = view.ticks.saturating_sub(view.entered);
    let clock = match elapsed / 3600 {
        0 => format!("{:02}:{:02}", elapsed / 60, elapsed % 60),
        hours => format!("{hours}:{:02}:{:02}", elapsed / 60 % 60, elapsed % 60),
    };
    div()
        .id("huddle-bar")
        .h(px(48.))
        .flex_none()
        .px(design::space::LG)
        .flex()
        .items_center()
        .gap(design::space::LG)
        .border_t_1()
        .border_color(theme.border)
        .bg(theme.background)
        // the name and clock give way first: in a float sized down to
        // the room's 480 every control still fits
        .child(
            div()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(design::text::BODY)
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child("Huddle"),
                )
                .child(
                    div()
                        .id("huddle-clock")
                        .font_family(design::fonts::FAMILY_MONO)
                        .text_size(design::text::CAPTION)
                        .text_color(theme.muted)
                        .truncate()
                        .child(format!("{clock} \u{b7} only you")),
                ),
        )
        .child(split(view, Device::Microphone, cx, theme).ml_auto())
        .children(
            view.troubled(Device::Microphone)
                .then(|| retry(Device::Microphone, cx, theme)),
        )
        .child(split(view, Device::Camera, cx, theme))
        .child(
            div()
                .id("huddle-leave")
                .flex_none()
                .h(px(32.))
                .px(design::space::LG)
                .flex()
                .items_center()
                .gap(design::space::XS)
                .bg(theme.danger)
                .text_color(white())
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .role(Role::Button)
                .focusable()
                .on_click(
                    cx.listener(|view: &mut Huddle, _: &ClickEvent, window, cx| {
                        leave(view, window, cx)
                    }),
                )
                .child(design::icon(LEAVE, px(14.)))
                .child("Leave"),
        )
        .child(
            div()
                .ml_auto()
                .flex_none()
                .flex()
                .items_center()
                .gap(design::space::SM)
                .child(
                    div()
                        .id("huddle-count")
                        .flex()
                        .items_center()
                        .gap(design::space::XXS)
                        .text_color(theme.muted)
                        .role(Role::Group)
                        .aria_label("1 in the huddle")
                        .child(design::icon(PEOPLE, px(14.)))
                        .child(
                            div()
                                .font_family(design::fonts::FAMILY_MONO)
                                .text_size(design::text::CAPTION)
                                .child("1"),
                        ),
                )
                .children(pin_back(view, false, cx, theme)),
        )
}

/// The pin and back after the bar's count, or at the float strip's end:
/// floating, the pin keeps the window on top or not (none where the host
/// keeps nothing on top) and back returns it to the full window; on the
/// desk the pin floats it again. Neither where the host cannot float.
fn pin_back(
    view: &Huddle,
    glassy: bool,
    cx: &mut Context<Huddle>,
    theme: &Theme,
) -> Vec<Stateful<Div>> {
    let floating = view.placement.floating;
    let on_top = floating && view.placement.on_top;
    let pin = (!view.no_float && (!floating || !view.no_keep_on_top)).then(|| {
        small(
            ("huddle-pin", if on_top { PIN } else { PIN_OFF }),
            if floating {
                "Keep on top"
            } else {
                "Float this window"
            },
            glassy,
            false,
            theme,
            cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                view.no_keep_on_top_line = false;
                view.pin(cx)
            }),
        )
        .aria_toggled(on_top.into())
    });
    let back = floating.then(|| {
        small(
            ("huddle-back", BACK),
            "Back to the full window",
            glassy,
            false,
            theme,
            cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.back(cx)),
        )
    });
    pin.into_iter().chain(back).collect()
}

/// A 28 px control: glass over the picture (`glassy`), else the bar's
/// hairline box; danger-filled while `off`.
fn small(
    (id, icon): (&'static str, &'static str),
    name: &'static str,
    glassy: bool,
    off: bool,
    theme: &Theme,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let theme = *theme;
    div()
        .id(id)
        .size(design::size::CONTROL)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .map(|button| match (off, glassy) {
            (true, _) => button.bg(theme.danger).text_color(white()),
            (false, true) => button.bg(glass()).text_color(white()),
            (false, false) => button
                .border_1()
                .border_color(theme.border_strong)
                .text_color(theme.foreground)
                .hover(move |style| style.bg(theme.surface)),
        })
        .role(Role::Button)
        .aria_label(name)
        .focusable()
        .on_click(click)
        .child(design::icon(icon, px(14.)))
}

// ---------- the compact room ----------

/// The compact room's one row over the tile's foot: your chip, the
/// microphone, the camera and Leave, then the pin and back. Always shown:
/// a small window whose controls hide is a guessing game, and a keyboard
/// or a screen reader never hovers.
fn float_strip(
    view: &Huddle,
    speaking: bool,
    cx: &mut Context<Huddle>,
    theme: &Theme,
) -> Stateful<Div> {
    let theme_ = *theme;
    let chip = div()
        .h(design::size::CONTROL)
        .min_w(px(0.))
        .flex_shrink(1.)
        .px(design::space::XS)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .bg(glass())
        .text_color(white())
        .text_size(design::text::CAPTION)
        .children(mark(view, speaking))
        .child(div().min_w(px(0.)).truncate().child("You"));
    let controls = div()
        .flex()
        .flex_none()
        .gap(design::space::XS)
        .child(
            small(
                ("huddle-mic", if mic_off(view) { MIC_OFF } else { MIC }),
                words::mute(view.muted),
                true,
                mic_off(view),
                &theme_,
                cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_mute(cx)),
            )
            .aria_toggled(view.muted.into()),
        )
        .child(
            small(
                (
                    "huddle-camera",
                    if view.camera_off { VIDEO_OFF } else { VIDEO },
                ),
                words::camera_toggle(view.camera_off),
                true,
                view.camera_off,
                &theme_,
                cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.toggle_camera(cx)),
            )
            .aria_toggled(view.camera_off.into()),
        )
        .child(small(
            ("huddle-leave", LEAVE),
            "Leave",
            true,
            true,
            &theme_,
            cx.listener(|view: &mut Huddle, _: &ClickEvent, window, cx| leave(view, window, cx)),
        ));
    div()
        .id("huddle-float-strip")
        .absolute()
        .left(design::space::SM)
        .right(design::space::SM)
        .bottom(design::space::SM)
        .h(design::size::CONTROL)
        .flex()
        .items_center()
        .child(chip)
        .child(div().flex_1().min_w(design::space::SM))
        .child(controls)
        .child(div().flex_1().min_w(design::space::SM))
        .child(
            div()
                .flex()
                .flex_none()
                .gap(design::space::XS)
                .children(pin_back(view, true, cx, theme)),
        )
}

/// The compact room's troubles in the tile's middle, the camera's over
/// the microphone's, each its card as one row.
fn trouble_rows(view: &Huddle, cx: &mut Context<Huddle>, theme: &Theme) -> Option<Div> {
    let rows: Vec<Div> = [Device::Camera, Device::Microphone]
        .into_iter()
        .filter(|device| view.troubled(*device))
        .map(|device| trouble_row(view, device, cx, theme))
        .collect();
    (!rows.is_empty()).then(|| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(design::space::SM)
            .children(rows)
    })
}

/// A device's trouble as one row: its glyph, its title and Try again.
fn trouble_row(view: &Huddle, device: Device, cx: &mut Context<Huddle>, theme: &Theme) -> Div {
    let state = state(view, device);
    let (title, _) = words::trouble(device, state);
    div()
        .flex()
        .items_center()
        .gap(design::space::SM)
        .child(design::icon(trouble_glyph(&state.status), px(16.)).text_color(theme.muted))
        .child(
            div()
                .text_size(design::text::BODY)
                .text_color(theme.foreground)
                .child(title),
        )
        .child(retry(device, cx, theme))
}

/// Try again, quiet: both pieces opened again, the lists read again.
fn retry(device: Device, cx: &mut Context<Huddle>, theme: &Theme) -> Stateful<Div> {
    let theme = *theme;
    div()
        .id(match device {
            Device::Camera => "huddle-camera-retry",
            Device::Microphone => "huddle-microphone-retry",
        })
        .h(design::size::CONTROL)
        .flex()
        .items_center()
        .text_size(design::text::SECONDARY)
        .text_color(theme.muted)
        .cursor_pointer()
        .hover(move |style| style.text_color(theme.foreground))
        .role(Role::Button)
        .focusable()
        .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| view.retry(cx)))
        .child("Try again")
}

/// What the float says once where the host keeps no window on top
/// (Wayland), over the strip, until closed or the next press.
fn no_keep_on_top_line(cx: &mut Context<Huddle>) -> Div {
    div()
        .absolute()
        .left(design::space::SM)
        .right(design::space::SM)
        .bottom(design::space::SM + design::size::CONTROL + design::space::XS)
        .h(px(22.))
        .px(design::space::XS)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .bg(glass())
        .text_color(white())
        .text_size(design::text::CAPTION)
        .child(design::icon(PIN_OFF, px(12.)))
        .child(
            div()
                .id("huddle-no-keep-on-top")
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .child("Wayland: no keep-on-top here."),
        )
        .child(
            div()
                .id("huddle-no-keep-on-top-close")
                .size(px(22.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .role(Role::Button)
                .aria_label("Close")
                .focusable()
                .on_click(cx.listener(|view: &mut Huddle, _: &ClickEvent, _, cx| {
                    view.no_keep_on_top_line = false;
                    cx.notify();
                }))
                .child(design::icon(CLOSE, px(12.))),
        )
}

/// A device's split control in the room's bar: its toggle, and the caret
/// that opens its menu upward (the microphone's lists the speakers too).
/// The toggle is danger-filled while off; the microphone's while in trouble
/// too, its trouble told to assistive technology (the camera's trouble is
/// its tile's card).
fn split(view: &Huddle, device: Device, cx: &mut Context<Huddle>, theme: &Theme) -> Div {
    let theme = *theme;
    let (id, name, off, alarm, which, caret_id, caret_name) = match device {
        Device::Microphone => (
            "huddle-mic",
            words::mute(view.muted),
            view.muted,
            mic_off(view),
            Which::Sound,
            "huddle-mic-menu",
            "Choose microphone and speaker",
        ),
        Device::Camera => (
            "huddle-camera",
            words::camera_toggle(view.camera_off),
            view.camera_off,
            view.camera_off,
            Which::Kind(Kind::Camera),
            "huddle-camera-menu",
            "Choose camera",
        ),
    };
    let icon = match (device, alarm) {
        (Device::Microphone, true) => MIC_OFF,
        (Device::Microphone, false) => MIC,
        (Device::Camera, true) => VIDEO_OFF,
        (Device::Camera, false) => VIDEO,
    };
    let trouble = (device == Device::Microphone && view.troubled(device))
        .then(|| words::trouble(device, &view.mic_state).0);
    let expanded = view.menu.is_some_and(|menu| menu.which == which);
    let caret = div()
        .id(caret_id)
        .w(px(18.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .border_l_1()
        .border_color(theme.border_strong)
        .text_color(theme.muted)
        .cursor_pointer()
        .hover(move |style| style.bg(theme.surface))
        .role(Role::Button)
        .aria_label(caret_name)
        .aria_has_popup(MENU_POPUP)
        .aria_expanded(expanded)
        .focusable()
        .on_click(
            cx.listener(move |view: &mut Huddle, _: &ClickEvent, window, cx| {
                open(view, which, caret_id, window, cx)
            }),
        )
        .child(design::icon(CARET, px(12.)));
    div()
        .flex()
        .flex_none()
        .h(px(32.))
        .border_1()
        .border_color(theme.border_strong)
        .child(
            div()
                .id(id)
                .w(px(36.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .map(|main| match alarm {
                    true => main.bg(theme.danger).text_color(white()),
                    false => main
                        .text_color(theme.foreground)
                        .hover(move |style| style.bg(theme.surface)),
                })
                .role(Role::Button)
                .aria_label(name)
                .when_some(trouble, |main, trouble| main.aria_description(trouble))
                .aria_toggled(off.into())
                .focusable()
                .on_click(cx.listener(
                    move |view: &mut Huddle, _: &ClickEvent, _, cx| match device {
                        Device::Microphone => view.toggle_mute(cx),
                        Device::Camera => view.toggle_camera(cx),
                    },
                ))
                .child(design::icon(icon, px(16.))),
        )
        .child(anchor(which, caret_id, caret, cx).h_full())
}

// ---------- the device menu ----------

/// The open device menu: one Tab stop whose ↑ ↓ walk the rows, Enter or
/// Space picks, Escape closes; under its picker on the check, over its
/// caret in the room.
fn menu(view: &Huddle, open: Menu, cx: &mut Context<Huddle>, theme: &Theme) -> AnyElement {
    let theme = *theme;
    let rows = view.rows(open.which);
    let count = rows.len();
    let active = open.cursor.min(count.saturating_sub(1));
    let mut lines: Vec<AnyElement> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        // the room's sound menu: its two groups, headed
        if open.which == Which::Sound && (index == 0 || rows[index - 1].kind != row.kind) {
            if index > 0 {
                lines.push(
                    div()
                        .h(px(1.))
                        .my(design::space::XXS)
                        .bg(theme.border)
                        .into_any_element(),
                );
            }
            lines.push(
                div()
                    .px(design::space::SM)
                    .pt(design::space::XXS)
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .text_size(design::text::CAPTION)
                    .text_color(theme.muted)
                    .child(words::kind(row.kind))
                    .into_any_element(),
            );
        }
        lines.push(
            design::item(
                menu_row(index, row, index == active, cx, &theme),
                Role::MenuItemRadio,
                index == active,
            )
            .into_any_element(),
        );
    }
    let escape = cx.listener(|view: &mut Huddle, event: &KeyDownEvent, window, cx| {
        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
            closed(view.close_menu(cx), window);
        }
    });
    let body = match count > 8 {
        // a long list scrolls in eight rows' height
        true => div()
            .id("huddle-menu-rows")
            .h(design::size::ROW * 8.)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(lines),
        false => div()
            .id("huddle-menu-rows")
            .flex()
            .flex_col()
            .children(lines),
    };
    let frame = design::composite(MENU, Role::Menu, words::menu(open.which))
        .active(active, count)
        .on_move(cx.processor(|view: &mut Huddle, index: usize, _, cx| {
            if let Some(menu) = &mut view.menu {
                menu.cursor = index;
            }
            cx.notify();
        }))
        .on_press(cx.processor(|view: &mut Huddle, index: usize, window, cx| {
            closed(view.pick(index, cx), window)
        }))
        .build()
        // Escape is the menu's: the overlay around it does not close it again
        .on_key_down(escape)
        .consumes_keys(["escape"])
        .w(px(COLUMN))
        .py(design::space::XXS)
        .flex()
        .flex_col()
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.background)
        .shadow_lg()
        .occlude()
        .child(body);
    // the ring would replace the shadow: the frame draws both
    let frame = design::focus_shown(frame, &theme, |style| style.shadow_lg());
    let (at, side) = place(view, open.which);
    anchored()
        .position_mode(AnchoredPositionMode::Window)
        .position(at)
        .anchor(side)
        .snap_to_window_with_margin(Edges::all(design::space::SM))
        .child(frame)
        .into_any_element()
}

/// Where a menu hangs: under its picker's left edge on the check; over its
/// split's left edge, a little above, in the room. Before layout told the
/// opener's bounds, the column's own geometry.
fn place(view: &Huddle, which: Which) -> (Point<Pixels>, Anchor) {
    let (width, height) = view.viewport;
    match (view.screen, view.desk.anchor(which)) {
        (Screen::Check, Some(bounds)) => (bounds.bottom_left(), Anchor::TopLeft),
        (Screen::Room, Some(bounds)) => (
            Point {
                x: bounds.origin.x - px(37.),
                y: bounds.origin.y - design::space::XS,
            },
            Anchor::BottomLeft,
        ),
        (Screen::Check, None) => (
            Point {
                x: px(width - f(design::space::BLOCK) - COLUMN),
                y: design::space::BLOCK + px(47.),
            },
            Anchor::TopLeft,
        ),
        (Screen::Room, None) => (
            Point {
                x: px(width / 2.),
                y: px(height - 48.),
            },
            Anchor::BottomLeft,
        ),
    }
}

fn menu_row(
    index: usize,
    row: &Row,
    active: bool,
    cx: &mut Context<Huddle>,
    theme: &Theme,
) -> Stateful<Div> {
    let theme = *theme;
    let empty = row.id.is_none();
    div()
        .id(SharedString::from(format!("huddle-menu-{index}")))
        .h(design::size::ROW)
        .flex_none()
        .px(design::space::SM)
        .flex()
        .items_center()
        .gap(design::space::SM)
        .text_size(design::text::SECONDARY)
        .text_color(match row.picked || active {
            true => theme.foreground,
            false => theme.muted,
        })
        .when(active && !empty, |row| row.bg(theme.surface_raised))
        .aria_toggled(row.picked.into())
        .map(|line| match empty {
            true => line.aria_disabled(true),
            false => line
                .cursor_pointer()
                .hover(move |style| style.bg(theme.surface_raised))
                .on_click(
                    cx.listener(move |view: &mut Huddle, _: &ClickEvent, window, cx| {
                        closed(view.pick(index, cx), window)
                    }),
                ),
        })
        .child(match row.picked {
            true => design::icon(CHECK, px(14.))
                .text_color(theme.foreground)
                .into_any_element(),
            false => div().size(px(14.)).flex_none().into_any_element(),
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .child(row.name.clone()),
        )
        .children(row.twin.clone().map(|id| {
            div()
                .flex_none()
                .font_family(design::fonts::FAMILY_MONO)
                .text_size(design::text::CAPTION)
                .text_color(theme.faint)
                .child(id)
        }))
}

// ---------- a build without camera and microphone ----------

fn unsupported(theme: &Theme) -> Stateful<Div> {
    div()
        .id("huddle-unsupported-screen")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .text_center()
        .child(design::icon(VIDEO_OFF, px(20.)).text_color(theme.foreground))
        .child(
            design::empty_state(
                "huddle-unsupported",
                VIDEO_OFF,
                "No camera or microphone in this build",
                "This copy of Ducktape was built without camera and microphone support, so a \
                 huddle has nothing to carry. Install a full build.",
                &design::POINTER,
                theme,
            )
            .items_center()
            .pt_0(),
        )
}
