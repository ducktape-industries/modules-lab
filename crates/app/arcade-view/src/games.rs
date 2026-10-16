//! The six games behind one enum, in [`arcade::GAMES`]' order, the
//! seeded randomness they share, and their still pictures. A game is its
//! own rules: it hears keys, steps on a tick, and says its score and
//! whether it is over. Nothing here knows the view; the view knows each
//! game's board (`play.rs`).
use gpui::{Point, point, size};
use view::Canvas;
use view::prelude::*;

use crate::breakout::{self, Breakout};
use crate::invaders::{self, Invaders};
use crate::merge::{self, Merge};
use crate::mines::{self, Mines};
use crate::snake::{self, Snake};
use crate::tetris::{self, Tetris};

/// A game in play, in [`arcade::GAMES`]' order.
pub(crate) enum Game {
    /// Boxed: its well is 200 cells.
    Tetris(Box<Tetris>),
    Snake(Snake),
    Merge(Merge),
    /// Boxed: its field is three grids of 256 cells.
    Mines(Box<Mines>),
    Breakout(Breakout),
    /// Boxed: its shields are 896 cells.
    Invaders(Box<Invaders>),
}

/// What a game shows beside its board under its score: a name and a value.
pub(crate) type Fact = (&'static str, String);

impl Game {
    /// Game `which` (its place in [`arcade::GAMES`]), dealt from `seed`.
    pub(crate) fn new(which: usize, seed: u64) -> Game {
        match which {
            0 => Game::Tetris(Box::new(Tetris::new(seed))),
            1 => Game::Snake(Snake::new(seed)),
            2 => Game::Merge(Merge::new(seed)),
            3 => Game::Mines(Box::new(Mines::new(seed))),
            4 => Game::Breakout(Breakout::new(seed)),
            _ => Game::Invaders(Box::new(Invaders::new(seed))),
        }
    }

    /// A key went down on the board, or the keyboard repeats one held.
    /// Whether the board changed at once: a game on a clock reads the key
    /// on its next tick instead.
    pub(crate) fn key_down(&mut self, key: &str, repeat: bool) -> bool {
        match self {
            Game::Tetris(game) => game.key_down(key, repeat),
            Game::Snake(game) => game.key(key),
            Game::Merge(game) => game.key(key),
            Game::Mines(game) => game.key(key, repeat),
            Game::Breakout(game) => game.key_down(key, repeat),
            Game::Invaders(game) => game.key_down(key, repeat),
        }
    }

    /// A key came up on the board: Breakout's paddle and the Invaders'
    /// cannon stop, and a Tetris piece stops sliding or dropping.
    pub(crate) fn key_up(&mut self, key: &str) {
        match self {
            Game::Tetris(game) => game.key_up(key),
            Game::Breakout(game) => game.key_up(key),
            Game::Invaders(game) => game.key_up(key),
            _ => {}
        }
    }

    /// The game pauses or goes off screen: what a key held did stops, as
    /// its coming up may go elsewhere, and 2048's tiles land.
    pub(crate) fn let_go(&mut self) {
        match self {
            Game::Tetris(game) => game.let_go(),
            Game::Merge(game) => game.motion = None,
            Game::Breakout(game) => {
                game.key_up("left");
                game.key_up("right");
            }
            Game::Invaders(game) => game.let_go(),
            _ => {}
        }
    }

    /// One step of the clock. Whether the board changed, or Tetris's
    /// effects did.
    pub(crate) fn tick(&mut self) -> bool {
        match self {
            Game::Tetris(game) => game.fade() | game.tick(),
            Game::Snake(game) => game.tick(),
            Game::Merge(game) => game.tick(),
            Game::Mines(game) => game.tick(),
            Game::Breakout(game) => game.tick(),
            Game::Invaders(game) => game.tick(),
        }
    }

    /// The clock's period while the game runs, in milliseconds: none while
    /// nothing moves until a key does.
    pub(crate) fn period(&self) -> Option<i64> {
        match self {
            Game::Tetris(_) => Some(tetris::PERIOD),
            Game::Snake(game) => game.period(),
            Game::Merge(game) => game.period(),
            Game::Mines(game) => game.period(),
            Game::Breakout(_) => Some(breakout::PERIOD),
            Game::Invaders(_) => Some(invaders::PERIOD),
        }
    }

    /// Tetris's effects show: the clock runs on past the game's end until
    /// they are done.
    pub(crate) fn animating(&self) -> bool {
        match self {
            Game::Tetris(game) => game.animating(),
            _ => false,
        }
    }

    pub(crate) fn score(&self) -> u64 {
        match self {
            Game::Tetris(game) => game.score(),
            Game::Snake(game) => game.score(),
            Game::Merge(game) => game.score(),
            Game::Mines(game) => game.score(),
            Game::Breakout(game) => game.score(),
            Game::Invaders(game) => game.score(),
        }
    }

    pub(crate) fn over(&self) -> bool {
        match self {
            Game::Tetris(game) => game.over(),
            Game::Snake(game) => game.over(),
            Game::Merge(game) => game.over(),
            Game::Mines(game) => game.over(),
            Game::Breakout(game) => game.over(),
            Game::Invaders(game) => game.over(),
        }
    }

    /// The panel's lines under the score.
    pub(crate) fn facts(&self) -> Vec<Fact> {
        match self {
            Game::Tetris(game) => game.facts(),
            Game::Snake(game) => game.facts(),
            Game::Merge(game) => game.facts(),
            Game::Mines(game) => game.facts(),
            Game::Breakout(game) => game.facts(),
            Game::Invaders(game) => game.facts(),
        }
    }

    /// The keys the game takes, as the panel tells them.
    pub(crate) fn keys(&self) -> &'static str {
        match self {
            Game::Tetris(_) => tetris::KEYS,
            Game::Snake(_) => snake::KEYS,
            Game::Merge(_) => merge::KEYS,
            Game::Mines(_) => mines::KEYS,
            Game::Breakout(_) => breakout::KEYS,
            Game::Invaders(_) => invaders::KEYS,
        }
    }
}

