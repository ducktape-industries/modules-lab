//! Browser: web pages in tabs, each page drawn by the host's browser engine
//! (the SDK's `web` piece, which only a view declaring `Capability::Web`
//! may place).
//!
//! Every tab keeps its page: all pieces stay in the tree, stacked in one
//! box, the inactive ones invisible, so switching tabs never reloads one. A
//! piece is keyed by its tab's id, never its place, so closing a tab
//! leaves the others' pages where they are.
//!
//! A phone's build has no web view: under a finger the view is one screen
//! that hands an address to the phone's own browser (`link.open`).
//!
//! This file is the model: the tabs, the address as typed, the links the
//! app sent here, the addresses handed out, and what each press changes.
//! What is drawn is `ui` (the desktop's tree, under a pointer) or
//! `ui::phone` (the phone's, under a finger), picked once at the root, in
//! the words of `words`.
use serde::{Deserialize, Serialize};
use view::methods::LinkOpen;
use view::prelude::*;
use view::wire::{ElementIdWire, WidgetCommand};

mod ui;
mod words;

/// What the new-tab button opens, and the phone's home row.
pub const HOME: &str = "https://www.wikipedia.org/";

/// How many addresses the phone lists of each kind: links sent here, and
/// addresses handed to its browser.
const LISTED: usize = 8;

/// The address field's id, at rest (a button) and typing (the field).
pub(crate) const ADDRESS: &str = "browser-address";

#[derive(Default, Serialize, Deserialize)]
pub struct Browser {
    /// The open tabs, in the bar's order. None open: the address field
    /// opens the first. A snapshot comes back with none (ponytail: a page
    /// is not saved; keep each tab's address when a restored browser
    /// should reopen its tabs).
    #[serde(skip)]
    pub(crate) tabs: Vec<Tab>,
    /// The tab on screen, a place in `tabs`.
    #[serde(skip)]
    pub(crate) active: usize,
    /// The next tab's id.
    #[serde(skip)]
    next: u64,
    /// The address field, as typed.
    #[serde(skip)]
    pub(crate) address: TextField,
    /// The address field is being typed in. The view hears no focus or
    /// blur from the host, so a press on the field starts it and Enter,
    /// Escape or a press outside ends it. With no tab open the field is
    /// always typing.
    #[serde(skip)]
    pub(crate) editing: bool,
    /// The window title last sent.
    #[serde(skip)]
    title: String,
    /// The keys go to the address field on the desktop's next frame: set
    /// as the view opens, so with no tab open typing goes there at once.
    /// The phone's screen gives the keys to nothing unasked, as a focused
    /// field raises the keyboard over half the screen.
    #[serde(skip)]
    pub(crate) keys_to_address: bool,
    /// The links the app sent here (`host.route`) and no design has taken
    /// yet, newest first, each once, at most [`LISTED`]. The desktop's
    /// tree opens each in a tab as it draws; the phone's lists them, as
    /// its browser opens a link only from a press.
    #[serde(skip)]
    pub(crate) routed: Vec<String>,
    /// The addresses handed to the phone's own browser this session,
    /// newest first, each once, at most [`LISTED`]. A snapshot comes back
    /// with none, as the desktop's tabs do.
    #[serde(skip)]
    pub(crate) handed: Vec<String>,
    /// The address the host last would not open, and its refusal: said
    /// under Open, with Retry, until an address opens.
    #[serde(skip)]
    pub(crate) refused: Option<(String, Error)>,
}

/// One page.
pub(crate) struct Tab {
    /// Its piece's key (`page-<id>`), never reused.
    pub(crate) id: u64,
    /// The address the piece opened on; later ones go through `nav`.
    pub(crate) opened: String,
    pub(crate) nav: WebHandle,
    /// What the page last told: its address, title, loading, history.
    pub(crate) state: WebState,
}

