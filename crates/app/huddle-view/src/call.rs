//! The call the room is in: a room of the `huddle` program, from the Start
//! or the Join that put the person in it to the Leave that takes them out,
//! and the calls ringing them.
//!
//! Who is in a call is its room's live tracks (`live.tracks`): an account
//! is in while its microphone track is up, muted or not, with its camera's
//! up or not. The program's `joined` lags them by a block and never hears a
//! crash, so the stage never reads it. The view places your camera and
//! microphone publishing into your own tracks (`cam/<you>`, `mic/<you>`),
//! and for each other person a voice (every one in the call: the stage
//! does not decide who is heard) and, on the stage, a picture.
//!
//! The node holds a track the program has not granted yet and says
//! `Waiting`; the view then sends `Join`, which writes the person's row,
//! once per `Waiting` (arcade's versus knocks the same way): Join is
//! idempotent, so a Join the row was on its way for anyway costs one op.
//!
//! The main tile is the last person whose voice said they speak, held two
//! seconds after they stop so two people trading words do not flip it each
//! word; before anyone speaks, the first other person. Never you with others
//! there.
//!
//! The calls ringing the person are the program's `Mine`, asked again on
//! each block that writes one and, while one is shown, every five seconds:
//! a ring runs out by the chain's clock, with no block to say so.
use std::collections::BTreeMap;

use huddle::{Listed, Op};
use identity::{Identity, Kind as Account, Profile};
use view::methods::{LiveAnnounce, Submitted, Watching};
use view::prelude::*;

use crate::{Huddle, Screen, words};

/// How long the main tile stays with whoever took it before the next one
/// to start speaking takes it, in the view's seconds.
const SPEAKER_HOLD: u64 = 2;

/// How long a person's track may stay `Starting` before the room says it
/// is reconnecting, in the view's seconds.
const SLOW_START: u64 = 2;

/// How often a shown ring is asked again, in the view's seconds.
const RING_CHECK: u64 = 5;

/// The most people the desktop's stage shows; the rest are a "+N" chip.
pub(crate) const STAGE: usize = 6;

/// The call the room is in.
pub(crate) struct Call {
    pub(crate) id: String,
    /// Everyone else in the room, by account: in while their microphone
    /// track is up.
    pub(crate) people: BTreeMap<u64, Person>,
    /// The room's track list, followed while in the call.
    _watch: Task<()>,
    /// A Join went out for the `Waiting` heard; cleared when a track is up
    /// or the Join is refused, so each `Waiting` sends one.
    knocked: bool,
    /// Who holds the main tile, and since when.
    speaker: Option<(u64, u64)>,
    /// The last person whose voice turned to speaking: the main tile's
    /// next holder once the hold is out.
    last_spoke: Option<u64>,
}

/// One other person in the call.
#[derive(Default)]
pub(crate) struct Person {
    /// Their camera's track is up: their picture can be drawn.
    pub(crate) camera: bool,
    /// What their voice's piece last told: whether they speak, and how its
    /// track stands.
    pub(crate) voice: MediaState,
    /// What their picture's piece last told.
    pub(crate) picture: MediaState,
    /// When one of their pieces began waiting for its first objects, by
    /// the view's seconds; `None` while none waits.
    starting_since: Option<u64>,
}

/// Which of a person's pieces told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Voice,
    Picture,
}

impl Person {
    /// Starts or stops the clock of a piece waiting: a piece placed anew
    /// waits (the host tells a state only when it moves off `Starting`),
    /// and a picture is drawn only while their camera is up.
    fn clock(&mut self, now: u64) {
        let starting = self.voice.status == MediaStatus::Starting
            || (self.camera && self.picture.status == MediaStatus::Starting);
        self.starting_since = match (starting, self.starting_since) {
            (true, None) => Some(now),
            (true, since) => since,
            (false, _) => None,
        };
    }
}

impl Call {
    fn new(id: String, watch: Task<()>) -> Self {
        Self {
            id,
            people: BTreeMap::new(),
            _watch: watch,
            knocked: false,
            speaker: None,
            last_spoke: None,
        }
    }

