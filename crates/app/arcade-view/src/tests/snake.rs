//! Snake on the fake host: the headline behaviours of a game on the clock.
use super::*;
use crate::snake::Heading;

fn head(opened: &mut Opened) -> (i32, i32) {
    opened.game(|game| match game {
        Game::Snake(snake) => snake.body[0],
        _ => panic!("snake is on screen"),
    })
}

#[test]
fn a_key_moves_the_snake_on_the_next_tick() {
    let mut opened = seated();
    opened.start(SNAKE);
    let (x, y) = head(&mut opened);
    let renders = opened.cx.renders();
    opened.key("up");
    assert_eq!(head(&mut opened), (x, y), "not before the tick");
    assert_eq!(
        opened.cx.renders(),
        renders,
        "a key draws nothing by itself"
    );
    opened.tick(1);
    assert_eq!(head(&mut opened), (x, y - 1));
    assert!(opened.cx.renders() > renders);
}

/// The reader seated with no best of Snake yet.
fn unbeaten() -> Opened {
    let mut world = World::default();
    world.bests[SNAKE] = 0;
    opened(world, Some(READER))
}

/// Sets the snake going and runs it right into the wall, past the food put
/// in its way.
fn crash(opened: &mut Opened) {
    opened.key("right");
    let x = opened.game(|game| match game {
        Game::Snake(snake) => {
            let (x, y) = snake.body[0];
            snake.food = Some((x + 1, y));
            assert_eq!(snake.heading, Heading::Right);
            x
        }
        _ => panic!("snake is on screen"),
    });
    // the last step is the one into the wall
    opened.tick((crate::snake::COLS - x) as usize);
}

#[test]
fn an_ended_game_posts_its_score_once_and_drops_the_clock() {
    let mut opened = unbeaten();
    opened.start(SNAKE);
    crash(&mut opened);
    assert_eq!(
        opened.ops(),
        [Op::Score {
            game: SNAKE as u8,
            score: 10
        }]
    );
    assert!(!opened.ticks.subscribed());
    assert_eq!(opened.post(), Post::Posted);
    shows(
        &opened.cx,
        &["Game over", "Posted to the board.", "Play again", "Top 10"],
    );
    // a block after it, or the view leaving the screen, posts nothing more
    opened.changes.send(None);
    opened.visible.send(false);
    opened.cx.run_until_parked();
    assert_eq!(opened.ops().len(), 1);
    opened.visible.send(true);
    opened.cx.run_until_parked();
    // Enter plays again: a new deal, waiting for its first arrow
    opened.key("enter");
    assert_eq!(opened.cx.host().requests::<HostId>().len(), 2);
    assert_eq!(opened.post(), Post::Playing);
    assert!(!opened.ticks.subscribed());
    opened.key("up");
    assert!(opened.ticks.subscribed());
}

#[test]
fn without_an_account_an_ended_game_posts_nothing_and_says_so() {
    let mut opened = opened(World::default(), None);
    opened.start(SNAKE);
    crash(&mut opened);
    assert_eq!(opened.ops(), []);
    assert_eq!(opened.post(), Post::Unsigned);
    shows(
        &opened.cx,
        &["Scores go on the board from an account; this one stays here."],
    );
}

#[test]
fn a_score_at_or_under_the_best_is_not_sent() {
    let mut opened = seated();
    opened.start(SNAKE);
    crash(&mut opened);
    // ten, under the reader's 120
    assert_eq!(opened.ops(), []);
    assert_eq!(opened.post(), Post::Kept);
    shows(&opened.cx, &["Your best stays 120."]);
}

#[test]
fn a_refused_score_says_why() {
    let mut opened = unbeaten();
    opened.world.borrow_mut().refusal = Some("the node is catching up".into());
    opened.start(SNAKE);
    crash(&mut opened);
    assert_eq!(opened.ops().len(), 1);
    shows(
        &opened.cx,
        &["The score was not posted: the node is catching up"],
    );
}
