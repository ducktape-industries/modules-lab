//! Breakout: a wall of six rows of thirteen bricks, a paddle and a ball.
//! The ball bounces off the walls, the bricks it breaks and the paddle,
//! which sends it off at the angle of where it struck; a ball the paddle
//! misses costs one of three lives. A brick of the two bottom rows is 1, of
//! the middle two 4 and of the top two 7, as Atari counted them. A wall
//! broken whole is built again, the ball a little faster. The tick is 16
//! ms, a frame: the paddle moves while an arrow is held, the newest of two
//! held, and Space serves.
//! The field counts in units of 1/16 px, so a tick moves the ball by whole
//! numbers and the same keys play the same game.
use ducktape_view_guest::Canvas;
use ducktape_view_guest::prelude::*;
use gpui::point;

use crate::games::{Fact, Rng, square};

/// The field, in px at a scale of one.
pub(crate) const WIDTH: i32 = 416;
pub(crate) const HEIGHT: i32 = 352;
pub(crate) const KEYS: &str = "← → move the paddle, Space serves";
pub(crate) const PERIOD: i64 = 16;

const BRICK_COLS: usize = 13;
const BRICK_ROWS: usize = 6;
const BRICK_W: i32 = 32;
const BRICK_H: i32 = 12;
const WALL_TOP: i32 = 48;
/// A row's points, the top row first.
const POINTS: [u64; BRICK_ROWS] = [7, 7, 4, 4, 1, 1];
const PADDLE_W: i32 = 64;
const PADDLE_H: i32 = 8;
const PADDLE_Y: i32 = HEIGHT - 24;
/// The paddle's step a tick, in px.
const PADDLE_STEP: i32 = 6;
const RADIUS: i32 = 4;
const LIVES: u8 = 3;
/// Sixteenths of a px: the unit the ball moves in.
const SUB: i32 = 16;
/// The ball's climb a tick on the first wall, and on each wall after it
/// one more px in four ticks, to at most 6 px a tick: under a brick's
/// height and the ball's width, so it never passes through one.
const CLIMB: i32 = 3 * SUB;
const CLIMB_MOST: i32 = 6 * SUB;
/// The most sideways a tick the paddle's edge sends it.
const SWING: i32 = 4 * SUB;

pub(crate) struct Breakout {
    pub(crate) bricks: [[bool; BRICK_COLS]; BRICK_ROWS],
    /// The paddle's middle, in px.
    pub(crate) paddle: i32,
    /// The ball's middle, in sixteenths of a px.
    pub(crate) ball: (i32, i32),
    pub(crate) speed: (i32, i32),
    /// The ball rests on the paddle until Space serves it.
    pub(crate) serving: bool,
    /// The arrows held, -1 left and 1 right, the newest last: the paddle
    /// goes its way.
    arrows: Vec<i32>,
    serve: bool,
    pub(crate) lives: u8,
    walls: u64,
    score: u64,
    rng: Rng,
}

impl Breakout {
    pub(crate) fn new(seed: u64) -> Breakout {
        let mut breakout = Breakout {
            bricks: [[true; BRICK_COLS]; BRICK_ROWS],
            paddle: WIDTH / 2,
            ball: (0, 0),
            speed: (0, 0),
            serving: true,
            arrows: Vec::new(),
            serve: false,
            lives: LIVES,
            walls: 0,
            score: 0,
            rng: Rng::new(seed),
        };
        breakout.rest();
        breakout
    }

    /// The ball on the paddle, waiting to be served.
    fn rest(&mut self) {
        self.serving = true;
        self.ball = (self.paddle * SUB, (PADDLE_Y - RADIUS) * SUB);
    }

    fn climb(&self) -> i32 {
        (CLIMB + self.walls as i32 * SUB / 4).min(CLIMB_MOST)
    }

