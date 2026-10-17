//! What huddle hands the host's notification centre, decided here and
//! nowhere else: a call ringing the reader. Its title is the caller (`Ada
//! is calling`), its body how many are asked in when more than two, its
//! tag the call (`call/<id>`), its link the call
//! (`duck://<chain>/huddle/call/<id>`, which the view answers at once),
//! its kind [`NoticeKind::Call`] (a carrier rings: a VoIP push on a phone,
//! a banner with Answer and Decline on a desktop), its `until` the end of
//! the reader's ring, in the chain's seconds, and its end link the call's
//! leave (`duck://<chain>/huddle/call/<id>/leave`, which the view leaves
//! that call by): what a carrier opens when the person ends the answered
//! call from the system's own call screen.
//!
//! This is huddle's NOTIFIER: the host runs it once per network, whether or
//! not a Huddle pane is open. It asks the program which calls ring the
//! reader ([`huddle::Query::Ringing`]) when a block writes a call, and every
//! five seconds while it has a ring out, so a ring that runs out is
//! withdrawn (`notify.seen`) as one that is answered, declined or ended is.
//! The program answers at the chain's time, so a run catching up after a
//! while away never rings a call that is over.
//!
//! What it rang it keeps under `rung/<signer>`: each call's id with the end
//! of its ring. A call rung again (asked in again, a new `until`) rings
//! again; a run that starts withdraws what an earlier run left ringing
//! that rings no more.
use std::collections::BTreeMap;

use huddle::Listed;
use identity::Identity;
use view::prelude::*;
use view::{Notifier, store};

use crate::words;

/// Where the notifier keeps what it rang.
fn rung_key(signer: &str) -> String {
    format!("rung/{signer}")
}

/// How often a ring out is checked, in milliseconds: a carrier also stops
/// a ring at its `until`, so this only takes the notice down.
const CHECK_MS: i64 = 5_000;

/// Huddle's notifier: who reads, what it rang, and the check that runs
/// while something rings.
#[derive(Default)]
pub struct HuddleNotices {
    session: Session,
    /// Moves with every new reader: what lands for an earlier one is dropped.
    run: u64,
    /// Each call rung, with when its ring stops; `None` until read off the
    /// store.
    rung: Option<BTreeMap<String, u64>>,
    /// Moves with every ask of the calls ringing: an answer to an older
    /// ask is dropped, so a slow one never rings a call a newer one saw
    /// stop.
    asked: u64,
    /// The check every [`CHECK_MS`], while a ring is out.
    clock: Option<Task<()>>,
}

impl Notifier for HuddleNotices {
    const NAME: &'static str = words::HUDDLE;
    const CAPABILITIES: &'static [Capability] = &[
        Capability::Program,
        Capability::Host,
        Capability::Notify,
        Capability::Store,
        Capability::Clock,
    ];
    const TARGETS: &'static [&'static str] = &[huddle::PROGRAM, identity::PROGRAM];

    fn new(cx: &mut Context<Self>) -> Self {
        cx.follow::<HostSession>((), |notices, session, cx| match session {
            Ok(session) => notices.session_changed(session, cx),
            Err(refusal) => log_refused(cx, "the session", &refusal),
        })
        .detach();
        // a block that wrote a call of the reader's asks again
        cx.follow::<Changes<huddle::Huddle>>((), |notices, change, cx| match change {
            Ok(change) => {
                let Some(account) = notices.session.account else {
                    return;
                };
                let ringing = huddle::ask::Ringing { account };
                if change.is_none_or(|change| change.touches::<huddle::Huddle, _>(&ringing)) {
                    notices.ask(cx);
                }
            }
            Err(refusal) => log_refused(cx, "huddle's blocks", &refusal),
        })
        .detach();
        Self::default()
    }
}

fn log_refused(cx: &mut Context<HuddleNotices>, what: &str, refusal: &Error) {
    cx.host().log(format!(
        "{}: {what} refused: {refusal}",
        HuddleNotices::NAME
    ));
}

