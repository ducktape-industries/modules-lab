//! Space Invaders, as the 1978 arcade plays it. Five rows of eleven
//! invaders march sideways and step down 8 px at a side, one invader a tick
//! in marching order: with all 55 standing each moves 2 px in 55 ticks, the
//! last one alone 2 px every tick. A squid of the top row is 30, a crab of
//! the next two 20 and an octopus of the bottom two 10, and the rack waits
//! while a hit one's blast shows. The cannon moves while an arrow is held,
//! the newest of two, and a press of Space fires its one shot; holding
//! Space fires no more, as in the arcade.
//!
//! Three kinds of invader shot fall, one of each at most, from the lowest
//! invader of a column: the rolling shot from the column over the cannon,
//! the plunger and the squiggly from one the seed picks. Another is fired
//! only once every shot falling is a distance on its way, the shorter the
//! higher the score (the arcade's reload table), and they fall faster with
//! eight or fewer invaders left. Four shields lose cells to shots from both
//! sides and to the invaders that reach them. The mystery ship crosses the
//! top every 20 to 30 s, from a side, both as the seed says, and is worth
//! what the arcade's table gives the count of shots fired: 50 to 300.
//!
//! A shot on the cannon costs one of three lives, and 1,500 points give
//! one more; invaders that reach the cannon's row end the game. A wave
//! shot down whole comes again lower, 56 px lower from the seventh on, and
//! the shields with it. The tick is 16 ms, the arcade's frame; shots and
//! the ship count in 1/16 px, so an invader shot falls 4 px in three ticks
//! as there, and the same keys play the same game.
use view::prelude::*;
use view::{Canvas, Div};

use crate::games::{Fact, Rng, square};

/// The field, in px at a scale of one.
pub(crate) const WIDTH: i32 = 224;
pub(crate) const HEIGHT: i32 = 216;
pub(crate) const KEYS: &str = "← → move the cannon, Space fires";
pub(crate) const PERIOD: i64 = 16;

const COLS: usize = 11;
const ROWS: usize = 5;
const SLOTS: usize = COLS * ROWS;
/// An invader's room in the rack, and its height, in px.
const SLOT: i32 = 16;
const TALL: i32 = 8;
/// The first wave's top left room, and how much lower each wave starts:
/// the arcade's table, its last for the seventh wave on.
const RACK: (i32, i32) = (24, 32);
const LOWER: [i32; 7] = [0, 24, 40, 48, 48, 48, 56];
/// The rack's step sideways and down, in px.
const MARCH: i32 = 2;
const DROP: i32 = 8;
const SHIP_Y: i32 = 8;
const SHIELD_Y: i32 = 160;
const CANNON_Y: i32 = 200;
const GROUND_Y: i32 = 212;
/// The cannon's middle where it starts, and the most it goes to a side.
const CANNON_START: i32 = 30;
const CANNON_REACH: (i32, i32) = (14, WIDTH - 14);
const SHOT_H: i32 = 4;
const BOLT_W: i32 = 3;
const BOLT_H: i32 = 7;
/// Sixteenths of a px: what shots and the ship move in.
const SUB: i32 = 16;
/// The cannon's shot climbs 4 px a tick; an invader's falls 4 px in three
/// ticks, 5 once eight or fewer invaders are left.
const CLIMB: i32 = 4 * SUB;
const FALL: i32 = 4 * SUB / 3;
const FALL_LATE: i32 = 5 * SUB / 3;
const SHIP_STEP: i32 = SUB;
/// How far every invader shot falling must be on its way, in px, before
/// another is fired, by the score reached: the arcade's reload table.
const RELOAD: [(u64, i32); 5] = [(3000, 28), (2000, 32), (1000, 44), (200, 64), (0, 192)];
/// What the ship is worth by the count of shots fired: the arcade's table.
const SHIP_POINTS: [u64; 16] = [
    100, 50, 50, 100, 150, 100, 100, 50, 300, 100, 100, 100, 50, 150, 100, 50,
];
const LIVES: u8 = 3;
const BONUS: u64 = 1500;
/// The ticks a hit invader's blast shows, the rack waiting; the ship's;
/// the cannon's, the whole field waiting.
const BLAST: u8 = 16;
const SHIP_BLAST: u8 = 32;
const DYING: u32 = 90;
/// The shields' band: cells of 2 px across the field, eight rows deep.
const CELL: i32 = 2;
const BAND_COLS: usize = (WIDTH / CELL) as usize;
const BAND_ROWS: usize = 8;
const SHIELD_LEFTS: [i32; 4] = [32, 78, 124, 170];
const SHIELD: [&str; BAND_ROWS] = [
    "..#######..",
    ".#########.",
    "###########",
    "###########",
    "###########",
    "###########",
    "####...####",
    "###.....###",
];

/// Pixel art, a row a line: `#` is lit.
type Sprite = &'static [&'static str];

/// A kind of invader: its two frames and its points.
struct Kind {
    frames: [Sprite; 2],
    points: u64,
}

