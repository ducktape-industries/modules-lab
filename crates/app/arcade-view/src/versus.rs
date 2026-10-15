//! Tetris versus: two wells side by side, each played on its own machine,
//! the two views talking through the node's live tracks in the room
//! [`arcade::VERSUS`] and never through the chain. The program only opens
//! the room ([`Op::OpenVersus`]).
//!
//! A side's tracks, named by its account `<me>` and the `<nonce>` its
//! lobby drew from an id the host minted:
//! - `wait/<me>`: up while it waits in the lobby, so who is up is who
//!   waits;
//! - `ask/<me>/<them>/<nonce>`: a challenge to `<them>`, and accepting
//!   one is asking back. Once each has asked the other the match is on,
//!   and the track goes on to carry what this side [`Said`], each a
//!   [`Word`] that names its round: its attacks, its top-out, a rematch;
//!   in order and reliably (stream mode). A round starts a new group, and
//!   a group the host cut is said again from the round's first word in a
//!   newer one;
//! - `board/<me>/<nonce>`: the well as the other side draws it beside its
//!   own, a byte a cell, the newest wins (datagram mode, a group a round:
//!   a full queue drops the oldest, and each object is the whole well).
//!
//! Both sides deal from both asks' accounts and nonces, so both get the
//! same pieces. Of the other side's `ask` track a side acts only on the
//! words of its current round, and of those only on the ones past what it
//! acted on: an object's number in its group is the word's place in its
//! round. A side whose `ask` track ends while the match is on, for any reason
//! the node gives (`publisher_gone` when its app went, `track_ended` when
//! it left), has left, and the other wins. A side's app that reconnects
//! ends nothing: the node hands its subscribers over to the new session,
//! its open group cut short.
//!
//! The node lets an account into the room only once the program wrote its
//! grant row, and holds what the lobby asks till then: a `Waiting` sends
//! `Op::OpenVersus`, one at a time, and the lobby's asks go through on the
//! block that writes the row: nothing is asked again. An open the program
//! refused is sent again on the next `Waiting` (a reconnect is told it
//! again). A `not_granted` is the grant taken back, and stops versus.
//!
//! Versus is played as the account the session seats: another account, or
//! none, leaves the lobby and joins it anew. A round runs on while the view
//! is off screen, the keys it held let go.
use std::collections::{BTreeMap, BTreeSet};

use ::identity::Identity;
use arcade::Op;
use borsh::{BorshDeserialize, BorshSerialize};
use program::code;
use view::live::{DEFAULT_PRIORITY, Received, Track, subscribe};
use view::prelude::*;
use view::{AnyElement, Div, FontWeight, Stateful};

use crate::Arcade;
use crate::games::seed;
use crate::play::{self, BAR_H};
use crate::tetris::{self, Attack, Tetris, Well};

/// What a side says on its `ask` track once the match is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Said {
    /// Rows of garbage for the other's well.
    Attack(Attack),
    /// This side topped out: the other wins the round.
    Out,
    /// This side will play another round.
    Rematch,
}

/// One object of a side's `ask` track once the match is on: what it said,
/// in which round. A group holds one round's words from its first, so an
/// object's number in its group is the word's place in the round, and a
/// group the host cut is said again whole in a newer one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Word {
    pub round: u64,
    pub said: Said,
}

/// The versus screen: its lobby, and the match once there is one.
#[derive(Default)]
pub(crate) struct Versus {
    /// The room's tracks up, as `live.tracks` tells them: each name, and
    /// the account that publishes it.
    room: BTreeMap<String, u64>,
    /// `live.tracks` of the room, while the screen is open.
    watch: Option<Task<()>>,
    /// `wait/<me>`, while this side waits.
    waiting: Option<Track<()>>,
    /// What tells this side's tracks from its earlier ones, minted as the
    /// lobby opens.
    nonce: Option<String>,
    /// This side's challenge: to whom, and its `ask` track.
    pub(crate) ask: Option<(u64, Track<Word>)>,
    pub(crate) game: Option<Playing>,
    /// What identity calls each account met; its number until it answers.
    names: BTreeMap<u64, String>,
    /// The program was asked to open the room and did not refuse: the
    /// `Waiting`s the room's list and `wait/<me>` hear meanwhile send
    /// nothing more. A refused open, a lobby joined anew and a reconnect
    /// clear it.
    knocked: bool,
    /// Why the lobby stopped, in a line.
    note: Option<String>,
    /// The element the next render gives the keyboard to.
    focus: Option<&'static str>,
    /// The answers the screen waits for (a nonce, a name, the room's
    /// opening): they go with it.
    waits: Vec<Task<()>>,
}

