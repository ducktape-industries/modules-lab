//! The five games behind one enum, in [`arcade::GAMES`]' order, and the
//! seeded randomness they share. A game is its own rules: it hears keys,
//! steps on a tick, and says its score and whether it is over. Nothing
//! here knows the view; the view knows each game's board (`play.rs`).
use ducktape_view_guest::prelude::*;
use gpui::{point, size};

use crate::snake::Snake;

/// A game in play.
pub(crate) enum Game {
    Snake(Snake),
}

/// What a game shows beside its board under its score: a name and a value.
pub(crate) type Fact = (&'static str, String);

impl Game {
    /// Game `which` (its place in [`arcade::GAMES`]), dealt from `seed`.
    pub(crate) fn new(which: usize, seed: u64) -> Game {
        match which {
            _ => Game::Snake(Snake::new(seed)),
        }
    }

    /// A key went down on the board. Whether the board changed at once: a
    /// game on a clock reads the key on its next tick instead.
    pub(crate) fn key_down(&mut self, key: &str) -> bool {
        match self {
            Game::Snake(game) => game.key(key),
        }
    }

    /// A key came up on the board.
    pub(crate) fn key_up(&mut self, _key: &str) {
        match self {
            Game::Snake(_) => {}
        }
    }

    /// One step of the clock. Whether the board changed.
    pub(crate) fn tick(&mut self) -> bool {
        match self {
            Game::Snake(game) => game.tick(),
        }
    }

    /// The clock's period while the game runs, in milliseconds: none for a
    /// game that moves only when a key does.
    pub(crate) fn period(&self) -> Option<i64> {
        match self {
            Game::Snake(game) => Some(game.period()),
        }
    }

    pub(crate) fn score(&self) -> u64 {
        match self {
            Game::Snake(game) => game.score(),
        }
    }

    pub(crate) fn over(&self) -> bool {
        match self {
            Game::Snake(game) => game.over(),
        }
    }

    /// The panel's lines under the score.
    pub(crate) fn facts(&self) -> Vec<Fact> {
        match self {
            Game::Snake(game) => game.facts(),
        }
    }

    /// The keys the game takes, as the panel tells them.
    pub(crate) fn keys(&self) -> &'static str {
        match self {
            Game::Snake(_) => crate::snake::KEYS,
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
