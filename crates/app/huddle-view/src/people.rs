//! What both designs draw of the other people in a call (`call.rs` is its
//! model): their voices, each person's tile and chip, and the chip past the
//! stage. The desktop's tree (`ui`) and the phone's (`ui::phone`) both read
//! these, at their own ladder; neither draws them its own way.
use view::design::Ladder;
use view::prelude::*;
use view::{Div, Stateful};

use crate::Huddle;
use crate::call::Piece;
use crate::look::{SPEAKING, black, glass, white};
use crate::words;

/// The people glyph, by the Lucide name `design::icon` draws.
pub(crate) const PEOPLE: &str = "users";

/// A voice for each other person in the call, whoever the stage shows:
/// zero-size, first in the room, so a layout change never moves one. Each
/// plays on the speaker you picked.
pub(crate) fn voices(view: &Huddle, cx: &mut Context<Huddle>) -> Option<Div> {
    let call = view.call.as_ref()?;
    let output = view.speaker.clone();
    let voices = call.people.keys().map(|who| {
        let who = *who;
        let track = LiveTrack {
            room: call.id.clone(),
            name: huddle::microphone_track(who),
        };
        remote_audio(SharedString::from(format!("huddle-voice-{who}")), track)
            .when_some(output.clone(), |voice, output| voice.output(output))
            .on_state(
                cx.listener(move |view: &mut Huddle, state: &MediaState, _, cx| {
                    view.heard_person(who, Piece::Voice, state, cx)
                }),
            )
    });
    Some(div().absolute().size(px(0.)).children(voices))
}

/// `who`'s tile, `width` across: their picture covering it while their
/// camera is up, else their face; a ring while they speak; their chip.
pub(crate) fn tile(
    view: &Huddle,
    who: u64,
    width: f32,
    cx: &mut Context<Huddle>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let person = view.call.as_ref().and_then(|call| call.people.get(&who));
    let camera = person.is_some_and(|person| person.camera);
    let speaking = person.is_some_and(|person| person.voice.speaking);
    let name = view.name(who);
    let picture = view.call.as_ref().filter(|_| camera).map(|call| LiveTrack {
        room: call.id.clone(),
        name: huddle::camera_track(who),
    });
    let face = match width {
        w if w >= 300. => 72.,
        w if w >= 180. => 48.,
        _ => 32.,
    };
    div()
        .id(SharedString::from(format!("huddle-tile-{who}")))
        .relative()
        .overflow_hidden()
        .role(Role::Group)
        .aria_label(name.clone())
        .bg(match camera {
            true => black(),
            false => theme.surface_raised,
        })
        .map(|tile| match picture {
            Some(track) => tile.child(
                remote_video(SharedString::from(format!("huddle-picture-{who}")), track)
                    .object_fit(ObjectFit::Cover)
                    .size_full()
                    .on_state(
                        cx.listener(move |view: &mut Huddle, state: &MediaState, _, cx| {
                            view.heard_person(who, Piece::Picture, state, cx)
                        }),
                    ),
            ),
            None => tile.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(design::avatar(&name, px(face), theme)),
            ),
        })
        .when(speaking, |tile| tile.child(ring(theme)))
        .child(
            div()
                .absolute()
                .left(design::space::SM)
                .bottom(design::space::SM)
                .child(chip(&name, speaking, ladder)),
        )
}

/// The ring round the tile of whoever speaks.
pub(crate) fn ring(theme: &Theme) -> Div {
    div()
        .absolute()
        .inset_0()
        .border_2()
        .border_color(theme.success)
}

/// A person's chip on the glass: the speaking mark, their name.
fn chip(name: &str, speaking: bool, ladder: &Ladder) -> Div {
    div()
        .flex_none()
        .h(px(22.))
        .px(design::space::XS)
        .flex()
        .items_center()
        .gap(design::space::XS)
        .bg(glass())
        .text_color(white())
        .text_type(ladder.caption)
        .when(speaking, |chip| {
            chip.child(
                div()
                    .id("huddle-mark-speaking")
                    .flex()
                    .role(Role::Image)
                    .aria_label(words::SPEAKING)
                    .child(design::icon(SPEAKING, px(12.))),
            )
        })
        .child(div().min_w(px(0.)).truncate().child(name.to_owned()))
}

/// The chip for the `more` people the stage does not show.
pub(crate) fn more(more: usize, ladder: &Ladder) -> Stateful<Div> {
    div()
        .id("huddle-more")
        .h(px(22.))
        .px(design::space::XS)
        .flex()
        .flex_none()
        .items_center()
        .gap(design::space::XXS)
        .bg(glass())
        .text_color(white())
        .text_type(ladder.caption)
        .role(Role::Group)
        .aria_label(words::more_in_the_huddle(more))
        .child(design::icon(PEOPLE, px(12.)))
        .child(
            div()
                .font_family(design::fonts::FAMILY_MONO)
                .child(words::more(more)),
        )
}