/// A match on screen: the step both sides play ([`Match`]), and the
/// tracks this view carries it on.
pub(crate) struct Playing {
    pub(crate) play: Match,
    board: Track<Vec<u8>>,
    /// The other side's `ask` track: what it says. Held: dropping it
    /// unsubscribes.
    _hearing: Task<()>,
    /// The other side's board track's name, and the subscription to it
    /// once it is up.
    theirs: String,
    seeing: Option<Task<()>>,
    pub(crate) their_well: Option<Well>,
    /// Shift is down, as the board last heard the modifiers.
    shift: bool,
}

impl Playing {
    /// The view went off screen: no key it holds will be heard coming up.
    pub(crate) fn let_go(&mut self) {
        self.play.tetris.let_go();
        self.shift = false;
    }
}

/// A match as both sides play it, this view and `tetris-bot` alike: this
/// side's well, what the other side said, and the rounds. Each step
/// decides, and gives back what goes out on this side's tracks
/// ([`Effect`]), which the caller carries out on its own.
pub struct Match {
    me: u64,
    pub them: u64,
    /// What every round is dealt from: both asks' accounts and nonces.
    basis: String,
    pub round: u64,
    pub tetris: Box<Tetris>,
    pub end: Option<End>,
    /// Who wants another round: this side, the other.
    pub again: (bool, bool),
    /// The other side left: no other round.
    pub gone: bool,
    /// The `ask` group the next word goes in, counted as the track counts
    /// them.
    group: u64,
    /// This round's words, and the last round's: its `Rematch` may be what
    /// a cut kept from the other side.
    words: Spoken,
    last: Option<Spoken>,
    /// How many of the other side's words of this round were acted on: a
    /// re-open replays its group, and a cut round comes again whole.
    heard: u32,
}

/// One round's words, in order, and the `ask` group they went out in last.
#[derive(Debug, Default)]
struct Spoken {
    group: u64,
    words: Vec<Said>,
}

impl Spoken {
    /// These words, round `round`'s, said again in a newer group `group`.
    fn again(&mut self, group: u64, round: u64) -> Vec<Effect> {
        self.group = group;
        let words = self
            .words
            .iter()
            .map(|said| Effect::Say(Word { round, said: *said }));
        std::iter::once(Effect::NextGroup).chain(words).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Won,
    Lost,
}

/// What a step of a [`Match`] sends on this side's tracks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// One object onto the `ask` track, in its current group.
    Say(Word),
    /// The well as it looks now onto the `board` track, in the round's
    /// group.
    Show,
    /// The `ask` track goes on in a new group, the round the same.
    NextGroup,
    /// A new round was dealt: the `ask` track goes on in a new group.
    NewRound,
}

impl Versus {
    /// The clock's period while a round runs, and while its effects play
    /// out after it.
    pub(crate) fn period(&self) -> Option<i64> {
        let play = &self.game.as_ref()?.play;
        (play.running() || play.tetris.animating()).then_some(tetris::PERIOD)
    }

    fn name(&self, account: u64) -> String {
        match self.names.get(&account) {
            Some(name) => name.clone(),
            None => format!("account {account}"),
        }
    }

    /// What a step of the match gave back, onto this side's tracks.
    fn act(&mut self, effects: Vec<Effect>, host: &Host) {
        let Some(game) = &mut self.game else {
            return;
        };
        for effect in effects {
            match effect {
                Effect::Say(word) => {
                    if let Some((_, ask)) = &mut self.ask {
                        ask.send(host, &word);
                    }
                }
                Effect::NextGroup => {
                    if let Some((_, ask)) = &mut self.ask {
                        ask.next_group(host);
                    }
                }
                Effect::Show => {
                    let cells = game.play.tetris.cells();
                    game.board.send_latest(host, game.play.round, &cells);
                }
                Effect::NewRound => {
                    if let Some((_, ask)) = &mut self.ask {
                        ask.next_group(host);
                    }
                    game.their_well = None;
                    self.focus = Some("versus-board");
                }
            }
        }
    }
}

