//! Live echo: what the people in the lobby type, line by line, live
//! through the node and never on the chain. The smallest view on the SDK's
//! `live` piece, and the proof that it carries a line from one app to
//! another.
//!
//! Each reader publishes the track `echo/<account>` in the room `lobby` in
//! stream mode: a line is one object, alone in its group, so a reader who
//! comes later is replayed each person's last line. The view follows the
//! room's tracks (`live.tracks`) and subscribes to every `echo/*` it hears
//! of, and lists the lines by account. A track that ends says why under
//! its account (`publisher_gone` when its app went away).
//!
//! The node lets an account into the room only as the program granted it.
//! A network nobody opened the lobby on refuses the first publish with
//! `not_granted`; the view then sends `live-echo`'s `Open { room }`, once
//! a session, and joins again on the next block of the program. A second
//! `not_granted` is shown, not retried.
use std::collections::BTreeMap;

use ducktape_view_guest::live::{Received, Track, subscribe};
use ducktape_view_guest::prelude::*;
use live_echo::{LOBBY, LiveEcho, Op};
use serde::{Deserialize, Serialize};

/// The name every reader's track starts with; the rest is its account.
const ECHO: &str = "echo/";

/// The lines kept a person: the oldest go first.
const KEPT: usize = 200;

#[derive(Default, Serialize, Deserialize)]
pub struct Echo {
    /// The line being typed.
    pub(crate) line: TextField,
    /// The session last heard: who reads, and whether the node is there.
    #[serde(skip)]
    session: Session,
    /// Each account heard in the lobby, the reader's own among them.
    #[serde(skip)]
    pub(crate) people: BTreeMap<u64, Person>,
    /// How many hear the reader's track, every node counted.
    #[serde(skip)]
    pub(crate) listeners: u32,
    /// Where the lobby stands for this reader.
    #[serde(skip)]
    pub(crate) lobby: Lobby,
    /// This session sent `Open` already: a second `not_granted` is shown,
    /// not answered with another.
    #[serde(skip)]
    opened: bool,
    /// The reader's track: dropping it unpublishes.
    #[serde(skip)]
    mine: Option<Track<String>>,
    /// The room's track list.
    #[serde(skip)]
    watch: Option<Task<()>>,
    /// A subscription per track heard, by its name.
    #[serde(skip)]
    hearing: BTreeMap<String, Task<()>>,
}

/// One account in the lobby.
#[derive(Default)]
pub(crate) struct Person {
    pub(crate) lines: Vec<String>,
    /// The refusal that ended its track, until it comes back.
    pub(crate) gone: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lobby {
    /// The session has not said who reads, or the reader holds no account.
    #[default]
    Away,
    /// Published and listening.
    In,
    /// The node refused the reader the room; `Open` is on its way, and the
    /// block that writes the grant joins again.
    Opening,
    /// What the node or the host refused, in its code: nothing more is
    /// tried until the session changes.
    Refused(String),
}

impl View for Echo {
    const NAME: &'static str = "Live echo";
    const DESCRIPTION: &'static str = "Lines typed in the lobby, live, by account.";
    const ICON: &'static str = "icons/radio.svg";
    // `live`: the tracks; `host`: the session; `module` and `op`: the
    // program's blocks and the op that opens the lobby
    const CAPABILITIES: &'static [Capability] = &[
        Capability::Live,
        Capability::Host,
        Capability::Module,
        Capability::Op,
    ];
    const TARGETS: &'static [&'static str] = &[live_echo::MODULE];
    const MIN_WINDOW_WIDTH: u32 = 320;

    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        cx.follow::<HostSession>((), |view, session, cx| match session {
            Ok(next) => view.session_changed(next, cx),
            Err(refusal) => cx.log_refused("the session", &refusal),
        })
        .detach();
        // the block that wrote the lobby's grant lets the reader in
        cx.follow::<Changes<LiveEcho>>((), |view, change, cx| match change {
            Ok(_) if view.lobby == Lobby::Opening => view.join(cx),
            Ok(_) => {}
            Err(refusal) => cx.log_refused("live-echo's blocks", &refusal),
        })
        .detach();
    }
}

