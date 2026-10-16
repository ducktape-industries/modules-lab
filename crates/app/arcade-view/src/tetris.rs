//! Tetris: the seven pieces fall into a well ten wide and twenty deep; a
//! full row clears. The tick is 16 ms, a frame: the pieces come from a
//! shuffled bag of the seven, each once a bag, and the queue shows the
//! next five. A key is read on the next tick. The rest is the Tetris
//! Guideline's: a turn that does not fit tries SRS's kicks, the I its own;
//! a piece that lands locks after half a second, which a move or a turn on
//! the ground starts again, 15 times a piece at most (the 16th locks it); a
//! hard drop locks at once; C or Shift puts the piece in hold and takes the
//! one there, or the next, once a piece.
//! A held key repeats on the game's own count, never the keyboard's (half
//! a second before its first repeat): a held arrow slides the piece again
//! after 160 ms and then a cell every 32 ms, a held ↓ drops a row every
//! other tick, and a turn, a drop or a hold does not repeat.
//!
//! The level sets how many ticks a row of fall takes, 48 at level 0 down to
//! one from level 29, as the NES has it. Alone, ten rows cleared raise it.
//! In versus it is the whole minutes the round has run, counted in the
//! round's own ticks (3,750 a minute), so both wells fall alike whatever
//! each cleared: Tetris 99 speeds everyone up over time ("piece gravity
//! begins to increase over time", tetris.wiki/Tetris_99) but publishes no
//! curve, and a level a minute is ours.
//!
//! # A lock ([`Clear`])
//! - **T-spin**, the Guideline's three corners (tetris.wiki/T-Spin): the
//!   piece is a T, the last move that moved it was a turn (a slide, a row of
//!   fall or soft drop, or a hard drop that moved undoes that), and three of
//!   the four corners of its 3×3 box are blocks, the walls or the floor. It
//!   is a mini when the two corners on the side the T points to are not
//!   both filled, unless the turn landed on SRS's last kick, the 1×2 one,
//!   which makes it a full T-spin.
//! - **Combo** (tetris.wiki/Combo): locks that each clear rows, one after
//!   another; the second is combo 1. A lock that clears nothing, a T-spin's
//!   included, ends it.
//! - **Back-to-back** (tetris.wiki/Scoring, T-Spin, Line_clear): a Tetris or
//!   a T-spin that clears rows, a mini's included, after another with no
//!   other line clear between. A Single, Double or Triple breaks the chain;
//!   a lock that clears nothing does not.
//! - **All clear** (tetris.wiki/Perfect_clear): the well is empty after the
//!   clear.
//!
//! # Attack, Tetris 99's (tetris.wiki/Tetris_99)
//!
//! | a lock | rows sent |
//! |---|---|
//! | Single, Double, Triple, Tetris | 0, 1, 2, 4 |
//! | T-spin Single, Double, Triple | 2, 4, 6 |
//! | Mini T-spin Single, Double | 0, 1 |
//! | back-to-back | +1 |
//! | combo 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 and on | +0, +1, +1, +2, +2, +3, +3, +4, +4, +4, +5 |
//! | all clear | +4 |
//!
//! The combo row is also tetris.wiki/Combo's Tetris 99 column. The most one
//! lock sends is 16, a T-spin Triple back to back at combo 10 that empties
//! the well. Not taken: Tetris 99's targeting and badges (a 1-on-1 match
//! has neither), Puyo Puyo Tetris's all clear of 10 and its combo table
//! (tetris.wiki/Combo's Guideline Standard column), and tetris.wiki/Garbage's
//! back-to-back of +2 for a T-spin Double or a Tetris and +3 for a Triple.
//!
//! # Garbage ([`Garbage`])
//! What a lock sends cancels the rows queued here first, oldest first, and
//! the rest rises in the other well (tetris.wiki/Garbage). A lock that clears
//! nothing raises at most 8 of the queued rows from the floor, oldest
//! first, each full but for its attack's hole; the rest waits for the next.
//! The 8 is TETR.IO's custom rooms' garbage cap, "only N lines allowed to
//! enter per piece placed that did not clear lines"
//! (tetris.wiki/TETR.IO/Seasonal_Events names the setting; its default of 8
//! is TETR.IO's own and not on the wiki). The Guideline sets no cap; Tetris
//! 99 caps the queue at 12 instead and loses the rest, not taken. A block
//! pushed out of the top ends the game.
//!
//! # Score, the Guideline's (tetris.wiki/Scoring), alone and in versus
//! Each lock's points times the level before it, plus one: the Guideline
//! counts levels from 1 and this game from 0.
//!
//! | a lock | points |
//! |---|---|
//! | Single, Double, Triple, Tetris | 100, 300, 500, 800 |
//! | T-spin with no rows, Single, Double, Triple | 400, 800, 1,200, 1,600 |
//! | Mini T-spin with no rows, Single, Double | 100, 200, 400 |
//! | back-to-back | the clear's × 1.5 |
//! | combo n | + 50 × n |
//! | all clear of 1, 2, 3, 4 rows; of a back-to-back Tetris | + 800, 1,200, 1,800, 2,000; 3,200 |
//!
//! A cell of soft drop is 1 and of hard drop 2, never times the level.
use std::collections::VecDeque;

use borsh::{BorshDeserialize, BorshSerialize};
use view::prelude::*;
use view::{Canvas, Div, FontWeight, Stateful};

use crate::games::{Area, Fact, Rng, Still, square};

pub const COLS: i32 = 10;
pub const ROWS: i32 = 20;
pub(crate) const KEYS: &str =
    "← → move   ↑ X turn   Z turn back   ↓ soft drop   Space drop   C Shift hold";
pub const PERIOD: i64 = 16;

/// The ticks a row of fall takes at each level; one from level 29 on.
const GRAVITY: [u32; 29] = [
    48, 43, 38, 33, 28, 23, 18, 13, 8, 6, 5, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];
/// The ticks of a minute: a level of versus.
const MINUTE: u64 = 60_000 / PERIOD as u64;
/// The keys that may wait for a tick.
const WAITING: usize = 8;
/// The pieces the queue shows coming.
const NEXT: usize = 5;
/// A piece in hold or coming, as a fraction of the well's cell: the held
/// one and the next larger, the rest of the queue smaller.
const NEAR: f32 = 0.75;
const FAR: f32 = 0.6;
/// SRS's kicks for a clockwise turn out of each of the four states, as the
/// Guideline writes them (x right, y up); a turn back out of state s + 1
/// tries the same, negated. The first that fits is the turn.
const KICKS: [[(i32, i32); 5]; 4] = [
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
];
/// The same for the I.
const I_KICKS: [[(i32, i32); 5]; 4] = [
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
];
/// How long a landed piece lies before it locks: half a second of ticks.
const LOCK_DELAY: u32 = (500 / PERIOD) as u32;
/// How many times a move or a turn on the ground starts the delay again.
const RESETS: u32 = 15;
/// The ticks an arrow is held before the piece slides on its own, 160 ms,
/// and the ticks a cell after that, 32 ms.
const DAS: u32 = 10;
const ARR: u32 = 2;
/// The ticks a row of a held ↓: NES's soft drop.
const SOFT: u32 = 2;
/// The kick of SRS's tables a turn that makes any T-spin a full one lands
/// on: the last, the 1×2.
const LAST_KICK: u8 = 4;
/// A lock's points at level 1 and the rows it sends, by the rows it clears:
/// plain, a mini T-spin's and a T-spin's. A T is three rows tall, so a
/// T-spin clears at most three; a mini at most two, as a row its empty
/// front corner lies in cannot clear.
const POINTS: [u64; 5] = [0, 100, 300, 500, 800];
const MINI_POINTS: [u64; 3] = [100, 200, 400];
const SPIN_POINTS: [u64; 4] = [400, 800, 1200, 1600];
const SENDS: [u8; 5] = [0, 0, 1, 2, 4];
const MINI_SENDS: [u8; 3] = [0, 0, 1];
const SPIN_SENDS: [u8; 4] = [0, 2, 4, 6];
/// What a combo adds, by its count; the last from there on.
const COMBO_SENDS: [u8; 11] = [0, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5];
const COMBO_POINTS: u64 = 50;
const BACK_TO_BACK: u8 = 1;
const ALL_CLEAR: u8 = 4;
/// An all clear's points by the rows that cleared, and a back-to-back
/// Tetris's.
const ALL_CLEAR_POINTS: [u64; 5] = [0, 800, 1200, 1800, 2000];
const ALL_CLEAR_BACK_TO_BACK: u64 = 3200;
/// The most rows one lock sends: a T-spin Triple back to back at the
/// combo's top that empties the well.
pub(crate) const MOST_SENT: u8 =
    SPIN_SENDS[3] + BACK_TO_BACK + COMBO_SENDS[COMBO_SENDS.len() - 1] + ALL_CLEAR;
/// The most queued rows a lock that clears nothing raises.
const RISE: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
    /// A block of a row of garbage the other side sent in versus. It lies
    /// in the well and never falls: the bag never deals it.
    Garbage,
}

/// What a well holds: a cell's block, if any, rows top first.
pub type Well = [[Option<Piece>; COLS as usize]; ROWS as usize];

const PIECES: [Piece; 7] = [
    Piece::I,
    Piece::O,
    Piece::T,
    Piece::S,
    Piece::Z,
    Piece::J,
    Piece::L,
];

impl Piece {
    /// The piece unturned in its box, and the box's side: SRS's shapes.
    fn shape(self) -> ([(i32, i32); 4], i32) {
        match self {
            Piece::I => ([(0, 1), (1, 1), (2, 1), (3, 1)], 4),
            Piece::O => ([(0, 0), (1, 0), (0, 1), (1, 1)], 2),
            Piece::T => ([(1, 0), (0, 1), (1, 1), (2, 1)], 3),
            Piece::S => ([(1, 0), (2, 0), (0, 1), (1, 1)], 3),
            Piece::Z => ([(0, 0), (1, 0), (1, 1), (2, 1)], 3),
            Piece::J => ([(0, 0), (0, 1), (1, 1), (2, 1)], 3),
            Piece::L => ([(2, 0), (0, 1), (1, 1), (2, 1)], 3),
            Piece::Garbage => unreachable!("garbage never falls, so it has no shape"),
        }
    }

    /// The piece's cells turned clockwise `turn` times, in its box. The O
    /// sits in the middle of a box of four.
    pub fn cells(self, turn: u8) -> [(i32, i32); 4] {
        let (mut cells, side) = self.shape();
        for _ in 0..turn % 4 {
            cells = cells.map(|(x, y)| (side - 1 - y, x));
        }
        let shift = i32::from(self == Piece::O);
        cells.map(|(x, y)| (x + shift, y))
    }

    pub(crate) fn color(self, theme: &Theme) -> Hsla {
        match self {
            Piece::I => theme.foreground,
            Piece::O => theme.warning,
            Piece::T => theme.agent,
            Piece::S => theme.success,
            Piece::Z => theme.danger,
            Piece::J => theme.muted,
            Piece::L => theme.warning.opacity(0.55),
            Piece::Garbage => theme.faint,
        }
    }
}

/// Versus: rows of garbage, each full but for one hole, all in one column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Attack {
    pub lines: u8,
    pub hole: u8,
}

impl Attack {
    /// Whether the other side's attack is one a well could have sent: at
    /// least a row, at most [`MOST_SENT`], its hole in the well.
    pub fn fair(self) -> bool {
        (1..=MOST_SENT).contains(&self.lines) && i32::from(self.hole) < COLS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spin {
    Mini,
    Full,
}

/// What one lock did, as the rules above name it: the one record the
/// score, the garbage and the screen's effects read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clear {
    /// The rows it cleared, 0 to 4.
    pub rows: u8,
    pub spin: Option<Spin>,
    /// A Tetris or a T-spin that cleared rows after another, with no other
    /// line clear between.
    pub back_to_back: bool,
    /// Its count among locks clearing rows one after another: 0 the
    /// first; 0 too for a lock that cleared nothing.
    pub combo: u32,
    /// The well was empty after it.
    pub perfect: bool,
    /// Versus: the rows of its attack that cancelled rows queued here, and
    /// that went to the other side; the queued rows that rose, for a lock
    /// that cleared nothing.
    pub cancelled: u8,
    pub sent: u8,
    pub rose: u8,
}

impl Clear {
    /// The rows it attacks with, before any cancel.
    pub fn attack(&self) -> u8 {
        if self.rows == 0 {
            return 0;
        }
        let rows = usize::from(self.rows);
        let lines = match self.spin {
            None => SENDS[rows],
            Some(Spin::Mini) => MINI_SENDS[rows],
            Some(Spin::Full) => SPIN_SENDS[rows],
        };
        let combo = COMBO_SENDS[(self.combo as usize).min(COMBO_SENDS.len() - 1)];
        lines
            + combo
            + if self.back_to_back { BACK_TO_BACK } else { 0 }
            + if self.perfect { ALL_CLEAR } else { 0 }
    }