/// xorshift64*: the same seed deals the same game.
#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        // splitmix64 of the seed: any seed, zero included, starts a walk
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Rng((z ^ (z >> 31)) | 1)
    }

    /// A number in `0..n`; `n` is at least one.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % n
    }
}

/// A seed from the id the host minted for a game: FNV-1a over its bytes.
pub(crate) fn seed(id: &str) -> u64 {
    id.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
    })
}

/// A rectangle of a canvas, in its pixels.
pub(crate) fn square(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// The box each game lays its still picture out in: the desktop card's
/// picture, 208 by 120 px (`hub.rs`).
pub(crate) const STILL: (f32, f32) = (208., 120.);

/// A part of [`STILL`]: its left, top, width and height.
pub(crate) type Area = (f32, f32, f32, f32);

/// What draws a still picture, where a [`Still`] puts it.
type Draw = fn(Still, &Theme) -> Canvas;

/// Each game's still picture, in [`arcade::GAMES`]' order: what draws it,
/// and the part of [`STILL`] it draws on.
const PICTURES: [(Draw, Area); 6] = [
    (tetris::preview, tetris::DRAWN),
    (snake::preview, snake::DRAWN),
    (merge::preview, merge::DRAWN),
    (mines::preview, mines::DRAWN),
    (breakout::preview, breakout::DRAWN),
    (invaders::preview, invaders::DRAWN),
];

/// Game `which`'s still picture as the desktop's card draws it: whole, in
/// [`STILL`].
pub(crate) fn picture(which: usize, theme: &Theme) -> Canvas {
    (PICTURES[which].0)(Still::WHOLE, theme)
}

/// Game `which`'s still picture in a box `size` large (the phone's row):
/// the part it draws on, as large as the box holds it.
pub(crate) fn thumbnail(which: usize, size: (f32, f32), theme: &Theme) -> Canvas {
    let (draw, drawn) = PICTURES[which];
    draw(Still::around(drawn, size), theme)
}

/// Where a still picture laid out in [`STILL`] lands in the box it is drawn
/// in: every place and length scaled by one factor and moved by one
/// offset.
#[derive(Clone, Copy)]
pub(crate) struct Still {
    scale: f32,
    left: f32,
    top: f32,
}

impl Still {
    /// The whole picture in [`STILL`]: each place and length the one the
    /// picture names.
    pub(crate) const WHOLE: Still = Still {
        scale: 1.,
        left: 0.,
        top: 0.,
    };

    /// The part `drawn` filling a box `size` large as far as it can,
    /// centred: what lies outside it may fall outside the box.
    pub(crate) fn around((x, y, width, height): Area, size: (f32, f32)) -> Still {
        let scale = (size.0 / width).min(size.1 / height);
        Still {
            scale,
            left: (size.0 - width * scale) / 2. - x * scale,
            top: (size.1 - height * scale) / 2. - y * scale,
        }
    }

    /// How much larger the box draws the picture than [`STILL`].
    pub(crate) fn scale(self) -> f32 {
        self.scale
    }

    /// The picture's `(x, y)` in the box.
    pub(crate) fn place(self, x: f32, y: f32) -> (f32, f32) {
        (self.left + x * self.scale, self.top + y * self.scale)
    }

    /// The picture's `(x, y)` in the box, as a point.
    pub(crate) fn point(self, x: f32, y: f32) -> Point<Pixels> {
        let (x, y) = self.place(x, y);
        point(px(x), px(y))
    }

    /// A length of the picture in the box.
    pub(crate) fn length(self, length: f32) -> Pixels {
        px(length * self.scale)
    }

    /// A rectangle of the picture in the box.
    pub(crate) fn square(self, x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
        let (x, y) = self.place(x, y);
        square(x, y, width * self.scale, height * self.scale)
    }

    /// Where a drawing that scales its places from the box's corner, by
    /// [`scale`](Self::scale), puts the picture's `(x, y)`: in the
    /// picture's units, moved by the offset the box draws it at.
    pub(crate) fn place_unscaled(self, x: f32, y: f32) -> (f32, f32) {
        (x + self.left / self.scale, y + self.top / self.scale)
    }
}
