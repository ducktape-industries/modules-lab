//! What the view follows and asks: the session, whether it is on screen,
//! the program's blocks, the reader's bests, the open game's board with
//! its accounts' names, and the one op it sends, an ended game's score.
use abi::role::identity;
use arcade::{Entry, Op, ask};
use ducktape_view_guest::prelude::*;
use program::role::Identity;

use crate::games::Game;
use crate::versus::Versus;
use crate::{Arcade, Post};

/// The rows a board shows.
pub(crate) const BOARD_ROWS: u32 = 10;

/// One row of a board: the entry, and what identity calls its account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) entry: Entry,
    pub(crate) name: String,
}

impl Arcade {
    /// Follows the session, the program's blocks and whether the view is on
    /// screen, for as long as the view runs.
    pub(crate) fn watch(&mut self, cx: &mut Context<Self>) {
        cx.follow::<HostSession>((), |view, session, cx| match session {
            Ok(next) => view.session_changed(next, cx),
            Err(refusal) => cx.log_refused("the session", &refusal),
        })
        .detach();
        // a block re-reads what it wrote to, as the program declares it
        cx.follow::<Changes<arcade::Arcade>>((), |view, change, cx| match change {
            Ok(change) => view.changed(change.as_ref(), cx),
            Err(refusal) => cx.log_refused("arcade's blocks", &refusal),
        })
        .detach();
        cx.follow::<HostVisible>((), |view, visible, cx| match visible {
            Ok(visible) => view.visibility(visible, cx),
            Err(refusal) => cx.log_refused("visibility", &refusal),
        })
        .detach();
    }

    fn session_changed(&mut self, next: Session, cx: &mut Context<Self>) {
        if next == self.session {
            return;
        }
        cx.notify();
        let prev = std::mem::replace(&mut self.session, next);
        if self.session.account != prev.account {
            // the bests on screen are another account's
            self.bests = Loadable::Idle;
            self.read_bests(cx);
        } else if self.session.connected && !prev.connected {
            self.changed(None, cx);
        }
    }

    /// A block of the program landed: the bests and the board are asked
    /// again where it wrote to them; `None`, a reopened link, asks both.
    fn changed(&mut self, change: Option<&Change>, cx: &mut Context<Self>) {
        let touched = |read: &dyn program::Reads| {
            change.is_none_or(|change| change.touches::<arcade::Arcade, _>(read))
        };
        if let Some(owner) = self.session.account
            && touched(&ask::Bests { owner })
        {
            self.read_bests(cx);
        }
        if let Some(play) = &self.play {
            let game = play.which as u8;
            if touched(&ask::Top {
                game,
                limit: BOARD_ROWS,
            }) {
                self.read_board(cx);
            }
        }
    }

    pub(crate) fn read_bests(&mut self, cx: &mut Context<Self>) {
        let Some(owner) = self.session.account else {
            return;
        };
        let bests = cx.host().query(ask::Bests { owner });
        cx.load(self, bests, |view| &mut view.bests);
    }

    /// The reader's best of game `which`, once the bests are read.
    pub(crate) fn best(&self, which: usize) -> Option<u64> {
        self.bests.ready()?.get(which).copied()
    }

    /// The open game's top ten, and the names of their accounts.
    pub(crate) fn read_board(&mut self, cx: &mut Context<Self>) {
        let Some(play) = &self.play else {
            return;
        };
        let board = board(cx.host(), play.which as u8);
        cx.load(self, board, |view| &mut view.board);
    }

    /// The host said whether the view is on screen: off it, a running game
    /// pauses, and stays paused until the reader goes on.
    fn visibility(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.hidden = !visible;
        if self.hidden
            && let Some(play) = &mut self.play
            && play.game.ready().is_some_and(|game| !game.over())
            && !play.paused
        {
            play.paused = true;
            if let Some(game) = play.game.ready_mut() {
                game.let_go();
            }
            cx.notify();
        }
        self.settle(cx);
    }

    /// After anything moved a game: a game that ended posts its score,
    /// once, and the clock runs while a game does and only then.
    pub(crate) fn settle(&mut self, cx: &mut Context<Self>) {
        // a score is an account's; the program keeps the best of them
        let seated = self.session.account.is_some();
        if let Some(play) = &mut self.play
            && play.post == Post::Playing
            && let Some(game) = play.game.ready().filter(|game| game.over())
        {
            let score = game.score();
            play.post = if seated {
                Post::Sending
            } else {
                Post::Unsigned
            };
            if play.post == Post::Sending {
                let op = Op::Score {
                    game: play.which as u8,
                    score,
                };
                let seed = play.seed;
                let submit = cx.host().ask::<Submit<arcade::Arcade>>(op);
                cx.land(submit, move |view, result, cx| {
                    if let Some(play) = view.play.as_mut().filter(|play| play.seed == seed) {
                        play.post = match result {
                            Ok(_) => Post::Posted,
                            Err(refused) => Post::Refused(refused.message),
                        };
                    }
                    view.changed(None, cx);
                    cx.notify();
                })
                .detach();
            }
            cx.notify();
        }
        self.run_clock(cx);
    }

    /// Asks for the clock at the running game's period, or a versus
    /// round's, drops it when none runs, and asks again when the period
    /// changes. A versus round runs on off screen: the other side plays on.
    fn run_clock(&mut self, cx: &mut Context<Self>) {
        let running = self
            .play
            .as_ref()
            .filter(|play| !play.paused && !self.hidden)
            .and_then(|play| play.game.ready())
            .filter(|game| !game.over());
        let period = running
            .and_then(Game::period)
            .or_else(|| self.versus.as_ref().and_then(Versus::period));
        if self.clock.as_ref().map(|(running, _)| *running) == period {
            return;
        }
        self.clock = period.map(|period| {
            let ticks = cx.follow::<ClockTicks>(period, |view, tick, cx| match tick {
                Ok(()) => view.ticked(cx),
                Err(refusal) => cx.log_refused("the clock", &refusal),
            });
            (period, ticks)
        });
    }

    /// One tick of the clock: one step of the running game.
    fn ticked(&mut self, cx: &mut Context<Self>) {
        if self.versus.is_some() {
            return self.versus_ticked(cx);
        }
        let Some(game) = self
            .play
            .as_mut()
            .filter(|play| !play.paused)
            .and_then(|play| play.game.ready_mut())
        else {
            return;
        };
        if game.tick() {
            cx.notify();
        }
        self.settle(cx);
    }
}

/// Game `game`'s best scores, each with its account's name: every name is
/// asked of identity at once, and an account it does not name is called by
/// its number.
async fn board(host: Host, game: u8) -> Result<Vec<Row>, Error> {
    let top = host
        .query(ask::Top {
            game,
            limit: BOARD_ROWS,
        })
        .await?;
    let names: Vec<_> = top
        .iter()
        .map(|entry| host.ask::<Query<Identity>>(identity::Query::Profile(entry.account)))
        .collect();
    let mut rows = Vec::with_capacity(top.len());
    for (entry, name) in top.into_iter().zip(names) {
        let name = match name.await {
            Ok(identity::Reply::Profile(Some(profile))) => profile.name,
            _ => format!("account {}", entry.account),
        };
        rows.push(Row { entry, name });
    }
    Ok(rows)
}