    /// Its points at level 1, the drops' cells aside.
    fn points(&self) -> u64 {
        let rows = usize::from(self.rows);
        let points = match self.spin {
            None => POINTS[rows],
            Some(Spin::Mini) => MINI_POINTS[rows],
            Some(Spin::Full) => SPIN_POINTS[rows],
        };
        let points = if self.back_to_back {
            points * 3 / 2
        } else {
            points
        };
        let perfect = match (self.perfect, self.back_to_back && rows == 4) {
            (false, _) => 0,
            (true, true) => ALL_CLEAR_BACK_TO_BACK,
            (true, false) => ALL_CLEAR_POINTS[rows],
        };
        points + COMBO_POINTS * u64::from(self.combo) + perfect
    }
}

/// One well's side of versus: the garbage on its way in and the attacks
/// its clears make.
pub struct Garbage {
    /// The other side's attacks, oldest first, to rise on a lock that
    /// clears nothing.
    pub incoming: VecDeque<Attack>,
    /// This well's attacks, oldest first, for the view to send.
    pub outgoing: Vec<Attack>,
    /// Where an attack's hole goes. Never the bag's own: a draw from it
    /// would deal this side other pieces than the other side's.
    holes: Rng,
    /// The rows this well's attacks carried, and the rows the other
    /// side's brought, this round.
    pub(crate) sent: u32,
    pub(crate) received: u32,
}

impl Garbage {
    pub(crate) fn new(seed: u64) -> Garbage {
        Garbage {
            incoming: VecDeque::new(),
            outgoing: Vec::new(),
            holes: Rng::new(seed),
            sent: 0,
            received: 0,
        }
    }

    /// The other side's attack, queued to rise.
    pub(crate) fn receive(&mut self, attack: Attack) {
        self.received += u32::from(attack.lines);
        self.incoming.push_back(attack);
    }

    /// The rows queued to rise.
    pub(crate) fn queued(&self) -> u32 {
        self.incoming
            .iter()
            .map(|attack| u32::from(attack.lines))
            .sum()
    }

    /// A lock, `clear`: its attack cancels what is queued here first and
    /// the rest goes out; a lock that cleared nothing raises up to [`RISE`]
    /// of the queued rows. What it cancelled, sent and raised goes into
    /// `clear`. Whether a block went out of the top.
    fn trade(&mut self, clear: &mut Clear, well: &mut Well) -> bool {
        if clear.rows == 0 {
            let risen = self.take(RISE);
            clear.rose = risen.iter().map(|attack| attack.lines).sum();
            return rise(well, &risen);
        }
        let attack = clear.attack();
        clear.cancelled = self.take(attack).iter().map(|attack| attack.lines).sum();
        clear.sent = attack - clear.cancelled;
        if clear.sent > 0 {
            let hole = self.holes.below(COLS as u64) as u8;
            self.outgoing.push(Attack {
                lines: clear.sent,
                hole,
            });
            self.sent += u32::from(clear.sent);
        }
        false
    }

    /// Up to `rows` of the queued rows, oldest first, out of the queue; an
    /// attack taken in part leaves the rest of its rows first in line.
    fn take(&mut self, mut rows: u8) -> Vec<Attack> {
        let mut taken = Vec::new();
        while rows > 0
            && let Some(first) = self.incoming.front_mut()
        {
            let lines = rows.min(first.lines);
            taken.push(Attack {
                lines,
                hole: first.hole,
            });
            first.lines -= lines;
            rows -= lines;
            if first.lines == 0 {
                self.incoming.pop_front();
            }
        }
        taken
    }
}

/// `attacks`' rows push the well up from the floor, each full of garbage
/// but for its attack's hole; the oldest rose first, so it lies highest.
/// Whether a block went out of the top.
fn rise(well: &mut Well, attacks: &[Attack]) -> bool {
    let rows: Vec<u8> = attacks
        .iter()
        .flat_map(|attack| std::iter::repeat_n(attack.hole, usize::from(attack.lines)))
        .collect();
    let risen = rows.len().min(ROWS as usize);
    let out = well[..risen].iter().flatten().any(Option::is_some);
    well.rotate_left(risen);
    for (row, hole) in well[ROWS as usize - risen..].iter_mut().zip(rows) {
        *row = [Some(Piece::Garbage); COLS as usize];
        row[usize::from(hole)] = None;
    }
    out
}

/// How many frames each effect shows for: a flash 256 ms (the rows a
/// clear took, the rows that rose, the meter, the other side's frame), the
/// well's nudge 128 ms, the rows sent a second, a callout 1.5 s and the end
/// of a game 320 ms.
const FLASH: u32 = 16;
const NUDGE: u32 = 8;
const SENT: u32 = 62;
pub(crate) const CALLOUT: u32 = 94;
pub(crate) const ENDING: u32 = 20;
/// How far the well nudges up as garbage rises. The view hears no
/// reduced-motion setting (the SDK sends none), so this is everyone's:
/// 0 turns the one motion here off, and the effects are fades only.
const NUDGE_PX: f32 = 3.;
/// The callouts a stack holds, the newest first.
const CALLOUTS: usize = 6;

/// An effect: what it shows, and the frames it has left of its length.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fade<T> {
    what: T,
    left: u32,
    length: u32,
}

impl<T> Fade<T> {
    fn new(what: T, length: u32) -> Fade<T> {
        Fade {
            what,
            left: length,
            length,
        }
    }

    /// 1 as it starts, toward 0 as it ends.
    fn strength(&self) -> f32 {
        self.left as f32 / self.length as f32
    }

    /// The frames it has shown.
    fn shown(&self) -> u32 {
        self.length - self.left
    }
}

/// One effect a frame older; gone at its end.
fn age<T>(fade: &mut Option<Fade<T>>) {
    *fade = fade.take().and_then(|mut fade| {
        fade.left -= 1;
        (fade.left > 0).then_some(fade)
    });
}

/// A line of the callouts, and how it is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Callout {
    text: String,
    /// In its piece's color, or quiet; the all clear's is large.
    tone: Tone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Piece(Piece),
    Quiet,
    Large,
}

/// What a clear is called, its notable parts a line each.
fn called(clear: &Clear) -> Vec<Callout> {
    let line = |text: String, tone| Callout { text, tone };
    let rows = ["", " SINGLE", " DOUBLE", " TRIPLE"];
    let mut lines = Vec::new();
    match (clear.spin, usize::from(clear.rows)) {
        (Some(Spin::Full), rows_cleared) => lines.push(line(
            format!("T-SPIN{}", rows[rows_cleared]),
            Tone::Piece(Piece::T),
        )),
        (Some(Spin::Mini), rows_cleared) => lines.push(line(
            format!("T-SPIN MINI{}", rows[rows_cleared]),
            Tone::Piece(Piece::T),
        )),
        (None, 4) => lines.push(line("TETRIS".into(), Tone::Piece(Piece::I))),
        _ => {}
    }
    if clear.back_to_back {
        lines.push(line("BACK-TO-BACK".into(), Tone::Quiet));
    }
    if clear.rows > 0 && clear.combo > 0 {
        lines.push(line(format!("{} COMBO", clear.combo), Tone::Quiet));
    }
    if clear.perfect {
        lines.push(line("ALL CLEAR".into(), Tone::Large));
    }
    lines
}

/// What the screen shows for a moment after the game did something, aged
/// a frame a tick: drawn only, never read by the rules or sent.
#[derive(Default)]
struct Effects {
    /// The rows the last clear took, where they were before the rows over
    /// them came down.
    cleared: Option<Fade<Vec<usize>>>,
    /// Rows of garbage that rose from the floor.
    rose: Option<Fade<u8>>,
    /// An attack came into the queue.
    arrived: Option<Fade<()>>,
    /// Rows of the queue a clear cancelled.
    cancelled: Option<Fade<u8>>,
    /// Rows this well sent, and the other side's frame struck by them.
    sent: Option<Fade<u8>>,
    struck: Option<Fade<()>>,
    /// The game or the round ended: won, or topped out.
    ended: Option<Fade<bool>>,
    /// What the last clears were called, the newest first.
    callouts: Vec<Fade<Callout>>,
}

impl Effects {
    fn any(&self) -> bool {
        self.cleared.is_some()
            || self.rose.is_some()
            || self.arrived.is_some()
            || self.cancelled.is_some()
            || self.sent.is_some()
            || self.struck.is_some()
            || self.ended.is_some()
            || !self.callouts.is_empty()
    }

    /// A frame on. Whether an effect was showing, so this frame differs.
    fn step(&mut self) -> bool {
        let showing = self.any();
        age(&mut self.cleared);
        age(&mut self.rose);
        age(&mut self.arrived);
        age(&mut self.cancelled);
        age(&mut self.sent);
        age(&mut self.struck);
        age(&mut self.ended);
        self.callouts.retain_mut(|callout| {
            callout.left -= 1;
            callout.left > 0
        });
        showing
    }

    /// A lock's effects: the rows it cleared, at `rows`, flash; the rows it
    /// raised flash and nudge the well; what it cancelled and sent show
    /// beside it; and what it was called stacks up.
    fn locked(&mut self, clear: &Clear, rows: Vec<usize>) {
        if !rows.is_empty() {
            self.cleared = Some(Fade::new(rows, FLASH));
        }
        if clear.rose > 0 {
            self.rose = Some(Fade::new(clear.rose, FLASH));
        }
        if clear.cancelled > 0 {
            self.cancelled = Some(Fade::new(clear.cancelled, FLASH));
        }
        if clear.sent > 0 {
            self.sent = Some(Fade::new(clear.sent, SENT));
            self.struck = Some(Fade::new((), FLASH));
        }
        let called = called(clear)
            .into_iter()
            .map(|line| Fade::new(line, CALLOUT));
        self.callouts.splice(0..0, called);
        self.callouts.truncate(CALLOUTS);
    }