    /// An arrow held, or Space; read on the next tick.
    pub(crate) fn key_down(&mut self, key: &str) -> bool {
        if let Some(arrow) = arrow(key) {
            self.arrows.retain(|held| *held != arrow);
            self.arrows.push(arrow);
        } else if matches!(key, "space" | "up" | "w") {
            self.serve = true;
        }
        false
    }

    /// An arrow came up: the other, if it is still held, moves the paddle.
    pub(crate) fn key_up(&mut self, key: &str) {
        if let Some(arrow) = arrow(key) {
            self.arrows.retain(|held| *held != arrow);
        }
    }

    /// One frame: the paddle, the serve, then the ball's step and what it
    /// struck.
    pub(crate) fn tick(&mut self) -> bool {
        if self.over() {
            return false;
        }
        let step = PADDLE_STEP * self.arrows.last().copied().unwrap_or(0);
        let paddle = (self.paddle + step).clamp(PADDLE_W / 2, WIDTH - PADDLE_W / 2);
        let moved = paddle != self.paddle;
        self.paddle = paddle;
        if self.serving {
            if std::mem::take(&mut self.serve) {
                self.serving = false;
                let side = if self.rng.below(2) == 0 { -1 } else { 1 };
                self.speed = (side * SWING / 2, -self.climb());
            } else {
                self.rest();
                return moved;
            }
        }
        self.serve = false;
        self.fly();
        true
    }

    fn fly(&mut self) {
        let (mut x, mut y) = (self.ball.0 + self.speed.0, self.ball.1 + self.speed.1);
        let (r, w) = (RADIUS * SUB, WIDTH * SUB);
        if x < r || x > w - r {
            x = x.clamp(r, w - r);
            self.speed.0 = -self.speed.0;
        }
        if y < r {
            y = r;
            self.speed.1 = self.speed.1.abs();
        }
        self.ball = (x, y);
        if y - r > HEIGHT * SUB {
            self.lives -= 1;
            self.rest();
            return;
        }
        self.paddled();
        self.broke();
    }

    /// A falling ball that meets the paddle goes back up, off to the side
    /// it struck by as much as it struck off the middle.
    fn paddled(&mut self) {
        let (x, y) = self.ball;
        let top = PADDLE_Y * SUB;
        let reach = (PADDLE_W / 2 + RADIUS) * SUB;
        let off = x - self.paddle * SUB;
        let level = y + RADIUS * SUB >= top && y + RADIUS * SUB - self.speed.1 <= top;
        if self.speed.1 > 0 && level && off.abs() <= reach {
            self.speed = (SWING * off / reach, -self.climb());
            self.ball.1 = top - RADIUS * SUB;
        }
    }

    /// The first brick the ball overlaps breaks, and the ball turns off the
    /// side it crossed last. A wall broken whole is built again.
    fn broke(&mut self) {
        let (x, y) = self.ball;
        let r = RADIUS * SUB;
        let struck = (0..BRICK_ROWS)
            .flat_map(|row| (0..BRICK_COLS).map(move |col| (row, col)))
            .filter(|(row, col)| self.bricks[*row][*col])
            .find_map(|(row, col)| {
                let (left, top) = (
                    col as i32 * BRICK_W * SUB,
                    (WALL_TOP + row as i32 * BRICK_H) * SUB,
                );
                let (right, bottom) = (left + BRICK_W * SUB, top + BRICK_H * SUB);
                let across = (x + r).min(right) - (x - r).max(left);
                let down = (y + r).min(bottom) - (y - r).max(top);
                (across > 0 && down > 0).then_some((row, col, across, down))
            });
        let Some((row, col, across, down)) = struck else {
            return;
        };
        self.bricks[row][col] = false;
        self.score += POINTS[row];
        // the side it crossed last turns it: of the two overlaps, the one
        // its speed made in the shorter time
        match across * self.speed.1.abs() < down * self.speed.0.abs() {
            true => self.speed.0 = -self.speed.0,
            false => self.speed.1 = -self.speed.1,
        }
        if self.bricks.iter().flatten().all(|brick| !brick) {
            self.bricks = [[true; BRICK_COLS]; BRICK_ROWS];
            self.walls += 1;
            self.rest();
        }
    }

