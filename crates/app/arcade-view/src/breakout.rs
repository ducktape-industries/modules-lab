//! Breakout: eight walls laid out by hand (`breakout/stages.rs`), then the
//! eight again, a paddle and a ball. The ball bounces off the walls, the
//! bricks and the paddle, which sends it off at the angle of where it
//! struck. A brick takes one to three hits and steel none; a wall is clear
//! when every brick that breaks is gone, which scores a bonus and builds
//! the next. Losing the last ball in play costs a life of the three it
//! starts with.
//!
//! As Atari's, the ball quickens after 4 and 12 bricks struck and on
//! reaching the upper rows, and the paddle halves once the ball reaches
//! the top wall; both last until the ball is lost or the wall is clear.
//! Each wall serves faster than the last, and each round of eight after the
//! first takes from the paddle.
//!
//! A broken brick may drop a capsule. Caught, it widens or shortens the
//! paddle, splits the ball in three, slows it or speeds it, makes the
//! paddle hold it, arms a laser or gives a life; a timed one lasts 12 s.
//!
//! The tick is 16 ms, a frame: the paddle moves while an arrow is held,
//! the newest of two held, and Space serves, lets a held ball go or fires.
//! The field counts in units of 1/16 px, so a tick moves the ball by whole
//! numbers and the same keys play the same game.
use ducktape_view_guest::Canvas;
use ducktape_view_guest::prelude::*;
use gpui::point;

use crate::games::{Fact, Rng, square};

mod stages;

use stages::{COLS, ROWS, STEEL, Wall, breakable};

/// The field, in px at a scale of one.
pub(crate) const WIDTH: i32 = 416;
pub(crate) const HEIGHT: i32 = 352;
pub(crate) const KEYS: &str =
    "← → move the paddle, Space serves, lets a held ball go and fires the laser";
pub(crate) const PERIOD: i64 = 16;

const BRICK_W: i32 = 32;
const BRICK_H: i32 = 12;
const WALL_TOP: i32 = 48;
/// A row's points a hit, the top row first: Atari's red 7, orange 5, green
/// 3 and yellow 1.
const POINTS: [u64; ROWS] = [7, 7, 5, 5, 3, 3, 1, 1, 1, 1];
/// A clear wall's bonus, times the walls cleared.
const CLEAR: u64 = 100;
/// Half the paddle, and what each round of eight walls after the first
/// takes from it, twice at most.
const HALF: i32 = 32;
const HALF_LESS: i32 = 4;
const PADDLE_H: i32 = 8;
const PADDLE_Y: i32 = HEIGHT - 24;
/// The paddle's step a tick, in px.
const PADDLE_STEP: i32 = 6;
const RADIUS: i32 = 4;
const LIVES: u8 = 3;
const LIVES_MOST: u8 = 5;
/// Sixteenths of a px: the unit the ball moves in.
pub(crate) const SUB: i32 = 16;
/// The ball's pace, its climb a tick: 3 px on the first wall and a quarter
/// px more on each after; each of Atari's speed-ups 3/8 px more, and Slow
/// or Fast a px less or more.
const PACE: i32 = 3 * SUB;
const PACE_WALL: i32 = SUB / 4;
const PACE_UP: i32 = 6;
const PACE_ITEM: i32 = SUB;
const PACE_LEAST: i32 = 2 * SUB;
/// 7 px a tick up or down and 8.75 across at most: a step is shorter than
/// a brick and the ball together on either axis (20 px down, 40 across),
/// so the overlap it ends in finds every brick in its way.
const PACE_MOST: i32 = 7 * SUB;
/// A ball's way a tick for each `UP` of the pace: `UP` up or down, and at
/// most `LEAN` across.
pub(crate) const UP: i32 = 16;
const LEAN: i32 = 20;

/// One broken brick in `DROP` drops a capsule, and these are the odds of
/// what it holds: each item as often as it is here.
const DROP: u64 = 8;
const BAG: [Item; 17] = [
    Item::Wide,
    Item::Wide,
    Item::Wide,
    Item::Multi,
    Item::Multi,
    Item::Multi,
    Item::Slow,
    Item::Slow,
    Item::Catch,
    Item::Catch,
    Item::Laser,
    Item::Laser,
    Item::Life,
    Item::Short,
    Item::Short,
    Item::Fast,
    Item::Fast,
];
/// The items that last a time, in the order the panel names those on.
const TIMED: [Item; 6] = [
    Item::Wide,
    Item::Short,
    Item::Slow,
    Item::Fast,
    Item::Catch,
    Item::Laser,
];
/// A timed item's ticks: 12 s.
const LASTS: u16 = 750;
/// A capsule's fall a tick in sixteenths (1.5 px), and its size in px.
const FALL: i32 = 24;
const CAPSULE: (i32, i32) = (24, 10);
/// A capsule's points when caught; a bad one is worth the risk.
const CAUGHT: u64 = 10;
const CAUGHT_BAD: u64 = 50;
/// A laser shot's climb a tick, under a brick's height, and its length.
const SHOT: i32 = 8 * SUB;
const SHOT_H: i32 = 6;

/// What a capsule holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    /// The paddle half again as wide.
    Wide,
    /// Two more balls.
    Multi,
    /// The ball a px a tick slower.
    Slow,
    /// The paddle holds the ball it meets until Space.
    Catch,
    /// Space fires two shots from the paddle's ends.
    Laser,
    /// One more life.
    Life,
    /// Bad: the paddle 5/8 as wide.
    Short,
    /// Bad: the ball a px a tick faster.
    Fast,
}

