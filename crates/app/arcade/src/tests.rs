//! The op once as meant and once as an attack: each refusal leaves the
//! store exactly as it was. The harness is the [`Arcade`] module over a
//! [`MockHost`]; the accounts that post each sign the env of their op.
use guest::{Env, Error, MockHost, Module, QueryCtx, code};

use crate::{Arcade, Entry, GAMES, MAX_TOP, MODULE, Op, Query, Reply, describe};

const ADA: u64 = 1;
const BO: u64 = 2;
const CY: u64 = 3;

/// Places in `GAMES`.
const TETRIS: u8 = 0;
const SNAKE: u8 = 1;

#[derive(Default)]
struct Hall {
    store: MockHost,
}

impl Hall {
    fn env(&self) -> Env {
        MockHost::env(MODULE)
    }

    fn reads(&self) -> QueryCtx {
        self.store.query(self.env())
    }

    /// Posts `score` of `game` as `who`. A refusal has left the store as
    /// it found it, or this panics.
    #[track_caller]
    fn post(&mut self, who: u64, game: u8, score: u64) -> Result<(), Error> {
        let signed = self.env().signed(who.to_be_bytes(), Some(who));
        let ctx = self.store.exec(signed);
        self.store
            .attempt(|| Arcade::execute(&ctx, Op::Score { game, score }))
    }

    #[track_caller]
    fn ok(&mut self, who: u64, game: u8, score: u64) {
        if let Err(refusal) = self.post(who, game, score) {
            panic!("refused: {refusal}");
        }
    }

    fn ask(&self, question: Query) -> Result<Reply, Error> {
        Arcade::query(&self.reads(), question)
    }

    fn top(&self, game: u8, limit: u32) -> Vec<(u64, u64)> {
        let Ok(Reply::Top(rows)) = self.ask(Query::Top { game, limit }) else {
            panic!("a top answers a top");
        };
        rows.iter()
            .map(|Entry { account, score }| (*account, *score))
            .collect()
    }

    fn bests(&self, owner: u64) -> Vec<u64> {
        let Ok(Reply::Bests(bests)) = self.ask(Query::Bests { owner }) else {
            panic!("bests answer bests");
        };
        bests
    }
}

#[test]
fn a_higher_score_becomes_the_best_and_one_at_or_under_it_changes_nothing() {
    let mut hall = Hall::default();
    assert_eq!(hall.bests(ADA), [0; GAMES.len()]);
    hall.ok(ADA, TETRIS, 1_200);
    assert_eq!(hall.bests(ADA), [1_200, 0, 0, 0, 0, 0]);
    let kept = hall.store.borrow().state.clone();
    for lower in [1_200, 800, 0] {
        hall.ok(ADA, TETRIS, lower);
        assert_eq!(hall.store.borrow().state, kept, "{lower}: nothing written");
    }
    hall.ok(ADA, TETRIS, 4_000);
    assert_eq!(hall.bests(ADA), [4_000, 0, 0, 0, 0, 0]);
    // the board holds the new best alone: the old row went with it
    assert_eq!(hall.top(TETRIS, MAX_TOP), [(ADA, 4_000)]);
    // a game is its own
    hall.ok(ADA, SNAKE, 90);
    assert_eq!(hall.bests(ADA), [4_000, 90, 0, 0, 0, 0]);
    assert_eq!(hall.bests(BO), [0; GAMES.len()]);
}

#[test]
fn a_score_of_nothing_is_not_on_the_board() {
    let mut hall = Hall::default();
    let empty = hall.store.borrow().state.clone();
    hall.ok(ADA, TETRIS, 0);
    assert_eq!(hall.store.borrow().state, empty);
    assert_eq!(hall.top(TETRIS, MAX_TOP), []);
}

#[test]
fn the_board_ranks_the_highest_first_one_row_an_account() {
    let mut hall = Hall::default();
    hall.ok(BO, TETRIS, 500);
    hall.ok(ADA, TETRIS, 900);
    hall.ok(CY, TETRIS, 500);
    hall.ok(BO, TETRIS, 700);
    hall.ok(CY, SNAKE, 40);
    // at one score the smaller account first
    hall.ok(ADA, SNAKE, 40);
    assert_eq!(
        hall.top(TETRIS, MAX_TOP),
        [(ADA, 900), (BO, 700), (CY, 500)]
    );
    assert_eq!(hall.top(SNAKE, MAX_TOP), [(ADA, 40), (CY, 40)]);
    assert_eq!(hall.top(TETRIS, 2), [(ADA, 900), (BO, 700)]);
    // one row at least: a limit of none is one
    assert_eq!(hall.top(TETRIS, 0), [(ADA, 900)]);
}

#[test]
fn the_board_answers_at_most_max_top_rows() {
    let mut hall = Hall::default();
    let posted = u64::from(MAX_TOP) + 5;
    for who in 1..=posted {
        hall.ok(who, TETRIS, who * 10);
    }
    let top = hall.top(TETRIS, u32::MAX);
    assert_eq!(top.len(), MAX_TOP as usize);
    assert_eq!(top[0], (posted, posted * 10));
    let scores: Vec<u64> = top.iter().map(|(_, score)| *score).collect();
    assert!(scores.is_sorted_by(|a, b| a >= b), "{scores:?}");
}

#[test]
fn a_key_that_holds_no_account_posts_nothing() {
    let hall = Hall::default();
    let unheld = hall.env().signed([9u8; 32], None);
    let ctx = hall.store.exec(unheld);
    let refusal = hall.store.refused(|| {
        Arcade::execute(
            &ctx,
            Op::Score {
                game: TETRIS,
                score: 100,
            },
        )
    });
    assert_eq!(refusal.code, code::UNAUTHORIZED);
}

#[test]
fn a_game_that_is_not_there_is_refused() {
    let mut hall = Hall::default();
    let past = GAMES.len() as u8;
    let refusal = hall.post(ADA, past, 100).expect_err("no such game");
    assert_eq!(refusal.code, code::NOT_FOUND);
    let refusal = hall
        .ask(Query::Top {
            game: past,
            limit: 10,
        })
        .expect_err("no such game");
    assert_eq!(refusal.code, code::NOT_FOUND);
}

#[test]
fn an_op_reads_as_a_person_would_say_it() {
    use ::describe::{Value, field};
    let said = describe(&Op::Score {
        game: SNAKE,
        score: 340,
    });
    assert_eq!(said.title, "Post a Snake score");
    assert_eq!(
        said.fields,
        [
            field("game", Value::text("Snake")),
            field("score", Value::count(340)),
        ]
    );
    let nowhere = describe(&Op::Score { game: 9, score: 1 });
    assert_eq!(nowhere.title, "Post a game 9 score");
}