    /// The person on the main tile: the speaker while they hold it, else
    /// the first other person; `None` alone.
    pub(crate) fn main(&self) -> Option<u64> {
        self.speaker
            .map(|(account, _)| account)
            .filter(|account| self.people.contains_key(account))
            .or_else(|| self.people.keys().next().copied())
    }

    /// Everyone else in stage order: the main tile's first, the rest by
    /// number.
    pub(crate) fn order(&self) -> Vec<u64> {
        let main = self.main();
        main.into_iter()
            .chain(self.people.keys().copied().filter(|n| Some(*n) != main))
            .collect()
    }

    /// Who is in the call, you among them.
    pub(crate) fn count(&self) -> usize {
        self.people.len() + 1
    }

    /// The main tile handed on, at `now`: to the last person whose voice
    /// turned to speaking, once its holder has had it [`SPEAKER_HOLD`]
    /// (at once when its holder left). No one speaking since, it stays.
    fn hand_on(&mut self, now: u64) {
        let held = self.speaker.is_some_and(|(account, since)| {
            self.people.contains_key(&account) && now < since + SPEAKER_HOLD
        });
        if held {
            return;
        }
        if let Some(last) = self.last_spoke.filter(|n| self.people.contains_key(n))
            && self.speaker.map(|(account, _)| account) != Some(last)
        {
            self.speaker = Some((last, now));
        }
    }

    /// Whether a person's track has waited longer than [`SLOW_START`].
    pub(crate) fn slow(&self, now: u64) -> bool {
        self.people.values().any(|person| {
            person
                .starting_since
                .is_some_and(|since| now >= since + SLOW_START)
        })
    }
}

impl Huddle {
    /// Follows what calls need for as long as the view runs: the session
    /// (who you are), the program's blocks (the calls ringing you) and the
    /// links opened into the view (`call/<id>`: answer it).
    pub(crate) fn watch_calls(&mut self, cx: &mut Context<Self>) {
        cx.follow::<HostSession>((), |view, session, cx| match session {
            Ok(next) => view.session_changed(next, cx),
            Err(refusal) => cx.log_refused("the session", &refusal),
        })
        .detach();
        cx.follow::<Changes<huddle::Huddle>>((), |view, change, cx| match change {
            Ok(change) => {
                let Some(account) = view.session.account else {
                    return;
                };
                let ringing = huddle::ask::Ringing { account };
                if change.is_none_or(|change| change.touches::<huddle::Huddle, _>(&ringing)) {
                    view.read_calls(cx);
                }
            }
            Err(refusal) => cx.log_refused("huddle's blocks", &refusal),
        })
        .detach();
        cx.follow::<HostRoute>((), |_, route, cx| match route {
            Ok(route) => {
                if let Some(id) = route.strip_prefix("call/") {
                    // after this update, where the window says what the
                    // pointer is
                    let id = id.to_owned();
                    cx.spawn(async move |view, cx| {
                        let _ = view
                            .update_in(cx, |view, window, cx| view.answer(id, window.touch(), cx));
                    })
                    .detach();
                }
            }
            Err(refusal) => cx.log_refused("the links opened here", &refusal),
        })
        .detach();
    }

    fn session_changed(&mut self, next: Session, cx: &mut Context<Self>) {
        if next == self.session {
            return;
        }
        let prev = std::mem::replace(&mut self.session, next);
        if self.session.account != prev.account {
            // another account's calls: the one under way is not theirs
            self.call = None;
            self.ringing = Loadable::Idle;
            self.invitable = Loadable::Idle;
            self.read_calls(cx);
        } else if self.session.connected && !prev.connected {
            // a reconnect hears `Waiting` again, and Joins again on it
            if let Some(call) = &mut self.call {
                call.knocked = false;
            }
            self.read_calls(cx);
        }
        cx.notify();
    }