impl View for Browser {
    const NAME: &'static str = words::BROWSER;
    const DESCRIPTION: &'static str = "Web pages in tabs.";
    const ICON: &'static str = "icons/globe.svg";
    // `web`: the pages; `host`: the links the app opens here
    // (`host.route`) and the window's title (`host.title`); `link`: the
    // phone's addresses handed to its own browser (`link.open`)
    const CAPABILITIES: &'static [Capability] =
        &[Capability::Web, Capability::Host, Capability::Link];

    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        // no tab open: typing goes to the address field at once, where a
        // tree gives it the keys
        self.keys_to_address = true;
        // a route is never empty, so every one is an address; each design
        // takes it as it draws
        cx.follow::<HostRoute>((), |view, route, cx| match route {
            Ok(link) => {
                if let Some(url) = address(&link) {
                    listed(&mut view.routed, url);
                    cx.notify();
                }
            }
            Err(refusal) => cx.log_refused("the route", &refusal),
        })
        .detach();
    }
}

/// What the address field opens for `typed`: as typed when it starts with
/// a scheme (`https://…`, `http://localhost…`; the host refuses any
/// other), else `https://` in front of it. Nothing for a blank field.
pub fn address(typed: &str) -> Option<String> {
    let typed = typed.trim();
    // a scheme is a letter, then letters, digits, `+`, `-` or `.`: a `://`
    // further on (`a.com/?next=https://b`) is not one
    let schemed = typed.split_once("://").is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    match typed {
        "" => None,
        typed if schemed => Some(typed.to_owned()),
        typed => Some(format!("https://{typed}")),
    }
}

/// What the address field shows of `url` at rest, and whether it earns the
/// lock. The host alone, as the URL standard reads it (the page's engine
/// reads it so too): never what comes before an `@`, a port the scheme
/// does not imply kept (`localhost:8000`), `www.` dropped where a dot stays
/// (`https://www.google.com/a?q=1` is `google.com`), an international name
/// in its `xn--` form. An address with a user name or password in it, or one
/// with no host or none the standard reads, shows whole and unlocked: a
/// clean-looking host is never made of it. The field cuts what it shows
/// from the start, so the right of a host, the name it is registered
/// under, stays in view.
pub fn host_shown(url: &str) -> (String, bool) {
    let whole = (url.to_owned(), false);
    let Ok(parsed) = url::Url::parse(url) else {
        return whole;
    };
    let Some(host) = parsed.host_str() else {
        return whole;
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return whole;
    }
    let host = match host.strip_prefix("www.") {
        Some(rest) if rest.contains('.') => rest,
        _ => host,
    };
    let shown = match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    };
    (shown, parsed.scheme() == "https")
}

/// Gives the keyboard to the address field with its text selected, so what
/// is typed replaces it.
pub(crate) fn focus_address(window: &mut Window) {
    window.focus(ADDRESS);
    window.dispatch(WidgetCommand::SelectAll {
        target: vec![ElementIdWire::Name(ADDRESS.into())],
    });
}

/// `url` first in `list`, and nowhere else in it, which keeps at most
/// [`LISTED`].
fn listed(list: &mut Vec<String>, url: String) {
    list.retain(|listed| *listed != url);
    list.insert(0, url);
    list.truncate(LISTED);
}

impl Browser {
    /// The tab on screen.
    pub(crate) fn tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    /// A new tab on `url`, on screen.
    pub(crate) fn open(&mut self, url: String, cx: &mut Context<Self>) {
        self.next += 1;
        self.tabs.push(Tab {
            id: self.next,
            opened: url.clone(),
            nav: WebHandle::new(),
            state: WebState {
                url,
                ..WebState::default()
            },
        });
        self.select(self.tabs.len() - 1, cx);
    }