const SQUID: Kind = Kind {
    frames: [
        &[
            "...##...", "..####..", ".######.", "##.##.##", "########", "..#..#..", ".#.##.#.",
            "#.#..#.#",
        ],
        &[
            "...##...", "..####..", ".######.", "##.##.##", "########", ".#.##.#.", "#......#",
            ".#....#.",
        ],
    ],
    points: 30,
};
const CRAB: Kind = Kind {
    frames: [
        &[
            "..#.....#..",
            "...#...#...",
            "..#######..",
            ".##.###.##.",
            "###########",
            "#.#######.#",
            "#.#.....#.#",
            "...##.##...",
        ],
        &[
            "..#.....#..",
            "#..#...#..#",
            "#.#######.#",
            "###.###.###",
            "###########",
            ".#########.",
            "..#.....#..",
            ".#.......#.",
        ],
    ],
    points: 20,
};
const OCTOPUS: Kind = Kind {
    frames: [
        &[
            "....####....",
            ".##########.",
            "############",
            "###..##..###",
            "############",
            "...##..##...",
            "..##.##.##..",
            "##........##",
        ],
        &[
            "....####....",
            ".##########.",
            "############",
            "###..##..###",
            "############",
            "..###..###..",
            ".##..##..##.",
            "..##....##..",
        ],
    ],
    points: 10,
};
/// Each row's kind, the top row first.
const KINDS: [&Kind; ROWS] = [&SQUID, &CRAB, &CRAB, &OCTOPUS, &OCTOPUS];

const CANNON: Sprite = &[
    "......#......",
    ".....###.....",
    ".....###.....",
    ".###########.",
    "#############",
    "#############",
    "#############",
    "#############",
];
/// The cannon's blast, its two frames.
const WRECK: [Sprite; 2] = [
    &[
        "..#.....#....#..",
        "#....#.....#....",
        "...#..##.#...#..",
        "..#.#####..#....",
        ".#..######.#.#..",
        "..###########...",
        ".#############..",
        "###############.",
    ],
    &[
        ".....#......#...",
        "..#.....#.......",
        "......#...#..#..",
        "....#.##.#......",
        "..#.######..#...",
        ".###########.#..",
        "..############..",
        ".##############.",
    ],
];
const SHIP: Sprite = &[
    ".....######.....",
    "...##########...",
    "..############..",
    ".##.##.##.##.##.",
    "################",
    "..###..##..###..",
    "...#........#...",
];
const SHIP_W: i32 = 16;
const SHIP_H: i32 = 7;
/// A hit invader's or ship's blast.
const BURST: Sprite = &[
    "....#...#....",
    ".#...#.#...#.",
    "..#.......#..",
    "...#.....#...",
    "##.........##",
    "...#.....#...",
    "..#.#...#.#..",
    ".#...#.#...#.",
];
/// The three kinds of invader shot, two frames each: the rolling shot,
/// which aims, the plunger and the squiggly.
const BOLTS: [[Sprite; 2]; 3] = [
    [
        &[".#.", ".#.", "##.", ".#.", ".##", ".#.", ".#."],
        &[".#.", ".##", ".#.", "##.", ".#.", ".##", ".#."],
    ],
    [
        &["###", ".#.", ".#.", ".#.", ".#.", ".#.", ".#."],
        &[".#.", ".#.", ".#.", ".#.", ".#.", ".#.", "###"],
    ],
    [
        &["#..", ".#.", "..#", ".#.", "#..", ".#.", "..#"],
        &["..#", ".#.", "#..", ".#.", "..#", ".#.", "#.."],
    ],
];
const ROLLING: usize = 0;

/// A box of the field: its left, top, width and height, in px.
type Area = (i32, i32, i32, i32);

/// An invader's shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bolt {
    /// Its top left, in sixteenths of a px.
    pub(crate) at: (i32, i32),
    /// Where it fell from, in sixteenths: how far it is on its way.
    from: i32,
}

impl Bolt {
    fn area(&self) -> Area {
        (whole(self.at.0), whole(self.at.1), BOLT_W, BOLT_H)
    }
}

pub(crate) struct Invaders {
    /// Which invaders stand, the top row first.
    pub(crate) alive: [[bool; COLS]; ROWS],
    /// The top left room once the sweep under way is done, in px.
    pub(crate) rack: (i32, i32),
    /// The sweep's step, in px: sideways, or down at a side.
    step: (i32, i32),
    /// Which way the rack goes, -1 left and 1 right.
    heading: i32,
    /// The rooms the sweep has passed, in marching order: the bottom row
    /// first, left to right. An invader in one stands at `rack`, the rest
    /// a step behind.
    moved: usize,
    /// The frame an invader takes as it moves; the rest show the other.
    legs: usize,
    /// A hit invader's blast: where, and for how many more ticks.
    blast: Option<((i32, i32), u8)>,
    /// The cannon's middle, in px.
    pub(crate) cannon: i32,
    /// The cannon's shot, its top left in sixteenths of a px.
    pub(crate) shot: Option<(i32, i32)>,
    /// The invaders' shots, one of each kind at most.
    pub(crate) bolts: [Option<Bolt>; 3],
    /// The shields' band, the top row first.
    pub(crate) shields: [[bool; BAND_COLS]; BAND_ROWS],
    /// The mystery ship while it crosses: its left in sixteenths of a px,
    /// and its heading.
    pub(crate) ship: Option<(i32, i32)>,
    /// The ticks until the next ship sets off.
    ship_wait: u32,
    /// The hit ship's blast: its left in px, and for how many more ticks.
    ship_blast: Option<(i32, u8)>,
    /// The arrows held, -1 left and 1 right, the newest last: the cannon
    /// goes its way.
    arrows: Vec<i32>,
    /// Space went down since the last tick.
    fire: bool,
    /// The shots the cannon has fired: what the ship is worth.
    pub(crate) fired: u64,
    /// The ticks the cannon's blast has left: the field waits for it.
    dying: u32,
    pub(crate) lives: u8,
    landed: bool,
    pub(crate) wave: u32,
    score: u64,
    ticks: u64,
    rng: Rng,
}