    /// The px the well stands up: up at once as garbage rises, back down
    /// over [`NUDGE`] frames.
    fn nudge(&self) -> f32 {
        self.rose
            .as_ref()
            .filter(|rose| rose.shown() < NUDGE)
            .map_or(0., |rose| {
                (NUDGE_PX * (1. - rose.shown() as f32 / NUDGE as f32)).round()
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Move {
    Left,
    Right,
    Turn,
    TurnBack,
    Down,
    Drop,
    Hold,
}

pub struct Tetris {
    /// The settled blocks.
    pub well: Well,
    pub piece: Piece,
    pub turn: u8,
    /// The top left corner of the falling piece's box.
    pub at: (i32, i32),
    /// The pieces coming, the first next.
    pub next: [Piece; NEXT],
    /// The piece put aside with C or Shift.
    pub hold: Option<Piece>,
    /// Hold was used for the piece falling: no hold again until it locks.
    pub held: bool,
    bag: Vec<Piece>,
    rng: Rng,
    moves: VecDeque<Move>,
    /// The arrows held to a side, the newest last: the piece slides to it.
    sides: Vec<Move>,
    /// Ticks since the newest of `sides` went down, or took over from a
    /// newer one let go.
    charge: u32,
    /// Ticks ↓ has been held.
    soft: Option<u32>,
    /// Ticks since the piece last fell a row.
    fell: u32,
    /// Ticks the piece has lain on the floor or a block.
    rest: u32,
    /// How often a move or a turn on the ground started `rest` again.
    resets: u32,
    /// The last move that moved the piece was a turn, which landed on this
    /// kick of its table.
    turned: Option<u8>,
    /// The last lock that cleared rows was a Tetris or a T-spin.
    chain: bool,
    /// The combo count of the last lock, if it cleared rows.
    combo: Option<u32>,
    /// The last lock's record, for the tests to read: the effects are
    /// handed theirs.
    #[cfg(test)]
    last: Option<Clear>,
    /// Ticks since the game was dealt: versus's level.
    ticks: u64,
    pub lines: u64,
    pub level: u64,
    score: u64,
    over: bool,
    /// The garbage this well trades in versus; none alone.
    pub versus: Option<Garbage>,
    effects: Effects,
}

impl Tetris {
    pub fn new(seed: u64) -> Tetris {
        let mut tetris = Tetris {
            well: [[None; COLS as usize]; ROWS as usize],
            piece: Piece::I,
            turn: 0,
            at: (0, 0),
            next: [Piece::I; NEXT],
            hold: None,
            held: false,
            bag: Vec::new(),
            rng: Rng::new(seed),
            moves: VecDeque::new(),
            sides: Vec::new(),
            charge: 0,
            soft: None,
            fell: 0,
            rest: 0,
            resets: 0,
            turned: None,
            chain: false,
            combo: None,
            #[cfg(test)]
            last: None,
            ticks: 0,
            lines: 0,
            level: 0,
            score: 0,
            over: false,
            versus: None,
            effects: Effects::default(),
        };
        tetris.next = std::array::from_fn(|_| tetris.draw_bag());
        tetris.spawn();
        tetris
    }

    /// A versus well: dealt from `seed` as the other side's is, its
    /// attacks' holes drawn from `holes`.
    pub fn versus(seed: u64, holes: u64) -> Tetris {
        Tetris {
            versus: Some(Garbage::new(holes)),
            ..Tetris::new(seed)
        }
    }

    /// The next piece from the bag, which is filled again, shuffled, when
    /// it runs out.
    fn draw_bag(&mut self) -> Piece {
        if self.bag.is_empty() {
            self.bag = PIECES.to_vec();
            for at in (1..self.bag.len()).rev() {
                let other = self.rng.below(at as u64 + 1) as usize;
                self.bag.swap(at, other);
            }
        }
        self.bag.pop().expect("a filled bag")
    }

    /// The next piece at the top of the well, and one more from the bag
    /// at the end of those coming.
    fn spawn(&mut self) {
        let next = self.next[0];
        self.next.rotate_left(1);
        self.next[NEXT - 1] = self.draw_bag();
        self.held = false;
        self.enter(next);
    }

    /// `piece` at the top of the well, unturned; one that does not fit
    /// there ends the game.
    fn enter(&mut self, piece: Piece) {
        self.piece = piece;
        self.turn = 0;
        self.at = (3, 0);
        self.fell = 0;
        self.rest = 0;
        self.resets = 0;
        self.turned = None;
        self.over = !self.fits(self.piece, self.turn, self.at);
    }

    pub fn fits(&self, piece: Piece, turn: u8, (x, y): (i32, i32)) -> bool {
        piece.cells(turn).iter().all(|(dx, dy)| {
            let (x, y) = (x + dx, y + dy);
            (0..COLS).contains(&x)
                && (0..ROWS).contains(&y)
                && self.well[y as usize][x as usize].is_none()
        })
    }

    /// The falling piece's cells in the well.
    pub(crate) fn falling(&self) -> [(i32, i32); 4] {
        let (x, y) = self.at;
        self.piece.cells(self.turn).map(|(dx, dy)| (x + dx, y + dy))
    }

    /// Where the falling piece would land, dropped now.
    pub(crate) fn landing(&self) -> i32 {
        let (x, mut y) = self.at;
        while self.fits(self.piece, self.turn, (x, y + 1)) {
            y += 1;
        }
        y
    }

    /// Moves the piece by `(dx, dy)` if it fits there.
    fn shift(&mut self, dx: i32, dy: i32) -> bool {
        let to = (self.at.0 + dx, self.at.1 + dy);
        let fits = self.fits(self.piece, self.turn, to);
        if fits {
            self.at = to;
            self.turned = None;
        }
        fits
    }

    /// Whether the piece lies on the floor or a block.
    fn landed(&self) -> bool {
        !self.fits(self.piece, self.turn, (self.at.0, self.at.1 + 1))
    }

    /// Turns the piece a quarter clockwise, or back, at the first of its
    /// kicks where it fits.
    fn rotate(&mut self, back: bool) -> bool {
        let table = match self.piece {
            Piece::I => I_KICKS,
            _ => KICKS,
        };
        let (turn, kicks) = match back {
            false => ((self.turn + 1) % 4, table[self.turn as usize]),
            true => {
                let turn = (self.turn + 3) % 4;
                (turn, table[turn as usize].map(|(x, y)| (-x, -y)))
            }
        };
        let (x, y) = self.at;
        let Some((kick, at)) = (0..)
            .zip(kicks)
            .map(|(kick, (dx, dy))| (kick, (x + dx, y - dy)))
            .find(|(_, at)| self.fits(self.piece, turn, *at))
        else {
            return false;
        };
        self.turn = turn;
        self.at = at;
        self.turned = Some(kick);
        true
    }

    /// The T-spin the piece makes, locking where it is.
    fn spin(&self) -> Option<Spin> {
        let kick = self.turned.filter(|_| self.piece == Piece::T)?;
        let (x, y) = self.at;
        let filled = |(dx, dy): (i32, i32)| {
            let (x, y) = (x + dx, y + dy);
            !(0..COLS).contains(&x)
                || y >= ROWS
                || (y >= 0 && self.well[y as usize][x as usize].is_some())
        };
        // the box's corners clockwise from the top left: a T turned `turn`
        // times points to corners `turn` and the one after
        let corners = [(0, 0), (2, 0), (2, 2), (0, 2)].map(filled);
        if corners.iter().filter(|filled| **filled).count() < 3 {
            return None;
        }
        let turn = usize::from(self.turn);
        Some(
            match corners[turn] && corners[(turn + 1) % 4] || kick == LAST_KICK {
                true => Spin::Full,
                false => Spin::Mini,
            },
        )
    }

    /// The piece settles where it is: full rows clear, what that was is
    /// scored and in versus traded, and the next piece comes.
    fn lock(&mut self) {
        let spin = self.spin();
        for (x, y) in self.falling() {
            self.well[y as usize][x as usize] = Some(self.piece);
        }
        let full: Vec<usize> = (0..ROWS as usize)
            .filter(|y| self.well[*y].iter().all(Option::is_some))
            .collect();
        let kept: Vec<_> = self
            .well
            .iter()
            .filter(|row| row.iter().any(Option::is_none))
            .copied()
            .collect();
        let rows = ROWS as usize - kept.len();
        let mut well = [[None; COLS as usize]; ROWS as usize];
        well[rows..].copy_from_slice(&kept);
        self.well = well;
        let difficult = rows == 4 || rows > 0 && spin.is_some();
        let combo = (rows > 0).then(|| self.combo.map_or(0, |combo| combo + 1));
        let mut clear = Clear {
            rows: rows as u8,
            spin,
            back_to_back: difficult && self.chain,
            combo: combo.unwrap_or(0),
            perfect: rows > 0 && self.well.iter().flatten().all(Option::is_none),
            ..Clear::default()
        };
        self.combo = combo;
        if rows > 0 {
            self.chain = difficult;
        }
        self.score += clear.points() * (self.level + 1);
        self.lines += rows as u64;
        let out = match &mut self.versus {
            Some(garbage) => garbage.trade(&mut clear, &mut self.well),
            None => {
                self.level = self.lines / 10;
                false
            }
        };
        #[cfg(test)]
        {
            self.last = Some(clear);
        }
        self.effects.locked(&clear, full);
        if out {
            self.over = true;
            return;
        }
        self.spawn();
    }

    /// Puts the piece in hold and brings in the one there, or the next.
    fn swap_hold(&mut self) -> bool {
        if self.held {
            return false;
        }
        match self.hold.replace(self.piece) {
            Some(held) => self.enter(held),
            None => self.spawn(),
        }
        self.held = true;
        true
    }

    fn apply(&mut self, held: Move) -> bool {
        let landed = self.landed();
        let moved = match held {
            Move::Left => self.shift(-1, 0),
            Move::Right => self.shift(1, 0),
            Move::Turn => self.rotate(false),
            Move::TurnBack => self.rotate(true),
            Move::Down => {
                let fell = self.shift(0, 1);
                if fell {
                    self.score += 1;
                    self.fell = 0;
                }
                return fell;
            }
            Move::Drop => {
                let rows = self.landing() - self.at.1;
                self.at.1 += rows;
                if rows > 0 {
                    self.turned = None;
                }
                self.score += 2 * rows as u64;
                self.lock();
                return true;
            }
            Move::Hold => return self.swap_hold(),
        };
        // a move on the ground starts the lock delay again, 15 times a
        // piece; the 16th locks it where it lies
        if moved && landed {
            if self.resets < RESETS {
                self.resets += 1;
                self.rest = 0;
            } else if self.landed() {
                self.lock();
            }
        }
        moved
    }

    /// A key waits for the next tick; an arrow is held until it comes up.
    /// The keyboard's own repeat of a held key is not read.
    pub fn key_down(&mut self, key: &str, repeat: bool) -> bool {
        let Some(held) = read(key) else {
            return false;
        };
        if repeat {
            return false;
        }
        match held {
            Move::Left | Move::Right => {
                self.sides.retain(|side| *side != held);
                self.sides.push(held);
                self.charge = 0;
            }
            Move::Down => self.soft = Some(0),
            _ => {}
        }
        if self.moves.len() < WAITING {
            self.moves.push_back(held);
        }
        false
    }

    /// An arrow came up: the other side's, if it is still held, slides
    /// the piece after its own count.
    pub fn key_up(&mut self, key: &str) {
        match read(key) {
            Some(Move::Down) => self.soft = None,
            Some(up @ (Move::Left | Move::Right)) => {
                let newest = self.sides.last().copied();
                self.sides.retain(|side| *side != up);
                if self.sides.last().copied() != newest {
                    self.charge = 0;
                }
            }
            _ => {}
        }
    }

    /// No key is held: a pause or a game off screen hears no key come up.
    pub(crate) fn let_go(&mut self) {
        self.sides.clear();
        self.soft = None;
    }

    /// The keys held: the newest side arrow slides the piece once DAS
    /// ticks have passed, then every ARR; ↓ drops a row every SOFT.
    fn repeat(&mut self) -> bool {
        let mut moved = false;
        if let Some(&side) = self.sides.last() {
            self.charge += 1;
            if self.charge >= DAS && (self.charge - DAS).is_multiple_of(ARR) {
                moved |= self.apply(side);
            }
        }
        if let Some(held) = self.soft.map(|held| held + 1) {
            self.soft = Some(held);
            if held.is_multiple_of(SOFT) && !self.over {
                moved |= self.apply(Move::Down);
            }
        }
        moved
    }

    /// One frame: the keys that waited, then gravity's count, or the lock
    /// delay's on the ground. In versus the frame counts toward the level.
    /// A game that ends on it flashes its well.
    pub fn tick(&mut self) -> bool {
        let over = self.over;
        let moved = self.step();
        if self.over && !over {
            self.effects.ended = Some(Fade::new(false, ENDING));
        }
        moved
    }

    fn step(&mut self) -> bool {
        self.ticks += 1;
        if self.versus.is_some() {
            self.level = self.ticks / MINUTE;
        }
        let mut moved = false;
        while let Some(held) = self.moves.pop_front() {
            if self.over {
                return moved;
            }
            moved |= self.apply(held);
        }
        if self.over {
            return moved;
        }
        moved |= self.repeat();
        if self.over {
            return moved;
        }
        if self.landed() {
            self.rest += 1;
            if self.rest >= LOCK_DELAY {
                self.lock();
                moved = true;
            }
        } else {
            self.fell += 1;
            if self.fell >= self.gravity() {
                self.fell = 0;
                self.at.1 += 1;
                self.turned = None;
                moved = true;
            }
        }
        moved
    }

    /// The effects a frame on: whether one was showing, so the frame
    /// changes.
    pub(crate) fn fade(&mut self) -> bool {
        self.effects.step()
    }

    /// An effect shows: the screen wants frames until it ends.
    pub(crate) fn animating(&self) -> bool {
        self.effects.any()
    }

    /// The well flashes for the game's end: its card waits.
    pub(crate) fn ending(&self) -> bool {
        self.effects.ended.is_some()
    }

    /// Versus: the other side's attack, queued to rise; the meter pulses.
    pub(crate) fn receive(&mut self, attack: Attack) {
        if let Some(garbage) = &mut self.versus {
            garbage.receive(attack);
            self.effects.arrived = Some(Fade::new((), FLASH));
        }
    }

    /// Versus: the other side topped out or left; the well flashes.
    pub(crate) fn won(&mut self) {
        self.effects.ended = Some(Fade::new(true, ENDING));
    }

    fn gravity(&self) -> u32 {
        GRAVITY.get(self.level as usize).copied().unwrap_or(1)
    }

    pub fn score(&self) -> u64 {
        self.score
    }

    pub fn over(&self) -> bool {
        self.over
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        vec![
            ("Level", self.level.to_string()),
            ("Lines", self.lines.to_string()),
        ]
    }

    /// The well with the piece falling in it, as versus shows it to the
    /// other side: a byte a cell, rows top first, 0 empty and else a
    /// block's [`Piece`] as `1 + its place` in [`BLOCKS`].
    pub fn cells(&self) -> Vec<u8> {
        let mut well = self.well;
        if !self.over {
            for (x, y) in self.falling() {
                well[y as usize][x as usize] = Some(self.piece);
            }
        }
        well.iter()
            .flatten()
            .map(|cell| cell.map_or(0, |piece| piece as u8 + 1))
            .collect()
    }

    /// The well, `cell` px a cell: the settled blocks, where the piece
    /// would land, and the piece; over them the rows a clear took flashing
    /// out where they were, a band that widens as it fades, the rows that
    /// rose in red, and at the game's end the whole well.
    pub(crate) fn draw(&self, cell: f32, theme: &Theme) -> Canvas {
        let block = |x: i32, y: i32| {
            square(
                x as f32 * cell + 1.,
                y as f32 * cell + 1.,
                cell - 2.,
                cell - 2.,
            )
        };
        let mut drawn = draw_well(&self.well, cell, theme);
        if !self.over {
            let fall = self.landing() - self.at.1;
            for (x, y) in self.falling() {
                drawn = drawn.rect(block(x, y + fall), theme.border_strong);
            }
            let color = self.piece.color(theme);
            for (x, y) in self.falling() {
                drawn = drawn.rect(block(x, y), color);
            }
        }
        let (width, height) = (COLS as f32 * cell, ROWS as f32 * cell);
        let effects = &self.effects;
        if let Some(cleared) = &effects.cleared {
            let strength = cleared.strength();
            let spread = (1. - strength) * cell / 4.;
            // a band a run of rows next to each other
            for run in cleared.what.chunk_by(|above, below| above + 1 == *below) {
                let top = (run[0] as f32 * cell - spread).max(0.);
                let bottom = ((run[run.len() - 1] + 1) as f32 * cell + spread).min(height);
                drawn = drawn.rect(
                    square(0., top, width, bottom - top),
                    bright(theme).opacity(0.8 * strength),
                );
            }
        }
        if let Some(rose) = &effects.rose {
            let rows = f32::from(rose.what).min(ROWS as f32) * cell;
            drawn = drawn.rect(
                square(0., height - rows, width, rows),
                theme.danger.opacity(0.45 * rose.strength()),
            );
        }
        if let Some(ended) = &effects.ended {
            let color = if ended.what {
                theme.success
            } else {
                theme.danger
            };
            drawn = drawn.rect(
                square(0., 0., width, height),
                color.opacity(0.3 * ended.strength()),
            );
        }
        drawn
    }

    /// The pieces coming, `width` px wide beside a well of `cell` px: the
    /// first on top, each centred in its slot.
    pub(crate) fn draw_next(&self, width: f32, cell: f32, theme: &Theme) -> Canvas {
        let drawn = canvas().w(px(width)).h(px(queue(cell)));
        let mut top = 0.;
        (0..).zip(self.next).fold(drawn, |drawn, (at, coming)| {
            let size = ((if at == 0 { NEAR } else { FAR }) * cell).floor();
            let slot = square(0., top, width, 3. * size);
            top += 3. * size;
            piece(drawn, coming, size, slot, coming.color(theme))
        })
    }

    /// The piece in hold, as the next is drawn; faint once hold is used
    /// for the piece falling.
    pub(crate) fn draw_hold(&self, width: f32, cell: f32, theme: &Theme) -> Canvas {
        let size = (NEAR * cell).floor();
        let drawn = canvas().w(px(width)).h(px(3. * size));
        let Some(held) = self.hold else {
            return drawn;
        };
        let color = match self.held {
            true => theme.border_strong,
            false => held.color(theme),
        };
        piece(drawn, held, size, square(0., 0., width, 3. * size), color)
    }
}

/// The px the queue takes beside a well of `cell` px: a slot a piece, its
/// two rows and one between, at the piece's own size.
fn queue(cell: f32) -> f32 {
    3. * ((NEAR * cell).floor() + (NEXT - 1) as f32 * (FAR * cell).floor())
}

/// Every block a cell may hold, in [`Tetris::cells`]' order.
const BLOCKS: [Piece; 8] = [
    Piece::I,
    Piece::O,
    Piece::T,
    Piece::S,
    Piece::Z,
    Piece::J,
    Piece::L,
    Piece::Garbage,
];

/// The well [`Tetris::cells`] wrote, if `cells` is one: a byte a cell of
/// the well, each a block or 0.
pub(crate) fn read_cells(cells: &[u8]) -> Option<Well> {
    if cells.len() != (COLS * ROWS) as usize {
        return None;
    }
    let mut well = [[None; COLS as usize]; ROWS as usize];
    for (at, cell) in well.iter_mut().flatten().zip(cells) {
        *at = match cell {
            0 => None,
            block => Some(*BLOCKS.get(usize::from(block - 1))?),
        };
    }
    Some(well)
}

/// `well`'s settled blocks, `cell` px a cell, on the well's ground.
pub(crate) fn draw_well(well: &Well, cell: f32, theme: &Theme) -> Canvas {
    let (width, height) = (COLS as f32 * cell, ROWS as f32 * cell);
    let mut drawn = canvas()
        .w(px(width))
        .h(px(height))
        .rect(square(0., 0., width, height), theme.surface);
    for (y, row) in (0..).zip(well) {
        for (x, settled) in (0..).zip(row) {
            if let Some(piece) = settled {
                let at = square(
                    x as f32 * cell + 1.,
                    y as f32 * cell + 1.,
                    cell - 2.,
                    cell - 2.,
                );
                drawn = drawn.rect(at, piece.color(theme));
            }
        }
    }
    drawn
}

/// The least and the most px a cell of the well takes on screen.
const CELL_MIN: u16 = 16;
const CELL_MAX: u16 = 40;
/// The screen's px beside its cells: the least room at an edge and
/// between versus's two groups; a box's border, both sides; a box's label;
/// and under the well a gap, then a line for the garbage coming and one
/// for the keys.
const GUTTER: f32 = 16.;
const FRAME: f32 = 2.;
const LABEL: f32 = 20.;
const FOOT_GAP: f32 = 6.;
const LINE: f32 = 17.;
const FOOTER: f32 = FOOT_GAP + 2. * LINE;

/// Where the screen's parts lie in its room, in px from its top left.
pub(crate) struct Layout {
    /// A cell of the well.
    pub(crate) cell: f32,
    pub(crate) hold: Bounds<Pixels>,
    /// Under hold, down to the well's foot.
    pub(crate) stats: Bounds<Pixels>,
    /// Versus: the garbage meter, between hold and the well.
    pub(crate) meter: Option<Bounds<Pixels>>,
    /// The well in its frame.
    pub(crate) well: Bounds<Pixels>,
    pub(crate) next: Bounds<Pixels>,
    /// Under hold, the well and next: the garbage coming and the keys.
    pub(crate) footer: Bounds<Pixels>,
    /// Versus: the other side's name and well, at half the cell.
    pub(crate) theirs: Option<Bounds<Pixels>>,
}

/// The screen in a room `width` by `height`: the largest whole cell from
/// [`CELL_MIN`] to [`CELL_MAX`] at which the well's rows and the footer
/// fit the height, and hold, the well and next (and the other's well)
/// the width. Hold and next lie hard against the well, level with its
/// top; the group, or versus's two, spaced evenly across.
pub(crate) fn layout((width, height): (f32, f32), versus: bool) -> Layout {
    let half = |cell: f32| (cell / 2.).floor();
    let widths = |cell: f32| {
        let meter = if versus { half(cell) } else { 0. };
        let theirs = if versus {
            COLS as f32 * half(cell) + FRAME
        } else {
            0.
        };
        (
            meter,
            8. * cell + meter + COLS as f32 * cell + FRAME,
            theirs,
        )
    };
    let gaps = if versus { 3. } else { 2. };
    let fits = |cell: &f32| {
        let (_, own, theirs) = widths(*cell);
        own + theirs + gaps * GUTTER <= width
            && ROWS as f32 * cell + FRAME + FOOTER + 2. * GUTTER <= height
    };
    let cell = (CELL_MIN..=CELL_MAX)
        .rev()
        .map(f32::from)
        .find(fits)
        .unwrap_or(f32::from(CELL_MIN));
    let (meter, own, theirs) = widths(cell);
    let (side, tall) = (4. * cell, ROWS as f32 * cell + FRAME);
    let left = ((width - own - theirs) / gaps).floor().max(0.);
    let top = ((height - tall - FOOTER) / 2.).floor().max(0.);
    let hold = square(left, top, side, FRAME + LABEL + 3. * (NEAR * cell).floor());
    let below = top + f32::from(hold.size.height) + half(cell);
    let well = left + side + meter;
    Layout {
        cell,
        hold,
        stats: square(left, below, side, top + tall - below),
        meter: versus.then(|| square(left + side, top, meter, tall)),
        well: square(well, top, COLS as f32 * cell + FRAME, tall),
        next: square(
            well + COLS as f32 * cell + FRAME,
            top,
            side,
            FRAME + LABEL + queue(cell),
        ),
        footer: square(left, top + tall, own, FOOTER),
        theirs: versus.then(|| {
            let tall = LABEL + ROWS as f32 * half(cell) + FRAME;
            square(2. * left + own, top, theirs, tall)
        }),
    }
}

/// The screen in `room`, alone or in versus: hold and `stats` under it,
/// the well drawn in `board`, the element that takes the keys, with
/// `card` centred on it, the queue, and under them the keys; in versus
/// the garbage meter and `theirs`, the other side's name and well.
pub(crate) fn stage(
    room: (f32, f32),
    tetris: &Tetris,
    board: Stateful<Div>,
    card: Option<Div>,
    stats: Vec<Fact>,
    theirs: Option<(String, Option<&Well>)>,
    theme: &Theme,
) -> Div {
    let garbage = tetris.versus.as_ref();
    let at = layout(room, garbage.is_some());
    let cell = at.cell;
    let caps = |text: &str| {
        div()
            .text_size(design::text::CAPTION)
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.muted)
            .child(text.to_uppercase())
    };
    let boxed = |bounds: Bounds<Pixels>, name: &str, drawn: Canvas| {
        place(div(), bounds)
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .flex()
            .flex_col()
            .items_center()
            .child(caps(name).h(px(LABEL)).flex().items_center())
            .child(drawn)
    };
    let inside = f32::from(at.hold.size.width) - FRAME;
    let effects = &tetris.effects;
    // what the last clears were called, under the stats, fading
    let callouts = effects.callouts.iter().map(|callout| {
        let (size, color) = match callout.what.tone {
            Tone::Piece(shown) => (design::text::SECTION, shown.color(theme)),
            Tone::Quiet => (design::text::SECONDARY, theme.muted),
            Tone::Large => (design::text::TITLE, theme.warning),
        };
        div()
            .text_size(size)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color.opacity(callout.strength()))
            .whitespace_nowrap()
            .child(callout.what.text.clone())
    });
    let stats = place(div(), at.stats)
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap(design::space::SM)
        .children(stats.into_iter().map(|(name, value)| {
            div().flex().flex_col().child(caps(name)).child(
                design::mono(value)
                    .text_size(design::text::SECTION)
                    .font_weight(FontWeight::MEDIUM),
            )
        }))
        .child(
            div()
                .pt(design::space::SM)
                .flex()
                .flex_col()
                .children(callouts),
        );
    let queued = garbage.map_or(0, Garbage::queued);
    let meter = at.meter.map(|bounds| {
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let ground = canvas()
            .w(px(width))
            .h(px(height))
            .rect(square(0., 0., width, height), theme.surface);
        // a well's cell a row coming, from the floor up
        let row = |row: u32| {
            let y = height - 1. - row as f32 * cell;
            square(1., y + 1., width - 2., cell - 2.)
        };
        let mut drawn = (1..=queued.min(ROWS as u32))
            .fold(ground, |drawn, at| drawn.rect(row(at), theme.danger));
        // an attack that came in pulses the rows coming; rows a clear
        // cancelled fade out over them
        if let Some(arrived) = &effects.arrived {
            let pulse = bright(theme).opacity(0.6 * arrived.strength());
            drawn =
                (1..=queued.min(ROWS as u32)).fold(drawn, |drawn, at| drawn.rect(row(at), pulse));
        }
        if let Some(cancelled) = &effects.cancelled {
            let gone = theme.success.opacity(cancelled.strength());
            let rows = queued + 1..=(queued + u32::from(cancelled.what)).min(ROWS as u32);
            drawn = rows.fold(drawn, |drawn, at| drawn.rect(row(at), gone));
        }
        place(div(), bounds).child(drawn)
    });
    // the rows this well just sent, under the queue
    let sent = effects.sent.as_ref().map(|sent| {
        let next = edges(at.next);
        place(
            div(),
            square(
                next[0],
                next[3] + (cell / 2.).floor(),
                next[2] - next[0],
                2. * LINE,
            ),
        )
        .flex()
        .justify_center()
        .text_size(design::text::TITLE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.danger.opacity(sent.strength()))
        .child(format!("+{}", sent.what))
    });
    let struck = effects.struck.as_ref().map_or(theme.border, |struck| {
        theme.border.blend(theme.danger.opacity(struck.strength()))
    });
    let coming = (queued > 0).then(|| {
        div()
            .flex()
            .gap(design::space::XXS)
            .child(caps("Incoming").text_color(theme.danger))
            .child(
                design::mono(queued.to_string())
                    .text_size(design::text::CAPTION)
                    .text_color(theme.danger),
            )
    });
    let footer = place(div(), at.footer)
        .pt(px(FOOT_GAP))
        .flex()
        .flex_col()
        .child(div().h(px(LINE)).pl(at.hold.size.width).children(coming))
        .child(
            div().h(px(LINE)).flex().justify_center().child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(design::text::CAPTION)
                    .text_color(theme.muted)
                    .child(KEYS),
            ),
        );
    let theirs = at.theirs.zip(theirs).map(|(bounds, (name, well))| {
        let empty = [[None; COLS as usize]; ROWS as usize];
        place(div(), bounds)
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(LABEL))
                    .text_size(design::text::SECONDARY)
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(name.clone()),
            )
            .child(
                div()
                    .id("versus-theirs")
                    .border_1()
                    .border_color(struck)
                    .role(Role::Figure)
                    .aria_label(format!("{name}'s well"))
                    .child(draw_well(
                        well.unwrap_or(&empty),
                        (cell / 2.).floor(),
                        theme,
                    )),
            )
    });
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .bg(theme.background)
        .child(boxed(
            at.hold,
            "Hold",
            tetris.draw_hold(inside, cell, theme),
        ))
        .child(stats)
        .children(meter)
        .child(place(board, nudged(at.well, effects.nudge())).child(tetris.draw(cell, theme)))
        .child(boxed(
            at.next,
            "Next",
            tetris.draw_next(inside, cell, theme),
        ))
        .child(footer)
        .children(sent)
        .children(theirs)
        .children(card.map(|card| place(div(), at.well).child(card)))
}

