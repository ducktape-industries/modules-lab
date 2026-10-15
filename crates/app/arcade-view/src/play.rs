//! A game on screen: a bar with the way back and the pause, the board, and
//! beside it the panel, its score, the reader's best and the game's own
//! facts. Paused or over, an overlay lies on the board and the panel shows
//! the game's top ten. Tetris has no panel: its screen is the Guideline's
//! ([`tetris::stage`]), and its overlay holds the top ten.
//!
//! The board takes the keys: it is focused as the game starts, and every
//! key the games read stops there. It hears Shift too, which gpui tells as
//! a change of the modifiers, not as a key.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{AnyElement, Div, FontWeight, Stateful};

use crate::data::Row;
use crate::games::{Game, seed};
use crate::tetris::Tetris;
use crate::{Arcade, Play, Post, breakout, invaders, merge, mines, snake, tetris};

/// The bar over the board, the panel's width and the room around a board.
pub(crate) const BAR_H: Pixels = px(44.);
const PANEL_W: f32 = 216.;
const GUTTER: f32 = 16.;

/// Every key a game or the screen reads: the board keeps them from the
/// panes around it.
pub(crate) const KEYS: [&str; 16] = [
    "up", "down", "left", "right", "space", "enter", "escape", "p", "w", "a", "s", "d", "x", "z",
    "f", "c",
];

impl Arcade {
    /// Starts game `which`: the screen opens with its board focused, and the
    /// game is dealt from the id the host mints for it.
    pub(crate) fn open(&mut self, which: usize, window: &mut Window, cx: &mut Context<Self>) {
        let id = cx.host().ask::<HostId>("arcade".into());
        let dealing = cx.land(id, move |view, id, cx| {
            let Some(play) = &mut view.play else {
                return;
            };
            match id {
                Ok(id) => {
                    play.seed = seed(&id);
                    play.game = Loadable::Ready(Game::new(which, play.seed));
                }
                Err(refused) => play.game = Loadable::Failed(refused),
            }
            cx.notify();
            view.settle(cx);
        });
        self.play = Some(Play {
            which,
            game: Loadable::Loading(dealing),
            seed: 0,
            paused: false,
            post: Post::Playing,
            field: None,
            shift: false,
        });
        self.board = Loadable::Idle;
        self.read_board(cx);
        window.focus("arcade-board");
        self.settle(cx);
        cx.notify();
    }