impl Invaders {
    pub(crate) fn new(seed: u64) -> Invaders {
        let mut rng = Rng::new(seed);
        let ship_wait = wait(&mut rng);
        let mut invaders = Invaders {
            alive: [[true; COLS]; ROWS],
            rack: RACK,
            step: (0, 0),
            heading: 1,
            moved: SLOTS,
            legs: 0,
            blast: None,
            cannon: CANNON_START,
            shot: None,
            bolts: [None; 3],
            shields: shields(),
            ship: None,
            ship_wait,
            ship_blast: None,
            arrows: Vec::new(),
            fire: false,
            fired: 0,
            dying: 0,
            lives: LIVES,
            landed: false,
            wave: 0,
            score: 0,
            ticks: 0,
            rng,
        };
        invaders.next_wave();
        invaders
    }

    /// A full rack, the lower the later the wave, and whole shields.
    fn next_wave(&mut self) {
        self.wave += 1;
        let lower = LOWER[(self.wave as usize - 1).min(LOWER.len() - 1)];
        self.alive = [[true; COLS]; ROWS];
        self.rack = (RACK.0, RACK.1 + lower);
        self.step = (0, 0);
        self.heading = 1;
        self.moved = SLOTS;
        self.shot = None;
        self.bolts = [None; 3];
        self.shields = shields();
    }

    /// An arrow held, or a press of Space; read on the next tick. The
    /// keyboard's repeat of a held Space fires nothing.
    pub(crate) fn key_down(&mut self, key: &str, repeat: bool) -> bool {
        if let Some(arrow) = arrow(key) {
            self.arrows.retain(|held| *held != arrow);
            self.arrows.push(arrow);
        } else if key == "space" && !repeat {
            self.fire = true;
        }
        false
    }

    /// An arrow came up: the other, if it is still held, moves the cannon.
    pub(crate) fn key_up(&mut self, key: &str) {
        if let Some(arrow) = arrow(key) {
            self.arrows.retain(|held| *held != arrow);
        }
    }

    pub(crate) fn let_go(&mut self) {
        self.arrows.clear();
        self.fire = false;
    }

    /// One frame: the cannon, the invaders' shots, the cannon's shot, the
    /// rack, the invaders' firing and the ship. Whether the board changed.
    pub(crate) fn tick(&mut self) -> bool {
        if self.over() {
            return false;
        }
        self.ticks += 1;
        if self.dying > 0 {
            self.fire = false;
            self.dying -= 1;
            if self.dying == 0 {
                self.cannon = CANNON_START;
            }
            // the blast's two frames take turns every 8 ticks
            return self.dying.is_multiple_of(8);
        }
        let mut changed = self.steer();
        changed |= self.fall();
        if self.dying > 0 {
            return true;
        }
        changed |= self.shoot();
        changed |= self.advance();
        if self.over() {
            return true;
        }
        changed |= self.fire_back();
        changed |= self.fly();
        changed
    }

    fn steer(&mut self) -> bool {
        let step = self.arrows.last().copied().unwrap_or(0);
        let cannon = (self.cannon + step).clamp(CANNON_REACH.0, CANNON_REACH.1);
        let moved = cannon != self.cannon;
        self.cannon = cannon;
        moved
    }

    /// The invaders' shots fall, and the first thing each meets takes it:
    /// the cannon, a shield or the ground.
    fn fall(&mut self) -> bool {
        let speed = if self.standing() <= 8 {
            FALL_LATE
        } else {
            FALL
        };
        let cannon = (
            self.cannon - width(CANNON) / 2,
            CANNON_Y,
            width(CANNON),
            TALL,
        );
        let mut moved = false;
        for kind in 0..BOLTS.len() {
            let Some(bolt) = &mut self.bolts[kind] else {
                continue;
            };
            bolt.at.1 += speed;
            let area = bolt.area();
            moved = true;
            if overlap(area, cannon) {
                self.hit();
                return true;
            }
            if let Some(cell) = under(area).find(|(row, col)| self.shields[*row][*col]) {
                self.erode(cell, 1);
                self.bolts[kind] = None;
            } else if area.1 + area.3 > GROUND_Y {
                self.bolts[kind] = None;
            }
        }
        moved
    }

    /// A shot struck the cannon: a life goes, every shot with it, and the
    /// field waits while the cannon's blast shows.
    fn hit(&mut self) {
        self.lives -= 1;
        self.dying = DYING;
        self.shot = None;
        self.bolts = [None; 3];
    }