impl HuddleNotices {
    fn session_changed(&mut self, next: Session, cx: &mut Context<Self>) {
        let reader = (&next.signer, &next.chain_id, next.account, next.connected);
        let was = (
            &self.session.signer,
            &self.session.chain_id,
            self.session.account,
            self.session.connected,
        );
        if reader == was {
            self.session = next;
            return;
        }
        // another reader, network or account: nothing known carries over
        *self = Self {
            session: next,
            run: self.run + 1,
            asked: self.asked,
            ..Self::default()
        };
        if !self.session.connected
            || self.session.signer.is_empty()
            || self.session.account.is_none()
        {
            return;
        }
        let kept = store::get::<BTreeMap<String, u64>>(&cx.host(), &rung_key(&self.session.signer));
        let run = self.run;
        cx.land(kept, move |notices, kept, cx| {
            if notices.run != run {
                return;
            }
            let kept = kept.unwrap_or_else(|refusal| {
                log_refused(cx, "what was rung", &refusal);
                None
            });
            notices.rung = Some(kept.unwrap_or_default());
            notices.ask(cx);
        })
        .detach();
    }

    /// The calls ringing the reader now, each with its caller's name; once
    /// what was rung is read.
    fn ask(&mut self, cx: &mut Context<Self>) {
        let (Some(account), Some(_)) = (self.session.account, &self.rung) else {
            return;
        };
        self.asked += 1;
        let (run, asked) = (self.run, self.asked);
        let ringing = ringing(cx.host(), account);
        cx.land(ringing, move |notices, ringing, cx| {
            if (notices.run, notices.asked) != (run, asked) {
                return;
            }
            match ringing {
                Ok(ringing) => notices.settle(account, ringing, cx),
                Err(refusal) => log_refused(cx, "the calls ringing", &refusal),
            }
        })
        .detach();
    }

    /// `ringing` is what rings `me` now: a call not rung yet, or rung to
    /// another `until`, rings; one rung that is not here any more is
    /// withdrawn. The check runs while anything rings.
    fn settle(&mut self, me: u64, ringing: Vec<(Listed, String)>, cx: &mut Context<Self>) {
        let Some(rung) = &mut self.rung else { return };
        let before = rung.clone();
        let chain = &self.session.chain_id;
        for (listed, caller) in &ringing {
            let Some(until) = listed.call.ring_until(me) else {
                continue;
            };
            if rung.insert(listed.id.clone(), until) != Some(until) {
                cx.host()
                    .notify::<NotifyPost>(notice(listed, caller, until, chain));
            }
        }
        rung.retain(|id, _| {
            let rings = ringing.iter().any(|(listed, _)| listed.id == *id);
            if !rings {
                cx.host().notify::<NotifySeen>(tag(id));
            }
            rings
        });
        if *rung != before {
            store::set(&cx.host(), &rung_key(&self.session.signer), Some(&*rung));
        }
        let out = !rung.is_empty();
        match (out, self.clock.is_some()) {
            (true, false) => {
                self.clock = Some(cx.follow::<ClockTicks>(
                    CHECK_MS,
                    |notices, tick, cx| match tick {
                        Ok(()) => notices.ask(cx),
                        Err(refusal) => log_refused(cx, "the clock", &refusal),
                    },
                ));
            }
            (false, true) => self.clock = None,
            _ => {}
        }
    }
}

/// The calls ringing `account`, each with what identity calls its caller
/// (their number where it names none).
async fn ringing(host: Host, account: u64) -> Result<Vec<(Listed, String)>, Error> {
    let calls = host.query(huddle::ask::Ringing { account }).await?;
    let mut named = Vec::with_capacity(calls.len());
    for listed in calls {
        let caller = listed.call.caller;
        let profile = host.ask::<Query<Identity>>(::identity::Query::Profile { number: caller });
        let name = match profile.await {
            Ok(::identity::Reply::Profile(Some(profile))) => profile.name,
            _ => words::account(caller),
        };
        named.push((listed, name));
    }
    Ok(named)
}

/// The tag a call's ring goes under.
pub(crate) fn tag(id: &str) -> String {
    format!("call/{id}")
}

/// The ring of `listed` for its invitee, from `caller`, until `until`.
pub(crate) fn notice(listed: &Listed, caller: &str, until: u64, chain: &str) -> Notification {
    Notification {
        title: words::calling(caller),
        body: words::huddle_with(listed.call.people().count()),
        tag: tag(&listed.id),
        // a notice without a link is one the centre cannot open, not a bad one
        link: ducklink::mint(chain, huddle::PROGRAM, &["call", &listed.id]).unwrap_or_default(),
        kind: NoticeKind::Call,
        until,
        end_link: ducklink::mint(chain, huddle::PROGRAM, &["call", &listed.id, "leave"])
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests;