    /// The calls ringing you, as the program lists them at the chain's
    /// time.
    pub(crate) fn read_calls(&mut self, cx: &mut Context<Self>) {
        let Some(account) = self.session.account else {
            return;
        };
        let ringing = cx.host().query(huddle::ask::Ringing { account });
        cx.land(ringing, |view, ringing, cx| {
            if let Ok(calls) = &ringing {
                let callers = calls.iter().map(|listed| listed.call.caller).collect();
                view.ask_names(callers, cx);
            }
            view.ringing = ringing.into();
            cx.notify();
        })
        .detach();
    }

    /// The calls ringing you, the one the room is in aside.
    pub(crate) fn rings(&self) -> Vec<&Listed> {
        let Some(ringing) = self.ringing.ready() else {
            return Vec::new();
        };
        let here = self.call.as_ref().map(|call| call.id.as_str());
        ringing
            .iter()
            .filter(|listed| Some(listed.id.as_str()) != here)
            .collect()
    }

    /// One second of the view's clock in a call: the main tile handed on,
    /// and a shown ring asked again every [`RING_CHECK`].
    pub(crate) fn call_tick(&mut self, cx: &mut Context<Self>) {
        let now = self.ticks;
        if let Some(call) = &mut self.call {
            call.hand_on(now);
        }
        if !self.rings().is_empty() && now.is_multiple_of(RING_CHECK) {
            self.read_calls(cx);
        }
    }

    /// What identity calls `account`, or its number until it answers.
    pub(crate) fn name(&self, account: u64) -> String {
        self.names
            .get(&account)
            .cloned()
            .unwrap_or_else(|| words::account(account))
    }

    /// Asks identity for each of `accounts` not named yet.
    pub(crate) fn ask_names(&mut self, accounts: Vec<u64>, cx: &mut Context<Self>) {
        for account in accounts {
            if self.names.contains_key(&account) {
                continue;
            }
            // asked once: the number stands in until the answer lands
            self.names.insert(account, words::account(account));
            let profile = cx
                .host()
                .ask::<Query<Identity>>(::identity::Query::Profile { number: account });
            cx.land(profile, move |view, profile, cx| {
                if let Ok(::identity::Reply::Profile(Some(profile))) = profile {
                    view.names.insert(account, profile.name);
                    cx.notify();
                }
            })
            .detach();
        }
    }

    /// Everyone you can invite: identity's people, you aside, read anew
    /// whenever the list opens.
    pub(crate) fn read_invitable(&mut self, cx: &mut Context<Self>) {
        let me = self.session.account;
        let host = cx.host();
        let people = async move {
            let mut people = Vec::new();
            let mut after = None;
            loop {
                let (profiles, next) = host
                    .query(::identity::ask::Profiles { after, limit: 100 })
                    .await?;
                people.extend(
                    profiles
                        .into_iter()
                        .filter(|profile| profile.kind == Account::User)
                        .filter(|profile| Some(profile.number) != me),
                );
                let Some(next) = next else { break };
                after = Some(next);
            }
            Ok::<Vec<Profile>, Error>(people)
        };
        cx.load(self, people, |view| &mut view.invitable);
    }

    /// Rings `account`: into the call under way, or a new call with you in
    /// it (its id minted by the host) when the room has none.
    pub(crate) fn invite(&mut self, account: u64, cx: &mut Context<Self>) {
        self.call_note = None;
        self.ask_names(vec![account], cx);
        if let Some(call) = &self.call {
            let op = Op::Invite {
                call: call.id.clone(),
                accounts: vec![account],
            };
            self.send(op, cx);
            return;
        }
        let minted = cx.host().ask::<HostId>("call".into());
        cx.land(minted, move |view, id, cx| {
            let id = match id {
                Ok(id) => id,
                Err(refusal) => return view.call_failed(&refusal.message, cx),
            };
            let op = Op::Start {
                call: id.clone(),
                invited: vec![account],
            };
            let started = cx.host().ask::<Submit<huddle::Huddle>>(op);
            cx.land(started, move |view, started, cx| match refused(started) {
                // still in the room it was started from
                None if view.screen == Screen::Room && view.call.is_none() => {
                    view.enter_call(id, cx)
                }
                // left, or in another call, before it landed: out of it,
                // so no one is rung into an empty room
                None => view.send(Op::Leave { call: id }, cx),
                Some(why) => view.call_failed(&why, cx),
            })
            .detach();
        })
        .detach();
    }