    /// Space fires the cannon's shot when none is up and no blast shows. It
    /// climbs, and the first thing it meets takes it: an invader's shot,
    /// an invader, the ship, a shield or the top.
    fn shoot(&mut self) -> bool {
        if std::mem::take(&mut self.fire) && self.shot.is_none() && self.blast.is_none() {
            self.shot = Some((self.cannon * SUB, CANNON_Y * SUB));
            self.fired += 1;
        }
        let Some((x, y)) = self.shot else {
            return false;
        };
        let y = y - CLIMB;
        self.shot = Some((x, y));
        // where it is and where it was: an invader or the ship that stepped
        // onto it since is struck, not passed through
        let area = (whole(x), whole(y), 1, SHOT_H + whole(CLIMB));
        let ship = self
            .ship
            .map(|(left, _)| whole(left))
            .filter(|left| overlap(area, (*left, SHIP_Y, SHIP_W, SHIP_H)));
        if let Some(kind) = (0..BOLTS.len())
            .find(|kind| self.bolts[*kind].is_some_and(|bolt| overlap(area, bolt.area())))
        {
            self.bolts[kind] = None;
        } else if let Some((row, col)) = self.struck(area) {
            self.alive[row][col] = false;
            let (x, y, w, _) = self.area(row, col);
            self.blast = Some(((x + w / 2 - width(BURST) / 2, y), BLAST));
            self.add(KINDS[row].points);
        } else if let Some(left) = ship {
            self.ship = None;
            self.ship_wait = wait(&mut self.rng);
            self.ship_blast = Some((left + (SHIP_W - width(BURST)) / 2, SHIP_BLAST));
            self.add(SHIP_POINTS[(self.fired % 16) as usize]);
        } else if let Some((row, col)) = under(area)
            .filter(|(row, col)| self.shields[*row][*col])
            .last()
        {
            self.erode((row, col), -1);
        } else if area.1 >= 0 {
            // nothing met, still on the field: it flies on
            return true;
        }
        self.shot = None;
        true
    }

    /// The first standing invader `area` overlaps.
    fn struck(&self, area: Area) -> Option<(usize, usize)> {
        (0..ROWS)
            .flat_map(|row| (0..COLS).map(move |col| (row, col)))
            .filter(|(row, col)| self.alive[*row][*col])
            .find(|(row, col)| overlap(area, self.area(*row, *col)))
    }

    /// A shot struck shield cell `(row, col)` going `way`, 1 down and -1
    /// up: the cell and the one beyond it go, and each beside it at a toss
    /// of the seed.
    fn erode(&mut self, (row, col): (usize, usize), way: i32) {
        let (row, col) = (row as i32, col as i32);
        let mut gone = vec![(row, col), (row + way, col)];
        for side in [-1, 1] {
            if self.rng.below(2) == 0 {
                gone.push((row, col + side));
            }
        }
        for (row, col) in gone {
            if (0..BAND_ROWS as i32).contains(&row) && (0..BAND_COLS as i32).contains(&col) {
                self.shields[row as usize][col as usize] = false;
            }
        }
    }

    /// Points to the score, and the life 1,500 gives.
    fn add(&mut self, points: u64) {
        if self.score < BONUS && self.score + points >= BONUS {
            self.lives += 1;
        }
        self.score += points;
    }

    /// The rack's tick: it waits while a blast shows, a wave shot down
    /// whole comes again, and otherwise one invader marches.
    fn advance(&mut self) -> bool {
        match self.blast {
            Some((spot, left)) if left > 1 => {
                self.blast = Some((spot, left - 1));
                return false;
            }
            Some(_) => self.blast = None,
            None if self.standing() == 0 => self.next_wave(),
            None => self.march(),
        }
        true
    }

    /// The next standing invader in marching order takes its step, through
    /// the shields it reaches; one reaching the cannon's row has landed.
    fn march(&mut self) {
        loop {
            if self.moved == SLOTS {
                self.sweep();
            }
            let (row, col) = (ROWS - 1 - self.moved / COLS, self.moved % COLS);
            self.moved += 1;
            if !self.alive[row][col] {
                continue;
            }
            let area = self.area(row, col);
            for (row, col) in under(area) {
                self.shields[row][col] = false;
            }
            if area.1 + area.3 > CANNON_Y {
                self.landed = true;
                self.dying = DYING;
            }
            return;
        }
    }

    /// A new sweep: down and about where the standing columns reached a
    /// side, else on sideways. Each invader takes its other frame as the
    /// sweep moves it.
    fn sweep(&mut self) {
        let columns = self.columns();
        let (first, last) = (
            columns.first().copied().unwrap_or(0) as i32,
            columns.last().copied().unwrap_or(0) as i32,
        );
        let blocked = match self.heading > 0 {
            true => self.rack.0 + (last + 1) * SLOT + MARCH > WIDTH,
            false => self.rack.0 + first * SLOT - MARCH < 0,
        };
        self.step = match blocked {
            true => {
                self.heading = -self.heading;
                (0, DROP)
            }
            false => (MARCH * self.heading, 0),
        };
        self.rack = (self.rack.0 + self.step.0, self.rack.1 + self.step.1);
        self.moved = 0;
        self.legs ^= 1;
    }

