//! The bot's side of Tetris versus. The match is arcade-view's own step
//! (`arcade_view::versus::Match`), the one a view plays: the same rules,
//! track names, deal and words. This side routes it: it hears the room and
//! the other side through the live client, and says what to send in
//! [`Bot::out`]:
//! - in the lobby it is up on `wait/<me>`;
//! - the first challenge it sees it accepts by asking back, and it never
//!   challenges; one match at a time;
//! - in a round the [`Player`] plays its well; its attacks and its top-out
//!   go out on its ask track, its well on its board track;
//! - after every round it asks for another;
//! - when the other side's ask track ends, the other side left: it waits
//!   in the lobby again, under a new nonce.
use std::collections::BTreeMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use arcade_view::versus::{
    Effect, End, Match, Said, ask_track, asked, basis, board_track, wait_track,
};
use live_wire::Mode;

use crate::ai::{Level, Player};

/// What the bot asks the live client to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    Publish {
        name: String,
        mode: Mode,
    },
    Unpublish {
        name: String,
    },
    /// From the group the node holds.
    Subscribe {
        name: String,
    },
    /// One object appended to a stream-mode track's `group`.
    Send {
        name: String,
        group: u64,
        body: Vec<u8>,
    },
    /// A stream-mode track's `group` closes.
    EndGroup {
        name: String,
        group: u64,
    },
    /// A datagram-mode track's newest object.
    Latest {
        name: String,
        group: u64,
        object: u32,
        body: Vec<u8>,
    },
}

pub struct Bot {
    me: u64,
    level: Level,
    /// What tells this lobby visit's tracks from the ones before it.
    nonce: String,
    /// The room's tracks up, each with the account that publishes it.
    room: BTreeMap<String, u64>,
    game: Option<Playing>,
    pub out: Vec<Out>,
    /// What happened, a line an event.
    pub log: Vec<String>,
}

/// A match the bot plays: the step every side plays ([`Match`]), its
/// player, and the tracks it carries the match on.
struct Playing {
    play: Match,
    player: Player,
    /// This side's ask track, its group, and the objects said in it.
    ask: String,
    group: u64,
    said: u32,
    /// This side's board track, and the wells shown this round.
    board: String,
    shown: u32,
    /// The other side's ask track: what it says.
    theirs: String,
}

/// A nonce no other lobby visit of this process drew.
fn nonce() -> String {
    format!("{:016x}", RandomState::new().build_hasher().finish())
}

impl Bot {
    /// The bot of account `me`, waiting in the lobby.
    pub fn new(me: u64, level: Level) -> Bot {
        let mut bot = Bot {
            me,
            level,
            nonce: String::new(),
            room: BTreeMap::new(),
            game: None,
            out: Vec::new(),
            log: Vec::new(),
        };
        bot.wait();
        bot
    }

    fn wait(&mut self) {
        self.nonce = nonce();
        self.out.push(Out::Publish {
            name: wait_track(self.me),
            mode: Mode::Stream,
        });
    }

    /// A track came up or went down in the room.
    pub fn announced(&mut self, name: String, by: u64, up: bool) {
        match up {
            true => self.room.insert(name, by),
            false => self.room.remove(&name),
        };
        self.accept();
    }

    /// Out of a match, the first challenge in the room is asked back, and
    /// with both asks up the match is on.
    fn accept(&mut self) {
        let me = self.me;
        if self.game.is_some() {
            return;
        }
        let challenge = self
            .room
            .values()
            .find_map(|by| asked(&self.room, *by, me).map(|their_nonce| (*by, their_nonce)));
        let Some((them, their_nonce)) = challenge else {
            return;
        };
        let ask = ask_track(me, them, &self.nonce);
        let board = board_track(me, &self.nonce);
        let theirs = ask_track(them, me, &their_nonce);
        self.out.extend([
            Out::Publish {
                name: ask.clone(),
                mode: Mode::Stream,
            },
            Out::Publish {
                name: board.clone(),
                mode: Mode::Datagram,
            },
            Out::Subscribe {
                name: theirs.clone(),
            },
            Out::Unpublish {
                name: wait_track(me),
            },
        ]);
        let (play, shown) = Match::new(me, them, basis(me, &self.nonce, them, &their_nonce));
        self.game = Some(Playing {
            play,
            player: Player::new(self.level),
            ask,
            group: 0,
            said: 0,
            board,
            shown: 0,
            theirs,
        });
        self.log
            .push(format!("challenge from account {them}: round 0"));
        self.act(shown);
    }

    /// One tick of the round's clock: a key at the player's pace, then the
    /// match's step.
    pub fn tick(&mut self) {
        let Some(game) = self.game.as_mut().filter(|game| game.play.running()) else {
            return;
        };
        game.player.step(&mut game.play.tetris);
        let effects = game.play.tick();
        self.act(effects);
        self.ended_round();
    }

    /// What the other side said on its ask track `name`: object `object`
    /// of group `group`.
    pub fn heard(&mut self, name: &str, group: u64, object: u32, said: Said) {
        let Some(game) = self.game.as_mut().filter(|game| game.theirs == name) else {
            return;
        };
        let effects = game.play.heard(group, object, said);
        self.act(effects);
        self.ended_round();
    }