    /// Answers call `id`: in the room at once (on a phone, `touch`, where
    /// the view is; else floated), the microphone on (a phone that rang
    /// wants the call, not a settings page), the Join on its way.
    pub(crate) fn answer(&mut self, id: String, touch: bool, cx: &mut Context<Self>) {
        if self.call.as_ref().is_some_and(|call| call.id == id) {
            return;
        }
        self.hang_up(cx);
        self.call_note = None;
        self.muted = false;
        self.enter_call(id, cx);
        self.knock(cx);
        match self.screen {
            Screen::Room => {}
            Screen::Check if touch => self.enter_room(cx),
            Screen::Check => self.join(cx),
        }
    }

    /// Says no to call `id`: its ring stops here at once, and for the
    /// program on the next block.
    pub(crate) fn decline(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(ringing) = self.ringing.ready_mut() {
            ringing.retain(|listed| listed.id != id);
        }
        self.send(Op::Decline { call: id }, cx);
        cx.notify();
    }

    /// Out of the call: the pieces leave the frame with it, and the
    /// program hears `Leave`.
    pub(crate) fn hang_up(&mut self, cx: &mut Context<Self>) {
        if let Some(call) = self.call.take() {
            self.send(Op::Leave { call: call.id }, cx);
        }
    }

    /// Into call `id`: its tracks followed; the pieces publish from the
    /// next frame.
    fn enter_call(&mut self, id: String, cx: &mut Context<Self>) {
        let watch = cx.follow::<LiveTracks>(id.clone(), |view, item, cx| match item {
            Ok(Watching::Waiting) => view.knock(cx),
            Ok(Watching::Announce(announce)) => view.announced(announce, cx),
            Err(refusal) => view.call_over(&refusal, cx),
        });
        self.call = Some(Call::new(id, watch));
        let title = words::in_the_room(1);
        cx.host().notify::<HostTitle>(title);
        cx.notify();
    }

    /// The program has not granted a track of the call yet: Join, unless
    /// one is out for this `Waiting`.
    pub(crate) fn knock(&mut self, cx: &mut Context<Self>) {
        let Some(call) = &mut self.call else { return };
        if std::mem::replace(&mut call.knocked, true) {
            return;
        }
        let id = call.id.clone();
        let joined = cx
            .host()
            .ask::<Submit<huddle::Huddle>>(Op::Join { call: id.clone() });
        cx.land(joined, move |view, joined, cx| {
            let Some(why) = refused(joined) else { return };
            let Some(call) = view.call.as_mut().filter(|call| call.id == id) else {
                return;
            };
            // the next `Waiting` asks again
            call.knocked = false;
            view.call_failed(&why, cx);
        })
        .detach();
    }

    /// A track of the call came up or went down: a microphone's brings its
    /// person in or out, a camera's their picture.
    fn announced(&mut self, announce: LiveAnnounce, cx: &mut Context<Self>) {
        let me = self.session.account;
        let Some(call) = &mut self.call else { return };
        let who = announce.by;
        if Some(who) == me {
            return;
        }
        if announce.name == huddle::microphone_track(who) {
            match announce.up {
                true => call.people.entry(who).or_default().clock(self.ticks),
                false => {
                    call.people.remove(&who);
                }
            }
            call.hand_on(self.ticks);
            let count = call.count();
            cx.host().notify::<HostTitle>(words::in_the_room(count));
            self.ask_names(vec![who], cx);
        } else if announce.name == huddle::camera_track(who)
            && let Some(person) = call.people.get_mut(&who)
        {
            // a picture placed anew starts over
            person.camera = announce.up;
            person.picture = MediaState::default();
            person.clock(self.ticks);
        }
        cx.notify();
    }