impl Echo {
    /// Another reader, or the node back: everything joins anew.
    fn session_changed(&mut self, next: Session, cx: &mut Context<Self>) {
        let again =
            next.account != self.session.account || (next.connected && !self.session.connected);
        self.session = next;
        if again {
            self.opened = false;
            self.join(cx);
        }
    }

    /// Publishes the reader's track and follows the room's, dropping what
    /// ran before.
    pub(crate) fn join(&mut self, cx: &mut Context<Self>) {
        self.mine = None;
        self.watch = None;
        self.hearing.clear();
        self.people.clear();
        self.listeners = 0;
        cx.notify();
        let Some(me) = self.session.account else {
            self.lobby = Lobby::Away;
            return;
        };
        self.lobby = Lobby::In;
        self.people.entry(me).or_default();
        self.mine = Some(Track::publish(
            cx,
            track(&format!("{ECHO}{me}")),
            LiveMode::Stream,
            |view: &mut Echo, item, cx| match item {
                Ok(Publishing::Subscribers(count)) => {
                    view.listeners = count;
                    cx.notify();
                }
                Err(refusal) => view.refused(refusal, cx),
            },
        ));
        self.watch = Some(
            cx.follow::<LiveTracks>(LOBBY.to_owned(), |view, item, cx| match item {
                Ok(announce) => view.announced(announce, cx),
                Err(refusal) => view.refused(refusal, cx),
            }),
        );
    }

    /// A track of the lobby came up or went down. The reader hears every
    /// `echo/*` but its own.
    fn announced(&mut self, announce: LiveAnnounce, cx: &mut Context<Self>) {
        let LiveAnnounce { name, by, up, .. } = announce;
        if !name.starts_with(ECHO) || Some(by) == self.session.account || !up {
            return;
        }
        self.people.entry(by).or_default().gone = None;
        let heard = subscribe(cx, track(&name), true, move |view: &mut Echo, item, cx| {
            let person = view.people.entry(by).or_default();
            match item {
                Ok(Received::Object { value, .. }) => {
                    person.lines.push(value);
                    let over = person.lines.len().saturating_sub(KEPT);
                    person.lines.drain(..over);
                }
                Ok(Received::Opened { .. } | Received::GroupEnd { .. }) => return,
                // a body another build wrote (the SDK's `unexpected_reply`):
                // the subscription goes on
                Err(refusal) if refusal.code == "unexpected_reply" => {
                    return cx.log_refused("a line", &refusal);
                }
                Err(refusal) => person.gone = Some(refusal.code),
            }
            cx.notify();
        });
        self.hearing.insert(name, heard);
        cx.notify();
    }

    /// A refusal on the reader's track or on the room's list. A refused
    /// line goes to the log and the track goes on; the first `not_granted`
    /// opens the lobby; anything else ended the track and stays on screen.
    fn refused(&mut self, refusal: Error, cx: &mut Context<Self>) {
        let one_line = [refusal::TOO_LARGE, refusal::RATE_LIMIT].contains(&refusal.code.as_str());
        match &self.lobby {
            Lobby::In if one_line => return cx.log_refused("a line", &refusal),
            Lobby::In if refusal.code == refusal::NOT_GRANTED && !self.opened => self.open(cx),
            // the room's list and the track both refuse; one opens
            Lobby::Opening if refusal.code == refusal::NOT_GRANTED => {}
            _ => {
                self.mine = None;
                self.watch = None;
                self.lobby = Lobby::Refused(refusal.code);
            }
        }
        cx.notify();
    }

    /// `Open { room: "lobby" }`, once a session. Its block joins again
    /// (`attach`); a refused op is shown.
    fn open(&mut self, cx: &mut Context<Self>) {
        self.mine = None;
        self.watch = None;
        self.lobby = Lobby::Opening;
        self.opened = true;
        let submit = cx.host().ask::<Submit<LiveEcho>>(Op::Open {
            room: LOBBY.to_owned(),
        });
        cx.land(submit, |view, answer, cx| {
            if let Err(refusal) = answer {
                view.lobby = Lobby::Refused(refusal.code);
                cx.notify();
            }
        })
        .detach();
    }