impl Match {
    /// Round 0 of `me` against `them`, dealt from `basis`, and its well
    /// shown.
    pub fn new(me: u64, them: u64, basis: String) -> (Match, Vec<Effect>) {
        let play = Match {
            me,
            them,
            tetris: deal(&basis, 0, me),
            basis,
            round: 0,
            end: None,
            again: (false, false),
            gone: false,
            group: 0,
            words: Spoken::default(),
            last: None,
            heard: 0,
        };
        (play, vec![Effect::Show])
    }

    pub fn running(&self) -> bool {
        self.end.is_none()
    }

    /// One tick of the round: the well steps, and its attacks, its top-out
    /// and its new look go out. After the round, nothing.
    pub fn tick(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        if !self.running() {
            return effects;
        }
        let moved = self.tetris.tick();
        let garbage = self.tetris.versus.as_mut().expect("a versus well");
        let attacks: Vec<_> = garbage.outgoing.drain(..).collect();
        for attack in attacks {
            effects.push(self.say(Said::Attack(attack)));
        }
        if self.tetris.over() {
            effects.push(self.say(Said::Out));
            self.end = Some(End::Lost);
        }
        if moved {
            effects.push(Effect::Show);
        }
        effects
    }

    /// What the other side said on its `ask` track: `word`, object
    /// `object` of its group, which is the word's place in its round. Only
    /// the current round's words count, and of them only those past the
    /// ones acted on: a re-open replays the node's latest group, which
    /// right after a rematch is still the last round's, and a round the
    /// host cut comes again from its first word. A word of the next round
    /// heard before the other side's rematch is lost with it: groups are
    /// independent streams, and a lost attack beats a misapplied one.
    pub fn heard(&mut self, object: u32, word: Word) -> Vec<Effect> {
        if word.round != self.round || object < self.heard {
            return Vec::new();
        }
        self.heard = object + 1;
        match word.said {
            Said::Attack(attack) if self.running() && attack.fair() => {
                self.tetris.receive(attack);
                Vec::new()
            }
            Said::Out if self.running() => {
                self.win();
                Vec::new()
            }
            Said::Rematch => {
                self.again.1 = true;
                self.again()
            }
            // an attack after the round ended, or one no well sends
            _ => Vec::new(),
        }
    }

    /// The other side's `ask` track ended, for whatever reason the node
    /// gave: it left, and this side wins.
    pub fn left(&mut self) {
        self.gone = true;
        self.win();
    }

    /// This side wants another round, once this one ended and while the
    /// other side is there.
    pub fn rematch(&mut self) -> Vec<Effect> {
        if self.running() || self.gone || self.again.0 {
            return Vec::new();
        }
        self.again.0 = true;
        let mut effects = vec![self.say(Said::Rematch)];
        effects.extend(self.again());
        effects
    }

    /// The host cut `group` of this side's `ask` track: the other side
    /// hears none of what it held past the cut. The round it held is said
    /// again whole in a newer group; when that was the last round, its
    /// words first (its `Rematch` among them), then this round's in a group
    /// after them. A cut of an older group is behind both sides.
    pub fn cut(&mut self, group: u64) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let Some(last) = self.last.as_mut().filter(|last| last.group == group) {
            self.group += 1;
            effects = last.again(self.group, self.round - 1);
        } else if self.words.group != group {
            return effects;
        }
        self.group += 1;
        effects.extend(self.words.again(self.group, self.round));
        effects
    }

    /// `said` goes out in this round's group, and is kept to say again.
    fn say(&mut self, said: Said) -> Effect {
        self.words.words.push(said);
        Effect::Say(Word {
            round: self.round,
            said,
        })
    }

    /// The other side topped out or left: this one wins, and its well
    /// flashes.
    fn win(&mut self) {
        if self.running() {
            self.end = Some(End::Won);
            self.tetris.won();
        }
    }

    /// Both sides want another round: it is dealt from its number, and
    /// each track goes on in a new group.
    fn again(&mut self) -> Vec<Effect> {
        if self.again != (true, true) || self.gone {
            return Vec::new();
        }
        self.round += 1;
        self.tetris = deal(&self.basis, self.round, self.me);
        self.end = None;
        self.again = (false, false);
        self.group += 1;
        let words = Spoken {
            group: self.group,
            words: Vec::new(),
        };
        self.last = Some(std::mem::replace(&mut self.words, words));
        self.heard = 0;
        vec![Effect::NewRound, Effect::Show]
    }
}

