//! The hub (phone boards 10 and 13, SPEC §1.2): the line that says the
//! games are played on a computer, then a row a game, its still picture,
//! name and line, and the reader's best. A row opens the game's board,
//! never the game. Tetris versus has no row: it is played live, on a
//! keyboard, and keeps no board to read.
//!
//! Reads the session and the reader's bests. Writes the board it opens
//! (`open_board`), and a second read of bests that were refused.
use view::design::{Ladder, Nav};
use view::prelude::*;
use view::{Div, Stateful};

use crate::Arcade;
use crate::games::picture;
use crate::words::{self, BLURBS};

/// A game's still picture leading its row, its hairline (`border_1`)
/// included (SPEC §1.2: 72 by 44, where the kit's lead column is 24).
const THUMB: (f32, f32) = (72., 44.);
const THUMB_EDGE: f32 = 1.;

/// The note's glyph (SPEC §1.2).
const NOTE_GLYPH: Pixels = px(18.);

pub(super) fn screen(
    view: &Arcade,
    cx: &mut Context<Arcade>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    // the view's one root page: the host's capsule closes it, so it has
    // no Back of its own
    design::page("arcade-hub", words::ARCADE, Nav::Child, ladder, theme)
        .caption(words::HUB_CAPTION)
        .child(
            div()
                .id("arcade-hub-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(note(ladder, theme))
                .children(refused_bests(view, cx, ladder, theme))
                .child(design::section_head(
                    "arcade-games-head",
                    words::GAMES,
                    ladder,
                    theme,
                ))
                .children(
                    (0..arcade::GAMES.len()).map(|which| game(view, which, cx, ladder, theme)),
                )
                .when(view.session.account.is_none(), |body| {
                    body.child(
                        div()
                            .id("arcade-unsigned")
                            .px(design::space::BLOCK)
                            .py(design::space::LG)
                            .text_type(ladder.secondary)
                            .text_color(theme.muted)
                            .child(words::UNSIGNED),
                    )
                }),
        )
}

/// The phone's honest line: a strip on the surface, the keyboard's glyph
/// and the sentence.
// SDK gap: note strip, swap when feat/phone-kit lands
fn note(ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    div()
        .id("arcade-phone-note")
        .flex()
        .items_start()
        .gap(design::space::MD)
        .py(design::space::MD)
        .px(design::space::BLOCK)
        .bg(theme.surface)
        .border_b_1()
        .border_color(theme.border)
        .text_type(ladder.secondary)
        .text_color(theme.muted)
        .child(design::icon("keyboard", NOTE_GLYPH))
        .child(div().flex_1().min_w_0().child(words::PHONE_ONLY_READS))
}

/// The bests could not be read: the refusal, and Retry.
fn refused_bests(
    view: &Arcade,
    cx: &mut Context<Arcade>,
    ladder: &Ladder,
    theme: &Theme,
) -> Option<Stateful<Div>> {
    let Loadable::Failed(refusal) = &view.bests else {
        return None;
    };
    Some(design::refused(
        "arcade-bests",
        design::sentence(refusal),
        ladder,
        theme,
        cx.listener(|view: &mut Arcade, _: &ClickEvent, _, cx| view.read_bests(cx)),
    ))
}

/// Game `which`'s row: its picture, name and line, the reader's best, and
/// the chevron of a page it opens.
fn game(
    view: &Arcade,
    which: usize,
    cx: &mut Context<Arcade>,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let name = arcade::GAMES[which];
    // a best is an account's: one not read yet is a dash, and a refused
    // read has none to say
    let best = match (view.session.account, &view.bests, view.best(which)) {
        (None, ..) | (_, Loadable::Failed(_), _) => None,
        (Some(_), _, None) => Some(words::NOT_KNOWN.to_owned()),
        (Some(_), _, best) => Some(words::best(best)),
    };
    design::pressed(
        design::row(format!("arcade-game-{which}"), ladder, theme),
        ladder,
    )
    .min_h(ladder.row_tall)
    .role(Role::Button)
    .aria_label(words::board_of(name))
    .focusable()
    .on_click(
        cx.listener(move |view: &mut Arcade, _: &ClickEvent, _, cx| view.open_board(which, cx)),
    )
    .child(thumb(which, theme))
    .child(
        design::row_words(design::row_title(name, ladder, theme)).child(design::row_caption(
            BLURBS[which],
            ladder,
            theme,
        )),
    )
    .children(best.map(|best| {
        div()
            .flex_none()
            .font_family(design::fonts::FAMILY_MONO)
            .text_type(ladder.secondary)
            .text_color(theme.muted)
            .child(best)
    }))
    .child(design::chevron(ladder, theme))
}

/// Game `which`'s still picture, on the surface in a hairline.
fn thumb(which: usize, theme: &Theme) -> Div {
    let inside = (THUMB.0 - 2. * THUMB_EDGE, THUMB.1 - 2. * THUMB_EDGE);
    div()
        .flex_none()
        .w(px(THUMB.0))
        .h(px(THUMB.1))
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .child(picture(which, inside, theme))
}
