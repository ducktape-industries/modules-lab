//! A game's board (phone boards 11 and 12, SPEC §1.3), pushed over the
//! hub: the top ten, each with its rank, its account's name and its
//! score, the reader's own row marked; the reader's name and best under
//! them when it is not in the ten; while the board is read, a line saying
//! so; an empty board's note; a refused read's sentence and Retry. Each
//! standing is a row of a table, read as one line.
//!
//! Reads the open board, the session, and the reader's bests and name.
//! Writes the board's closing (Back) and a second read of a refused one.
use view::design::{Ladder, Nav};
use view::prelude::*;
use view::{AnyElement, Div, Stateful};

use crate::Arcade;
use crate::data::Row;
use crate::words::{self, BLURBS};

/// How deep the board sits over the hub, the root.
const DEPTH: usize = 1;

/// A rank's column (SPEC §1.3).
const RANK_W: Pixels = px(28.);

/// The loading line's glyph (SPEC §1.3).
const LOADING_GLYPH: Pixels = px(16.);

pub(super) fn screen(
    view: &Arcade,
    which: usize,
    cx: &mut Context<Arcade>,
    ladder: &Ladder,
    theme: &Theme,
) -> impl IntoElement {
    let name = arcade::GAMES[which];
    let back = cx.listener(|view: &mut Arcade, _: &ClickEvent, _, cx| view.close_board(cx));
    design::page("arcade-board", name, Nav::back(DEPTH, back), ladder, theme)
        .caption(words::board_caption(BLURBS[which]))
        .child(
            div()
                .id("arcade-board-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(body(view, which, cx, ladder, theme)),
        )
}

/// What the board's state draws under the title.
fn body(
    view: &Arcade,
    which: usize,
    cx: &mut Context<Arcade>,
    ladder: &Ladder,
    theme: &Theme,
) -> Vec<AnyElement> {
    let top_ten = || {
        design::section_head("arcade-top-head", words::TOP_TEN, ladder, theme).into_any_element()
    };
    match &view.board.rows {
        Loadable::Failed(refusal) => vec![
            design::refused(
                "arcade-board",
                design::sentence(refusal),
                ladder,
                theme,
                cx.listener(|view: &mut Arcade, _: &ClickEvent, _, cx| view.read_board(cx)),
            )
            .into_any_element(),
        ],
        Loadable::Idle | Loadable::Loading(_) => {
            vec![top_ten(), reading(ladder, theme).into_any_element()]
        }
        Loadable::Ready(rows) | Loadable::Reloading(rows, _) if rows.is_empty() => vec![
            design::empty_state(
                "arcade-board-empty",
                "trophy",
                words::NO_SCORES,
                words::first_score(arcade::GAMES[which]),
                ladder,
                theme,
            )
            .into_any_element(),
        ],
        Loadable::Ready(rows) | Loadable::Reloading(rows, _) => {
            let reader = view.session.account;
            let ten = rows.iter().enumerate().map(|(rank, row)| {
                let own = reader == Some(row.entry.account);
                standing(Some(rank), &row.name, row.entry.score, own, ladder, theme)
                    .into_any_element()
            });
            let mut drawn = vec![
                top_ten(),
                table("arcade-ten", words::TOP_TEN, ten).into_any_element(),
            ];
            drawn.extend(outside(view, which, rows, ladder, theme));
            drawn
        }
    }
}

/// The board is being read: the loading glyph and the line.
// SDK gap: loading line, swap when feat/phone-kit lands
fn reading(ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    div()
        .id("arcade-board-reading")
        .flex()
        .items_center()
        .gap(design::space::SM)
        .py(design::space::MD)
        .px(design::space::BLOCK)
        .text_type(ladder.secondary)
        .text_color(theme.muted)
        .child(design::icon("loader-circle", LOADING_GLYPH))
        .child(words::READING_BOARD)
}

/// What follows the ten: with no account, the line that says whose best
/// would show; with one not in the ten, the reader's name and best under
/// their own head. The program gives no rank outside the ten.
fn outside(
    view: &Arcade,
    which: usize,
    rows: &[Row],
    ladder: &Ladder,
    theme: &Theme,
) -> Vec<AnyElement> {
    let Some(reader) = view.session.account else {
        return vec![
            div()
                .id("arcade-board-unsigned")
                .px(design::space::BLOCK)
                .py(design::space::LG)
                .text_type(ladder.secondary)
                .text_color(theme.muted)
                .child(words::OWN_BEST_NEEDS_AN_ACCOUNT)
                .into_any_element(),
        ];
    };
    let in_the_ten = rows.iter().any(|row| row.entry.account == reader);
    let Some(best) = view.best(which).filter(|best| *best > 0 && !in_the_ten) else {
        return Vec::new();
    };
    // the name still being read, the account's number stands in
    let name = view
        .reader_name
        .ready()
        .cloned()
        .unwrap_or_else(|| words::unnamed(reader));
    let you = standing(None, &name, best, true, ladder, theme);
    vec![
        design::section_head("arcade-you-head", words::YOU, ladder, theme).into_any_element(),
        table("arcade-you", words::YOU, [you.into_any_element()]).into_any_element(),
    ]
}

/// Standings, as one table named `name`.
fn table(
    id: &'static str,
    name: &'static str,
    rows: impl IntoIterator<Item = AnyElement>,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Table)
        .aria_label(name)
        .children(rows)
}

