//! `cargo run -p arcade-view --example live`: Arcade in the real app over a
//! fake node, its boards seeded so each state of the phone's boards shows.
//! The reader is the dev account (`dev`), the one the app signs in as.
//!
//! - Tetris: a full ten, the reader second (board 11);
//! - Snake: a full ten, the reader under it (board 11, outside the ten);
//! - 2048: three, none the reader's (the hub's "No score yet");
//! - Minesweeper: the reader alone;
//! - Breakout: two, the reader first;
//! - Invaders: no one (board 12, empty).
//!
//! `-- refused` refuses every read of the program, as a node in trouble
//! does (board 12, refused).
use arcade::Op;
use program::code;
use view::methods::Query;
use view::rig::{self, Network};

/// Everyone on the boards, by the name identity keeps for them, and their
/// Tetris best; the rest of their bests follow from it.
const PEOPLE: [(&str, u64); 11] = [
    ("Ada", 48_210),
    ("Marcus", 33_050),
    ("Hana", 29_910),
    ("Yuna", 18_870),
    ("Tom", 15_200),
    ("Ivo", 12_120),
    ("Lee", 9_070),
    ("Noor", 6_890),
    ("Sam", 4_410),
    ("Maximilian Featherstonehaugh", 3_980),
    ("Bo", 2_150),
];

/// Places in `arcade::GAMES`.
const TETRIS: u8 = 0;
const SNAKE: u8 = 1;
const MERGE: u8 = 2;
const MINES: u8 = 3;
const BREAKOUT: u8 = 4;

/// The argument that refuses the program's reads.
const REFUSED: &str = "refused";

fn main() {
    let refused = std::env::args().any(|arg| arg == REFUSED);
    rig::run(arcade::PROGRAM, |net| {
        net.seat::<arcade::Arcade>(arcade::PROGRAM);
        let me = net.me();
        for (name, tetris) in PEOPLE {
            let who = net.person(name);
            score(net, who, TETRIS, tetris)?;
            // Snake's ten are a tenth of Tetris's, all above the reader's
            score(net, who, SNAKE, tetris / 10)?;
        }
        score(net, me, TETRIS, 41_880)?;
        score(net, me, SNAKE, 120)?;
        for (name, best) in [("Ada", 2_048), ("Hana", 1_024), ("Tom", 512)] {
            score(net, net.person(name), MERGE, best)?;
        }
        score(net, me, MINES, 240)?;
        score(net, me, BREAKOUT, 41_880)?;
        score(net, net.person("Ivo"), BREAKOUT, 12_300)?;
        if refused {
            net.refuse::<Query<arcade::Arcade>>(code::TRAP, "arcade ran out of fuel");
        }
        Ok(())
    })
}

/// `who` posts `score` in game `game`.
fn score(net: &Network, who: u64, game: u8, score: u64) -> Result<(), rig::Error> {
    net.submit(who, arcade::PROGRAM, &Op::Score { game, score })
        .map(drop)
}