impl Item {
    fn name(self) -> &'static str {
        match self {
            Item::Wide => "Wide paddle",
            Item::Multi => "Three balls",
            Item::Slow => "Slow ball",
            Item::Catch => "Sticky paddle",
            Item::Laser => "Laser",
            Item::Life => "Extra life",
            Item::Short => "Short paddle",
            Item::Fast => "Fast ball",
        }
    }

    fn bad(self) -> bool {
        matches!(self, Item::Short | Item::Fast)
    }

    /// The timed item this one ends.
    fn opposite(self) -> Option<Item> {
        match self {
            Item::Wide => Some(Item::Short),
            Item::Short => Some(Item::Wide),
            Item::Slow => Some(Item::Fast),
            Item::Fast => Some(Item::Slow),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ball {
    /// Its middle, in sixteenths of a px.
    pub(crate) at: (i32, i32),
    /// Its way: across and down, in `UP` a tick for each `UP` of the pace.
    pub(crate) way: (i32, i32),
    /// Where it sits on the paddle, in px from the middle, until Space lets
    /// it go.
    pub(crate) held: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Capsule {
    pub(crate) item: Item,
    /// Its middle, in sixteenths of a px.
    pub(crate) at: (i32, i32),
}

#[derive(Debug)]
pub(crate) struct Breakout {
    pub(crate) bricks: Wall,
    /// The walls cleared: the stage on screen, the first 0.
    pub(crate) stage: u32,
    /// The paddle's middle, in px.
    pub(crate) paddle: i32,
    /// At least one, until the last is lost.
    pub(crate) balls: Vec<Ball>,
    pub(crate) capsules: Vec<Capsule>,
    /// The laser's shots in flight, each its tip, in sixteenths of a px.
    pub(crate) shots: Vec<(i32, i32)>,
    /// Each timed item's ticks left, by `Item as usize`.
    pub(crate) left: [u16; 8],
    /// The bricks a ball struck since the serve, and the top row it struck.
    hits: u32,
    top: usize,
    /// A ball reached the top wall since the serve: the paddle is half.
    pub(crate) halved: bool,
    /// The arrows held, -1 left and 1 right, the newest last: the paddle
    /// goes its way.
    arrows: Vec<i32>,
    space: bool,
    pub(crate) lives: u8,
    score: u64,
    rng: Rng,
}

impl Breakout {
    pub(crate) fn new(seed: u64) -> Breakout {
        let mut breakout = Breakout {
            bricks: stages::wall(0),
            stage: 0,
            paddle: WIDTH / 2,
            balls: Vec::new(),
            capsules: Vec::new(),
            shots: Vec::new(),
            left: [0; 8],
            hits: 0,
            top: ROWS,
            halved: false,
            arrows: Vec::new(),
            space: false,
            lives: LIVES,
            score: 0,
            rng: Rng::new(seed),
        };
        breakout.rest();
        breakout
    }

    /// One ball on the paddle's middle, waiting to be served. What the last
    /// ones left ends: capsules, shots, items, speed-ups and the half paddle.
    fn rest(&mut self) {
        self.balls = vec![Ball {
            at: (0, 0),
            way: (0, -UP),
            held: Some(0),
        }];
        self.capsules.clear();
        self.shots.clear();
        self.left = [0; 8];
        self.hits = 0;
        self.top = ROWS;
        self.halved = false;
        self.ride();
    }

    /// A ball waits on the paddle.
    #[cfg(test)]
    pub(crate) fn serving(&self) -> bool {
        self.balls.iter().any(|ball| ball.held.is_some())
    }

    fn on(&self, item: Item) -> bool {
        self.left[item as usize] > 0
    }

    /// Half the paddle's width, in px.
    pub(crate) fn half(&self) -> i32 {
        let round = (self.stage / stages::COUNT).min(2) as i32;
        let half = HALF - HALF_LESS * round;
        let half = match (self.on(Item::Wide), self.on(Item::Short)) {
            (true, _) => half * 3 / 2,
            (_, true) => half * 5 / 8,
            _ => half,
        };
        if self.halved { half / 2 } else { half }
    }

    /// The balls' climb a tick, in sixteenths of a px.
    pub(crate) fn pace(&self) -> i32 {
        let ups = [self.hits >= 4, self.hits >= 12, self.top < 4, self.top < 2]
            .into_iter()
            .filter(|up| *up)
            .count() as i32;
        let item = match (self.on(Item::Slow), self.on(Item::Fast)) {
            (true, _) => -PACE_ITEM,
            (_, true) => PACE_ITEM,
            _ => 0,
        };
        let wall = PACE_WALL * self.stage.min(64) as i32;
        (PACE + wall + PACE_UP * ups + item).clamp(PACE_LEAST, PACE_MOST)
    }

    /// An arrow held, or Space; read on the next tick. Space acts once a
    /// press: its repeat would let a held ball go as it lands.
    pub(crate) fn key_down(&mut self, key: &str, repeat: bool) -> bool {
        if let Some(arrow) = arrow(key) {
            self.arrows.retain(|held| *held != arrow);
            self.arrows.push(arrow);
        } else if matches!(key, "space" | "up" | "w") && !repeat {
            self.space = true;
        }
        false
    }

    /// An arrow came up: the other, if it is still held, moves the paddle.
    pub(crate) fn key_up(&mut self, key: &str) {
        if let Some(arrow) = arrow(key) {
            self.arrows.retain(|held| *held != arrow);
        }
    }

    /// One frame: the paddle, Space, then the balls, the shots and the
    /// capsules; then a clear wall, or a lost ball.
    pub(crate) fn tick(&mut self) -> bool {
        if self.over() {
            return false;
        }
        let half = self.half();
        let step = PADDLE_STEP * self.arrows.last().copied().unwrap_or(0);
        let paddle = (self.paddle + step).clamp(half, WIDTH - half);
        let moved = paddle != self.paddle;
        self.paddle = paddle;
        if std::mem::take(&mut self.space) {
            self.space();
        }
        self.ride();
        let still = self.balls.iter().all(|ball| ball.held.is_some())
            && self.capsules.is_empty()
            && self.shots.is_empty()
            && self.left == [0; 8];
        if still {
            return moved;
        }
        for left in &mut self.left {
            *left = left.saturating_sub(1);
        }
        for ball in 0..self.balls.len() {
            self.fly(ball);
        }
        self.shoot();
        self.fall();
        if !self.bricks.iter().flatten().any(|brick| breakable(*brick)) {
            self.stage += 1;
            self.score += CLEAR * u64::from(self.stage);
            self.bricks = stages::wall(self.stage);
            self.rest();
            return true;
        }
        let floor = (HEIGHT + RADIUS) * SUB;
        self.balls.retain(|ball| ball.at.1 <= floor);
        if self.balls.is_empty() {
            self.lives -= 1;
            self.rest();
        }
        true
    }

    /// Space: the balls on the paddle go, off to the side they sit on, a
    /// ball on the middle to a side the dice pick; one held past the end of
    /// a paddle that has since shrunk goes as from the end. With none there,
    /// the laser fires if it is on and its last shots are gone.
    fn space(&mut self) {
        let half = self.half();
        let reach = half + RADIUS;
        let mut freed = false;
        for ball in &mut self.balls {
            let Some(off) = ball.held.take() else {
                continue;
            };
            let lean = match off.clamp(-half, half) {
                0 if self.rng.below(2) == 0 => -LEAN / 2,
                0 => LEAN / 2,
                off => LEAN * off / reach,
            };
            ball.way = (lean, -UP);
            freed = true;
        }
        if !freed && self.on(Item::Laser) && self.shots.is_empty() {
            self.shots = [-1, 1]
                .map(|side| ((self.paddle + side * (half - 4)) * SUB, PADDLE_Y * SUB))
                .to_vec();
        }
    }

    /// The balls on the paddle go with it.
    fn ride(&mut self) {
        let half = self.half();
        for ball in &mut self.balls {
            if let Some(off) = ball.held {
                let x = self.paddle + off.clamp(-half, half);
                ball.at = (x * SUB, (PADDLE_Y - RADIUS) * SUB);
            }
        }
    }

    /// Ball `which`'s step, across and then down: a brick in its way takes
    /// the step back and turns it, so it never ends a tick inside one.
    fn fly(&mut self, which: usize) {
        let mut ball = self.balls[which];
        if ball.held.is_some() {
            return;
        }
        let pace = self.pace();
        let (r, w) = (RADIUS * SUB, WIDTH * SUB);
        let across = pace * ball.way.0 / UP;
        ball.at.0 += across;
        if let Some(brick) = self.struck(ball.at, r) {
            ball.at.0 -= across;
            ball.way.0 = -ball.way.0;
            self.bounce(&mut ball, brick);
        } else if ball.at.0 < r || ball.at.0 > w - r {
            ball.at.0 = ball.at.0.clamp(r, w - r);
            ball.way.0 = -ball.way.0;
        }
        let down = pace * ball.way.1 / UP;
        ball.at.1 += down;
        if let Some(brick) = self.struck(ball.at, r) {
            ball.at.1 -= down;
            ball.way.1 = -ball.way.1;
            self.bounce(&mut ball, brick);
        } else if ball.at.1 < r {
            ball.at.1 = r;
            ball.way.1 = UP;
            self.halved = true;
        }
        self.balls[which] = self.paddled(ball, down);
    }

    /// A falling ball that meets the paddle goes back up, off to the side
    /// it struck by as much as it struck off the middle; a sticky paddle
    /// holds it there instead.
    fn paddled(&self, mut ball: Ball, down: i32) -> Ball {
        let (top, r) = (PADDLE_Y * SUB, RADIUS * SUB);
        let half = self.half();
        let reach = (half + RADIUS) * SUB;
        let off = ball.at.0 - self.paddle * SUB;
        let level = ball.at.1 + r >= top && ball.at.1 + r - down <= top;
        if down > 0 && level && off.abs() <= reach {
            ball.at.1 = top - r;
            match self.on(Item::Catch) {
                true => ball.held = Some((off / SUB).clamp(-half, half)),
                false => ball.way = (LEAN * off / reach, -UP),
            }
        }
        ball
    }

    /// The first standing brick, the top row first, that a square `r`
    /// either side of `at` overlaps.
    fn struck(&self, (x, y): (i32, i32), r: i32) -> Option<(usize, usize)> {
        (0..ROWS)
            .flat_map(|row| (0..COLS).map(move |col| (row, col)))
            .filter(|(row, col)| self.bricks[*row][*col] != 0)
            .find(|(row, col)| {
                let left = *col as i32 * BRICK_W * SUB;
                let top = (WALL_TOP + *row as i32 * BRICK_H) * SUB;
                x + r > left
                    && x - r < left + BRICK_W * SUB
                    && y + r > top
                    && y - r < top + BRICK_H * SUB
            })
    }

    /// A ball struck a brick: it counts toward the speed-ups. Steel turns
    /// it a sixteenth askew, a way the dice pick, so a ball shut in by
    /// steel finds its way out rather than going round the same path.
    fn bounce(&mut self, ball: &mut Ball, (row, col): (usize, usize)) {
        self.hits += 1;
        self.top = self.top.min(row);
        if self.bricks[row][col] == STEEL {
            let askew = if self.rng.below(2) == 0 { -1 } else { 1 };
            ball.way.0 = (ball.way.0 + askew).clamp(-LEAN, LEAN);
        }
        self.strike((row, col));
    }

    /// A brick struck: steel stands; another loses a hit and scores its
    /// row's points, and broken, one in `DROP` drops a capsule.
    fn strike(&mut self, (row, col): (usize, usize)) {
        let brick = &mut self.bricks[row][col];
        if *brick == STEEL {
            return;
        }
        *brick -= 1;
        self.score += POINTS[row];
        if *brick == 0 && self.rng.below(DROP) == 0 {
            let item = BAG[self.rng.below(BAG.len() as u64) as usize];
            let x = col as i32 * BRICK_W + BRICK_W / 2;
            let y = WALL_TOP + row as i32 * BRICK_H + BRICK_H / 2;
            self.capsules.push(Capsule {
                item,
                at: (x * SUB, y * SUB),
            });
        }
    }

    /// The shots climb: one that meets a brick strikes it and is gone, and
    /// so is one past the top.
    fn shoot(&mut self) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|shot| {
            shot.1 -= SHOT;
            match self.struck(*shot, SUB) {
                Some(brick) => {
                    self.strike(brick);
                    false
                }
                None => shot.1 > 0,
            }
        });
        self.shots = shots;
    }

    /// The capsules fall: one the paddle meets is caught, one past the
    /// bottom is gone.
    fn fall(&mut self) {
        let half = self.half() * SUB;
        let (w, h) = (CAPSULE.0 * SUB / 2, CAPSULE.1 * SUB / 2);
        let (left, right) = (self.paddle * SUB - half, self.paddle * SUB + half);
        let (top, bottom) = (PADDLE_Y * SUB, (PADDLE_Y + PADDLE_H) * SUB);
        let mut caught = Vec::new();
        self.capsules.retain_mut(|capsule| {
            capsule.at.1 += FALL;
            let (x, y) = capsule.at;
            let meets = y + h >= top && y - h <= bottom && x + w >= left && x - w <= right;
            if meets {
                caught.push(capsule.item);
            }
            !meets && y - h <= HEIGHT * SUB
        });
        for item in caught {
            self.catch(item);
        }
    }

    /// An item caught: three balls, a life (five at most), or a timed one
    /// that starts its time and ends its opposite.
    pub(crate) fn catch(&mut self, item: Item) {
        self.score += if item.bad() { CAUGHT_BAD } else { CAUGHT };
        match item {
            Item::Multi => self.split(),
            Item::Life => self.lives = (self.lives + 1).min(LIVES_MOST),
            timed => {
                self.left[timed as usize] = LASTS;
                if let Some(opposite) = timed.opposite() {
                    self.left[opposite as usize] = 0;
                }
            }
        }
    }

    /// The first ball goes on and two more leave where it is, leaning 10
    /// and 20 sixteenths further the other way (a held one's from straight
    /// up).
    fn split(&mut self) {
        let Some(&ball) = self.balls.first() else {
            return;
        };
        let (lean, down) = match ball.held {
            Some(_) => (0, -UP),
            None => ball.way,
        };
        let side = if lean < 0 { 1 } else { -1 };
        for turn in [10, 20] {
            self.balls.push(Ball {
                at: ball.at,
                way: (lean + side * turn, down),
                held: None,
            });
        }
    }

    pub(crate) fn score(&self) -> u64 {
        self.score
    }

    pub(crate) fn over(&self) -> bool {
        self.lives == 0
    }

    /// The stage, the lives, and each timed item on with its seconds left.
    pub(crate) fn facts(&self) -> Vec<Fact> {
        let mut facts = vec![
            ("Stage", (self.stage + 1).to_string()),
            ("Lives", self.lives.to_string()),
        ];
        for item in TIMED {
            let left = u64::from(self.left[item as usize]);
            if left > 0 {
                let seconds = (left * PERIOD as u64).div_ceil(1000);
                facts.push((item.name(), format!("{seconds} s")));
            }
        }
        facts
    }

    /// The field at `scale` px a px: the wall, the capsules, the shots, the
    /// paddle and the balls.
    pub(crate) fn draw(&self, scale: f32, theme: &Theme) -> Canvas {
        let at = |x: i32, y: i32, w: i32, h: i32| {
            square(
                x as f32 * scale,
                y as f32 * scale,
                w as f32 * scale,
                h as f32 * scale,
            )
        };
        let (width, height) = (WIDTH as f32 * scale, HEIGHT as f32 * scale);
        let mut drawn = canvas()
            .w(px(width))
            .h(px(height))
            .rect(square(0., 0., width, height), theme.surface);
        for (row, bricks) in self.bricks.iter().enumerate() {
            let color = match row / 2 {
                0 => theme.danger,
                1 => theme.warning,
                2 => theme.success,
                _ => theme.agent,
            };
            for (col, brick) in bricks.iter().enumerate() {
                let (x, y) = (col as i32 * BRICK_W, WALL_TOP + row as i32 * BRICK_H);
                let face = at(x + 1, y + 1, BRICK_W - 2, BRICK_H - 2);
                drawn = match *brick {
                    0 => drawn,
                    // steel: a grey plate with a sunk middle
                    STEEL => drawn.rect(face, theme.muted).rect(
                        at(x + 4, y + 4, BRICK_W - 8, BRICK_H - 8),
                        theme.border_strong,
                    ),
                    // a dot for each hit past the last
                    hits => {
                        let dots = i32::from(hits) - 1;
                        (0..dots).fold(drawn.rect(face, color), |drawn, dot| {
                            let middle = x + BRICK_W / 2 + (2 * dot - dots + 1) * 4;
                            drawn.rect(at(middle - 2, y + 4, 4, 4), theme.surface)
                        })
                    }
                };
            }
        }
        for item in &self.capsules {
            let middle = (item.at.0 as f32 / SUB as f32, item.at.1 as f32 / SUB as f32);
            drawn = capsule(drawn, item.item, middle, scale, theme);
        }
        for (x, y) in &self.shots {
            drawn = drawn.rect(at(x / SUB - 1, y / SUB, 2, SHOT_H), theme.warning);
        }
        let half = self.half();
        let left = self.paddle - half;
        drawn = drawn.rect(at(left, PADDLE_Y, 2 * half, PADDLE_H), theme.foreground);
        if self.on(Item::Catch) {
            drawn = drawn.rect(at(left, PADDLE_Y, 2 * half, 2), theme.success);
        }
        if self.on(Item::Laser) {
            for side in [-1, 1] {
                let x = self.paddle + side * (half - 4);
                drawn = drawn.rect(at(x - 2, PADDLE_Y - 3, 4, 3), theme.foreground);
            }
        }
        if self.over() {
            return drawn;
        }
        for ball in &self.balls {
            let middle = point(
                px(ball.at.0 as f32 / SUB as f32 * scale),
                px(ball.at.1 as f32 / SUB as f32 * scale),
            );
            drawn = drawn.circle(middle, px(RADIUS as f32 * scale), theme.foreground);
        }
        drawn
    }
}

/// The key's way for the paddle, if it is an arrow.
fn arrow(key: &str) -> Option<i32> {
    match key {
        "left" | "a" => Some(-1),
        "right" | "d" => Some(1),
        _ => None,
    }
}

/// A capsule with its middle at `(x, y)` px, drawn at `scale`: a pill in
/// the item's colour (green the paddle's, purple the ball's, ink the
/// laser, amber a life, red bad) with the item's mark on it.
fn capsule(drawn: Canvas, item: Item, (x, y): (f32, f32), scale: f32, theme: &Theme) -> Canvas {
    let spot = |dx: f32, dy: f32| point(px((x + dx) * scale), px((y + dy) * scale));
    let (w, h) = (CAPSULE.0 as f32, CAPSULE.1 as f32);
    let r = h / 2.;
    let color = match item {
        Item::Wide | Item::Catch => theme.success,
        Item::Multi | Item::Slow => theme.agent,
        Item::Laser => theme.foreground,
        Item::Life => theme.warning,
        Item::Short | Item::Fast => theme.danger,
    };
    let pill = drawn
        .rect(
            square(
                (x - w / 2. + r) * scale,
                (y - r) * scale,
                (w - h) * scale,
                h * scale,
            ),
            color,
        )
        .circle(spot(r - w / 2., 0.), px(r * scale), color)
        .circle(spot(w / 2. - r, 0.), px(r * scale), color);
    let mark = theme.surface;
    let line = |drawn: Canvas, from: (f32, f32), to: (f32, f32)| {
        drawn.line(
            spot(from.0, from.1),
            spot(to.0, to.1),
            px(1.5 * scale),
            mark,
        )
    };
    let dot = |drawn: Canvas, dx: f32, r: f32| drawn.circle(spot(dx, 0.), px(r * scale), mark);
    match item {
        // a long bar, a short one
        Item::Wide => line(pill, (-7., 0.), (7., 0.)),
        Item::Short => line(pill, (-2.5, 0.), (2.5, 0.)),
        // three balls, one big slow one
        Item::Multi => dot(dot(dot(pill, -5., 1.5), 0., 1.5), 5., 1.5),
        Item::Slow => dot(pill, 0., 2.5),
        // a cup
        Item::Catch => line(
            line(line(pill, (-4., -2.5), (-4., 2.5)), (-4., 2.5), (4., 2.5)),
            (4., 2.5),
            (4., -2.5),
        ),
        // two guns
        Item::Laser => line(line(pill, (-3., -3.), (-3., 3.)), (3., -3.), (3., 3.)),
        // a plus
        Item::Life => line(line(pill, (-4., 0.), (4., 0.)), (0., -3.5), (0., 3.5)),
        // two chevrons
        Item::Fast => [-3., 2.].into_iter().fold(pill, |drawn, dx| {
            line(
                line(drawn, (dx - 2., -3.), (dx + 2., 0.)),
                (dx + 2., 0.),
                (dx - 2., 3.),
            )
        }),
    }
}

/// The hub's picture of the game: a wall with a gap, a capsule falling and
/// the ball on its way.
pub(crate) fn preview(theme: &Theme) -> Canvas {
    let mut drawn = canvas().size_full();
    for row in 0..4 {
        let color = match row / 2 {
            0 => theme.danger,
            _ => theme.warning,
        };
        for col in 0..9 {
            if row == 3 && (3..5).contains(&col) {
                continue;
            }
            let at = square(14. + col as f32 * 20., 14. + row as f32 * 9., 18., 7.);
            drawn = drawn.rect(at, color);
        }
    }
    let drawn = capsule(drawn, Item::Wide, (154., 80.), 1., theme);
    drawn
        .rect(square(80., 104., 44., 5.), theme.foreground)
        .circle(point(px(112.), px(72.)), px(4.), theme.foreground)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ball in flight at `(x, y)` px, going `way`, on the first wall at
    /// its first pace.
    fn flying(x: i32, y: i32, way: (i32, i32)) -> Breakout {
        let mut breakout = Breakout::new(1);
        breakout.balls = vec![Ball {
            at: (x * SUB, y * SUB),
            way,
            held: None,
        }];
        breakout
    }

    /// The first wall's bottom, in px.
    const BOTTOM: i32 = WALL_TOP + 8 * BRICK_H;

    fn broken(breakout: &Breakout) -> usize {
        let first = stages::wall(0);
        (0..ROWS)
            .flat_map(|row| (0..COLS).map(move |col| (row, col)))
            .filter(|(row, col)| breakout.bricks[*row][*col] != first[*row][*col])
            .count()
    }

    #[test]
    fn a_brick_struck_from_below_breaks_scores_and_turns_the_ball_down() {
        // under the bottom row's sixth brick, rising
        let mut breakout = flying(5 * BRICK_W + 16, BOTTOM + RADIUS + 1, (0, -UP));
        breakout.tick();
        assert_eq!(breakout.bricks[7][5], 0);
        assert_eq!(breakout.score(), 1);
        assert!(breakout.balls[0].way.1 > 0, "it falls again");
        assert_eq!(broken(&breakout), 1);
    }

    #[test]
    fn a_ball_rising_straight_into_a_bricks_corner_turns_down() {
        // straight up, its left edge 1 px under the bottom row's seventh
        // brick and the rest under the eighth: it goes in through the bottom
        let mut breakout = flying(7 * BRICK_W + RADIUS - 1, BOTTOM + RADIUS + 1, (0, -UP));
        breakout.tick();
        assert!(breakout.balls[0].way.1 > 0, "{:?}", breakout.balls);
        assert_eq!(broken(&breakout), 1);
    }

    #[test]
    fn a_brick_struck_on_its_side_turns_the_ball_sideways() {
        // an empty slot in the bottom row, the ball going right into its
        // neighbour's left side
        let mut breakout = flying(0, 0, (UP, 0));
        breakout.bricks[7][3] = 0;
        let y = WALL_TOP + 7 * BRICK_H + BRICK_H / 2;
        breakout.balls[0].at = ((4 * BRICK_W - RADIUS) * SUB - SUB, y * SUB);
        breakout.tick();
        assert_eq!(breakout.bricks[7][4], 0);
        assert!(breakout.balls[0].way.0 < 0);
    }

    #[test]
    fn the_paddle_sends_the_ball_up_off_to_the_side_it_struck() {
        let y = PADDLE_Y - RADIUS - 1;
        let mut breakout = flying(WIDTH / 2, y, (0, UP));
        breakout.tick();
        assert_eq!(breakout.balls[0].way, (0, -UP), "the middle: straight up");
        let mut breakout = flying(WIDTH / 2 + HALF, y, (0, UP));
        breakout.tick();
        let way = breakout.balls[0].way;
        assert!(way.0 > 0 && way.1 < 0, "{way:?}");
        let mut breakout = flying(WIDTH / 2 - 20, y, (0, UP));
        breakout.tick();
        assert!(breakout.balls[0].way.0 < 0);
    }

    #[test]
    fn the_walls_turn_the_ball_back() {
        let mut breakout = flying(RADIUS + 1, 200, (-UP, UP));
        breakout.tick();
        assert!(breakout.balls[0].way.0 > 0);
        let mut breakout = flying(200, RADIUS + 1, (0, -UP));
        breakout.tick();
        assert!(breakout.balls[0].way.1 > 0);
    }

    #[test]
    fn a_missed_ball_costs_a_life_and_the_last_ends_the_game() {
        let mut breakout = flying(20, HEIGHT - 2, (0, UP));
        breakout.paddle = WIDTH - HALF;
        for _ in 0..4 {
            breakout.tick();
        }
        assert_eq!(breakout.lives, LIVES - 1);
        assert!(breakout.serving());
        breakout.lives = 1;
        breakout.balls[0] = Ball {
            at: (20 * SUB, (HEIGHT - 2) * SUB),
            way: (0, UP),
            held: None,
        };
        for _ in 0..4 {
            breakout.tick();
        }
        assert!(breakout.over());
        assert!(!breakout.tick());
    }

    #[test]
    fn the_paddle_moves_while_an_arrow_is_held_and_space_serves() {
        let mut breakout = Breakout::new(1);
        let from = breakout.paddle;
        breakout.key_down("left", false);
        assert!(breakout.tick());
        assert!(breakout.tick());
        assert_eq!(breakout.paddle, from - 2 * PADDLE_STEP);
        assert_eq!(
            breakout.balls[0].at.0,
            breakout.paddle * SUB,
            "the served ball rides it"
        );
        breakout.key_up("left");
        assert!(!breakout.tick(), "nothing moves while it waits");
        breakout.key_down("space", false);
        breakout.tick();
        assert!(!breakout.serving());
        assert!(breakout.balls[0].way.1 < 0);
    }

    #[test]
    fn of_two_arrows_held_the_newest_moves_the_paddle_and_up_it_gives_the_other_back() {
        let mut breakout = Breakout::new(1);
        let from = breakout.paddle;
        breakout.key_down("left", false);
        breakout.tick();
        assert_eq!(breakout.paddle, from - PADDLE_STEP);
        // right goes down before left comes up, as a quick turn does
        breakout.key_down("right", false);
        breakout.tick();
        assert_eq!(breakout.paddle, from, "it turned at once");
        breakout.key_up("right");
        breakout.tick();
        assert_eq!(breakout.paddle, from - PADDLE_STEP, "left again");
        breakout.key_up("left");
        assert!(!breakout.tick(), "none held: it stays");
    }

    /// A wall of one brick of `hits` at `(row, col)`, and one in the top
    /// left corner that keeps it standing.
    fn lone(breakout: &mut Breakout, (row, col): (usize, usize), hits: u8) {
        breakout.bricks = [[0; COLS]; ROWS];
        breakout.bricks[0][0] = 1;
        breakout.bricks[row][col] = hits;
    }

    /// A ball rising into the bottom row's sixth brick from 1 px under it.
    fn under_the_sixth(way: (i32, i32)) -> Breakout {
        flying(5 * BRICK_W + 16, BOTTOM + RADIUS + 1, way)
    }

    #[test]
    fn a_wall_with_only_steel_standing_is_clear_scores_its_bonus_and_builds_the_next() {
        let mut breakout = under_the_sixth((0, -UP));
        lone(&mut breakout, (7, 5), 1);
        breakout.bricks[0][0] = STEEL;
        breakout.bricks[7][0] = STEEL;
        breakout.catch(Item::Wide);
        breakout.tick();
        assert_eq!(breakout.stage, 1);
        assert_eq!(breakout.score(), CAUGHT + POINTS[7] + CLEAR);
        assert_eq!(breakout.bricks, stages::wall(1));
        assert!(breakout.serving());
        assert_eq!(breakout.left, [0; 8], "a new wall starts clean");
        assert_eq!(breakout.facts()[0], ("Stage", "2".to_owned()));
        assert_eq!(breakout.pace(), PACE + PACE_WALL);
        // the second wall's bonus is twice the first's
        breakout.balls = under_the_sixth((0, -UP)).balls;
        lone(&mut breakout, (7, 5), 1);
        breakout.bricks[0][0] = 0;
        let before = breakout.score();
        breakout.tick();
        assert_eq!(breakout.score() - before, POINTS[7] + 2 * CLEAR);
    }

    #[test]
    fn a_brick_of_three_hits_takes_three_each_scores_and_a_ball_turned_off_it_is_out() {
        let mut breakout = under_the_sixth((0, -UP));
        lone(&mut breakout, (7, 5), 3);
        breakout.tick();
        assert_eq!(breakout.bricks[7][5], 2);
        assert!(breakout.balls[0].way.1 > 0, "it falls again");
        // the ball ended the tick outside the brick: the next tick strikes
        // nothing
        breakout.tick();
        assert_eq!(breakout.bricks[7][5], 2);
        for left in [1, 0] {
            breakout.balls = under_the_sixth((0, -UP)).balls;
            breakout.tick();
            assert_eq!(breakout.bricks[7][5], left);
        }
        assert_eq!(breakout.score(), 3 * POINTS[7]);
    }

    #[test]
    fn steel_turns_the_ball_a_sixteenth_askew_scores_nothing_and_stands() {
        let mut breakout = under_the_sixth((6, -UP));
        lone(&mut breakout, (7, 5), STEEL);
        breakout.tick();
        assert_eq!(breakout.bricks[7][5], STEEL);
        assert_eq!(breakout.score(), 0);
        let way = breakout.balls[0].way;
        assert!(way.1 > 0 && (way.0 - 6).abs() == 1, "{way:?}");
    }

    /// The ball strikes a brick of the wall's row `row`, one that stands.
    fn struck(breakout: &mut Breakout, row: usize) {
        let mut ball = breakout.balls[0];
        breakout.bricks[row][12] = 3;
        breakout.bounce(&mut ball, (row, 12));
    }

    #[test]
    fn four_and_twelve_bricks_struck_and_the_upper_rows_each_quicken_the_ball() {
        let mut breakout = Breakout::new(1);
        assert_eq!(breakout.pace(), PACE);
        for _ in 0..3 {
            struck(&mut breakout, 8);
        }
        assert_eq!(breakout.pace(), PACE);
        struck(&mut breakout, 8);
        assert_eq!(breakout.pace(), PACE + PACE_UP, "after 4");
        for _ in 4..11 {
            struck(&mut breakout, 8);
        }
        assert_eq!(breakout.pace(), PACE + PACE_UP);
        struck(&mut breakout, 8);
        assert_eq!(breakout.pace(), PACE + 2 * PACE_UP, "after 12");
        struck(&mut breakout, 4);
        assert_eq!(breakout.pace(), PACE + 2 * PACE_UP);
        struck(&mut breakout, 3);
        assert_eq!(breakout.pace(), PACE + 3 * PACE_UP, "the orange rows");
        struck(&mut breakout, 1);
        assert_eq!(breakout.pace(), PACE + 4 * PACE_UP, "the red rows");
        // and the ball's step is the pace
        breakout.balls = flying(200, 250, (0, -UP)).balls;
        breakout.tick();
        assert_eq!(breakout.balls[0].at.1, 250 * SUB - PACE - 4 * PACE_UP);
    }

    #[test]
    fn the_ball_on_the_top_wall_halves_the_paddle_until_the_ball_is_lost() {
        let mut breakout = flying(200, RADIUS + 1, (0, -UP));
        assert_eq!(breakout.half(), HALF);
        breakout.tick();
        assert!(breakout.halved);
        assert_eq!(breakout.half(), HALF / 2);
        breakout.balls[0] = Ball {
            at: (20 * SUB, (HEIGHT - 2) * SUB),
            way: (0, UP),
            held: None,
        };
        breakout.paddle = WIDTH - HALF;
        for _ in 0..4 {
            breakout.tick();
        }
        assert_eq!(breakout.lives, LIVES - 1);
        assert_eq!(breakout.half(), HALF);
    }

    #[test]
    fn each_wall_serves_faster_and_each_round_after_the_first_shortens_the_paddle() {
        let mut breakout = Breakout::new(1);
        let mut on = |stage| {
            breakout.stage = stage;
            (breakout.pace(), breakout.half())
        };
        assert_eq!(on(0), (PACE, HALF));
        assert_eq!(on(1), (PACE + PACE_WALL, HALF));
        assert_eq!(on(7), (PACE + 7 * PACE_WALL, HALF));
        assert_eq!(on(8), (PACE + 8 * PACE_WALL, HALF - HALF_LESS));
        assert_eq!(on(16), (PACE_MOST, HALF - 2 * HALF_LESS));
        assert_eq!(on(40), (PACE_MOST, HALF - 2 * HALF_LESS));
    }

    /// A game at its fastest: a late wall, every speed-up and Fast.
    fn fastest() -> Breakout {
        let mut breakout = Breakout::new(1);
        breakout.stage = 99;
        breakout.hits = 12;
        breakout.top = 0;
        breakout.catch(Item::Fast);
        breakout
    }

    #[test]
    fn no_step_carries_the_ball_past_a_brick_even_at_the_fastest() {
        assert_eq!(fastest().pace(), PACE_MOST);
        let r = RADIUS * SUB;
        // up into a row of steel, from every gap under it a step can close
        let bottom = (WALL_TOP + 6 * BRICK_H) * SUB;
        for lean in [-LEAN, 0, LEAN] {
            for x in [50, 151, 260] {
                for gap in 1..=PACE_MOST {
                    let mut breakout = fastest();
                    breakout.bricks[5] = [STEEL; COLS];
                    breakout.balls[0] = Ball {
                        at: (x * SUB, bottom + r + gap),
                        way: (lean, -UP),
                        held: None,
                    };
                    for _ in 0..3 {
                        breakout.tick();
                        let ball = breakout.balls[0];
                        assert!(ball.at.1 - r >= bottom, "lean {lean}, gap {gap}: {ball:?}");
                    }
                }
            }
        }
        // across into a column of steel
        let left = 6 * BRICK_W * SUB;
        for gap in 1..=PACE_MOST * LEAN / UP {
            let mut breakout = fastest();
            for row in &mut breakout.bricks {
                row[6] = STEEL;
            }
            breakout.balls[0] = Ball {
                at: (left - r - gap, 100 * SUB),
                way: (LEAN, -UP),
                held: None,
            };
            breakout.tick();
            let ball = breakout.balls[0];
            assert!(ball.at.0 + r <= left, "gap {gap}: {ball:?}");
        }
    }

    /// Every brick of the wall, the top row first.
    fn everywhere() -> impl Iterator<Item = (usize, usize)> {
        (0..ROWS).flat_map(|row| (0..COLS).map(move |col| (row, col)))
    }

    #[test]
    fn one_broken_brick_in_eight_drops_a_capsule_and_a_brick_still_standing_none() {
        let mut breakout = Breakout::new(3);
        breakout.bricks = [[1; COLS]; ROWS];
        for brick in everywhere() {
            breakout.strike(brick);
        }
        let dropped = breakout.capsules.len();
        assert!((8..=26).contains(&dropped), "{dropped} of 130");
        for capsule in &breakout.capsules {
            let (x, y) = (capsule.at.0 / SUB, capsule.at.1 / SUB - WALL_TOP);
            let off = ((x - BRICK_W / 2) % BRICK_W, (y - BRICK_H / 2) % BRICK_H);
            assert_eq!(off, (0, 0), "from a brick's middle");
        }
        let mut breakout = Breakout::new(3);
        breakout.bricks = [[3; COLS]; ROWS];
        for brick in everywhere() {
            breakout.strike(brick);
            breakout.strike(brick);
        }
        assert!(breakout.capsules.is_empty());
    }

    #[test]
    fn a_capsule_the_paddle_meets_is_caught_and_scores_and_one_it_misses_falls_away() {
        let mut breakout = Breakout::new(1);
        let y = (PADDLE_Y - 20) * SUB;
        breakout.capsules = vec![
            Capsule {
                item: Item::Wide,
                at: ((breakout.paddle + HALF) * SUB, y),
            },
            Capsule {
                item: Item::Fast,
                at: (20 * SUB, y),
            },
        ];
        for _ in 0..30 {
            breakout.tick();
        }
        assert!(breakout.on(Item::Wide));
        assert_eq!(breakout.capsules.len(), 1, "the missed one still falls");
        for _ in 0..30 {
            breakout.tick();
        }
        assert!(breakout.capsules.is_empty());
        assert!(!breakout.on(Item::Fast));
        assert_eq!(breakout.score(), CAUGHT);
        breakout.capsules = vec![Capsule {
            item: Item::Short,
            at: (breakout.paddle * SUB, y),
        }];
        for _ in 0..30 {
            breakout.tick();
        }
        assert!(breakout.on(Item::Short));
        assert_eq!(breakout.score(), CAUGHT + CAUGHT_BAD);
    }

    #[test]
    fn each_timed_item_lasts_twelve_seconds_and_the_panel_counts_it_down() {
        for item in TIMED {
            let mut breakout = Breakout::new(1);
            breakout.catch(item);
            assert_eq!(breakout.facts()[2], (item.name(), "12 s".to_owned()));
            for _ in 0..63 {
                breakout.tick();
            }
            assert_eq!(breakout.facts()[2], (item.name(), "11 s".to_owned()));
            for _ in 63..LASTS - 1 {
                breakout.tick();
            }
            assert!(breakout.on(item), "{item:?}");
            breakout.tick();
            assert!(!breakout.on(item), "{item:?}");
            assert_eq!(breakout.facts().len(), 2);
        }
    }

    #[test]
    fn wide_and_short_size_the_paddle_and_each_ends_the_other() {
        let mut breakout = Breakout::new(1);
        breakout.catch(Item::Wide);
        assert_eq!(breakout.half(), HALF * 3 / 2);
        breakout.catch(Item::Short);
        assert!(!breakout.on(Item::Wide));
        assert_eq!(breakout.half(), HALF * 5 / 8);
        breakout.catch(Item::Wide);
        assert!(!breakout.on(Item::Short));
        // the wide paddle meets a ball past the plain one's end
        let mut breakout = flying(WIDTH / 2 + HALF + 10, PADDLE_Y - RADIUS - 1, (0, UP));
        breakout.catch(Item::Wide);
        breakout.tick();
        assert!(breakout.balls[0].way.1 < 0);
    }

    #[test]
    fn slow_and_fast_set_the_balls_step_and_each_ends_the_other() {
        let mut breakout = flying(200, 250, (0, -UP));
        breakout.catch(Item::Slow);
        breakout.tick();
        let slow = 250 * SUB - (PACE - PACE_ITEM);
        assert_eq!(breakout.balls[0].at.1, slow);
        breakout.catch(Item::Fast);
        assert!(!breakout.on(Item::Slow));
        breakout.tick();
        assert_eq!(breakout.balls[0].at.1, slow - (PACE + PACE_ITEM));
    }

    #[test]
    fn three_balls_and_a_life_goes_only_with_the_last() {
        let mut breakout = flying(200, 250, (6, -UP));
        breakout.catch(Item::Multi);
        let ways: Vec<_> = breakout.balls.iter().map(|ball| ball.way).collect();
        assert_eq!(ways, [(6, -UP), (-4, -UP), (-14, -UP)]);
        let floor = (HEIGHT + 10) * SUB;
        breakout.balls[1].at.1 = floor;
        breakout.balls[2].at.1 = floor;
        breakout.tick();
        assert_eq!(breakout.balls.len(), 1);
        assert_eq!(breakout.lives, LIVES);
        breakout.balls[0].at.1 = floor;
        breakout.tick();
        assert_eq!(breakout.lives, LIVES - 1);
        assert!(breakout.serving());
    }

    #[test]
    fn a_sticky_paddle_holds_the_ball_where_it_lands_and_space_lets_it_go_that_way() {
        let mut breakout = flying(WIDTH / 2 + 10, PADDLE_Y - RADIUS - 1, (0, UP));
        breakout.catch(Item::Catch);
        breakout.tick();
        assert_eq!(breakout.balls[0].held, Some(10));
        breakout.key_down("right", false);
        breakout.tick();
        breakout.key_up("right");
        let rides = (breakout.paddle + 10) * SUB;
        assert_eq!(breakout.balls[0].at.0, rides, "it rides the paddle");
        breakout.key_down("space", false);
        breakout.tick();
        let ball = breakout.balls[0];
        assert_eq!(ball.held, None);
        assert!(ball.way.0 > 0 && ball.way.1 < 0, "{ball:?}");
    }

    #[test]
    fn a_ball_held_past_the_end_of_a_paddle_that_shrank_goes_at_the_ends_angle() {
        // held at the wide paddle's end, then Short ends Wide
        let wide = HALF * 3 / 2;
        let mut breakout = flying(WIDTH / 2 + wide, PADDLE_Y - RADIUS - 1, (0, UP));
        breakout.catch(Item::Wide);
        breakout.catch(Item::Catch);
        breakout.tick();
        assert_eq!(breakout.balls[0].held, Some(wide));
        breakout.catch(Item::Short);
        breakout.key_down("space", false);
        breakout.tick();
        let short = HALF * 5 / 8;
        let way = (LEAN * short / (short + RADIUS), -UP);
        assert_eq!(
            breakout.balls[0].way, way,
            "no flatter than a hit on the end"
        );
    }

    #[test]
    fn the_laser_fires_from_the_paddles_ends_one_volley_at_a_time() {
        // the ball in flight at the far right, under a row of bricks
        let mut breakout = flying(400, 250, (0, -UP));
        breakout.bricks = [[0; COLS]; ROWS];
        breakout.bricks[7] = [1; COLS];
        breakout.key_down("space", false);
        breakout.tick();
        assert!(breakout.shots.is_empty(), "no laser, no shot");
        breakout.catch(Item::Laser);
        breakout.key_down("space", false);
        breakout.tick();
        let ends = [WIDTH / 2 - (HALF - 4), WIDTH / 2 + (HALF - 4)];
        let shots: Vec<_> = breakout.shots.iter().map(|(x, _)| x / SUB).collect();
        assert_eq!(shots, ends);
        // Space again while they fly: the same two climb, no new ones
        let climbed: Vec<_> = breakout.shots.iter().map(|(x, y)| (*x, y - SHOT)).collect();
        breakout.key_down("space", false);
        breakout.tick();
        assert_eq!(breakout.shots, climbed, "one volley at a time");
        for _ in 0..30 {
            breakout.tick();
        }
        assert!(breakout.shots.is_empty());
        assert_eq!(breakout.bricks[7][5], 0);
        assert_eq!(breakout.bricks[7][7], 0);
        assert_eq!(breakout.bricks[7][6], 1, "between the shots");
    }

    #[test]
    fn an_extra_life_gives_one_up_to_five() {
        let mut breakout = Breakout::new(1);
        for lives in [4, 5, 5] {
            breakout.catch(Item::Life);
            assert_eq!(breakout.lives, lives);
        }
    }

    #[test]
    fn a_lost_ball_ends_the_items_capsules_shots_and_speed_ups() {
        let mut breakout = flying(20, HEIGHT - 2, (0, UP));
        breakout.paddle = WIDTH - 3 * HALF;
        breakout.catch(Item::Laser);
        breakout.catch(Item::Fast);
        breakout.hits = 12;
        breakout.top = 0;
        breakout.halved = true;
        breakout.shots = vec![(300 * SUB, 300 * SUB)];
        breakout.capsules = vec![Capsule {
            item: Item::Life,
            at: (300 * SUB, 100 * SUB),
        }];
        for _ in 0..4 {
            breakout.tick();
        }
        assert_eq!(breakout.lives, LIVES - 1);
        assert_eq!(breakout.left, [0; 8]);
        assert!(breakout.capsules.is_empty() && breakout.shots.is_empty());
        assert_eq!((breakout.pace(), breakout.half()), (PACE, HALF));
    }

    #[test]
    fn a_held_space_acts_once() {
        let mut breakout = Breakout::new(1);
        breakout.key_down("space", true);
        breakout.tick();
        assert!(breakout.serving());
    }

    /// Plays frames `ticks` by a rule of thumb: the paddle chases the
    /// lowest falling ball, meeting it at a place that moves along the
    /// paddle, and Space goes down every half second.
    fn pilot(breakout: &mut Breakout, ticks: std::ops::Range<usize>) {
        let mut held = 0;
        for tick in ticks {
            let half = breakout.half();
            let aim = breakout
                .balls
                .iter()
                .filter(|ball| ball.held.is_none() && ball.way.1 > 0)
                .max_by_key(|ball| ball.at.1)
                .map_or(WIDTH / 2, |ball| ball.at.0 / SUB)
                + (tick / 61 % 9) as i32 * half / 5
                - half * 4 / 5;
            let way = (aim - breakout.paddle).signum();
            if way != held {
                breakout.key_up("left");
                breakout.key_up("right");
                match way {
                    -1 => breakout.key_down("left", false),
                    1 => breakout.key_down("right", false),
                    _ => false,
                };
                held = way;
            }
            if tick % 30 == 0 {
                breakout.key_down("space", false);
            }
            breakout.tick();
        }
    }

    #[test]
    fn the_same_seed_and_keys_play_the_same_game() {
        let played = |seed| {
            let mut breakout = Breakout::new(seed);
            pilot(&mut breakout, 0..20_000);
            breakout
        };
        let game = played(5);
        assert!(game.score() > 300, "{}", game.score());
        assert_eq!(format!("{game:?}"), format!("{:?}", played(5)));
        assert_ne!(format!("{game:?}"), format!("{:?}", played(6)));
    }

    #[test]
    fn the_pilot_clears_every_wall_of_two_rounds_given_lives_enough() {
        for stage in 0..2 * stages::COUNT {
            let mut breakout = Breakout::new(u64::from(stage));
            breakout.stage = stage;
            breakout.bricks = stages::wall(stage);
            for from in (0..100_000).step_by(100) {
                pilot(&mut breakout, from..from + 100);
                breakout.lives = LIVES;
                if breakout.stage > stage {
                    break;
                }
            }
            assert_eq!(breakout.stage, stage + 1, "wall {stage}");
        }
    }
}