    /// One kind of invader shot a tick, in turn, may fall from the lowest
    /// invader of its column: when none of its kind falls and every one
    /// that does is the reload's distance on its way.
    fn fire_back(&mut self) -> bool {
        let kind = (self.ticks % 3) as usize;
        let reload = RELOAD
            .iter()
            .find(|(score, _)| self.score >= *score)
            .map_or(0, |(_, reload)| reload * SUB);
        if self.bolts[kind].is_some()
            || self
                .bolts
                .iter()
                .flatten()
                .any(|bolt| bolt.at.1 - bolt.from < reload)
        {
            return false;
        }
        let columns = self.columns();
        let col = match kind {
            ROLLING => usize::try_from((self.cannon - self.rack.0).div_euclid(SLOT))
                .ok()
                .filter(|col| columns.contains(col)),
            _ if columns.is_empty() => None,
            _ => Some(columns[self.rng.below(columns.len() as u64) as usize]),
        };
        let Some(col) = col else {
            return false;
        };
        let Some(row) = (0..ROWS).rev().find(|row| self.alive[*row][col]) else {
            return false;
        };
        let (x, y, w, h) = self.area(row, col);
        let at = ((x + w / 2 - BOLT_W / 2) * SUB, (y + h) * SUB);
        self.bolts[kind] = Some(Bolt { at, from: at.1 });
        true
    }

    /// The ship: it sets off when its wait runs out with eight or more
    /// invaders standing, from the side the seed picks, and crosses.
    fn fly(&mut self) -> bool {
        let mut changed = false;
        if let Some((left, ticks)) = self.ship_blast {
            self.ship_blast = (ticks > 1).then_some((left, ticks - 1));
            changed = ticks == 1;
        }
        match self.ship {
            Some((left, heading)) => {
                let left = left + heading * SHIP_STEP;
                self.ship = (0..=(WIDTH - SHIP_W) * SUB)
                    .contains(&left)
                    .then_some((left, heading));
                if self.ship.is_none() {
                    self.ship_wait = wait(&mut self.rng);
                }
                true
            }
            None if self.ship_wait > 0 => {
                self.ship_wait -= 1;
                changed
            }
            None if self.standing() < 8 => {
                self.ship_wait = wait(&mut self.rng);
                changed
            }
            None => {
                let heading = if self.rng.below(2) == 0 { 1 } else { -1 };
                let left = if heading > 0 { 0 } else { WIDTH - SHIP_W };
                self.ship = Some((left * SUB, heading));
                true
            }
        }
    }

    /// Where the invader of `row` and `col` stands, its sprite's box in
    /// px: a step back while the sweep has not reached it.
    fn area(&self, row: usize, col: usize) -> Area {
        let (dx, dy) = match order(row, col) < self.moved {
            true => (0, 0),
            false => self.step,
        };
        let w = width(KINDS[row].frames[0]);
        (
            self.rack.0 + col as i32 * SLOT + (SLOT - w) / 2 - dx,
            self.rack.1 + row as i32 * SLOT - dy,
            w,
            TALL,
        )
    }

    fn standing(&self) -> usize {
        self.alive.iter().flatten().filter(|alive| **alive).count()
    }

    /// The columns with an invader standing, left to right.
    fn columns(&self) -> Vec<usize> {
        (0..COLS)
            .filter(|col| (0..ROWS).any(|row| self.alive[row][*col]))
            .collect()
    }

    pub(crate) fn score(&self) -> u64 {
        self.score
    }

    pub(crate) fn over(&self) -> bool {
        self.lives == 0 || self.landed
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        vec![
            ("Lives", self.lives.to_string()),
            ("Wave", self.wave.to_string()),
        ]
    }

    /// The field at `scale` px a px, in layers: the ground with the
    /// shields, a canvas for each invader of the rack, and over them the
    /// blasts, the ship, the shots and the cannon. A tick moves one invader,
    /// so the host is sent its canvas and the top one, not the whole rack.
    pub(crate) fn draw(&self, scale: f32, theme: &Theme) -> Div {
        let at = grid(scale, (0., 0.));
        let (wide, high) = (WIDTH as f32 * scale, HEIGHT as f32 * scale);
        let layer = || canvas().absolute().top_0().left_0().w(px(wide)).h(px(high));
        let mut ground = layer().rect(square(0., 0., wide, high), theme.surface);
        for (row, cells) in self.shields.iter().enumerate() {
            for (col, len) in runs(cells.iter().copied()) {
                let y = SHIELD_Y + row as i32 * CELL;
                ground = ground.rect(at(col * CELL, y, len * CELL, CELL), theme.success);
            }
        }
        ground = ground.rect(at(0, GROUND_Y, WIDTH, 1), theme.success);
        // every invader keeps its canvas, empty once it is hit, so the
        // canvases after it are not sent again
        let rack = (0..ROWS)
            .flat_map(|row| (0..COLS).map(move |col| (row, col)))
            .map(|(row, col)| match self.alive[row][col] {
                true => {
                    let frame = self.legs ^ usize::from(order(row, col) >= self.moved);
                    let (x, y, _, _) = self.area(row, col);
                    let art = KINDS[row].frames[frame];
                    sprite(layer(), art, (x, y), theme.foreground, &at)
                }
                false => layer(),
            });
        let mut top = layer();
        if let Some((spot, _)) = self.blast {
            top = sprite(top, BURST, spot, theme.foreground, &at);
        }
        if let Some((left, _)) = self.ship {
            top = sprite(top, SHIP, (whole(left), SHIP_Y), theme.danger, &at);
        }
        if let Some((left, _)) = self.ship_blast {
            top = sprite(top, BURST, (left, SHIP_Y), theme.danger, &at);
        }
        let frame = (self.ticks / 4 % 2) as usize;
        for (kind, bolt) in self.bolts.iter().enumerate() {
            if let Some(bolt) = bolt {
                let spot = (whole(bolt.at.0), whole(bolt.at.1));
                top = sprite(top, BOLTS[kind][frame], spot, theme.foreground, &at);
            }
        }
        if let Some((x, y)) = self.shot {
            top = top.rect(at(whole(x), whole(y), 1, SHOT_H), theme.foreground);
        }
        let cannon = match self.dying > 0 {
            true => WRECK[(self.dying / 8 % 2) as usize],
            false => CANNON,
        };
        let spot = (self.cannon - width(cannon) / 2, CANNON_Y);
        div()
            .relative()
            .w(px(wide))
            .h(px(high))
            .child(ground)
            .children(rack)
            .child(sprite(top, cannon, spot, theme.success, &at))
    }
}

