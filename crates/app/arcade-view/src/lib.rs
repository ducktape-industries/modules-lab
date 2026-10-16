//! Arcade: six classic games, Tetris, Snake, 2048, Minesweeper, Breakout
//! and Space Invaders, and a board of the best scores the `arcade` program
//! keeps for everyone on the network; and Tetris versus, two people's
//! wells over the node's live tracks (`versus.rs`).
//!
//! The hub (`hub.rs`) lists the games; a game (`play.rs`) is its board,
//! a panel of its score, the reader's best and the game's own facts, and,
//! paused or over, an overlay and the top ten. Each game is one file of
//! its rules and its board's drawing (`tetris.rs`, `snake.rs`, `merge.rs`
//! for 2048, `mines.rs`, `breakout.rs`, `invaders.rs`), behind one enum
//! (`games.rs`). `data.rs` is everything the view follows and asks, and
//! `words.rs` what both trees say.
//!
//! Two designs, the model shared: under a pointer the desktop's tree
//! (`ui.rs`, the hub, the games and versus), under a finger the phone's
//! (`ui/phone/`), where no game is played and the boards are read. The
//! root picks one once.
//!
//! A game runs on the host's clock: `clock.ticks` delivers one item per
//! redraw however far behind the host is, with no time on it, so a tick
//! is a step and a game counts them. A host that falls behind slows the
//! game; nothing is skipped. The clock is asked for only while a game
//! runs: the hub, a paused game, an ended one and a game at rest until a
//! key, as 2048 is between its moves, ask for none, so an idle Arcade
//! costs nothing.
mod breakout;
mod data;
mod games;
mod hub;
mod invaders;
mod merge;
mod mines;
mod play;
mod snake;
pub mod tetris;
mod ui;
pub mod versus;
mod words;

use serde::{Deserialize, Serialize};
use view::prelude::*;

use data::Board;
use games::Game;
use versus::Versus;

#[derive(Default, Serialize, Deserialize)]
pub struct Arcade {
    #[serde(skip)]
    pub(crate) session: Session,
    /// The game on screen; none is the hub. A game is never saved: a
    /// snapshot comes back to the hub.
    #[serde(skip)]
    pub(crate) play: Option<Play>,
    /// The reader's best of every game, as the program keeps it.
    #[serde(skip)]
    pub(crate) bests: Loadable<Vec<u64>>,
    /// The board open: the open game's on the desktop, the one the reader
    /// opened on the phone.
    #[serde(skip)]
    pub(crate) board: Board,
    /// The clock while a game runs: its period, and the subscription,
    /// which ends when it is dropped.
    #[serde(skip)]
    pub(crate) clock: Option<(i64, Task<()>)>,
    /// The host said the view is off screen.
    #[serde(skip)]
    pub(crate) hidden: bool,
    /// The pane's size, read from the window each render.
    #[serde(skip)]
    pub(crate) viewport: (f32, f32),
    /// Tetris versus on screen: its lobby or its match. Never saved: its
    /// tracks are live, and a snapshot comes back to the hub.
    #[serde(skip)]
    pub(crate) versus: Option<Versus>,
}

/// A game on screen, from the moment its card is pressed.
pub(crate) struct Play {
    /// Its place in [`arcade::GAMES`].
    pub(crate) which: usize,
    /// Dealt once the host's id for it lands.
    pub(crate) game: Loadable<Game>,
    /// What it was dealt from: which game a late answer is for.
    pub(crate) seed: u64,
    pub(crate) paused: bool,
    pub(crate) post: Post,
    /// Where the host laid Minesweeper's field out, in the window's
    /// pixels: what a press is measured from.
    pub(crate) field: Option<(f32, f32)>,
    /// Shift is down, as the board last heard the modifiers.
    pub(crate) shift: bool,
}

/// What became of an ended game's score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Post {
    /// The game has not ended.
    Playing,
    Sending,
    Posted,
    Refused(String),
    /// No account is seated: scores stay on this screen.
    Unsigned,
}

impl View for Arcade {
    const NAME: &'static str = words::ARCADE;
    const DESCRIPTION: &'static str = "Tetris, Snake, 2048, Minesweeper, Breakout and Space Invaders, with a board of everyone's best scores, and Tetris versus.";
    const ICON: &'static str = "icons/gamepad-2.svg";
    const CAPABILITIES: &'static [Capability] = &[
        Capability::Program,
        Capability::Op,
        Capability::Host,
        Capability::Clock,
        Capability::Live,
    ];
    const TARGETS: &'static [&'static str] = &[arcade::PROGRAM, ::identity::PROGRAM];

    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.watch(cx);
    }
}

impl Render for Arcade {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = window.viewport_size();
        self.viewport = (size.width.into(), size.height.into());
        // one wasm, two designs, the model shared: a finger gets the
        // phone's screens, a pointer the desktop's
        if window.touch() {
            ui::phone::render(self, cx)
        } else {
            ui::render(self, window, cx)
        }
    }
}

// Writes the wasm exports the host calls and the manifest it reads.
export_view!(Arcade);

#[cfg(test)]
mod tests;