/// What a flash is drawn in: the dark theme's light ink; on the light
/// theme, where nothing is lighter than the well, its gold.
fn bright(theme: &Theme) -> Hsla {
    if theme.dark {
        theme.foreground
    } else {
        theme.warning
    }
}

/// `bounds`' left, top, right and bottom.
fn edges(bounds: Bounds<Pixels>) -> [f32; 4] {
    let (x, y) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    [x, y, x + w, y + h]
}

/// `bounds` `up` px higher.
fn nudged(bounds: Bounds<Pixels>, up: f32) -> Bounds<Pixels> {
    let [left, top, right, bottom] = edges(bounds);
    square(left, top - up, right - left, bottom - top)
}

/// `element` at `bounds` in its parent.
fn place<E: Styled>(element: E, bounds: Bounds<Pixels>) -> E {
    element
        .absolute()
        .left(bounds.origin.x)
        .top(bounds.origin.y)
        .w(bounds.size.width)
        .h(bounds.size.height)
}

/// The key's move, if it is one.
fn read(key: &str) -> Option<Move> {
    Some(match key {
        "left" | "a" => Move::Left,
        "right" | "d" => Move::Right,
        "up" | "x" | "w" => Move::Turn,
        "z" => Move::TurnBack,
        "down" | "s" => Move::Down,
        "space" => Move::Drop,
        "c" | "shift" => Move::Hold,
        _ => return None,
    })
}