/// A track of the versus room.
fn track(name: String) -> LiveTrack {
    LiveTrack {
        room: arcade::VERSUS.into(),
        name,
    }
}

/// `wait/<me>`: up while `me` waits in the lobby.
pub fn wait_track(me: u64) -> String {
    format!("wait/{me}")
}

/// `ask/<by>/<to>/<nonce>`: `by`'s challenge to `to`, then what `by` says.
pub fn ask_track(by: u64, to: u64, nonce: &str) -> String {
    format!("ask/{by}/{to}/{nonce}")
}

/// `board/<by>/<nonce>`: `by`'s well as the other side draws it.
pub fn board_track(by: u64, nonce: &str) -> String {
    format!("board/{by}/{nonce}")
}

/// What every round of a match is dealt from: both asks' accounts and
/// nonces, the lower account first, so both sides name it alike.
pub fn basis(me: u64, nonce: &str, them: u64, their_nonce: &str) -> String {
    match me < them {
        true => format!("{me}:{nonce}/{them}:{their_nonce}"),
        false => format!("{them}:{their_nonce}/{me}:{nonce}"),
    }
}

/// This side's track `name` in the room, as it publishes it.
fn publish(name: String, mode: LiveMode) -> LivePublish {
    LivePublish {
        track: track(name),
        mode,
        priority: DEFAULT_PRIORITY,
    }
}

/// `track` heard from the group the node holds.
fn latest(track: LiveTrack) -> LiveSubscribe {
    LiveSubscribe {
        track,
        from: LiveStart::Latest,
    }
}

/// The nonce of `by`'s ask of `to`, if the room has it up.
pub fn asked(room: &BTreeMap<String, u64>, by: u64, to: u64) -> Option<String> {
    let prefix = ask_track(by, to, "");
    room.iter()
        .filter(|(_, publisher)| **publisher == by)
        .find_map(|(name, _)| name.strip_prefix(&prefix))
        .map(str::to_owned)
}

/// Round `round`'s well for side `me`: the pieces from what both sides
/// share, the holes of its attacks its own.
pub fn deal(basis: &str, round: u64, me: u64) -> Box<Tetris> {
    let pieces = seed(&format!("{basis}/{round}"));
    Box::new(Tetris::versus(pieces, pieces ^ me))
}

