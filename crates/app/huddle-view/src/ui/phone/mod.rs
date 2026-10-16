//! The phone's Huddle, the view's second design (`wt/briefs/huddle/
//! design-phone/SPEC.md`, boards 10 to 30; Expo has no huddle screen, so
//! these are new screens in the Expo kit's look, `design::TOUCH`). The
//! view's root picks it once, when the window's pointer is a finger;
//! nothing under here asks again.
//!
//! A phone keeps one full-screen window: Join keeps the view where it is
//! and nothing here floats it. The phone's route sets its microphone and
//! speaker together, so one Sound row and its sheet stand for both menus;
//! its front and back cameras make a flip, not a menu.
//!
//! It reads the model (`lib.rs`) and the words both designs show
//! (`words.rs`), and writes only through the model's verbs; it never
//! reaches into the desktop's `ui/` files. One file per screen:
//! - `check.rs`: the device check;
//! - `room.rs`: the room;
//! - `parts.rs`: what both draw.
//!
//! This file draws the screen up, the sound sheet over it, and the screen
//! of a build without camera and microphone.
use view::design::{Ladder, Nav};
use view::prelude::*;
use view::{AnyElement, Div, Stateful};

use crate::look::{HEADPHONES, SPEAKER, VIDEO_OFF};
use crate::words;
use crate::{Huddle, Kind, Row, Screen, Which};

mod check;
mod parts;
mod room;

/// The sound sheet is the speakers' menu: on a phone the speaker picked is
/// the route, which the microphone follows.
const SOUND: Which = Which::Kind(Kind::Speaker);

/// The screen, and the sound sheet over it. Back closes that sheet, and is
/// there exactly while one is drawn: on the check the way out is the
/// host's capsule, and the room ends by Leave alone.
pub(crate) fn render(view: &Huddle, cx: &mut Context<Huddle>) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let ladder = &design::TOUCH;
    let supported = !view.no_support();
    // what is drawn, decided once: the screen and the sheet over it
    let sheet = supported
        .then(|| sound_sheet(view, cx, ladder, &theme))
        .flatten();
    let screen = match (supported, view.screen) {
        (false, _) => unsupported(ladder, &theme).into_any_element(),
        (true, Screen::Check) => check::screen(view, cx, ladder, &theme).into_any_element(),
        (true, Screen::Room) => room::screen(view, cx, ladder, &theme).into_any_element(),
    };
    div()
        .id("huddle")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_type(ladder.body)
        .when(sheet.is_some(), |root| {
            root.on_back(cx.listener(|view: &mut Huddle, _: &BackEvent, _, cx| {
                view.close_menu(cx);
            }))
        })
        .child(screen)
        .children(sheet)
        .into_any_element()
}

/// The sound sheet (SPEC §1.2), while it is open: the routes the host
/// lists, the one in use checked; a pick plays through it and closes.
fn sound_sheet(
    view: &Huddle,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Option<AnyElement> {
    let menu = view.menu.filter(|menu| menu.which == SOUND)?;
    let close = cx.listener(|view: &mut Huddle, _: &(), _, cx| {
        view.close_menu(cx);
    });
    let routes = view
        .rows(menu.which)
        .into_iter()
        .enumerate()
        .map(|(index, row)| route(view, index, row, cx, ladder, theme))
        .collect::<Vec<_>>();
    let sheet = div()
        .flex()
        .flex_col()
        .child(design::sheet_title(
            "huddle-sound-sheet-head",
            words::PLAY_THROUGH,
            ladder,
            theme,
        ))
        .children(routes)
        .child(
            div()
                .px(design::SHEET_INSET)
                .py(design::space::LG)
                .text_type(ladder.secondary)
                .text_color(theme.muted)
                .child(words::MICROPHONE_FOLLOWS),
        );
    Some(
        design::sheet("huddle-sound-sheet", words::PLAY_THROUGH, ladder, theme)
            .on_dismiss(close)
            .child(sheet)
            .into_any_element(),
    )
}

/// Row `index` of the sound sheet: the route's glyph by what it is, its
/// name, and a check on the one in use; or, with none listed, why.
fn route(
    view: &Huddle,
    index: usize,
    row: Row,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> AnyElement {
    let Some(id) = row.id else {
        return nothing(row.name, ladder, theme).into_any_element();
    };
    let glyph = match view.speaker(&id).map(|speaker| speaker.kind) {
        Some(AudioKind::Headphones) => HEADPHONES,
        Some(AudioKind::Earpiece) => parts::SMARTPHONE,
        _ => SPEAKER,
    };
    let pick = cx.listener(move |view: &mut Huddle, _: &ClickEvent, _, cx| {
        view.pick(index, cx);
    });
    design::sheet_row(
        format!("huddle-sound-{index}"),
        row.name,
        ladder,
        theme,
        pick,
    )
    .icon(glyph)
    .when_some(row.twin, |row, twin| row.hint(twin))
    .into_element()
    .aria_toggled(row.picked.into())
    .when(row.picked, |route| {
        route.child(design::icon("check", ladder.icon_sheet).text_color(theme.foreground))
    })
    .into_any_element()
}

/// The sheet's one line where the host lists no route: still asking, its
/// refusal, or none found.
fn nothing(why: String, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    div()
        .id("huddle-sound-none")
        .h(ladder.sheet_row)
        .px(design::SHEET_INSET)
        .flex()
        .items_center()
        .text_type(ladder.body)
        .text_color(theme.muted)
        .child(why)
}

/// A build without camera and microphone: said once, under the title.
fn unsupported(ladder: &Ladder, theme: &Theme) -> impl IntoElement {
    design::page(
        "huddle-unsupported-page",
        words::HUDDLE,
        Nav::Child,
        ladder,
        theme,
    )
    .child(
        div()
            .flex_1()
            .flex()
            .flex_col()
            .justify_center()
            .child(design::empty_state(
                "huddle-unsupported",
                VIDEO_OFF,
                words::UNSUPPORTED,
                words::UNSUPPORTED_DETAIL,
                ladder,
                theme,
            )),
    )
}