    /// Track `name` ended: the other side's ask track ending is its leaving.
    pub fn ended(&mut self, name: &str) {
        let Some(game) = self.game.take_if(|game| game.theirs == name) else {
            return;
        };
        // gone, whether or not the room has said so yet
        self.room.remove(name);
        self.log
            .push(format!("account {} left the match", game.play.them));
        self.out.extend([
            Out::Unpublish { name: game.ask },
            Out::Unpublish { name: game.board },
        ]);
        self.wait();
        self.accept();
    }

    /// A round that just ended is logged and another asked for.
    fn ended_round(&mut self) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let play = &mut game.play;
        let Some(end) = play.end.filter(|_| !play.again.0) else {
            return;
        };
        self.log.push(format!(
            "round {} against account {}: {}, score {}, {} rows",
            play.round,
            play.them,
            match end {
                End::Won => "won",
                End::Lost => "lost",
            },
            play.tetris.score(),
            play.tetris.lines
        ));
        let effects = play.rematch();
        self.act(effects);
    }

    /// What a step of the match gave back, onto the bot's tracks.
    fn act(&mut self, effects: Vec<Effect>) {
        let Some(game) = &mut self.game else {
            return;
        };
        for effect in effects {
            match effect {
                Effect::Say(said) => {
                    self.out.push(Out::Send {
                        name: game.ask.clone(),
                        group: game.group,
                        body: borsh::to_vec(&said).expect("borsh encodes an in-memory value"),
                    });
                    game.said += 1;
                }
                Effect::Show => {
                    let cells = game.play.tetris.cells();
                    self.out.push(Out::Latest {
                        name: game.board.clone(),
                        group: game.play.round,
                        object: game.shown,
                        body: borsh::to_vec(&cells).expect("borsh encodes an in-memory value"),
                    });
                    game.shown += 1;
                }
                Effect::NewRound => {
                    if game.said > 0 {
                        self.out.push(Out::EndGroup {
                            name: game.ask.clone(),
                            group: game.group,
                        });
                    }
                    game.group += 1;
                    game.said = 0;
                    game.shown = 0;
                    self.log.push(format!(
                        "round {} against account {}",
                        game.play.round, game.play.them
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use arcade_view::tetris::{Attack, COLS, Piece, ROWS};
    use arcade_view::versus::deal;

    use super::*;

    const ME: u64 = 13;
    const THEM: u64 = 9;

    fn said(said: Said) -> Vec<u8> {
        borsh::to_vec(&said).unwrap()
    }

    /// The bot in a match account 9 asked for under nonce `abc`.
    fn matched() -> Bot {
        let mut bot = Bot::new(ME, Level::Normal);
        bot.announced(wait_track(THEM), THEM, true);
        bot.announced(ask_track(THEM, ME, "abc"), THEM, true);
        bot.out.clear();
        bot.log.clear();
        bot
    }

    fn game(bot: &mut Bot) -> &mut Playing {
        bot.game.as_mut().expect("a match")
    }

    #[test]
    fn in_the_lobby_it_waits_and_never_challenges() {
        let mut bot = Bot::new(ME, Level::Normal);
        assert_eq!(
            bot.out,
            [Out::Publish {
                name: "wait/13".into(),
                mode: Mode::Stream
            }]
        );
        bot.out.clear();
        bot.announced(wait_track(THEM), THEM, true);
        // a challenge to someone else is not to it
        bot.announced(ask_track(THEM, 7, "abc"), THEM, true);
        for _ in 0..100 {
            bot.tick();
        }
        assert!(bot.out.is_empty() && bot.game.is_none());
    }

    #[test]
    fn a_challenge_is_asked_back_at_once_and_both_deal_alike() {
        let mut bot = Bot::new(ME, Level::Normal);
        let nonce = bot.nonce.clone();
        bot.out.clear();
        bot.announced(ask_track(THEM, ME, "abc"), THEM, true);
        assert_eq!(
            bot.out[..4],
            [
                Out::Publish {
                    name: format!("ask/13/9/{nonce}"),
                    mode: Mode::Stream
                },
                Out::Publish {
                    name: format!("board/13/{nonce}"),
                    mode: Mode::Datagram
                },
                Out::Subscribe {
                    name: "ask/9/13/abc".into()
                },
                Out::Unpublish {
                    name: "wait/13".into()
                },
            ]
        );
        assert!(
            matches!(&bot.out[4], Out::Latest { name, group: 0, object: 0, body } if *name == format!("board/13/{nonce}") && body.len() == 4 + 200)
        );
        assert_eq!(bot.log, ["challenge from account 9: round 0"]);
        // the other side deals from the same basis, its own way round
        let theirs = deal(&basis(THEM, "abc", ME, &nonce), 0, THEM);
        let ours = &game(&mut bot).play.tetris;
        assert_eq!((ours.piece, ours.next), (theirs.piece, theirs.next));
        // one match at a time: a second challenge waits
        bot.out.clear();
        bot.announced(ask_track(7, ME, "def"), 7, true);
        assert!(bot.out.is_empty());
    }

    #[test]
    fn in_a_round_it_lands_pieces_shows_its_well_and_takes_garbage() {
        let mut bot = matched();
        game(&mut bot)
            .play
            .tetris
            .versus
            .as_mut()
            .unwrap()
            .incoming
            .push_back(Attack { lines: 2, hole: 4 });
        for _ in 0..3000 {
            bot.tick();
        }
        let tetris = &game(&mut bot).play.tetris;
        let blocks = tetris
            .well
            .iter()
            .flatten()
            .filter(|cell| cell.is_some())
            .count();
        assert!(
            blocks > 0 && tetris.score() > 0,
            "pieces dropped and landed"
        );
        let garbage = tetris.versus.as_ref().unwrap();
        assert!(garbage.incoming.is_empty(), "the garbage rose");
        let shown = bot
            .out
            .iter()
            .filter(|out| matches!(out, Out::Latest { .. }))
            .count();
        assert!(shown > 10, "its board went out as it moved");
    }

    #[test]
    fn rows_cleared_attack_on_its_ask_track() {
        let mut bot = matched();
        let ask = game(&mut bot).ask.clone();
        let tetris = &mut game(&mut bot).play.tetris;
        for row in &mut tetris.well[ROWS as usize - 4..] {
            *row = [Some(Piece::O); COLS as usize];
            row[0] = None;
        }
        // a block over them: a Tetris, not an all clear too
        tetris.well[ROWS as usize - 5][9] = Some(Piece::O);
        tetris.piece = Piece::I;
        for _ in 0..2000 {
            bot.tick();
        }
        let attacks: Vec<_> = bot
            .out
            .iter()
            .filter_map(|out| match out {
                Out::Send {
                    name,
                    group: 0,
                    body,
                } if *name == ask => borsh::from_slice::<Said>(body).ok(),
                _ => None,
            })
            .collect();
        assert!(
            matches!(attacks[0], Said::Attack(Attack { lines: 4, .. })),
            "{attacks:?}"
        );
    }

    #[test]
    fn a_round_won_or_lost_asks_for_another_and_both_asking_deals_it() {
        let mut bot = matched();
        let ask = game(&mut bot).ask.clone();
        bot.heard("ask/9/13/abc", 0, 0, Said::Out);
        assert_eq!(game(&mut bot).play.end, Some(End::Won));
        assert_eq!(
            bot.out,
            [Out::Send {
                name: ask.clone(),
                group: 0,
                body: said(Said::Rematch)
            }]
        );
        assert!(bot.log[0].starts_with("round 0 against account 9: won, score 0"));
        bot.out.clear();
        bot.heard("ask/9/13/abc", 0, 1, Said::Rematch);
        assert_eq!(
            bot.out[0],
            Out::EndGroup {
                name: ask.clone(),
                group: 0
            }
        );
        assert_eq!(game(&mut bot).play.round, 1);
        assert_eq!(game(&mut bot).play.end, None);
        let dealt = deal(&basis(ME, &bot.nonce, THEM, "abc"), 1, ME);
        assert_eq!(game(&mut bot).play.tetris.next, dealt.next);
        // its well tops out: it says so, and asks for another
        bot.out.clear();
        let tetris = &mut game(&mut bot).play.tetris;
        // blocks where every piece comes in, beside a column the O falls
        for row in &mut tetris.well[..2] {
            row[3..7].fill(Some(Piece::Z));
        }
        tetris.piece = Piece::O;
        tetris.at = (7, 0);
        while game(&mut bot).play.running() {
            bot.tick();
        }
        assert_eq!(game(&mut bot).play.end, Some(End::Lost));
        let words: Vec<Said> = bot
            .out
            .iter()
            .filter_map(|out| match out {
                Out::Send {
                    name,
                    group: 1,
                    body,
                } if *name == ask => borsh::from_slice(body).ok(),
                _ => None,
            })
            .collect();
        assert_eq!(words, [Said::Out, Said::Rematch]);
    }

    #[test]
    fn when_the_other_side_leaves_it_waits_again_and_takes_the_next_challenge() {
        let mut bot = matched();
        let (ask, board) = (game(&mut bot).ask.clone(), game(&mut bot).board.clone());
        let before = bot.nonce.clone();
        // a track that is not theirs ending is not their leaving
        bot.ended(&board);
        assert!(bot.game.is_some());
        bot.ended("ask/9/13/abc");
        assert!(bot.game.is_none());
        assert_ne!(bot.nonce, before, "a new nonce");
        assert_eq!(
            bot.out,
            [
                Out::Unpublish { name: ask },
                Out::Unpublish { name: board },
                Out::Publish {
                    name: "wait/13".into(),
                    mode: Mode::Stream
                }
            ]
        );
        assert_eq!(bot.log, ["account 9 left the match"]);
        bot.announced(ask_track(7, ME, "def"), 7, true);
        assert_eq!(game(&mut bot).play.them, 7);
    }
}
