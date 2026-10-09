//! Snake: steer a snake to the food on a walled field. Each food makes it
//! a cell longer and is worth 10; running into a wall or into itself ends
//! the game. The snake waits for the first arrow; from then a tick is one
//! step, so the clock's period is the snake's speed: 160 ms a step,
//! quicker by 10 ms for every five foods, down to 70 ms. A turn is read on
//! the next step; two turns may wait, so a quick U-turn is not lost.
use std::collections::VecDeque;

use ducktape_view_guest::Canvas;
use ducktape_view_guest::prelude::*;
use gpui::point;

use crate::games::{Fact, Rng, square};

pub(crate) const COLS: i32 = 24;
pub(crate) const ROWS: i32 = 16;
pub(crate) const KEYS: &str = "Arrows steer; the first one starts";

/// The turns that may wait for a step.
const WAITING: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Heading {
    Up,
    Down,
    Left,
    Right,
}

impl Heading {
    fn step(self) -> (i32, i32) {
        match self {
            Heading::Up => (0, -1),
            Heading::Down => (0, 1),
            Heading::Left => (-1, 0),
            Heading::Right => (1, 0),
        }
    }

    fn back(self) -> Heading {
        match self {
            Heading::Up => Heading::Down,
            Heading::Down => Heading::Up,
            Heading::Left => Heading::Right,
            Heading::Right => Heading::Left,
        }
    }
}

pub(crate) struct Snake {
    /// Head first.
    pub(crate) body: VecDeque<(i32, i32)>,
    pub(crate) heading: Heading,
    turns: VecDeque<Heading>,
    pub(crate) food: Option<(i32, i32)>,
    eaten: u64,
    /// Whether an arrow has set it going.
    started: bool,
    over: bool,
    rng: Rng,
}

impl Snake {
    /// Three cells long in the middle of the field, heading right.
    pub(crate) fn new(seed: u64) -> Snake {
        let (x, y) = (COLS / 2, ROWS / 2);
        let mut snake = Snake {
            body: VecDeque::from([(x, y), (x - 1, y), (x - 2, y)]),
            heading: Heading::Right,
            turns: VecDeque::new(),
            food: None,
            eaten: 0,
            started: false,
            over: false,
            rng: Rng::new(seed),
        };
        snake.food = snake.free_cell();
        snake
    }

    /// An arrow queues a turn for a coming step: never straight back, and
    /// never the way the snake will already be heading.
    pub(crate) fn key(&mut self, key: &str) -> bool {
        let turn = match key {
            "up" | "w" => Heading::Up,
            "down" | "s" => Heading::Down,
            "left" | "a" => Heading::Left,
            "right" | "d" => Heading::Right,
            _ => return false,
        };
        let last = self.turns.back().copied().unwrap_or(self.heading);
        if turn != last.back() {
            self.started = true;
        }
        if turn != last && turn != last.back() && self.turns.len() < WAITING {
            self.turns.push_back(turn);
        }
        false
    }

    /// One step: the next turn read, the head moved on, the tail after it
    /// unless the head found the food.
    pub(crate) fn tick(&mut self) -> bool {
        if self.over {
            return false;
        }
        self.heading = self.turns.pop_front().unwrap_or(self.heading);
        let (dx, dy) = self.heading.step();
        let (x, y) = self.body[0];
        let head = (x + dx, y + dy);
        let fed = self.food == Some(head);
        // the tail moves out of its cell this step unless the snake grows
        let stays = self.body.len() - usize::from(!fed);
        let walled = !(0..COLS).contains(&head.0) || !(0..ROWS).contains(&head.1);
        if walled || self.body.iter().take(stays).any(|cell| *cell == head) {
            self.over = true;
            return true;
        }
        self.body.push_front(head);
        if fed {
            self.eaten += 1;
            self.food = self.free_cell();
            // a field the snake fills has no food left: that is the end
            self.over = self.food.is_none();
        } else {
            self.body.pop_back();
        }
        true
    }

    /// A cell the snake is not on, any of them alike.
    fn free_cell(&mut self) -> Option<(i32, i32)> {
        let free = (COLS * ROWS) as usize - self.body.len();
        if free == 0 {
            return None;
        }
        let pick = self.rng.below(free as u64) as usize;
        let cells = (0..ROWS).flat_map(|y| (0..COLS).map(move |x| (x, y)));
        cells.filter(|cell| !self.body.contains(cell)).nth(pick)
    }

    /// A step's length once an arrow has set the snake going; no clock
    /// before.
    pub(crate) fn period(&self) -> Option<i64> {
        self.started
            .then(|| (160 - 10 * (self.eaten / 5) as i64).max(70))
    }

    pub(crate) fn score(&self) -> u64 {
        self.eaten * 10
    }

    pub(crate) fn over(&self) -> bool {
        self.over
    }

    pub(crate) fn facts(&self) -> Vec<Fact> {
        vec![("Length", self.body.len().to_string())]
    }