    /// Back to the hub, onto the card of the game that was open.
    pub(crate) fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(play) = self.play.take() else {
            return;
        };
        self.board = Loadable::Idle;
        window.focus(format!("arcade-card-{}", play.which));
        self.settle(cx);
        cx.notify();
    }

    /// Pauses a running game, or goes on with a paused one.
    fn pause(&mut self, cx: &mut Context<Self>) {
        let Some(play) = &mut self.play else {
            return;
        };
        if play.game.ready().is_none_or(Game::over) {
            return;
        }
        play.paused = !play.paused;
        // the board is read again for the top ten a pause shows
        if play.paused {
            if let Some(game) = play.game.ready_mut() {
                game.let_go();
            }
            self.read_board(cx);
        }
        self.settle(cx);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(play) = &mut self.play else {
            return;
        };
        let key = event.keystroke.key.as_str();
        let (which, over) = (play.which, play.game.ready().is_some_and(Game::over));
        // the screen's keys act once a press: the keyboard's repeat of one
        // held would flip the pause back, leave the game or deal another
        if event.is_held
            && (matches!(key, "escape" | "p") || key == "enter" && (over || play.paused))
        {
            return;
        }
        match key {
            "escape" if over || play.paused => return self.back(window, cx),
            "enter" if over => return self.open(which, window, cx),
            "escape" | "p" => return self.pause(cx),
            "enter" if play.paused => return self.pause(cx),
            _ => {}
        }
        if play.paused {
            return;
        }
        let Some(game) = play.game.ready_mut().filter(|game| !game.over()) else {
            return;
        };
        if game.key_down(key, event.is_held) {
            cx.notify();
        }
        self.settle(cx);
    }

    /// What the bar and the panel leave a board.
    fn room(&self) -> (f32, f32) {
        (
            self.viewport.0 - PANEL_W - 2. * GUTTER,
            self.viewport.1 - f32::from(BAR_H) - 2. * GUTTER,
        )
    }

    /// What the bar leaves Tetris's screen.
    pub(crate) fn stage_room(&self) -> (f32, f32) {
        (self.viewport.0, self.viewport.1 - f32::from(BAR_H))
    }

    /// A press on Minesweeper's field: the left button shows the cell under
    /// it, the right flags it.
    fn press(&mut self, event: &MouseDownEvent, right: bool, cx: &mut Context<Self>) {
        let cell = field(self.room());
        let Some(play) = self.play.as_mut().filter(|play| !play.paused) else {
            return;
        };
        let (Some((left, top)), Some(Game::Mines(mines))) = (play.field, play.game.ready_mut())
        else {
            return;
        };
        let at = (
            ((f32::from(event.position.x) - left) / cell).floor() as i32,
            ((f32::from(event.position.y) - top) / cell).floor() as i32,
        );
        if mines.press(at, right) {
            cx.notify();
        }
        self.settle(cx);
    }

    /// Shift alone is no key to gpui, only a change of the modifiers: it
    /// going down is a press of the key "shift", and up its release. A
    /// change that leaves it as it was, another modifier's, is neither.
    fn modifiers_changed(&mut self, event: &ModifiersChangedEvent, cx: &mut Context<Self>) {
        let Some(play) = &mut self.play else {
            return;
        };
        let shift = event.modifiers.shift;
        if std::mem::replace(&mut play.shift, shift) == shift {
            return;
        }
        let Some(game) = play.game.ready_mut() else {
            return;
        };
        if !shift {
            return game.key_up("shift");
        }
        if play.paused || game.over() {
            return;
        }
        if game.key_down("shift", false) {
            cx.notify();
        }
        self.settle(cx);
    }

    fn key_up(&mut self, event: &KeyUpEvent) {
        if let Some(game) = self.play.as_mut().and_then(|play| play.game.ready_mut()) {
            game.key_up(event.keystroke.key.as_str());
        }
    }

    pub(crate) fn play_screen(
        &mut self,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let theme = *theme;
        let play = self.play.as_ref().expect("a game is on screen");
        let name = arcade::GAMES[play.which];
        let running = play.game.ready().is_some_and(|game| !game.over());
        let pause = running.then(|| {
            design::button(
                "arcade-pause",
                if play.paused { "Resume" } else { "Pause" },
                &theme,
                cx.listener(|view, _: &ClickEvent, _, cx| view.pause(cx)),
            )
            .kind(design::Kind::Outline)
        });
        let bar = div()
            .flex_none()
            .h(BAR_H)
            .px(design::space::LG)
            .flex()
            .items_center()
            .gap(design::space::MD)
            .border_b_1()
            .border_color(theme.border)
            .child(
                design::button(
                    "arcade-back",
                    "Games",
                    &theme,
                    cx.listener(|view, _: &ClickEvent, window, cx| view.back(window, cx)),
                )
                .kind(design::Kind::Quiet),
            )
            .child(
                div()
                    .text_size(design::text::SECTION)
                    .font_weight(FontWeight::MEDIUM)
                    .child(name),
            )
            .child(div().flex_1())
            .children(pause);
        let body = match play.game.ready() {
            Some(Game::Tetris(tetris)) => self.tetris(tetris, cx, &theme).into_any_element(),
            _ => div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(self.stage(cx, &theme))
                .child(self.panel(&theme))
                .into_any_element(),
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(bar)
            .child(body)
    }

    /// Tetris's screen: its score, lines, level and the reader's best
    /// under hold, and on the well the overlay, with the top ten.
    fn tetris(&self, tetris: &Tetris, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let play = self.play.as_ref().expect("a game is on screen");
        let mut stats = vec![("Score", design::grouped(tetris.score()))];
        stats.extend(tetris.facts());
        stats.push(("Your best", self.best_shown(play.which, tetris.score())));
        tetris::stage(
            self.stage_room(),
            tetris,
            self.board(cx, theme),
            self.overlay(cx, theme, true),
            stats,
            None,
            theme,
        )
        .id("arcade-stage")
    }

    /// The board, centred in what the bar and the panel leave, with the
    /// overlay over it while the game is paused or over.
    fn stage(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let play = self.play.as_ref().expect("a game is on screen");
        let room = self.room();
        let drawn = match play.game.ready() {
            Some(Game::Mines(field)) => {
                // Minesweeper's field tells where the host laid it, so a
                // press's place in the window is a cell
                let measured = cx.listener(|view: &mut Arcade, bounds: &Bounds<Pixels>, _, _| {
                    if let Some(play) = &mut view.play {
                        play.field = Some((bounds.origin.x.into(), bounds.origin.y.into()));
                    }
                });
                sensor("arcade-field", field.draw(self::field(room), theme))
                    .on_bounds(measured)
                    .into_any_element()
            }
            Some(game) => draw(game, room, theme),
            None => div().into_any_element(),
        };
        div()
            .id("arcade-stage")
            .relative()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.background)
            .child(self.board(cx, theme).child(drawn))
            .children(self.overlay(cx, theme, false))
    }

    /// The board's frame, which takes the keys, and on Minesweeper's field
    /// the presses.
    fn board(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let play = self.play.as_ref().expect("a game is on screen");
        let label = match play.game.ready() {
            Some(game) => label(game, play),
            None => format!("{} board", arcade::GAMES[play.which]),
        };
        let mines = matches!(play.game.ready(), Some(Game::Mines(_)));
        div()
            .id("arcade-board")
            .relative()
            .flex_none()
            .border_1()
            .border_color(theme.border)
            .role(Role::Figure)
            .aria_label(label)
            .focusable()
            .consumes_keys(KEYS)
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                view.key_down(event, window, cx)
            }))
            .on_key_up(cx.listener(|view, event: &KeyUpEvent, _, _| view.key_up(event)))
            .on_modifiers_changed(cx.listener(|view, event: &ModifiersChangedEvent, _, cx| {
                view.modifiers_changed(event, cx)
            }))
            .when(mines, |board| {
                board
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|view, event: &MouseDownEvent, _, cx| {
                            view.press(event, false, cx)
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|view, event: &MouseDownEvent, _, cx| {
                            view.press(event, true, cx)
                        }),
                    )
            })
    }

    /// What lies on the board: why there is no game, the pause, or the
    /// end; with `top`, the top ten under it.
    fn overlay(&self, cx: &mut Context<Self>, theme: &Theme, top: bool) -> Option<Div> {
        let theme = *theme;
        let play = self.play.as_ref()?;
        let back = design::button(
            "arcade-leave",
            "Back to games",
            &theme,
            cx.listener(|view, _: &ClickEvent, window, cx| view.back(window, cx)),
        )
        .kind(design::Kind::Outline);
        let which = play.which;
        let (title, lines, first): (&str, Vec<String>, Option<AnyElement>) = match &play.game {
            Loadable::Failed(refused) => (
                "This game could not start",
                vec![refused.message.clone()],
                None,
            ),
            Loadable::Ready(game) if game.over() => (
                "Game over",
                vec![posted(&play.post)],
                Some(
                    design::button(
                        "arcade-again",
                        "Play again",
                        &theme,
                        cx.listener(move |view, _: &ClickEvent, window, cx| {
                            view.open(which, window, cx)
                        }),
                    )
                    .kind(design::Kind::Primary)
                    .into_any_element(),
                ),
            ),
            Loadable::Ready(_) if play.paused => (
                "Paused",
                vec!["P or Enter goes on.".into()],
                Some(
                    design::button(
                        "arcade-resume",
                        "Resume",
                        &theme,
                        cx.listener(|view, _: &ClickEvent, _, cx| view.pause(cx)),
                    )
                    .kind(design::Kind::Primary)
                    .into_any_element(),
                ),
            ),
            _ => return None,
        };
        let score = play
            .game
            .ready()
            .filter(|game| game.over())
            .map(|game| design::mono(design::grouped(game.score())).text_size(px(28.)));
        let top = top.then(|| self.top(&theme).w_full());
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id("arcade-overlay")
                        .min_w(px(240.))
                        .p(design::space::XL)
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(design::space::SM)
                        .border_1()
                        .border_color(theme.border_strong)
                        .bg(theme.background)
                        .role(Role::Dialog)
                        .aria_label(title)
                        .child(
                            div()
                                .text_size(design::text::TITLE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .children(score)
                        .children(
                            lines.into_iter().map(|line| {
                                design::quiet(line, &theme).text_center().max_w(px(280.))
                            }),
                        )
                        .child(
                            div()
                                .pt(design::space::SM)
                                .flex()
                                .gap(design::space::SM)
                                .children(first)
                                .child(back),
                        )
                        .children(top),
                ),
        )
    }

    /// The panel: the score, the reader's best, the game's facts and keys,
    /// and while the game is paused or over, the top ten.
    fn panel(&self, theme: &Theme) -> Div {
        let theme = *theme;
        let play = self.play.as_ref().expect("a game is on screen");
        let game = play.game.ready();
        let fact = |label: &'static str, value: String| {
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .text_size(design::text::CAPTION)
                        .text_color(theme.muted)
                        .child(label),
                )
                .child(
                    design::mono(value)
                        .text_size(design::text::SECTION)
                        .font_weight(FontWeight::MEDIUM),
                )
        };
        let score = game.map_or(0, Game::score);
        let best = self.best_shown(play.which, score);
        let facts = game
            .map(Game::facts)
            .unwrap_or_default()
            .into_iter()
            .map(|(label, value)| fact(label, value));
        let still = game.is_none_or(Game::over) || play.paused;
        div()
            .flex_none()
            .w(px(PANEL_W))
            .p(design::space::LG)
            .flex()
            .flex_col()
            .gap(design::space::MD)
            .border_l_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(design::text::CAPTION)
                            .text_color(theme.muted)
                            .child("Score"),
                    )
                    .child(
                        design::mono(design::grouped(score))
                            .text_size(px(24.))
                            .font_weight(FontWeight::MEDIUM),
                    ),
            )
            .child(fact("Your best", best))
            .children(facts)
            .children(game.map(|game| design::quiet(game.keys(), &theme)))
            .when(still, |panel| panel.child(self.top(&theme)))
    }

    /// The reader's best of game `which` with `score` played: none without
    /// an account.
    fn best_shown(&self, which: usize, score: u64) -> String {
        match (self.session.account, self.best(which)) {
            (None, _) => "—".to_owned(),
            (Some(_), best) => design::grouped(best.unwrap_or(0).max(score)),
        }
    }

    /// The game's top ten, or what stands in for them.
    fn top(&self, theme: &Theme) -> Div {
        let theme = *theme;
        let rows: AnyElement = match &self.board {
            Loadable::Failed(refused) => {
                design::quiet(refused.message.clone(), &theme).into_any_element()
            }
            Loadable::Ready(rows) | Loadable::Reloading(rows, _) if rows.is_empty() => {
                design::quiet("No scores yet.", &theme).into_any_element()
            }
            Loadable::Ready(rows) | Loadable::Reloading(rows, _) => {
                let mine = self.session.account;
                div()
                    .flex()
                    .flex_col()
                    .children(
                        rows.iter()
                            .enumerate()
                            .map(|(rank, row)| entry(rank, row, mine, &theme)),
                    )
                    .into_any_element()
            }
            Loadable::Idle | Loadable::Loading(_) => {
                design::quiet("Reading the board…", &theme).into_any_element()
            }
        };
        div()
            .pt(design::space::SM)
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(design::space::XS)
            .child(
                div()
                    .text_size(design::text::CAPTION)
                    .text_color(theme.muted)
                    .child("Top 10"),
            )
            .child(rows)
    }
}