    /// One of your own publishing pieces told how its track stands:
    /// `Waiting` asks for the row, `Up` clears the ask for the next one.
    pub(crate) fn heard_own_track(&mut self, track: &TrackState, cx: &mut Context<Self>) {
        match track {
            TrackState::Waiting => self.knock(cx),
            TrackState::Up { .. } => {
                if let Some(call) = &mut self.call {
                    call.knocked = false;
                }
            }
            TrackState::None | TrackState::Ended { .. } => {}
        }
    }

    /// A piece of `who`'s told its state: a voice's speaking hands the main
    /// tile on; a piece waiting for its first objects starts the clock the
    /// network row reads.
    pub(crate) fn heard_person(
        &mut self,
        who: u64,
        piece: Piece,
        state: &MediaState,
        cx: &mut Context<Self>,
    ) {
        let now = self.ticks;
        let Some(call) = &mut self.call else { return };
        let Some(person) = call.people.get_mut(&who) else {
            return;
        };
        let was_speaking = person.voice.speaking;
        match piece {
            Piece::Voice => person.voice = state.clone(),
            Piece::Picture => person.picture = state.clone(),
        }
        person.clock(now);
        if piece == Piece::Voice && !was_speaking && state.speaking {
            call.last_spoke = Some(who);
        }
        call.hand_on(now);
        cx.notify();
    }

    /// Whether the room says it is reconnecting: a track of yours not up,
    /// or someone's slow to come through.
    pub(crate) fn reconnecting(&self) -> bool {
        let Some(call) = &self.call else { return false };
        let down = |state: &MediaState| {
            matches!(state.track, TrackState::Waiting | TrackState::Ended { .. })
        };
        down(&self.mic_state)
            || (self.camera_placed() && down(&self.camera_state))
            || call.slow(self.ticks)
    }

    /// What the room's line (the network row) says, if anything: why the
    /// call's last op did not go through, reconnecting, or the camera
    /// sending less picture (the host's words).
    pub(crate) fn network(&self) -> Option<String> {
        if let Some(note) = &self.call_note {
            return Some(note.clone());
        }
        if self.reconnecting() {
            return Some(words::RECONNECTING.into());
        }
        let camera = &self.camera_state;
        (self.call.is_some()
            && self.camera_placed()
            && camera.status == MediaStatus::Live
            && !camera.detail.is_empty())
        .then(|| camera.detail.clone())
    }

    /// Who is in the room, you among them.
    pub(crate) fn people_here(&self) -> usize {
        self.call.as_ref().map_or(1, Call::count)
    }

    /// The call's tracks were refused for good: the grant taken back (the
    /// caller ended it, or it was over: nothing to leave), or no live
    /// connection (left, so no one waits on you). Out of the room and back
    /// on the desk, with why.
    fn call_over(&mut self, refusal: &Error, cx: &mut Context<Self>) {
        let note = match refusal.code.as_str() {
            "not_granted" => {
                self.call = None;
                words::CALL_ENDED.into()
            }
            _ => words::call_trouble(&refusal.message),
        };
        self.leave(cx);
        self.call_note = Some(note);
    }

    /// An op of the call refused: said, and the room left when its call
    /// cannot go on.
    fn call_failed(&mut self, why: &str, cx: &mut Context<Self>) {
        self.call_note = Some(words::call_trouble(why));
        cx.notify();
    }

    /// Sends `op`; a refusal is said in the room.
    fn send(&mut self, op: Op, cx: &mut Context<Self>) {
        let sent = cx.host().ask::<Submit<huddle::Huddle>>(op);
        cx.land(sent, |view, sent, cx| {
            if let Some(why) = refused(sent) {
                view.call_failed(&why, cx);
            }
        })
        .detach();
    }
}

/// Why an op did not go through, or `None` when it did.
fn refused(submitted: Result<Submitted, Error>) -> Option<String> {
    match submitted {
        Ok(Submitted::Applied { .. }) => None,
        Ok(Submitted::Refused { refusal, .. }) | Err(refusal) => Some(refusal.message),
        Ok(Submitted::InDoubt { .. }) => Some(words::NO_ANSWER.into()),
    }
}