/// A row's name, and on the reader's own the tag that says so.
fn named(name: &str, own: bool, ladder: &Ladder, theme: &Theme) -> Div {
    let title = div()
        .flex()
        .items_center()
        .gap(design::space::SM)
        .child(design::row_title(name.to_owned(), ladder, theme).min_w_0())
        .when(own, |title| title.child(you(ladder, theme)));
    design::row_words(title)
}

/// The tag after the reader's own name.
// SDK gap: tag chip after a title, swap when feat/phone-kit lands
fn you(ladder: &Ladder, theme: &Theme) -> Div {
    div()
        .flex_none()
        .px(design::space::XS)
        .border_1()
        .border_color(theme.border)
        .font_family(design::fonts::FAMILY_MONO)
        .text_type(ladder.tiny)
        .text_color(theme.muted)
        .child(words::YOU)
}

/// One standing on the board: its rank (0 is first; none for the reader's
/// best outside the ten, which the program does not rank, and the row says
/// so under the name), the account's name and its score; the reader's own
/// on a raised ground, its rank in ink. A standing presses nothing;
/// assistive technology reads it as one row: "3, ada, 420", and on the
/// reader's own ", you" after it.
fn standing(
    rank: Option<usize>,
    name: &str,
    score: u64,
    own: bool,
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    let outside = rank.is_none();
    // its id, its place as said, and its rank as drawn
    let (id, place, drawn) = match rank {
        Some(rank) => (
            format!("arcade-rank-{rank}"),
            (rank + 1).to_string(),
            (rank + 1).to_string(),
        ),
        None => (
            "arcade-rank-you".to_owned(),
            words::NOT_IN_TOP_TEN.to_owned(),
            words::NOT_KNOWN.to_owned(),
        ),
    };
    let mono = || div().flex_none().font_family(design::fonts::FAMILY_MONO);
    design::row(id, ladder, theme)
        .when(outside, |row| row.min_h(ladder.row_tall))
        .when(own, |row| row.bg(theme.surface_raised))
        .role(Role::Row)
        .aria_label(words::standing(&place, name, score, own))
        .child(
            mono()
                .w(RANK_W)
                .text_type(ladder.secondary)
                .text_color(if own { theme.foreground } else { theme.muted })
                .child(drawn),
        )
        .child(named(name, own, ladder, theme).when(outside, |column| {
            column.child(design::row_caption(words::NOT_IN_TOP_TEN, ladder, theme))
        }))
        .child(mono().text_type(ladder.body).child(design::grouped(score)))
}
