//! The hub: a card for each game, each a button with the game's name, a
//! line of what it is, a still picture of it, and the reader's best; and
//! last, Tetris versus's, which opens its lobby.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{App, Canvas, Div, FontWeight, Stateful};

use crate::{Arcade, breakout, invaders, merge, mines, snake, tetris};

/// A card's width and its picture's height.
const CARD_W: Pixels = px(208.);
const PICTURE_H: Pixels = px(120.);

/// What each game is, in a line, in [`arcade::GAMES`]' order.
const BLURBS: [&str; 6] = [
    "Fill rows to clear them.",
    "Eat, grow, and keep off your tail.",
    "Slide equal tiles together.",
    "Clear the field without a mine.",
    "Eight walls, steel and falling items.",
    "Shoot the march down before it lands.",
];
/// Tetris versus's line.
const VERSUS_BLURB: &str = "Two wells, the same pieces; rows you clear rise in theirs.";

impl Arcade {
    pub(crate) fn hub(&self, cx: &mut Context<Self>, theme: &Theme) -> impl IntoElement + use<> {
        let mut cards: Vec<_> = (0..arcade::GAMES.len())
            .map(|which| self.card(which, cx, theme))
            .collect();
        cards.push(self.versus_card(cx, theme));
        let unsigned = self.session.account.is_none().then(|| {
            design::quiet(
                "Scores go on the board from an account. Without one the games play and nothing is posted.",
                theme,
            )
        });
        div()
            .id("arcade-hub")
            .size_full()
            .overflow_y_scroll()
            .p(design::space::XL)
            .flex()
            .flex_col()
            .gap(design::space::BLOCK)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(design::space::XXS)
                    .child(design::heading("arcade-title", "Arcade", 1, theme))
                    .child(design::quiet(
                        "Your best of each game goes on a board everyone sees. Nothing checks a score: it is play.",
                        theme,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(design::space::BLOCK)
                    .children(cards),
            )
            .children(unsigned)
    }

    /// Game `which`'s card: pressed, the game starts.
    fn card(&self, which: usize, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let name = arcade::GAMES[which];
        let best = match (self.session.account, self.best(which)) {
            (None, _) => None,
            (Some(_), Some(0)) => Some("No score yet".to_owned()),
            (Some(_), Some(best)) => Some(format!("Your best {}", design::grouped(best))),
            (Some(_), None) => Some("Your best —".to_owned()),
        };
        let open =
            cx.listener(move |view, _: &ClickEvent, window, cx| view.open(which, window, cx));
        tile(
            format!("arcade-card-{which}"),
            (name, BLURBS[which]),
            best,
            picture(which, theme),
            open,
            theme,
        )
    }

    /// Tetris versus's card: pressed, its lobby opens.
    fn versus_card(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let open = cx.listener(|view, _: &ClickEvent, window, cx| view.open_versus(window, cx));
        tile(
            "arcade-card-versus".into(),
            ("Tetris versus", VERSUS_BLURB),
            None,
            tetris::preview(theme),
            open,
            theme,
        )
    }
}

/// A card: a button with a still picture over the game's name and its
/// line, and the reader's best where there is one.
fn tile(
    id: String,
    (name, blurb): (&'static str, &'static str),
    best: Option<String>,
    picture: Canvas,
    open: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    theme: &Theme,
) -> Stateful<Div> {
    let theme = *theme;
    let label = match &best {
        Some(best) => format!("{name}. {blurb} {best}."),
        None => format!("{name}. {blurb}"),
    };
    div()
        .id(id)
        .w(CARD_W)
        .flex()
        .flex_col()
        .border_1()
        .border_color(theme.border)
        .bg(theme.background)
        .hover(move |style| style.border_color(theme.border_strong))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(label)
        .focusable()
        .on_click(open)
        .child(
            div()
                .h(PICTURE_H)
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.border)
                .child(picture),
        )
        .child(
            div()
                .p(design::space::LG)
                .flex()
                .flex_col()
                .gap(design::space::XXS)
                .child(
                    div()
                        .text_size(design::text::SECTION)
                        .font_weight(FontWeight::MEDIUM)
                        .child(name),
                )
                .child(design::quiet(blurb, &theme))
                .children(best.map(|best| {
                    design::mono(best)
                        .pt(design::space::XS)
                        .text_color(theme.muted)
                })),
        )
}

/// Game `which`'s still picture, drawn in its card's 208 x 120 px.
fn picture(which: usize, theme: &Theme) -> Canvas {
    match which {
        0 => tetris::preview(theme),
        1 => snake::preview(theme),
        2 => merge::preview(theme),
        3 => mines::preview(theme),
        4 => breakout::preview(theme),
        _ => invaders::preview(theme),
    }
}