/// `shown` unturned in `color` on `drawn`, `cell` px a cell, centred in
/// `slot`.
fn piece(mut drawn: Canvas, shown: Piece, cell: f32, slot: Bounds<Pixels>, color: Hsla) -> Canvas {
    let cells = shown.cells(0);
    // the first cell and the px across, on one axis, centred in `room`
    let span = |axis: fn(&(i32, i32)) -> i32, room: Pixels| {
        let first = cells.iter().map(axis).min().unwrap_or(0);
        let last = cells.iter().map(axis).max().unwrap_or(0);
        let across = (last - first + 1) as f32 * cell;
        (first, ((f32::from(room) - across) / 2.).floor())
    };
    let (left, x) = span(|cell| cell.0, slot.size.width);
    let (top, y) = span(|cell| cell.1, slot.size.height);
    let (x, y) = (f32::from(slot.origin.x) + x, f32::from(slot.origin.y) + y);
    for (dx, dy) in cells {
        let at = square(
            x + (dx - left) as f32 * cell + 1.,
            y + (dy - top) as f32 * cell + 1.,
            cell - 2.,
            cell - 2.,
        );
        drawn = drawn.rect(at, color);
    }
    drawn
}

/// The hub's picture of the game: a well's last rows and a piece falling,
/// a letter a piece.
const PICTURE: [&str; 9] = [
    "...TTT....",
    "....T.....",
    "..........",
    "........OO",
    "I.......OO",
    "I.L..SS.ZZ",
    "I.L.SSJZZ.",
    "ILL.JJJ.SS",
    "IZZ..LJSS.",
];

/// A cell of [`PICTURE`], and its top left, in the middle of its
/// [`STILL`](crate::games::STILL).
const PICTURE_CELL: f32 = 11.;
const PICTURE_AT: (f32, f32) = (49., 11.);

/// The part of [`STILL`](crate::games::STILL) the picture draws on.
pub(crate) const DRAWN: Area = (
    PICTURE_AT.0,
    PICTURE_AT.1,
    PICTURE[0].len() as f32 * PICTURE_CELL,
    PICTURE.len() as f32 * PICTURE_CELL,
);