    pub(crate) fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.active = index;
        self.editing = false;
        self.settle_title(cx);
        cx.notify();
    }

    pub(crate) fn close(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        let active = match self.active {
            active if active > index => active - 1,
            active => active.min(self.tabs.len().saturating_sub(1)),
        };
        self.select(active, cx);
    }

    /// A press on the address field at rest: the whole address, selected.
    pub(crate) fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self
            .tab()
            .map_or(String::new(), |tab| tab.state.url.clone());
        self.address.reset(url);
        self.editing = true;
        focus_address(window);
        cx.notify();
    }

    /// Escape, or a press outside the field: the host back, what was typed
    /// dropped.
    pub(crate) fn rest(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            cx.notify();
        }
    }

    /// Enter in the address field: the page on screen goes there, or the
    /// first tab opens on it. The field shows the page's host again, which
    /// moves when the page says where it went.
    pub(crate) fn go(&mut self, cx: &mut Context<Self>) {
        let Some(url) = address(&self.address.text()) else {
            return;
        };
        self.editing = false;
        match self.tab() {
            Some(tab) => {
                tab.nav.load(url);
                cx.notify();
            }
            None => self.open(url, cx),
        }
    }

    pub(crate) fn navigate(&mut self, command: fn(&WebHandle), cx: &mut Context<Self>) {
        if let Some(tab) = self.tab() {
            command(&tab.nav);
            cx.notify();
        }
    }

    /// The page of tab `id` says where it is.
    pub(crate) fn heard(&mut self, id: u64, state: &WebState, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return;
        };
        tab.state = state.clone();
        self.settle_title(cx);
        cx.notify();
    }

    /// Hands `url` to the phone's own browser (the phone's build has no
    /// web view, so the host opens an `https` link outside the app). An
    /// `http://` address goes as `https://`: the host hands on no other,
    /// and the phone's browser upgrades one anyway. Once the host has
    /// opened it, it is first among the addresses handed this session and
    /// no longer a link waiting; refused, the refusal is said in its
    /// place. The host takes it only from a press or a key, which the ask
    /// carries as it is sent.
    pub(crate) fn open_outside(&mut self, url: String, cx: &mut Context<Self>) {
        let sent = match url.get(..7) {
            Some(scheme) if scheme.eq_ignore_ascii_case("http://") => {
                format!("https://{}", &url[7..])
            }
            _ => url.clone(),
        };
        let opening = cx.host().ask::<LinkOpen>(sent.clone());
        cx.land(opening, move |view, opened, cx| {
            match opened {
                Ok(()) => {
                    view.refused = None;
                    view.routed.retain(|routed| *routed != url);
                    listed(&mut view.handed, sent);
                }
                Err(refusal) => view.refused = Some((sent, refusal)),
            }
            cx.notify();
        })
        .detach();
    }

    /// The address was edited: a refusal of the one before no longer
    /// stands.
    pub(crate) fn edited(&mut self, cx: &mut Context<Self>) {
        if self.refused.take().is_some() {
            cx.notify();
        }
    }

    /// The phone's Open, or Enter in its field: what is typed, as the
    /// address field reads it, handed to the phone's browser. The field
    /// keeps the text, so a typo is mended, not typed again.
    pub(crate) fn open_typed(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = address(&self.address.text()) {
            self.open_outside(url, cx);
        }
    }

    /// The window's title is the page's: its title, or its host before it
    /// has one. Sent when it changes.
    fn settle_title(&mut self, cx: &mut Context<Self>) {
        let title = self.tab().map_or(String::new(), |tab| label(&tab.state));
        if title != self.title {
            cx.host().notify::<HostTitle>(title.clone());
            self.title = title;
        }
    }
}

/// A tab's name: its page's title, else its host.
pub(crate) fn label(state: &WebState) -> String {
    match state.title.trim() {
        "" => host_shown(&state.url).0,
        title => title.to_owned(),
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // one wasm, two designs, the model shared: a finger gets the
        // phone's screen, a pointer the desktop's
        if window.touch() {
            ui::phone::render(self, cx)
        } else {
            ui::render(self, window, cx)
        }
    }
}

// Writes the wasm exports the host calls and the manifest it reads.
export_view!(Browser);

#[cfg(test)]
mod tests;