    /// The field, `cell` px a cell: the snake in ink, the food in red.
    pub(crate) fn draw(&self, cell: f32, theme: &Theme) -> Canvas {
        let mut drawn = canvas()
            .w(px(COLS as f32 * cell))
            .h(px(ROWS as f32 * cell))
            .rect(
                square(0., 0., COLS as f32 * cell, ROWS as f32 * cell),
                theme.surface,
            );
        if let Some((x, y)) = self.food {
            let middle = point(px((x as f32 + 0.5) * cell), px((y as f32 + 0.5) * cell));
            drawn = drawn.circle(middle, px(cell * 0.32), theme.danger);
        }
        for (x, y) in &self.body {
            let at = square(
                *x as f32 * cell + 1.,
                *y as f32 * cell + 1.,
                cell - 2.,
                cell - 2.,
            );
            drawn = drawn.rect(at, theme.foreground);
        }
        drawn
    }
}

/// The hub's picture of the game: a snake turning toward its food, in the
/// middle of its 208 x 120 px.
pub(crate) fn preview(theme: &Theme) -> Canvas {
    let (cell, left, top) = (12., 48., 18.);
    let body = [
        (0, 5),
        (1, 5),
        (2, 5),
        (3, 5),
        (3, 4),
        (3, 3),
        (4, 3),
        (5, 3),
        (6, 3),
    ];
    let mut drawn = canvas().size_full();
    for (x, y) in body {
        let at = square(
            left + x as f32 * cell + 1.,
            top + y as f32 * cell - 18.,
            cell - 2.,
            cell - 2.,
        );
        drawn = drawn.rect(at, theme.foreground);
    }
    let food = point(px(left + 9.5 * cell), px(top + 3.5 * cell - 18.));
    drawn.circle(food, px(4.), theme.danger)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A snake laid out by hand, heading `heading`, with no food on the field.
    fn laid(body: &[(i32, i32)], heading: Heading) -> Snake {
        let mut snake = Snake::new(1);
        snake.body = body.iter().copied().collect();
        snake.heading = heading;
        snake.food = None;
        snake
    }

    #[test]
    fn a_turn_is_read_on_the_next_step() {
        let mut snake = laid(&[(5, 5), (4, 5), (3, 5)], Heading::Right);
        assert!(!snake.key("up"), "a key draws nothing by itself");
        assert_eq!(snake.body[0], (5, 5));
        assert!(snake.tick());
        assert_eq!(snake.body, [(5, 4), (5, 5), (4, 5)]);
    }

    #[test]
    fn straight_back_is_no_turn_and_two_turns_wait() {
        let mut snake = laid(&[(5, 5), (4, 5), (3, 5)], Heading::Right);
        snake.key("left");
        snake.tick();
        assert_eq!(snake.body[0], (6, 5), "it kept on right");
        // up then left, both before a step: a U-turn over two steps
        snake.key("up");
        snake.key("left");
        snake.key("down");
        snake.tick();
        snake.tick();
        assert_eq!(snake.body[0], (5, 4));
        assert_eq!(snake.heading, Heading::Left);
    }

    #[test]
    fn running_into_itself_ends_the_game() {
        // a hook: going down from (5, 5) lands on (5, 6), its own body
        let body = [(5, 5), (6, 5), (6, 6), (5, 6), (4, 6)];
        let mut snake = laid(&body, Heading::Left);
        snake.key("down");
        assert!(snake.tick());
        assert!(snake.over());
        assert_eq!(snake.body.len(), 5, "an ended snake does not move");
        assert!(!snake.tick());
    }

    #[test]
    fn the_cell_its_tail_leaves_is_free_to_enter() {
        // a ring of four: the head steps where the tail just was
        let body = [(5, 5), (5, 6), (6, 6), (6, 5)];
        let mut snake = laid(&body, Heading::Up);
        snake.key("right");
        snake.tick();
        assert!(!snake.over());
        assert_eq!(snake.body[0], (6, 5));
    }

    #[test]
    fn a_wall_ends_the_game() {
        let mut snake = laid(&[(COLS - 1, 3), (COLS - 2, 3)], Heading::Right);
        snake.tick();
        assert!(snake.over());
    }

    #[test]
    fn food_grows_it_scores_ten_and_comes_back_elsewhere() {
        let mut snake = laid(&[(5, 5), (4, 5), (3, 5)], Heading::Right);
        snake.food = Some((6, 5));
        snake.tick();
        assert_eq!(snake.body, [(6, 5), (5, 5), (4, 5), (3, 5)]);
        assert_eq!(snake.score(), 10);
        let food = snake.food.expect("new food");
        assert!(!snake.body.contains(&food));
    }

    #[test]
    fn it_starts_on_an_arrow_and_quickens_every_five_foods_to_seventy_ms() {
        let mut snake = laid(&[(5, 5)], Heading::Right);
        assert_eq!(snake.period(), None, "it waits for an arrow");
        snake.key("left");
        assert_eq!(snake.period(), None, "straight back does not start it");
        snake.key("right");
        assert_eq!(snake.period(), Some(160));
        snake.eaten = 5;
        assert_eq!(snake.period(), Some(150));
        snake.eaten = 500;
        assert_eq!(snake.period(), Some(70));
    }

    #[test]
    fn one_seed_deals_one_game() {
        let food = |seed| Snake::new(seed).food;
        assert_eq!(food(7), food(7));
        assert_ne!(
            (1..20)
                .map(food)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            1
        );
    }
}