    pub(crate) fn score(&self) -> u64 {
        self.score
    }

    pub(crate) fn over(&self) -> bool {
        self.lives == 0
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        vec![
            ("Lives", self.lives.to_string()),
            ("Wall", (self.walls + 1).to_string()),
        ]
    }

    /// The field at `scale` px a px: the wall, the paddle and the ball.
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
                _ => theme.success,
            };
            for (col, standing) in bricks.iter().enumerate() {
                if *standing {
                    let (x, y) = (col as i32 * BRICK_W, WALL_TOP + row as i32 * BRICK_H);
                    drawn = drawn.rect(at(x + 1, y + 1, BRICK_W - 2, BRICK_H - 2), color);
                }
            }
        }
        let paddle = at(self.paddle - PADDLE_W / 2, PADDLE_Y, PADDLE_W, PADDLE_H);
        drawn = drawn.rect(paddle, theme.foreground);
        if self.over() {
            return drawn;
        }
        let middle = point(
            px(self.ball.0 as f32 / SUB as f32 * scale),
            px(self.ball.1 as f32 / SUB as f32 * scale),
        );
        drawn.circle(middle, px(RADIUS as f32 * scale), theme.foreground)
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

/// The hub's picture of the game: a wall with a gap, the ball on its way.
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
    drawn
        .rect(square(80., 104., 44., 5.), theme.foreground)
        .circle(point(px(112.), px(72.)), px(4.), theme.foreground)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ball in flight at `(x, y)` px, moving `speed` sixteenths a tick.
    fn flying(x: i32, y: i32, speed: (i32, i32)) -> Breakout {
        let mut breakout = Breakout::new(1);
        breakout.serving = false;
        breakout.ball = (x * SUB, y * SUB);
        breakout.speed = speed;
        breakout
    }

    #[test]
    fn a_brick_struck_from_below_breaks_scores_and_turns_the_ball_down() {
        // under the bottom row's sixth brick, rising
        let bottom = WALL_TOP + BRICK_ROWS as i32 * BRICK_H;
        let mut breakout = flying(5 * BRICK_W + 16, bottom + RADIUS + 1, (0, -2 * SUB));
        breakout.tick();
        assert!(!breakout.bricks[5][5]);
        assert_eq!(breakout.score(), 1);
        assert!(breakout.speed.1 > 0, "it falls again");
        assert_eq!(breakout.bricks.iter().flatten().filter(|b| !**b).count(), 1);
    }

    #[test]
    fn a_ball_rising_straight_into_a_bricks_corner_turns_down() {
        // straight up, its left edge 1 px under the bottom row's seventh
        // brick and the rest under the eighth: it goes in through the bottom
        let mut breakout = flying(7 * BRICK_W + RADIUS - 1, 125, (0, -3 * SUB));
        breakout.tick();
        assert!(breakout.speed.1 > 0, "{:?}", breakout.speed);
        assert_eq!(breakout.bricks.iter().flatten().filter(|b| !**b).count(), 1);
    }

    #[test]
    fn a_brick_struck_on_its_side_turns_the_ball_sideways() {
        // an empty slot in the bottom row, the ball going right into its
        // neighbour's left side
        let mut breakout = flying(0, 0, (0, 0));
        breakout.bricks[5][3] = false;
        let y = WALL_TOP + 5 * BRICK_H + BRICK_H / 2;
        breakout.ball = ((4 * BRICK_W - RADIUS) * SUB - SUB, y * SUB);
        breakout.speed = (2 * SUB, 0);
        breakout.tick();
        assert!(!breakout.bricks[5][4]);
        assert!(breakout.speed.0 < 0);
    }

    #[test]
    fn the_paddle_sends_the_ball_up_off_to_the_side_it_struck() {
        let mut breakout = flying(WIDTH / 2, PADDLE_Y - RADIUS - 1, (0, 2 * SUB));
        breakout.tick();
        assert_eq!(breakout.speed, (0, -CLIMB), "the middle: straight up");
        let mut breakout = flying(
            WIDTH / 2 + PADDLE_W / 2,
            PADDLE_Y - RADIUS - 1,
            (0, 2 * SUB),
        );
        breakout.tick();
        assert!(
            breakout.speed.0 > 0 && breakout.speed.1 < 0,
            "{:?}",
            breakout.speed
        );
        let mut breakout = flying(WIDTH / 2 - 20, PADDLE_Y - RADIUS - 1, (0, 2 * SUB));
        breakout.tick();
        assert!(breakout.speed.0 < 0);
    }

    #[test]
    fn the_walls_turn_the_ball_back() {
        let mut breakout = flying(RADIUS + 1, 200, (-3 * SUB, 2 * SUB));
        breakout.tick();
        assert!(breakout.speed.0 > 0);
        let mut breakout = flying(200, RADIUS + 1, (SUB, -3 * SUB));
        breakout.bricks = [[false; BRICK_COLS]; BRICK_ROWS];
        breakout.bricks[0][0] = true;
        breakout.tick();
        assert!(breakout.speed.1 > 0);
    }

    #[test]
    fn a_missed_ball_costs_a_life_and_the_last_ends_the_game() {
        let mut breakout = flying(20, HEIGHT - 2, (0, 4 * SUB));
        breakout.paddle = WIDTH - PADDLE_W;
        for _ in 0..4 {
            breakout.tick();
        }
        assert_eq!(breakout.lives, LIVES - 1);
        assert!(breakout.serving);
        breakout.lives = 1;
        breakout.serving = false;
        breakout.ball = (20 * SUB, (HEIGHT - 2) * SUB);
        breakout.speed = (0, 4 * SUB);
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
        breakout.key_down("left");
        assert!(breakout.tick());
        assert!(breakout.tick());
        assert_eq!(breakout.paddle, from - 2 * PADDLE_STEP);
        assert_eq!(
            breakout.ball.0,
            breakout.paddle * SUB,
            "the served ball rides it"
        );
        breakout.key_up("left");
        assert!(!breakout.tick(), "nothing moves while it waits");
        breakout.key_down("space");
        breakout.tick();
        assert!(!breakout.serving);
        assert!(breakout.speed.1 < 0);
    }

    #[test]
    fn of_two_arrows_held_the_newest_moves_the_paddle_and_up_it_gives_the_other_back() {
        let mut breakout = Breakout::new(1);
        let from = breakout.paddle;
        breakout.key_down("left");
        breakout.tick();
        assert_eq!(breakout.paddle, from - PADDLE_STEP);
        // right goes down before left comes up, as a quick turn does
        breakout.key_down("right");
        breakout.tick();
        assert_eq!(breakout.paddle, from, "it turned at once");
        breakout.key_up("right");
        breakout.tick();
        assert_eq!(breakout.paddle, from - PADDLE_STEP, "left again");
        breakout.key_up("left");
        assert!(!breakout.tick(), "none held: it stays");
    }

    #[test]
    fn a_wall_broken_whole_is_built_again_and_the_ball_climbs_faster() {
        let bottom = WALL_TOP + BRICK_ROWS as i32 * BRICK_H;
        let mut breakout = flying(16, bottom + RADIUS + 1, (0, -2 * SUB));
        breakout.bricks = [[false; BRICK_COLS]; BRICK_ROWS];
        breakout.bricks[5][0] = true;
        breakout.tick();
        assert!(breakout.bricks.iter().flatten().all(|brick| *brick));
        assert!(breakout.serving);
        assert_eq!(breakout.facts()[1].1, "2");
        assert!(breakout.climb() > CLIMB);
    }
}