/// The key's way for the cannon, if it is an arrow.
fn arrow(key: &str) -> Option<i32> {
    match key {
        "left" | "a" => Some(-1),
        "right" | "d" => Some(1),
        _ => None,
    }
}

/// The room of `row` and `col` in marching order: the bottom row first,
/// left to right.
fn order(row: usize, col: usize) -> usize {
    (ROWS - 1 - row) * COLS + col
}

/// Sixteenths of a px to whole px, down.
fn whole(sub: i32) -> i32 {
    sub.div_euclid(SUB)
}

/// The ticks until the next ship: 20 to 30 s, as the seed says.
fn wait(rng: &mut Rng) -> u32 {
    1250 + rng.below(625) as u32
}

fn width(sprite: Sprite) -> i32 {
    sprite[0].len() as i32
}

fn overlap(a: Area, b: Area) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Four whole shields in their band.
fn shields() -> [[bool; BAND_COLS]; BAND_ROWS] {
    let mut band = [[false; BAND_COLS]; BAND_ROWS];
    for left in SHIELD_LEFTS {
        for (row, line) in SHIELD.iter().enumerate() {
            for (col, cell) in line.bytes().enumerate() {
                band[row][(left / CELL) as usize + col] = cell == b'#';
            }
        }
    }
    band
}

/// The shield band's cells under `area`, the top row first.
fn under((x, y, w, h): Area) -> impl Iterator<Item = (usize, usize)> {
    let rows = (y - SHIELD_Y).div_euclid(CELL).max(0)
        ..=(y + h - 1 - SHIELD_Y)
            .div_euclid(CELL)
            .min(BAND_ROWS as i32 - 1);
    let cols = x.div_euclid(CELL).max(0)..=(x + w - 1).div_euclid(CELL).min(BAND_COLS as i32 - 1);
    rows.flat_map(move |row| cols.clone().map(move |col| (row as usize, col as usize)))
}

/// Where each run of lit cells along a row starts, and its length.
fn runs(row: impl IntoIterator<Item = bool>) -> Vec<(i32, i32)> {
    let mut runs = Vec::new();
    let mut start = None;
    for (i, lit) in row.into_iter().chain([false]).enumerate() {
        match (lit, start) {
            (true, None) => start = Some(i),
            (false, Some(from)) => {
                runs.push((from as i32, (i - from) as i32));
                start = None;
            }
            _ => {}
        }
    }
    runs
}

/// A box of the field's px on the canvas, at `scale` from `origin`: its
/// edges round to whole px, so the art stays sharp.
fn grid(scale: f32, origin: (f32, f32)) -> impl Fn(i32, i32, i32, i32) -> Bounds<Pixels> {
    move |x, y, w, h| {
        let edge = |at: i32| (at as f32 * scale).round();
        square(
            origin.0 + edge(x),
            origin.1 + edge(y),
            edge(x + w) - edge(x),
            edge(y + h) - edge(y),
        )
    }
}

/// `rows` lit with its top left at `(x, y)`: a rect for each run of a row.
fn sprite(
    mut drawn: Canvas,
    rows: Sprite,
    (x, y): (i32, i32),
    color: Hsla,
    at: &impl Fn(i32, i32, i32, i32) -> Bounds<Pixels>,
) -> Canvas {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, len) in runs(row.bytes().map(|pixel| pixel == b'#')) {
            drawn = drawn.rect(at(x + dx, y + dy as i32, len, 1), color);
        }
    }
    drawn
}