impl Arcade {
    /// Opens the versus lobby from the hub.
    pub(crate) fn open_versus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.play = None;
        self.versus = Some(Versus::default());
        self.join(cx);
        window.focus("versus-back");
        cx.notify();
    }

    /// Back to the hub, onto versus's card: every track of this side goes
    /// down with the screen.
    fn leave_versus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.versus = None;
        window.focus("arcade-card-versus");
        self.settle(cx);
        cx.notify();
    }

    /// The lobby: what ran before dropped, then, for a seated account, the
    /// room followed, `wait/<me>` up, and a nonce minted for its asks.
    fn join(&mut self, cx: &mut Context<Self>) {
        let Some(versus) = &mut self.versus else {
            return;
        };
        versus.room.clear();
        versus.watch = None;
        versus.waiting = None;
        versus.game = None;
        versus.ask = None;
        versus.nonce = None;
        versus.note = None;
        versus.knocked = false;
        let Some(me) = self.session.account else {
            return;
        };
        versus.watch = Some(
            cx.follow::<LiveTracks>(arcade::VERSUS.into(), |view, item, cx| match item {
                Ok(Watching::Waiting) => view.knock(cx),
                Ok(Watching::Announce(announce)) => view.announced(announce, cx),
                Err(refusal) => view.refused(refusal, cx),
            }),
        );
        versus.waiting = Some(Track::publish(
            cx,
            publish(wait_track(me), LiveMode::Stream),
            Self::published,
        ));
        let minted = cx.host().ask::<HostId>("versus".into());
        versus.waits.push(cx.land(minted, |view, id, cx| {
            let Some(versus) = &mut view.versus else {
                return;
            };
            match id {
                Ok(id) => versus.nonce = Some(format!("{:016x}", seed(&id))),
                Err(refused) => versus.note = Some(refused.message),
            }
            cx.notify();
        }));
    }

    /// What the host says of one of this side's tracks: that it waits for
    /// the room; how many listen, which nothing here needs; a group it cut,
    /// which the match says again in a newer one, or an object it dropped
    /// (`too_large`), after which the track goes on and so does the round;
    /// or why the track ended.
    fn published(&mut self, said: Result<Publishing, Error>, cx: &mut Context<Self>) {
        match said {
            Ok(Publishing::Waiting) => self.knock(cx),
            Ok(Publishing::Subscribers(_)) => {}
            // only `ask/` sends stream objects (`wait/` sends none, `board/`
            // is datagram mode, where nothing is cut): a group of it, its
            // queue to the node full. Not a reason to end the match.
            Ok(Publishing::Cut { group }) => {
                cx.host().log(format!(
                    "{}: the host cut group {group} of this side's versus ask; it is said again",
                    <Self as View>::NAME
                ));
                if let Some(versus) = &mut self.versus
                    && let Some(game) = &mut versus.game
                {
                    let effects = game.play.cut(group);
                    versus.act(effects, &cx.host());
                }
            }
            Err(dropped) if dropped.code == refusal::TOO_LARGE => {
                cx.log_refused("an object on a versus track", &dropped)
            }
            Err(refusal) => self.refused(refusal, cx),
        }
    }

    /// The room is not open yet: the program is asked to open it, unless
    /// it is asked already, and the asks wait for its row.
    fn knock(&mut self, cx: &mut Context<Self>) {
        let Some(versus) = &mut self.versus else {
            return;
        };
        if std::mem::replace(&mut versus.knocked, true) {
            return;
        }
        versus.note = None;
        let submit = cx.host().ask::<Submit<arcade::Arcade>>(Op::OpenVersus);
        versus.waits.push(cx.land(submit, |view, opened, cx| {
            if let (Some(versus), Err(refused)) = (&mut view.versus, opened) {
                // the next `Waiting` asks again
                versus.knocked = false;
                versus.note = Some(format!(
                    "The arcade could not open the versus room: {}",
                    refused.message
                ));
                cx.notify();
            }
        }));
    }

    /// The session moved from `prev`: another account, or none, joins the
    /// lobby anew; a reconnect asks the room again, and a room still not
    /// open is asked open again on the `Waiting` it is told.
    pub(crate) fn versus_session(&mut self, prev: &Session, cx: &mut Context<Self>) {
        let Some(versus) = &mut self.versus else {
            return;
        };
        if self.session.account != prev.account {
            self.join(cx);
            self.settle(cx);
        } else if self.session.connected && !prev.connected {
            versus.knocked = false;
        }
    }

    /// A `live.*` the node refused, the grant taken back too: it stops the
    /// lobby, with why.
    fn refused(&mut self, refusal: Error, cx: &mut Context<Self>) {
        let Some(versus) = &mut self.versus else {
            return;
        };
        versus.watch = None;
        versus.waiting = None;
        versus.ask = None;
        versus.game = None;
        versus.note = Some(format!("Versus stopped: {}", refusal.message));
        self.settle(cx);
        cx.notify();
    }

    /// A track came up or went down in the room.
    fn announced(&mut self, announce: LiveAnnounce, cx: &mut Context<Self>) {
        let me = self.session.account;
        let Some(versus) = &mut self.versus else {
            return;
        };
        let LiveAnnounce { name, by, up, .. } = announce;
        if up {
            versus.room.insert(name, by);
        } else {
            versus.room.remove(&name);
        }
        if Some(by) != me && !versus.names.contains_key(&by) {
            versus.names.insert(by, format!("account {by}"));
            let profile = cx
                .host()
                .ask::<Query<Identity>>(::identity::Query::Profile { number: by });
            versus
                .waits
                .push(cx.land(profile, move |view, profile, cx| {
                    if let (Some(versus), Ok(::identity::Reply::Profile(Some(profile)))) =
                        (&mut view.versus, profile)
                    {
                        versus.names.insert(by, profile.name);
                        cx.notify();
                    }
                }));
        }
        self.check(cx);
        cx.notify();
    }

    /// Starts the match once each side has asked the other; in a match,
    /// subscribes to the other side's board once it is up.
    fn check(&mut self, cx: &mut Context<Self>) {
        let (Some(me), Some(versus)) = (self.session.account, &mut self.versus) else {
            return;
        };
        if let Some(game) = &mut versus.game {
            if game.seeing.is_none() && versus.room.get(&game.theirs) == Some(&game.play.them) {
                let board = track(game.theirs.clone());
                game.seeing = Some(subscribe(cx, latest(board), Self::seen));
            }
            return;
        }
        let (Some((them, _)), Some(nonce)) = (&versus.ask, &versus.nonce) else {
            return;
        };
        let them = *them;
        let Some(their_nonce) = asked(&versus.room, them, me) else {
            return;
        };
        let (play, shown) = Match::new(me, them, basis(me, nonce, them, &their_nonce));
        let board = Track::publish(
            cx,
            publish(board_track(me, nonce), LiveMode::Datagram),
            Self::published,
        );
        let their_ask = track(ask_track(them, me, &their_nonce));
        versus.game = Some(Playing {
            play,
            board,
            _hearing: subscribe(cx, latest(their_ask), Self::heard),
            theirs: board_track(them, &their_nonce),
            seeing: None,
            their_well: None,
            shift: false,
        });
        versus.act(shown, &cx.host());
        // waiting no more
        versus.waiting = None;
        versus.focus = Some("versus-board");
        self.check(cx);
        self.settle(cx);
    }

    /// The challenge button of `them`: asks them, or takes this side's ask
    /// of them back.
    fn challenge(&mut self, them: u64, cx: &mut Context<Self>) {
        let (Some(me), Some(versus)) = (self.session.account, &mut self.versus) else {
            return;
        };
        if versus.game.is_some() {
            return;
        }
        if versus.ask.as_ref().is_some_and(|(asked, _)| *asked == them) {
            versus.ask = None;
            return cx.notify();
        }
        let Some(nonce) = &versus.nonce else {
            return;
        };
        let ask = Track::publish(
            cx,
            publish(ask_track(me, them, nonce), LiveMode::Stream),
            Self::published,
        );
        versus.ask = Some((them, ask));
        self.check(cx);
        cx.notify();
    }

    /// What the other side said.
    fn heard(&mut self, said: Result<Received<Word>, Error>, cx: &mut Context<Self>) {
        let Some(versus) = &mut self.versus else {
            return;
        };
        let Some(game) = &mut versus.game else {
            return;
        };
        match said {
            Ok(Received::Object { object, value, .. }) => {
                let effects = game.play.heard(object, value);
                versus.act(effects, &cx.host());
            }
            // a re-open (`Opened`) replays the node's latest group, and a
            // cut group (`GroupEnd`) comes again whole in a newer one: what
            // the match acted on is behind its count of the round's words
            Ok(Received::Waiting | Received::Opened { .. } | Received::GroupEnd { .. }) => return,
            // a body that does not decode, the one item that does not end
            // the subscription
            Err(refusal) if refusal.code == code::UNEXPECTED_REPLY => {
                return cx.log_refused("what the other side said", &refusal);
            }
            // any other refusal is the subscription's last item: `publisher_gone`
            // when their app went, `track_ended` when they left,
            // `unknown_track` when they were gone before it was asked
            Err(_) => {
                game.play.left();
                // nobody is left to play again: this side's challenge ends
                // with the match, or it reads as a fresh one when they
                // come back
                versus.ask = None;
            }
        }
        self.settle(cx);
        cx.notify();
    }

    /// The other side's well.
    fn seen(&mut self, board: Result<Received<Vec<u8>>, Error>, cx: &mut Context<Self>) {
        let Some(game) = self.versus.as_mut().and_then(|versus| versus.game.as_mut()) else {
            return;
        };
        if let Ok(Received::Object { value, .. }) = board
            && let Some(well) = tetris::read_cells(&value)
        {
            game.their_well = Some(well);
            cx.notify();
        }
    }

    /// One tick of a round: the match steps, and what it says goes out.
    /// After the round only its effects go on.
    pub(crate) fn versus_ticked(&mut self, cx: &mut Context<Self>) {
        let Some(versus) = &mut self.versus else {
            return;
        };
        let Some(game) = &mut versus.game else {
            return;
        };
        if game.play.tetris.fade() {
            cx.notify();
        }
        let effects = game.play.tick();
        let ended = !game.play.running();
        if !effects.is_empty() {
            versus.act(effects, &cx.host());
            cx.notify();
        }
        if ended {
            self.settle(cx);
        }
    }

    /// This side wants another round.
    fn rematch(&mut self, cx: &mut Context<Self>) {
        let Some(versus) = &mut self.versus else {
            return;
        };
        let Some(game) = &mut versus.game else {
            return;
        };
        let effects = game.play.rematch();
        versus.act(effects, &cx.host());
        self.settle(cx);
        cx.notify();
    }

    /// Out of the match, back to the lobby, waiting again.
    fn back_to_lobby(&mut self, cx: &mut Context<Self>) {
        self.join(cx);
        if let Some(versus) = &mut self.versus {
            versus.focus = Some("versus-back");
        }
        self.settle(cx);
        cx.notify();
    }

    fn versus_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if let Some(game) = self.running_game()
            && game
                .play
                .tetris
                .key_down(&event.keystroke.key, event.is_held)
        {
            cx.notify();
        }
    }

    fn versus_key_up(&mut self, event: &KeyUpEvent) {
        if let Some(game) = self.running_game() {
            game.play.tetris.key_up(&event.keystroke.key);
        }
    }

    /// Shift alone holds, as on the single game's board.
    fn versus_modifiers(&mut self, event: &ModifiersChangedEvent) {
        let Some(game) = self.running_game() else {
            return;
        };
        let shift = event.modifiers.shift;
        if std::mem::replace(&mut game.shift, shift) == shift {
            return;
        }
        if shift {
            game.play.tetris.key_down("shift", false);
        } else {
            game.play.tetris.key_up("shift");
        }
    }

    fn running_game(&mut self) -> Option<&mut Playing> {
        let game = self.versus.as_mut()?.game.as_mut()?;
        game.play.running().then_some(game)
    }

    pub(crate) fn versus_screen(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let theme = *theme;
        let versus = self.versus.as_mut().expect("versus is on screen");
        if let Some(id) = versus.focus.take() {
            window.focus(id);
        }
        let against = versus
            .game
            .as_ref()
            .map(|game| format!("against {}", versus.name(game.play.them)));
        let bar = div()
            .flex_none()
            .h(BAR_H)
            .px(design::space::LG)
            .flex()
            .items_center()
            .gap(design::space::MD)
            .border_b_1()
            .border_color(theme.border)
            .child(
                design::button(
                    "versus-back",
                    "Games",
                    &theme,
                    cx.listener(|view, _: &ClickEvent, window, cx| view.leave_versus(window, cx)),
                )
                .kind(design::Kind::Quiet),
            )
            .child(
                div()
                    .text_size(design::text::SECTION)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Tetris versus"),
            )
            .children(against.map(|against| design::quiet(against, &theme)));
        let body = match versus.game.is_some() {
            true => self.match_view(cx, &theme).into_any_element(),
            false => self.lobby(cx, &theme).into_any_element(),
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(bar)
            .child(body)
    }

    /// Who waits and who asked: a row an account, its button challenging
    /// it, accepting its challenge, or taking this side's back.
    fn lobby(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let theme = *theme;
        let versus = self.versus.as_ref().expect("versus is on screen");
        let me = self.session.account;
        let mut people = BTreeSet::new();
        for (name, by) in &versus.room {
            let mut parts = name.split('/');
            let named = match (parts.next(), parts.next(), parts.next()) {
                (Some("wait"), Some(who), None) => who.parse().ok(),
                (Some("ask"), Some(who), Some(to)) if to.parse().ok() == me => who.parse().ok(),
                _ => None,
            };
            if named == Some(*by) && Some(*by) != me {
                people.insert(*by);
            }
        }
        let mine = versus.ask.as_ref().map(|(them, _)| *them);
        let rows: Vec<AnyElement> = people
            .into_iter()
            .map(|them| {
                let asks_me = me.is_some_and(|me| asked(&versus.room, them, me).is_some());
                let (state, action) = match (mine == Some(them), asks_me) {
                    (true, _) => ("Asked. Waiting for them…", "Take back"),
                    (false, true) => ("Challenges you", "Accept"),
                    (false, false) => ("Waiting", "Challenge"),
                };
                div()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .gap(design::space::MD)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(versus.name(them)),
                    )
                    .child(design::quiet(state, &theme))
                    .child(
                        design::button(
                            format!("versus-player-{them}"),
                            action,
                            &theme,
                            cx.listener(move |view, _: &ClickEvent, _, cx| {
                                view.challenge(them, cx)
                            }),
                        )
                        .kind(match asks_me {
                            true => design::Kind::Primary,
                            false => design::Kind::Outline,
                        })
                        .enabled(versus.nonce.is_some()),
                    )
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty().then(|| {
            design::quiet(
                "No one is waiting yet. Whoever opens Tetris versus next shows up here.",
                &theme,
            )
        });
        let unsigned = me.is_none().then(|| {
            design::quiet(
                "Versus is played from an account: seat one to wait here.",
                &theme,
            )
        });
        div()
            .id("versus-lobby")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(design::space::XL)
            .flex()
            .flex_col()
            .gap(design::space::BLOCK)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(design::space::XXS)
                    .child(design::heading("versus-title", "Who is waiting", 1, &theme))
                    .child(design::quiet(
                        "Press a name to challenge them. When they accept you both start, on the same pieces; the rows you clear rise in their well.",
                        &theme,
                    )),
            )
            .children(unsigned)
            .children(versus.note.clone().map(|note| design::quiet(note, &theme)))
            .child(div().max_w(px(560.)).flex().flex_col().children(rows))
            .children(empty)
    }

    /// The match: this side's well, taking the keys, its hold, next and
    /// the garbage on its way, and beside it the other's.
    fn match_view(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let theme = *theme;
        let versus = self.versus.as_ref().expect("versus is on screen");
        let game = versus.game.as_ref().expect("a match");
        let them = versus.name(game.play.them);
        let board = div()
            .id("versus-board")
            .border_1()
            .border_color(theme.border)
            .role(Role::Figure)
            .aria_label(format!("Your well, against {them}. {}", tetris::KEYS))
            .focusable()
            .consumes_keys(play::KEYS)
            .on_key_down(
                cx.listener(|view, event: &KeyDownEvent, _, cx| view.versus_key_down(event, cx)),
            )
            .on_key_up(cx.listener(|view, event: &KeyUpEvent, _, _| view.versus_key_up(event)))
            .on_modifiers_changed(cx.listener(|view, event: &ModifiersChangedEvent, _, _| {
                view.versus_modifiers(event)
            }));
        let garbage = game.play.tetris.versus.as_ref().expect("a versus well");
        let stats = vec![
            ("Sent", garbage.sent.to_string()),
            ("Received", garbage.received.to_string()),
            ("Level", game.play.tetris.level.to_string()),
        ];
        tetris::stage(
            self.stage_room(),
            &game.play.tetris,
            board,
            self.ended(&game.play, &them, cx, &theme),
            stats,
            Some((them.clone(), game.their_well.as_ref())),
            &theme,
        )
        .id("versus-stage")
    }

    /// What lies on the well once a round ends: who won, and another round
    /// or the lobby.
    fn ended(
        &self,
        game: &Match,
        them: &str,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> Option<Div> {
        let end = game.end.filter(|_| !game.tetris.ending())?;
        let theme = *theme;
        let title = match end {
            End::Won => "You win",
            End::Lost => "You lose",
        };
        let line = match (game.gone, game.again) {
            (true, _) => format!("{them} left the match."),
            (false, (true, _)) => format!("Waiting for {them} to play again…"),
            (false, (false, true)) => format!("{them} wants a rematch."),
            (false, (false, false)) => match end {
                End::Won => format!("{them} topped out."),
                End::Lost => "Your well topped out.".to_owned(),
            },
        };
        let again = (!game.gone && !game.again.0).then(|| {
            design::button(
                "versus-again",
                "Rematch",
                &theme,
                cx.listener(|view, _: &ClickEvent, _, cx| view.rematch(cx)),
            )
            .kind(design::Kind::Primary)
        });
        let lobby = design::button(
            "versus-lobby-back",
            "Leave",
            &theme,
            cx.listener(|view, _: &ClickEvent, _, cx| view.back_to_lobby(cx)),
        )
        .kind(design::Kind::Outline);
        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id("versus-overlay")
                        .min_w(px(240.))
                        .p(design::space::XL)
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(design::space::SM)
                        .border_1()
                        .border_color(theme.border_strong)
                        .bg(theme.background)
                        .role(Role::Dialog)
                        .aria_label(title)
                        .child(
                            div()
                                .text_size(design::text::TITLE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(design::quiet(line, &theme).text_center().max_w(px(280.)))
                        .child(
                            div()
                                .pt(design::space::SM)
                                .flex()
                                .gap(design::space::SM)
                                .children(again)
                                .child(lobby),
                        ),
                ),
        )
    }
}