    /// Enter in the field, or Send: the line goes out as one group of one
    /// object, and onto the reader's own list.
    pub(crate) fn say(&mut self, cx: &mut Context<Self>) {
        let line = self.line.text().trim().to_owned();
        let (Some(me), Some(mine)) = (self.session.account, self.mine.as_mut()) else {
            return;
        };
        if line.is_empty() {
            return;
        }
        let host = cx.host();
        mine.send(&host, &line);
        mine.next_group(&host);
        self.people.entry(me).or_default().lines.push(line);
        self.line.reset("");
        cx.notify();
    }
}

fn track(name: &str) -> LiveTrack {
    LiveTrack {
        room: LOBBY.to_owned(),
        name: name.to_owned(),
    }
}

impl Render for Echo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.global::<Theme>();
        let me = self.session.account;
        let status = match &self.lobby {
            Lobby::Away => "Take an account to talk in the lobby.".to_owned(),
            Lobby::Opening => "Opening the lobby…".to_owned(),
            Lobby::Refused(code) => format!("The lobby refused: {code}"),
            Lobby::In => match self.listeners {
                1 => "1 listening".to_owned(),
                count => format!("{count} listening"),
            },
        };
        // the reader first, then everyone else by account
        let people = me
            .and_then(|me| self.people.get(&me).map(|person| (me, person)))
            .into_iter()
            .chain(
                self.people
                    .iter()
                    .filter(|(account, _)| Some(**account) != me)
                    .map(|(a, p)| (*a, p)),
            );
        let people: Vec<_> = people
            .map(|(account, person)| person_card(account, Some(account) == me, person, &theme))
            .collect();
        let open = self.lobby == Lobby::In;
        let field = Input::new("line", &self.line, "Line")
            .flex_1()
            .h(design::size::CONTROL)
            .px(design::space::SM)
            .border_1()
            .border_color(theme.border_strong)
            .placeholder("Say something to the lobby")
            .disabled(!open)
            .on_submit(cx.listener(|view: &mut Echo, _: &(), _, cx| view.say(cx)));
        let send = design::button(
            "send",
            "Send",
            &theme,
            cx.listener(|view: &mut Echo, _: &ClickEvent, _, cx| view.say(cx)),
        )
        .enabled(open);
        div()
            .id("echo")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(design::text::BODY)
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap(design::space::SM)
                    .px(design::space::LG)
                    .py(design::space::MD)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(design::heading("echo-title", "Lobby", 1, &theme))
                    .child(design::quiet(status, &theme).id("echo-status")),
            )
            .child(
                div()
                    .id("people")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(design::space::BLOCK)
                    .p(design::space::LG)
                    .children(people),
            )
            .child(
                div()
                    .flex()
                    .gap(design::space::SM)
                    .p(design::space::LG)
                    .border_t_1()
                    .border_color(theme.border)
                    .child(field)
                    .child(send),
            )
    }
}

/// An account's lines under its name, with why its track ended when it did.
fn person_card(account: u64, mine: bool, person: &Person, theme: &Theme) -> impl IntoElement {
    let name = match mine {
        true => format!("Account {account} (you)"),
        false => format!("Account {account}"),
    };
    let lines = person.lines.iter().enumerate().map(|(at, line)| {
        div()
            .id(ElementId::Name(format!("line-{account}-{at}").into()))
            .child(line.clone())
    });
    div()
        .id(ElementId::Name(format!("person-{account}").into()))
        .flex()
        .flex_col()
        .gap(design::space::XXS)
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(design::space::SM)
                .child(design::heading(
                    ElementId::Name(format!("account-{account}").into()),
                    name,
                    2,
                    theme,
                ))
                .children(
                    person
                        .gone
                        .clone()
                        .map(|code| design::quiet(format!("left: {code}"), theme)),
                ),
        )
        .when(person.lines.is_empty(), |card| {
            card.child(design::quiet("Nothing yet.", theme))
        })
        .children(lines)
}

export_view!(Echo);

#[cfg(test)]
mod tests;