/// One row of the top ten: the rank, the name and the score; the reader's
/// own in ink.
fn entry(rank: usize, row: &Row, mine: Option<u64>, theme: &Theme) -> Div {
    let own = mine == Some(row.entry.account);
    div()
        .h(px(22.))
        .flex()
        .items_center()
        .gap(design::space::SM)
        .text_size(design::text::SECONDARY)
        .text_color(if own { theme.foreground } else { theme.muted })
        .when(own, |row| row.font_weight(FontWeight::MEDIUM))
        .child(
            design::mono((rank + 1).to_string())
                .w(px(18.))
                .text_color(theme.faint),
        )
        .child(div().flex_1().min_w_0().truncate().child(row.name.clone()))
        .child(design::mono(design::grouped(row.entry.score)))
}

/// What became of the score, in a line.
fn posted(post: &Post) -> String {
    match post {
        Post::Playing | Post::Sending => "Sending your score…".into(),
        Post::Posted => "Sent to the board, which keeps your best.".into(),
        Post::Refused(why) => format!("The score was not posted: {why}"),
        Post::Unsigned => "Scores go on the board from an account; this one stays here.".into(),
    }
}

/// The game's board, as large as `room` holds it.
fn draw(game: &Game, room: (f32, f32), theme: &Theme) -> AnyElement {
    match game {
        Game::Tetris(_) => unreachable!("Tetris lays out its own screen"),
        Game::Snake(snake) => snake
            .draw(cell(room, (snake::COLS, snake::ROWS)), theme)
            .into_any_element(),
        Game::Merge(merge) => {
            let side = merge::SIDE as f32 + 0.5;
            let tile = (room.0.min(room.1) / side).floor().clamp(40., 120.);
            merge.draw(tile, theme).into_any_element()
        }
        Game::Mines(mines) => mines.draw(field(room), theme).into_any_element(),
        Game::Breakout(breakout) => {
            let (width, height) = (breakout::WIDTH as f32, breakout::HEIGHT as f32);
            let scale = (room.0 / width).min(room.1 / height).clamp(0.5, 2.);
            breakout.draw(scale, theme).into_any_element()
        }
        Game::Invaders(invaders) => {
            let (width, height) = (invaders::WIDTH as f32, invaders::HEIGHT as f32);
            let scale = (room.0 / width).min(room.1 / height).clamp(0.5, 4.);
            invaders.draw(scale, theme).into_any_element()
        }
    }
}

/// The whole pixels a cell of a `cols` x `rows` grid takes in `room`.
fn cell(room: (f32, f32), (cols, rows): (i32, i32)) -> f32 {
    (room.0 / cols as f32)
        .min(room.1 / rows as f32)
        .floor()
        .clamp(6., 40.)
}

/// A Minesweeper cell's pixels in `room`.
fn field(room: (f32, f32)) -> f32 {
    cell(room, (mines::COLS, mines::ROWS))
}

/// What the board says to a screen reader: the game, its score and state.
fn label(game: &Game, play: &Play) -> String {
    let name = arcade::GAMES[play.which];
    let state = match (game.over(), play.paused) {
        (true, _) => "Game over. Enter plays again, Escape goes back",
        (false, true) => "Paused. P goes on",
        (false, false) => game.keys(),
    };
    format!("{name}, score {}. {state}.", design::grouped(game.score()))
}