/// The hub's picture of the game: three rows of the rack over the cannon
/// and its shot.
pub(crate) fn preview(theme: &Theme) -> Canvas {
    let at = grid(2., (24., 8.));
    let mut drawn = canvas().size_full();
    for (row, kind) in [&SQUID, &CRAB, &OCTOPUS].iter().enumerate() {
        let w = width(kind.frames[0]);
        for col in 0..5 {
            let spot = (col * SLOT + (SLOT - w) / 2, row as i32 * 12);
            drawn = sprite(drawn, kind.frames[0], spot, theme.foreground, &at);
        }
    }
    drawn = drawn.rect(at(40, 37, 1, SHOT_H), theme.foreground);
    sprite(drawn, CANNON, (34, 46), theme.success, &at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(invaders: &Invaders) -> usize {
        invaders
            .shields
            .iter()
            .flatten()
            .filter(|lit| **lit)
            .count()
    }

    #[test]
    fn the_rack_marches_an_invader_a_tick_and_steps_down_and_about_at_a_side() {
        let mut invaders = Invaders::new(1);
        let (first, next) = (invaders.area(4, 0), invaders.area(4, 1));
        invaders.tick();
        assert_eq!(
            invaders.area(4, 0).0,
            first.0 + MARCH,
            "the bottom left first"
        );
        assert_eq!(invaders.area(4, 1), next, "the rest wait their turn");
        let top = invaders.area(0, 10);
        for _ in 1..SLOTS {
            invaders.tick();
        }
        assert_eq!(invaders.area(0, 10).0, top.0 + MARCH, "the top right last");
        // the right side reached: the next sweep steps down and turns
        invaders.rack.0 = WIDTH - COLS as i32 * SLOT;
        let (x, y, _, _) = invaders.area(4, 0);
        invaders.tick();
        assert_eq!(invaders.area(4, 0).1, y + DROP);
        assert_eq!(invaders.area(4, 0).0, x, "straight down");
        assert_eq!(invaders.area(4, 1).1, y, "the rest after it");
        for _ in 0..SLOTS {
            invaders.tick();
        }
        assert_eq!(invaders.area(4, 0).0, x - MARCH, "back the other way");
    }

    #[test]
    fn the_fewer_invaders_stand_the_faster_each_marches() {
        // how far the bottom left invader goes in twelve ticks
        let gone = |standing: &[(usize, usize)]| {
            let mut invaders = Invaders::new(1);
            invaders.alive = [[false; COLS]; ROWS];
            for (row, col) in standing {
                invaders.alive[*row][*col] = true;
            }
            let from = invaders.area(4, 0).0;
            for _ in 0..12 {
                invaders.tick();
            }
            invaders.area(4, 0).0 - from
        };
        let rack: Vec<_> = (0..ROWS)
            .flat_map(|row| (0..COLS).map(move |col| (row, col)))
            .collect();
        assert_eq!(gone(&rack), MARCH, "once in 55 ticks");
        let bottom: Vec<_> = (0..COLS).map(|col| (4, col)).collect();
        assert_eq!(gone(&bottom), 2 * MARCH, "once in 11");
        assert_eq!(gone(&[(4, 0)]), 12 * MARCH, "every tick");
    }

    #[test]
    fn one_shot_flies_at_a_time_and_holding_space_fires_no_more() {
        let mut invaders = Invaders::new(1);
        invaders.key_down("space", false);
        invaders.tick();
        let (x, y) = invaders.shot.expect("fired");
        assert_eq!(whole(x), invaders.cannon);
        invaders.key_down("space", false);
        invaders.tick();
        assert_eq!(invaders.shot, Some((x, y - CLIMB)), "the one shot climbs");
        assert_eq!(invaders.fired, 1);
        // the press made while it flew is not kept for later
        invaders.shot = None;
        invaders.tick();
        assert_eq!(invaders.shot, None);
        // the keyboard repeating a held Space fires nothing; a press does
        invaders.key_down("space", true);
        invaders.tick();
        assert_eq!(invaders.shot, None);
        invaders.key_down("space", false);
        invaders.tick();
        assert!(invaders.shot.is_some());
        assert_eq!(invaders.fired, 2);
    }

    #[test]
    fn an_invader_that_steps_onto_the_shot_is_struck() {
        // the bottom left steps down onto the shot just under it
        let mut invaders = Invaders::new(1);
        invaders.rack.0 = WIDTH - COLS as i32 * SLOT;
        let (x, y, w, h) = invaders.area(4, 0);
        invaders.shot = Some(((x + w / 2) * SUB, (y + h + SHOT_H) * SUB));
        invaders.tick();
        assert!(invaders.alive[4][0], "it stepped down onto the shot");
        invaders.tick();
        assert!(!invaders.alive[4][0]);
        // the next one steps aside onto the shot at its height
        let mut invaders = Invaders::new(1);
        invaders.tick();
        let (x, y, w, _) = invaders.area(4, 1);
        invaders.shot = Some(((x + w) * SUB, (y + SHOT_H) * SUB));
        invaders.tick();
        assert!(invaders.alive[4][1], "it stepped aside onto the shot");
        invaders.tick();
        assert!(!invaders.alive[4][1]);
        assert_eq!(invaders.score(), 10);
    }

    #[test]
    fn shots_wear_a_shield_away_from_above_and_below_and_invaders_crush_it() {
        let mut invaders = Invaders::new(1);
        let full = lit(&invaders);
        // an invader shot onto the first shield's top, over its eighth px
        let from = 150 * SUB;
        invaders.bolts[1] = Some(Bolt {
            at: (40 * SUB, from),
            from,
        });
        for _ in 0..5 {
            invaders.tick();
        }
        // gone, though another plunger may already be falling elsewhere
        assert_ne!(invaders.bolts[1].map(|bolt| bolt.at.0), Some(40 * SUB));
        assert!(!invaders.shields[0][20] && !invaders.shields[1][20]);
        let after = lit(&invaders);
        assert!((2..=4).contains(&(full - after)), "{}", full - after);
        // the cannon's shot up through the arch, into the shield's belly
        invaders.cannon = 40;
        invaders.key_down("space", false);
        for _ in 0..8 {
            invaders.tick();
        }
        assert_eq!(invaders.shot, None);
        assert!(!invaders.shields[5][20] && !invaders.shields[4][20]);
        assert!(invaders.shields[3][20], "two cells deep");
        // the bottom left invader marches into the shield's top two rows
        let mut invaders = Invaders::new(1);
        invaders.rack = (30, SHIELD_Y - 4 * SLOT - 4);
        invaders.tick();
        let (x, _, w, _) = invaders.area(4, 0);
        let cols = (x / CELL) as usize..((x + w - 1) / CELL) as usize + 1;
        assert!(cols.clone().all(|col| !invaders.shields[0][col]));
        assert!(cols.clone().all(|col| !invaders.shields[1][col]));
        assert!(cols.clone().all(|col| invaders.shields[2][col]));
    }

    #[test]
    fn an_invader_shot_on_the_cannon_costs_a_life_and_the_last_ends_the_game() {
        let mut invaders = Invaders::new(1);
        let onto = |invaders: &mut Invaders| {
            let at = ((invaders.cannon - 1) * SUB, (CANNON_Y - BOLT_H) * SUB);
            invaders.bolts[ROLLING] = Some(Bolt { at, from: at.1 });
        };
        onto(&mut invaders);
        invaders.cannon = 60;
        invaders.tick();
        assert_eq!(invaders.lives, LIVES, "it moved out from under");
        onto(&mut invaders);
        invaders.tick();
        assert_eq!(invaders.lives, LIVES - 1);
        assert!(invaders.bolts.iter().all(Option::is_none));
        // the field waits for the blast, then the cannon is back at the left
        let rack = invaders.rack;
        for _ in 0..DYING {
            invaders.tick();
        }
        assert_eq!(invaders.rack, rack);
        assert_eq!(invaders.cannon, CANNON_START);
        invaders.lives = 1;
        onto(&mut invaders);
        invaders.tick();
        assert!(invaders.over());
        assert!(!invaders.tick());
    }

    #[test]
    fn the_rolling_shot_falls_from_the_column_over_the_cannon() {
        let mut invaders = Invaders::new(1);
        invaders.cannon = RACK.0 + 7 * SLOT + SLOT / 2;
        // the next tick is the rolling shot's turn
        invaders.ticks = 2;
        invaders.tick();
        let bolt = invaders.bolts[ROLLING].expect("fired");
        let (x, y, w, h) = invaders.area(4, 7);
        assert_eq!(whole(bolt.at.0), x + w / 2 - 1);
        assert_eq!(whole(bolt.from), y + h);
    }

    #[test]
    fn the_mystery_ship_is_worth_the_tables_points_for_the_shots_fired() {
        let hit = |fired: u64| {
            let mut invaders = Invaders::new(1);
            invaders.ship = Some((100 * SUB, 1));
            invaders.shot = Some((108 * SUB, (SHIP_Y + SHIP_H) * SUB));
            invaders.fired = fired;
            invaders.tick();
            assert_eq!(invaders.ship, None);
            invaders.score()
        };
        assert_eq!(hit(8), 300);
        assert_eq!(hit(1), 50);
        assert_eq!(hit(4), 150);
        assert_eq!(hit(16), 100);
    }

    #[test]
    fn fifteen_hundred_points_give_a_life_once() {
        let mut invaders = Invaders::new(1);
        invaders.add(1490);
        assert_eq!(invaders.lives, LIVES);
        invaders.add(10);
        assert_eq!(invaders.lives, LIVES + 1);
        invaders.add(1500);
        assert_eq!(invaders.lives, LIVES + 1);
    }

    #[test]
    fn a_wave_shot_down_whole_comes_again_lower_with_whole_shields() {
        let mut invaders = Invaders::new(1);
        invaders.alive = [[false; COLS]; ROWS];
        invaders.alive[4][0] = true;
        invaders.shields[0] = [false; BAND_COLS];
        let (x, y, _, h) = invaders.area(4, 0);
        invaders.shot = Some(((x + 1) * SUB, (y + h + 2) * SUB));
        invaders.tick();
        assert_eq!(invaders.score(), 10);
        for _ in 0..20 {
            invaders.tick();
        }
        assert_eq!(invaders.wave, 2);
        assert_eq!(invaders.rack.1, RACK.1 + LOWER[1]);
        assert_eq!(invaders.standing(), SLOTS);
        assert_eq!(invaders.shields, shields());
        assert_eq!(invaders.facts()[1].1, "2");
    }

    #[test]
    fn invaders_reaching_the_cannons_row_end_the_game() {
        let mut invaders = Invaders::new(1);
        // at the right side, the bottom row's feet on the cannon's row
        invaders.rack = (WIDTH - COLS as i32 * SLOT, CANNON_Y - 4 * SLOT - TALL);
        invaders.tick();
        assert!(invaders.over());
        assert_eq!(invaders.lives, LIVES);
        assert!(!invaders.tick());
    }

    #[test]
    fn one_seed_and_the_same_keys_play_the_same_game() {
        let play = |seed: u64| {
            let mut invaders = Invaders::new(seed);
            for tick in 0..3000 {
                match tick % 240 {
                    0 => {
                        invaders.key_down("left", false);
                    }
                    120 => {
                        invaders.key_down("right", false);
                    }
                    200 => {
                        invaders.key_up("left");
                        invaders.key_up("right");
                    }
                    _ => {}
                }
                if tick % 25 == 0 {
                    invaders.key_down("space", false);
                }
                invaders.tick();
            }
            (
                invaders.score(),
                invaders.lives,
                invaders.wave,
                invaders.rack,
                invaders.cannon,
                invaders.bolts,
                invaders.ship,
                invaders.alive,
                invaders.shields,
            )
        };
        assert_eq!(play(7), play(7));
        assert_ne!(play(7), play(8));
    }
}