/// [`PICTURE`], where `still` puts it.
pub(crate) fn preview(still: Still, theme: &Theme) -> Canvas {
    let (cell, (left, top)) = (PICTURE_CELL, PICTURE_AT);
    let mut drawn = canvas().size_full();
    for (y, row) in PICTURE.iter().enumerate() {
        for (x, letter) in row.chars().enumerate() {
            let shown = match letter {
                'I' => Piece::I,
                'O' => Piece::O,
                'T' => Piece::T,
                'S' => Piece::S,
                'Z' => Piece::Z,
                'J' => Piece::J,
                'L' => Piece::L,
                _ => continue,
            };
            let at = still.square(
                left + x as f32 * cell + 1.,
                top + y as f32 * cell + 1.,
                cell - 2.,
                cell - 2.,
            );
            drawn = drawn.rect(at, shown.color(theme));
        }
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Tetris {
        /// `key` goes down and up before the next tick.
        fn tap(&mut self, key: &str) {
            self.key_down(key, false);
            self.key_up(key);
        }
    }

    /// A game with the well cleared and `piece` at the top.
    fn with(piece: Piece) -> Tetris {
        let mut tetris = Tetris::new(1);
        tetris.piece = piece;
        tetris.turn = 0;
        tetris.at = (3, 0);
        tetris
    }

    #[test]
    fn a_turn_against_a_wall_moves_the_piece_off_it() {
        let mut tetris = with(Piece::I);
        // upright, on the right wall: its cells are its box's third column
        tetris.turn = 1;
        tetris.at = (COLS - 3, 5);
        assert_eq!(tetris.falling().map(|(x, _)| x), [COLS - 1; 4]);
        tetris.tap("up");
        assert!(tetris.tick());
        assert_eq!(tetris.turn, 2);
        let mut columns = tetris.falling().map(|(x, _)| x);
        columns.sort();
        assert_eq!(
            columns,
            [COLS - 4, COLS - 3, COLS - 2, COLS - 1],
            "kicked one left"
        );
    }

    #[test]
    fn a_piece_lying_flat_on_the_floor_turns_upright_either_way() {
        let mut stuck = Vec::new();
        for shown in [Piece::T, Piece::S, Piece::Z, Piece::J, Piece::L] {
            for key in ["up", "z"] {
                let mut tetris = with(shown);
                // flat, its lower row on the floor
                tetris.at = (3, ROWS - 2);
                tetris.tap(key);
                tetris.tick();
                let rows = tetris.falling().map(|(_, y)| y);
                let upright = tetris.turn == if key == "up" { 1 } else { 3 }
                    && rows.iter().min() == Some(&(ROWS - 3))
                    && rows.iter().max() == Some(&(ROWS - 1));
                if !upright {
                    stuck.push(format!("{shown:?} {key}"));
                }
            }
        }
        assert!(stuck.is_empty(), "not upright on the floor: {stuck:?}");
    }

    #[test]
    fn an_i_lying_flat_on_the_floor_against_a_wall_turns_upright_on_it() {
        let mut tetris = with(Piece::I);
        tetris.at = (COLS - 4, ROWS - 2);
        tetris.tap("up");
        tetris.tick();
        assert_eq!(tetris.turn, 1);
        let mut cells = tetris.falling().to_vec();
        cells.sort();
        assert_eq!(
            cells,
            [
                (COLS - 1, ROWS - 4),
                (COLS - 1, ROWS - 3),
                (COLS - 1, ROWS - 2),
                (COLS - 1, ROWS - 1)
            ],
            "upright in the last column, on the floor"
        );
    }

    #[test]
    fn a_turn_with_no_room_on_either_side_stays_put() {
        let mut tetris = with(Piece::I);
        tetris.turn = 1;
        tetris.at = (2, 5);
        // walls of blocks two cells either side of the upright I's column
        for y in 0..ROWS as usize {
            tetris.well[y][3] = Some(Piece::O);
            tetris.well[y][5] = Some(Piece::O);
        }
        tetris.tap("up");
        tetris.tick();
        assert_eq!(tetris.turn, 1);
        assert_eq!(tetris.at, (2, 5));
    }

    #[test]
    fn every_turn_of_a_piece_is_four_cells_in_its_box() {
        for shown in PIECES {
            for turn in 0..4 {
                let cells = shown.cells(turn);
                assert!(
                    cells
                        .iter()
                        .all(|(x, y)| (0..4).contains(x) && (0..4).contains(y))
                );
                let mut unique = cells.to_vec();
                unique.sort();
                unique.dedup();
                assert_eq!(unique.len(), 4, "{shown:?} turn {turn}");
            }
            assert_eq!(shown.cells(4), shown.cells(0));
        }
    }

    #[test]
    fn a_row_falls_after_the_levels_count_of_ticks() {
        let mut tetris = with(Piece::T);
        for _ in 0..47 {
            assert!(!tetris.tick(), "nothing moves before the 48th tick");
        }
        assert_eq!(tetris.at, (3, 0));
        assert!(tetris.tick());
        assert_eq!(tetris.at, (3, 1));
        tetris.level = 29;
        assert!(tetris.tick(), "a row a tick from level 29");
    }

    #[test]
    fn a_full_row_clears_and_scores_a_single() {
        let mut tetris = with(Piece::I);
        // the bottom row full but for the four cells the I lies in
        for x in 4..COLS as usize {
            tetris.well[ROWS as usize - 1][x] = Some(Piece::O);
        }
        tetris.well[ROWS as usize - 2][9] = Some(Piece::Z);
        tetris.at = (0, 3);
        tetris.tap("space");
        tetris.tick();
        // the drop's 15 rows at 2 and a Single at level 0
        assert_eq!(tetris.score(), 2 * 15 + 100);
        assert_eq!(tetris.lines, 1);
        // the row above came down
        assert_eq!(tetris.well[ROWS as usize - 1][9], Some(Piece::Z));
        assert!(
            tetris.well[ROWS as usize - 1][..9]
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    fn four_rows_at_once_score_a_tetris_and_ten_rows_raise_the_level() {
        let mut tetris = with(Piece::I);
        tetris.lines = 8;
        tetris.level = 0;
        for y in ROWS as usize - 4..ROWS as usize {
            for x in 1..COLS as usize {
                tetris.well[y][x] = Some(Piece::O);
            }
        }
        // upright in the first column
        tetris.turn = 1;
        tetris.at = (-2, 0);
        tetris.tap("space");
        tetris.tick();
        // a Tetris that empties the well: an all clear too
        assert_eq!(tetris.score(), 2 * 16 + 800 + 2000);
        assert_eq!((tetris.lines, tetris.level), (12, 1));
        assert!(tetris.well.iter().flatten().all(Option::is_none));
    }

    #[test]
    fn a_soft_drop_scores_a_cell_and_on_the_floor_leaves_the_piece_to_its_delay() {
        let mut tetris = with(Piece::O);
        tetris.at = (3, ROWS - 3);
        tetris.tap("down");
        tetris.tick();
        assert_eq!((tetris.at.1, tetris.score()), (ROWS - 2, 1));
        tetris.tap("down");
        assert!(!tetris.tick(), "on the floor a soft drop does nothing");
        assert_eq!(tetris.score(), 1);
        for _ in 2..LOCK_DELAY {
            tetris.tick();
        }
        assert!(tetris.well[ROWS as usize - 1][4].is_some(), "locked");
        assert_eq!(tetris.at, (3, 0), "the next piece at the top");
    }

    #[test]
    fn a_landed_piece_locks_after_half_a_second_of_ticks() {
        assert_eq!(LOCK_DELAY, 31);
        let mut tetris = with(Piece::T);
        tetris.at = (3, ROWS - 2);
        for _ in 1..LOCK_DELAY {
            assert!(!tetris.tick(), "it lies there, not locked yet");
        }
        assert!(tetris.tick());
        assert!(tetris.well[ROWS as usize - 1][4].is_some(), "locked");
        assert_eq!(tetris.at, (3, 0), "the next piece at the top");
    }

    #[test]
    fn a_move_on_the_ground_starts_the_delay_again_15_times_and_the_16th_locks() {
        let mut tetris = with(Piece::O);
        tetris.at = (3, ROWS - 2);
        for step in 1..=RESETS {
            // all but the last tick of a delay, then a step aside
            for _ in 2..LOCK_DELAY {
                tetris.tick();
            }
            tetris.tap(if step % 2 == 1 { "left" } else { "right" });
            tetris.tick();
            assert!(
                tetris.well.iter().flatten().all(Option::is_none),
                "move {step} keeps it up"
            );
        }
        tetris.tap("left");
        tetris.tick();
        assert!(
            tetris.well[ROWS as usize - 1].iter().any(Option::is_some),
            "the 16th locks it"
        );
    }

    #[test]
    fn a_hard_drop_locks_at_once() {
        let mut tetris = with(Piece::O);
        tetris.tap("space");
        tetris.tick();
        assert!(tetris.well[ROWS as usize - 1][4].is_some());
        assert_eq!(tetris.at, (3, 0), "the next piece at the top");
    }

    #[test]
    fn a_piece_that_cannot_come_in_ends_the_game() {
        let mut tetris = with(Piece::O);
        // blocks where every piece comes in
        for y in 0..2 {
            for x in 3..7 {
                tetris.well[y][x] = Some(Piece::Z);
            }
        }
        tetris.at = (7, 0);
        tetris.tap("space");
        tetris.tick();
        assert!(tetris.well[ROWS as usize - 1][8].is_some(), "the O settled");
        assert!(tetris.over());
        assert!(!tetris.tick(), "an ended game does not move");
    }

    #[test]
    fn a_held_arrow_slides_the_piece_after_das_ticks_then_a_cell_every_arr() {
        let mut tetris = with(Piece::T);
        tetris.at = (5, 0);
        tetris.key_down("left", false);
        let mut columns = Vec::new();
        for _ in 0..DAS + 2 * ARR {
            tetris.tick();
            columns.push(tetris.at.0);
        }
        let mut slid = vec![4; DAS as usize - 1];
        slid.extend([3, 3, 2, 2, 1]);
        assert_eq!(columns, slid, "once at once, again at DAS, then every ARR");
        tetris.key_up("left");
        for _ in 0..DAS {
            tetris.tick();
        }
        assert_eq!(tetris.at.0, 1, "up, it stops");
    }

    #[test]
    fn the_keyboards_own_repeat_is_not_read() {
        let mut tetris = with(Piece::T);
        tetris.at = (5, 0);
        for key in ["left", "up", "z", "down", "space", "c"] {
            tetris.key_down(key, true);
        }
        for _ in 0..DAS + ARR {
            assert!(!tetris.tick());
        }
        assert_eq!((tetris.at, tetris.turn, tetris.hold), ((5, 0), 0, None));
    }

    #[test]
    fn the_newest_arrow_held_slides_and_up_it_gives_the_other_its_own_count() {
        let mut tetris = with(Piece::T);
        tetris.at = (4, 0);
        tetris.key_down("left", false);
        tetris.key_down("right", false);
        for _ in 0..DAS {
            tetris.tick();
        }
        assert_eq!(tetris.at.0, 5, "a cell left, one back, then right's slide");
        tetris.key_up("right");
        for _ in 1..DAS {
            tetris.tick();
        }
        assert_eq!(tetris.at.0, 5, "left counts its DAS again");
        tetris.tick();
        assert_eq!(tetris.at.0, 4);
    }

    #[test]
    fn a_held_down_drops_a_row_every_soft_ticks_and_scores_each() {
        let mut tetris = with(Piece::O);
        tetris.key_down("down", false);
        for _ in 0..5 * SOFT {
            tetris.tick();
        }
        assert_eq!(
            (tetris.at.1, tetris.score()),
            (6, 6),
            "one at once, then five"
        );
        tetris.key_up("down");
        for _ in 0..5 * SOFT {
            tetris.tick();
        }
        assert_eq!(tetris.at.1, 6, "up, it falls by gravity's count alone");
    }

    #[test]
    fn letting_go_stops_what_a_held_key_did() {
        let mut tetris = with(Piece::T);
        tetris.at = (5, 0);
        tetris.key_down("left", false);
        tetris.key_down("down", false);
        tetris.let_go();
        for _ in 0..DAS + 2 * ARR {
            tetris.tick();
        }
        assert_eq!(tetris.at, (4, 1), "the presses alone, read on the tick");
    }

    #[test]
    fn c_puts_the_piece_in_hold_once_a_piece_and_brings_it_back_unturned_at_the_top() {
        let mut tetris = with(Piece::T);
        let next = tetris.next[0];
        tetris.turn = 1;
        tetris.at = (5, 7);
        tetris.tap("c");
        assert!(tetris.tick());
        assert_eq!(
            (tetris.hold, tetris.piece, tetris.turn, tetris.at),
            (Some(Piece::T), next, 0, (3, 0)),
            "an empty hold takes the T and the next comes in"
        );
        tetris.tap("c");
        assert!(!tetris.tick(), "not twice a piece");
        assert_eq!((tetris.piece, tetris.hold), (next, Some(Piece::T)));
        tetris.tap("space");
        tetris.tick();
        let falling = tetris.piece;
        tetris.tap("c");
        tetris.tick();
        assert_eq!(
            (tetris.piece, tetris.hold, tetris.at),
            (Piece::T, Some(falling), (3, 0)),
            "a new piece holds again and takes the T back"
        );
    }

    #[test]
    fn the_queue_shows_five_coming_and_they_come_in_that_order() {
        let mut tetris = Tetris::new(9);
        let coming = tetris.next;
        assert_eq!(coming.len(), 5);
        let mut came = Vec::new();
        for _ in 0..5 {
            tetris.tap("space");
            tetris.tick();
            came.push(tetris.piece);
        }
        assert_eq!(came, coming);
        // one held comes from the first in line, and the line moves up
        let after = tetris.next;
        tetris.tap("c");
        tetris.tick();
        assert_eq!(tetris.piece, after[0]);
        assert_eq!(tetris.next[..4], after[1..]);
    }

    /// The attacks `garbage` made, emptied.
    fn sent(garbage: &mut Garbage) -> Vec<u8> {
        garbage
            .outgoing
            .drain(..)
            .map(|attack| attack.lines)
            .collect()
    }

    #[test]
    fn queued_garbage_rises_on_a_lock_that_clears_nothing_holes_and_all() {
        let mut tetris = Tetris::versus(1, 1);
        tetris.piece = Piece::O;
        tetris.at = (3, 0);
        let garbage = tetris.versus.as_mut().expect("versus");
        garbage
            .incoming
            .extend([Attack { lines: 1, hole: 2 }, Attack { lines: 2, hole: 7 }]);
        tetris.tap("space");
        tetris.tick();
        let rows = |tetris: &Tetris, y: usize| {
            tetris.well[y]
                .iter()
                .map(|cell| match cell {
                    None => '.',
                    Some(Piece::Garbage) => 'G',
                    Some(_) => 'O',
                })
                .collect::<String>()
        };
        let bottom: Vec<String> = (ROWS as usize - 5..ROWS as usize)
            .map(|y| rows(&tetris, y))
            .collect();
        assert_eq!(
            bottom,
            [
                "....OO....",
                "....OO....",
                "GG.GGGGGGG",
                "GGGGGGG.GG",
                "GGGGGGG.GG",
            ],
            "the O pushed up three, the oldest row highest"
        );
        assert!(tetris.versus.as_ref().expect("versus").incoming.is_empty());
        assert!(!tetris.over());
    }

    #[test]
    fn garbage_that_pushes_a_block_out_of_the_top_ends_the_game() {
        let mut tetris = Tetris::versus(1, 1);
        tetris.piece = Piece::O;
        tetris.at = (3, 0);
        // a block in the top row, away from where pieces come in
        tetris.well[0][0] = Some(Piece::I);
        tetris.well[1][0] = Some(Piece::I);
        let garbage = tetris.versus.as_mut().expect("versus");
        garbage.incoming.push_back(Attack { lines: 1, hole: 0 });
        tetris.tap("space");
        tetris.tick();
        assert!(tetris.over());
        assert!(!tetris.tick(), "an ended game does not move");
    }

    #[test]
    fn two_versus_wells_of_one_seed_deal_alike_whatever_garbage_they_trade() {
        let (mut ours, mut theirs) = (Tetris::versus(42, 1), Tetris::versus(42, 2));
        let mut dealt = (Vec::new(), Vec::new());
        for drop in 0..21 {
            // ours takes a row of garbage, which rises on the drop, or
            // attacks, by turns; theirs does neither
            let garbage = ours.versus.as_mut().expect("versus");
            if drop % 2 == 0 {
                garbage.incoming.push_back(Attack { lines: 1, hole: 3 });
            } else {
                let mut triple = Clear {
                    rows: 3,
                    ..Clear::default()
                };
                garbage.trade(&mut triple, &mut ours.well);
                assert_eq!(sent(garbage), [2]);
            }
            for (tetris, dealt) in [(&mut ours, &mut dealt.0), (&mut theirs, &mut dealt.1)] {
                tetris.well = [[None; COLS as usize]; ROWS as usize];
                tetris.tap("space");
                tetris.tick();
                dealt.push(tetris.piece);
            }
        }
        assert!(!ours.over() && !theirs.over());
        assert_eq!(dealt.0, dealt.1);
    }

    #[test]
    fn a_well_read_back_from_its_cells_is_the_well_with_the_piece_in_it() {
        let mut tetris = Tetris::versus(3, 3);
        tetris.well[ROWS as usize - 1] = [Some(Piece::Garbage); COLS as usize];
        tetris.well[ROWS as usize - 1][4] = None;
        tetris.well[ROWS as usize - 2][0] = Some(Piece::Z);
        let cells = tetris.cells();
        assert_eq!(cells.len(), 200, "a byte a cell");
        let mut shown = tetris.well;
        for (x, y) in tetris.falling() {
            shown[y as usize][x as usize] = Some(tetris.piece);
        }
        assert_eq!(read_cells(&cells), Some(shown));
        assert_eq!(read_cells(&cells[1..]), None, "a short well is no well");
        let mut past = cells.clone();
        past[0] = 9;
        assert_eq!(read_cells(&past), None, "no block is 9");
    }

    #[test]
    fn the_screen_takes_one_cell_from_the_room_and_lays_its_parts_apart_inside_it() {
        // the three windows as the view's pane, less the bar: the cell,
        // the group's left edge and top, and versus's other well's left
        // edge; then the panes the app leaves the view in those windows
        // (its sidebar and title bar off; 700 wide leaves 480, the least
        // the view asks for), one between, and the tests' own pane
        let placed = [
            ((1600., 956.), false, Some((40., 439., 57., None))),
            ((1600., 956.), true, Some((40., 218., 57., Some(1178.)))),
            ((1000., 656.), false, Some((29., 238., 17., None))),
            ((1000., 656.), true, Some((29., 106., 17., Some(750.)))),
            ((700., 856.), false, Some((37., 16., 37., None))),
            ((700., 856.), true, Some((27., 22., 137., Some(545.)))),
        ];
        let panes = [
            (1380., 920.),
            (780., 620.),
            (560., 820.),
            (480., 820.),
            (766., 457.),
        ]
        .into_iter()
        .flat_map(|room| [(room, false, None), (room, true, None)]);
        for (room, versus, place) in placed.into_iter().chain(panes) {
            let (width, height) = room;
            let case = format!("{width} x {height}, versus {versus}");
            let at = layout(room, versus);
            let cell = at.cell;
            let [left, top, ..] = edges(at.hold);
            if let Some(place) = place {
                let theirs = at.theirs.map(|theirs| edges(theirs)[0]);
                assert_eq!((cell, left, top, theirs), place, "{case}");
            }
            assert!(
                (f32::from(CELL_MIN)..=f32::from(CELL_MAX)).contains(&cell),
                "{case}"
            );
            let mut parts = vec![at.hold, at.stats, at.well, at.next, at.footer];
            parts.extend(at.meter.into_iter().chain(at.theirs));
            assert_eq!(parts.len(), if versus { 7 } else { 5 }, "{case}");
            for (n, part) in parts.iter().enumerate() {
                let [l, t, r, b] = edges(*part);
                assert!(
                    l >= 0. && t >= 0. && r <= width && b <= height,
                    "{case}: {part:?} leaves the room"
                );
                for other in &parts[n + 1..] {
                    assert!(!part.intersects(other), "{case}: {part:?} on {other:?}");
                }
            }
            // the well is ten by twenty cells in its frame; hold, or in
            // versus the meter, and next lie against it, level with its top
            let [wl, wt, wr, wb] = edges(at.well);
            assert_eq!(
                (wr - wl, wb - wt),
                (10. * cell + 2., 20. * cell + 2.),
                "{case}"
            );
            let [_, hold_top, hold_right, hold_bottom] = edges(at.hold);
            let [next_left, next_top, next_right, _] = edges(at.next);
            assert_eq!((hold_top, next_top, next_left), (wt, wt, wr), "{case}");
            match at.meter.map(edges) {
                Some([ml, mt, mr, mb]) => {
                    assert_eq!((ml, mr, mt, mb), (hold_right, wl, wt, wb), "{case}")
                }
                None => assert_eq!(hold_right, wl, "{case}"),
            }
            // under hold, room for four stats; under it all, the footer
            let [stats_left, stats_top, _, stats_bottom] = edges(at.stats);
            assert!(stats_left == left && stats_top > hold_bottom, "{case}");
            assert!(stats_bottom - stats_top >= 4. * 40., "{case}");
            let [fl, ft, fr, fb] = edges(at.footer);
            assert_eq!((fl, ft, fr), (left, wb, next_right), "{case}");
            // even margins: above and below, and either side of each group
            assert!(top >= GUTTER && (height - fb - top).abs() <= 1., "{case}");
            let gaps = match at.theirs.map(edges) {
                Some([tl, tt, tr, tb]) => {
                    let half = (cell / 2.).floor();
                    assert_eq!(
                        (tr - tl, tb - tt, tt),
                        (10. * half + 2., LABEL + 20. * half + 2., wt),
                        "{case}: the other well at half the cell, level with ours"
                    );
                    vec![left, tl - next_right, width - tr]
                }
                None => vec![left, width - next_right],
            };
            let (least, most) = gaps
                .iter()
                .fold((f32::MAX, 0f32), |(l, m), gap| (l.min(*gap), m.max(*gap)));
            assert!(least >= GUTTER && most - least <= 2., "{case}: {gaps:?}");
        }
    }

    /// A well of `rows`, its bottom rows as text, `#` a block.
    fn well_of(rows: &[&str]) -> Well {
        let mut well = [[None; COLS as usize]; ROWS as usize];
        for (row, text) in well[ROWS as usize - rows.len()..].iter_mut().zip(rows) {
            for (cell, at) in row.iter_mut().zip(text.chars()) {
                if at == '#' {
                    *cell = Some(Piece::O);
                }
            }
        }
        well
    }

    /// A piece, its turn and its box's top left corner.
    type Placed = (Piece, u8, (i32, i32));

    /// The I upright in the first column, at the top.
    const I_UP: Placed = (Piece::I, 1, (-2, 0));
    /// The O at the top, which falls to the floor of an empty well.
    const O_TOP: Placed = (Piece::O, 0, (3, 0));
    /// Four rows the I upright clears, a block over them to keep the well
    /// from emptying.
    const TETRIS: [&str; 5] = [
        ".........#",
        ".#########",
        ".#########",
        ".#########",
        ".#########",
    ];
    const SINGLE: [&str; 1] = [".#########"];
    /// A T-spin Double's slot under an overhang: the T upright beside it,
    /// pointing left, turns back into it pointing down, three corners
    /// filled and both of those it points to.
    const TSD: [&str; 3] = [".....#....", "###...####", "####.#####"];
    const TSD_T: Placed = (Piece::T, 3, (3, 17));
    /// The same slot over rows that do not fill: a T-spin with no rows.
    const TSZ: [&str; 3] = [".....#....", "###...###.", "####.####."];
    /// A mini: the T flat in the corner turns upright against the wall on
    /// SRS's second kick; of the two corners it points to one is filled.
    const MINI: [&str; 2] = ["...#######", ".#########"];
    const MINI_T: Placed = (Piece::T, 0, (0, 17));

    /// `placed` in a well of `rows`, `keys` tapped a tick each, then
    /// dropped: the lock's record.
    fn lock_in(tetris: &mut Tetris, rows: &[&str], placed: Placed, keys: &[&str]) -> Clear {
        let (piece, turn, at) = placed;
        tetris.well = well_of(rows);
        (tetris.piece, tetris.turn, tetris.at, tetris.turned) = (piece, turn, at, None);
        tetris.last = None;
        for key in keys.iter().chain(&["space"]) {
            tetris.tap(key);
            tetris.tick();
        }
        tetris.last.expect("a lock")
    }

    #[test]
    fn a_t_turned_into_a_slot_with_three_corners_filled_is_a_t_spin_double() {
        let mut tetris = with(Piece::T);
        let clear = lock_in(&mut tetris, &TSD, TSD_T, &["z"]);
        assert_eq!((clear.rows, clear.spin), (2, Some(Spin::Full)));
        assert_eq!(clear.attack(), 4);
        assert_eq!(tetris.score(), 1200, "at level 1, its drop moved nothing");
        // set in the slot, not turned into it: a Double
        let mut set = with(Piece::T);
        let clear = lock_in(&mut set, &TSD, (Piece::T, 2, (3, 17)), &[]);
        assert_eq!((clear.rows, clear.spin, clear.attack()), (2, None, 1));
        assert_eq!(set.score(), 300);
    }

    #[test]
    fn only_a_t_turned_with_three_corners_filled_spins() {
        let spin = |rows: &[&str], placed: Placed| {
            let mut tetris = with(Piece::T);
            lock_in(&mut tetris, rows, placed, &["up"])
        };
        // the T turned upright where it locks, the corner over its left
        // gone: two corners filled
        let two = spin(
            &["..........", "..........", "#.########"],
            (Piece::T, 0, (0, 17)),
        );
        assert_eq!((two.rows, two.spin), (1, None));
        // a J turned where a T would make a mini
        let j = spin(&MINI, (Piece::J, 0, (0, 17)));
        assert_eq!((j.rows, j.spin), (1, None));
    }

    #[test]
    fn a_t_spin_with_one_of_the_corners_it_points_to_filled_is_a_mini() {
        let mut tetris = with(Piece::T);
        let clear = lock_in(&mut tetris, &MINI, MINI_T, &["up"]);
        assert_eq!((clear.rows, clear.spin), (1, Some(Spin::Mini)));
        assert_eq!(clear.attack(), 0);
        assert_eq!(tetris.score(), 200);
        // on the floor, which fills the two corners behind a T pointing up:
        // turned back from upright, it kicks a cell right and one down
        let floor = ["#..#......", "#...######"];
        let mut tetris = with(Piece::T);
        let clear = lock_in(&mut tetris, &floor, (Piece::T, 1, (0, 17)), &["z"]);
        assert_eq!((clear.rows, clear.spin), (1, Some(Spin::Mini)));
    }

    #[test]
    fn a_turn_onto_srs_last_kick_makes_the_t_spin_full_with_one_corner_it_points_to() {
        // the T flat over a shaft at the wall: only the last kick, one
        // left and two down, takes it in, upright, pointing right
        let rows = [
            "#.........",
            "..........",
            ".#########",
            "..########",
            "..########",
        ];
        let mut tetris = with(Piece::T);
        tetris.well = well_of(&rows);
        (tetris.turn, tetris.at) = (0, (0, 15));
        tetris.tap("up");
        tetris.tick();
        assert_eq!(
            (tetris.turn, tetris.at, tetris.turned),
            (1, (-1, 17), Some(LAST_KICK))
        );
        tetris.tap("space");
        tetris.tick();
        let clear = tetris.last.expect("a lock");
        assert_eq!(
            (clear.rows, clear.spin, clear.attack()),
            (2, Some(Spin::Full), 4)
        );
    }

    #[test]
    fn a_turn_counts_as_the_last_move_until_a_slide_a_row_of_fall_or_a_drop_moves_the_t() {
        // upright against the wall at the floor: three corners filled, the
        // wall's two and one of the two it points to
        let rows = ["#.........", "..........", "#.########"];
        let spin = |placed: Placed, keys: &[&str]| {
            let mut tetris = with(Piece::T);
            lock_in(&mut tetris, &rows, placed, keys).spin
        };
        assert_eq!(
            spin((Piece::T, 0, (0, 17)), &["up"]),
            Some(Spin::Mini),
            "turned where it locks"
        );
        assert_eq!(
            spin((Piece::T, 0, (0, 15)), &["up", "down", "down"]),
            None,
            "soft dropped"
        );
        assert_eq!(
            spin((Piece::T, 0, (0, 15)), &["up"]),
            None,
            "hard dropped two rows"
        );
        // turned, then slid in the open
        let mut slid = with(Piece::T);
        slid.at = (4, 5);
        slid.tap("up");
        slid.tick();
        assert_eq!(slid.turned, Some(0));
        slid.tap("left");
        slid.tick();
        assert_eq!(slid.turned, None, "slid");
        // turned, then two rows of gravity's
        let mut tetris = with(Piece::T);
        tetris.well = well_of(&rows);
        (tetris.turn, tetris.at) = (0, (0, 15));
        tetris.tap("up");
        while tetris.last.is_none() {
            tetris.tick();
        }
        assert_eq!(
            tetris.last.map(|clear| (clear.rows, clear.spin)),
            Some((1, None))
        );
    }

    #[test]
    fn locks_clearing_rows_one_after_another_add_the_combo_table_and_one_clearing_nothing_ends_it()
    {
        let mut tetris = Tetris::versus(1, 1);
        let mut sent = Vec::new();
        for _ in 0..13 {
            sent.push(lock_in(&mut tetris, &SINGLE, I_UP, &[]).sent);
        }
        assert_eq!(
            sent,
            [0, 1, 1, 2, 2, 3, 3, 4, 4, 4, 5, 5, 5],
            "Singles, combo 0 to 12"
        );
        let score = tetris.score();
        // a T-spin with no rows ends it too
        lock_in(&mut tetris, &TSZ, TSD_T, &["z"]);
        assert_eq!(tetris.score() - score, 400);
        let clear = lock_in(&mut tetris, &SINGLE, I_UP, &[]);
        assert_eq!((clear.combo, clear.sent), (0, 0));
        let score = tetris.score();
        let clear = lock_in(&mut tetris, &SINGLE, I_UP, &[]);
        assert_eq!((clear.combo, clear.sent), (1, 1));
        assert_eq!(
            tetris.score() - score,
            100 + 50 + 2 * 16,
            "a Single, combo 1, the drop"
        );
    }

    #[test]
    fn tetrises_and_t_spins_clearing_rows_chain_back_to_back_and_only_another_clear_breaks_it() {
        let mut tetris = Tetris::versus(1, 1);
        // a lock that clears nothing between two clears keeps the combo out
        let mut lock = |rows: &[&str], placed, keys: &[&str]| {
            let clear = lock_in(&mut tetris, rows, placed, keys);
            lock_in(&mut tetris, &[], O_TOP, &[]);
            (clear.back_to_back, clear.sent)
        };
        let chain = [
            lock(&TETRIS, I_UP, &[]),
            lock(&TETRIS, I_UP, &[]),
            lock(&SINGLE, I_UP, &[]),
            lock(&TETRIS, I_UP, &[]),
            lock(&TSD, TSD_T, &["z"]),
            lock(&TSZ, TSD_T, &["z"]),
            lock(&MINI, MINI_T, &["up"]),
            lock(&TETRIS, I_UP, &[]),
        ];
        assert_eq!(
            chain,
            [
                (false, 4),
                (true, 5),
                (false, 0),
                (false, 4),
                (true, 5),
                (false, 0),
                (true, 1),
                (true, 5),
            ],
            "a Single breaks the chain; a T-spin Double goes on from a Tetris; a T-spin with no rows, as a lock clearing nothing, keeps it; a mini goes on with it"
        );
        let score = tetris.score();
        lock_in(&mut tetris, &TSD, TSD_T, &["z"]);
        assert_eq!(
            tetris.score() - score,
            1200 * 3 / 2,
            "a T-spin Double back to back"
        );
    }

    #[test]
    fn an_all_clear_sends_four_more_and_scores_its_bonus_by_the_rows() {
        let mut tetris = with(Piece::I);
        let clear = lock_in(&mut tetris, &TETRIS[1..], I_UP, &[]);
        assert_eq!((clear.perfect, clear.attack()), (true, 8));
        assert_eq!(tetris.score(), 2 * 16 + 800 + 2000);
        lock_in(&mut tetris, &[], O_TOP, &[]);
        let score = tetris.score();
        let clear = lock_in(&mut tetris, &TETRIS[1..], I_UP, &[]);
        assert_eq!(
            (clear.perfect, clear.back_to_back, clear.attack()),
            (true, true, 9)
        );
        assert_eq!(tetris.score() - score, 2 * 16 + 800 * 3 / 2 + 3200);
        // a block left over: no all clear
        let clear = lock_in(&mut tetris, &TETRIS, I_UP, &[]);
        assert!(!clear.perfect);
        let clear = lock_in(&mut tetris, &SINGLE[..], I_UP, &[]);
        assert!(!clear.perfect, "the I's three cells are left");
    }

    /// `tetris`' well's row `y` as text: `.` empty, `G` garbage, `#` a
    /// piece's block.
    fn text(tetris: &Tetris, y: usize) -> String {
        tetris.well[y]
            .iter()
            .map(|cell| match cell {
                None => '.',
                Some(Piece::Garbage) => 'G',
                Some(_) => '#',
            })
            .collect()
    }

    #[test]
    fn a_lock_that_clears_nothing_raises_at_most_eight_queued_rows_oldest_first() {
        let mut tetris = Tetris::versus(1, 1);
        tetris.versus.as_mut().expect("versus").incoming.extend([
            Attack { lines: 5, hole: 1 },
            Attack { lines: 4, hole: 2 },
            Attack { lines: 2, hole: 3 },
        ]);
        let clear = lock_in(&mut tetris, &[], O_TOP, &[]);
        assert_eq!(clear.rose, 8);
        let bottom: Vec<String> = (ROWS as usize - 10..ROWS as usize)
            .map(|y| text(&tetris, y))
            .collect();
        let mut rows = vec!["....##....".to_owned(); 2];
        rows.extend(vec!["G.GGGGGGGG".to_owned(); 5]);
        rows.extend(vec!["GG.GGGGGGG".to_owned(); 3]);
        assert_eq!(
            bottom, rows,
            "five of the first attack, three of the second"
        );
        let garbage = tetris.versus.as_ref().expect("versus");
        assert_eq!(
            garbage.incoming,
            [Attack { lines: 1, hole: 2 }, Attack { lines: 2, hole: 3 }]
        );
        assert_eq!(
            lock_in(&mut tetris, &[], O_TOP, &[]).rose,
            3,
            "the rest, on the next"
        );
    }

    #[test]
    fn what_a_lock_sends_cancels_the_garbage_queued_first() {
        let mut tetris = Tetris::versus(1, 1);
        let garbage = tetris.versus.as_mut().expect("versus");
        garbage.receive(Attack { lines: 3, hole: 0 });
        garbage.receive(Attack { lines: 2, hole: 9 });
        let clear = lock_in(&mut tetris, &TSD, TSD_T, &["z"]);
        assert_eq!(
            (clear.cancelled, clear.sent),
            (4, 0),
            "a T-spin Double's four"
        );
        let garbage = tetris.versus.as_mut().expect("versus");
        assert!(sent(garbage).is_empty());
        assert_eq!(garbage.incoming, [Attack { lines: 1, hole: 9 }]);
        // back to back at combo 1: four, one and one
        let clear = lock_in(&mut tetris, &TETRIS, I_UP, &[]);
        assert_eq!((clear.cancelled, clear.sent), (1, 5));
        let garbage = tetris.versus.as_mut().expect("versus");
        assert_eq!(sent(garbage), [5]);
        assert!(garbage.incoming.is_empty());
        assert_eq!(
            (garbage.sent, garbage.received),
            (5, 5),
            "sent counts what left, received what came, cancelled or not"
        );
    }

    #[test]
    fn versus_wells_fall_alike_at_equal_ticks_whatever_each_cleared() {
        let (mut ours, mut theirs) = (Tetris::versus(5, 1), Tetris::versus(5, 2));
        // ours clears forty rows, a Tetris a tick; theirs plays nothing
        for _ in 0..10 {
            lock_in(&mut ours, &TETRIS, I_UP, &[]);
            theirs.tick();
            assert_eq!(
                (ours.level, ours.gravity()),
                (theirs.level, theirs.gravity())
            );
        }
        assert_eq!((ours.lines, theirs.lines), (40, 0));
        while ours.ticks < MINUTE - 1 {
            ours.tick();
            theirs.tick();
        }
        assert_eq!((ours.level, theirs.level), (0, 0), "a minute less a tick");
        ours.tick();
        theirs.tick();
        assert_eq!((ours.level, theirs.level), (1, 1), "a minute");
        assert_eq!((ours.gravity(), theirs.gravity()), (GRAVITY[1], GRAVITY[1]));
        // alone, ten rows are a level
        let mut alone = with(Piece::I);
        lock_in(&mut alone, &TETRIS, I_UP, &[]);
        lock_in(&mut alone, &TETRIS, I_UP, &[]);
        lock_in(&mut alone, &TETRIS, I_UP, &[]);
        assert_eq!(alone.level, 1);
    }

    #[test]
    fn the_largest_attack_a_lock_sends_is_fair() {
        let most = Clear {
            rows: 3,
            spin: Some(Spin::Full),
            back_to_back: true,
            combo: 40,
            perfect: true,
            ..Clear::default()
        };
        assert_eq!((most.attack(), MOST_SENT), (16, 16));
        assert!(
            Attack {
                lines: MOST_SENT,
                hole: 0
            }
            .fair()
        );
        assert!(
            !Attack {
                lines: MOST_SENT + 1,
                hole: 0
            }
            .fair()
        );
    }

    /// The callouts' lines, the newest first.
    fn callouts(tetris: &Tetris) -> Vec<&str> {
        tetris
            .effects
            .callouts
            .iter()
            .map(|callout| callout.what.text.as_str())
            .collect()
    }

    #[test]
    fn a_clear_flashes_its_rows_and_calls_what_it_was_and_both_fade_out() {
        let mut tetris = Tetris::versus(1, 1);
        lock_in(&mut tetris, &TETRIS, I_UP, &[]);
        assert_eq!(callouts(&tetris), ["TETRIS"]);
        assert_eq!(
            tetris
                .effects
                .cleared
                .as_ref()
                .map(|cleared| cleared.what.clone()),
            Some(vec![16, 17, 18, 19])
        );
        lock_in(&mut tetris, &TSD, TSD_T, &["z"]);
        assert_eq!(
            callouts(&tetris),
            ["T-SPIN DOUBLE", "BACK-TO-BACK", "1 COMBO", "TETRIS"],
            "the newest on top"
        );
        assert_eq!(
            tetris
                .effects
                .cleared
                .as_ref()
                .map(|cleared| cleared.what.clone()),
            Some(vec![18, 19]),
            "where they were"
        );
        lock_in(&mut tetris, &MINI, MINI_T, &["up"]);
        lock_in(&mut tetris, &TETRIS[1..], I_UP, &[]);
        assert_eq!(
            callouts(&tetris),
            [
                "TETRIS",
                "BACK-TO-BACK",
                "3 COMBO",
                "ALL CLEAR",
                "T-SPIN MINI SINGLE",
                "BACK-TO-BACK"
            ],
            "six lines at most"
        );
        assert_eq!(
            tetris.effects.callouts[3].what.tone,
            Tone::Large,
            "the all clear's large"
        );
        // a lock that clears nothing calls nothing; each frame fades
        lock_in(&mut tetris, &[], O_TOP, &[]);
        assert_eq!(callouts(&tetris).len(), 6);
        for _ in 0..CALLOUT {
            assert!(tetris.fade(), "showing");
        }
        assert!(!tetris.animating());
        assert!(!tetris.fade(), "nothing left to change a frame");
    }

    #[test]
    fn garbage_rising_flashes_red_and_nudges_the_well_up_then_back() {
        let mut tetris = Tetris::versus(1, 1);
        tetris.receive(Attack { lines: 2, hole: 0 });
        assert!(tetris.effects.arrived.is_some(), "the meter pulses");
        let clear = lock_in(&mut tetris, &[], O_TOP, &[]);
        assert_eq!(clear.rose, 2);
        let mut nudges = vec![tetris.effects.nudge()];
        for _ in 0..NUDGE {
            tetris.fade();
            nudges.push(tetris.effects.nudge());
        }
        assert_eq!(nudges, [3., 3., 2., 2., 2., 1., 1., 0., 0.]);
        for _ in NUDGE..FLASH - 1 {
            assert!(tetris.effects.rose.is_some(), "the red outlasts the nudge");
            tetris.fade();
            assert_eq!(tetris.effects.nudge(), 0.);
        }
        tetris.fade();
        assert!(tetris.effects.rose.is_none());
    }

    #[test]
    fn a_clear_that_cancels_and_sends_flashes_the_meter_and_shows_what_went() {
        let mut tetris = Tetris::versus(1, 1);
        tetris.receive(Attack { lines: 1, hole: 0 });
        lock_in(&mut tetris, &TSD, TSD_T, &["z"]);
        let effects = &tetris.effects;
        assert_eq!(effects.cancelled.as_ref().map(|fade| fade.what), Some(1));
        assert_eq!(effects.sent.as_ref().map(|fade| fade.what), Some(3));
        assert!(effects.struck.is_some(), "the other's frame flashes");
        for _ in 0..FLASH {
            tetris.fade();
        }
        let effects = &tetris.effects;
        assert!(effects.cancelled.is_none() && effects.struck.is_none());
        assert!(effects.sent.is_some(), "the +3 stays a second");
    }

    #[test]
    fn the_bag_deals_each_piece_once_in_seven() {
        let mut tetris = Tetris::new(42);
        tetris.bag.clear();
        let mut dealt: Vec<Piece> = (0..7).map(|_| tetris.draw_bag()).collect();
        dealt.sort_by_key(|shown| *shown as u8);
        assert_eq!(dealt, PIECES);
        let deal = |seed| {
            let mut tetris = Tetris::new(seed);
            (0..14).map(|_| tetris.draw_bag()).collect::<Vec<_>>()
        };
        assert_eq!(deal(7), deal(7), "one seed, one deal");
        assert_ne!(deal(7), deal(8));
    }
}
