//! Browser: web pages in tabs, each page drawn by the host's browser engine
//! (the SDK's `web` piece, which only a view declaring `Capability::Web`
//! may place).
//!
//! The bar holds the tabs (each named by its page's title), a new-tab and a
//! close-tab button; under it back, forward, reload (stop while a page
//! loads) and the address. Enter opens what the address says: a bare word,
//! or anything without `://`, gets `https://` in front. A link the app
//! hands the view (`host.route`: the app sends `https://` links here when
//! this view is the one that declares `Web`) opens in a new tab.
//!
//! Every tab keeps its page: all pieces stay in the tree, stacked in one
//! box, the inactive ones invisible, so switching tabs never reloads one. A
//! piece is keyed by its tab's id, never its place, so closing a tab
//! leaves the others' pages where they are.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::{Div, Stateful};
use serde::{Deserialize, Serialize};

/// What the new-tab button opens.
pub const HOME: &str = "https://www.wikipedia.org/";

#[derive(Default, Serialize, Deserialize)]
pub struct Browser {
    /// The open tabs, in the bar's order. None open: the address bar opens
    /// the first. A snapshot comes back with none (ponytail: a page is not
    /// saved; keep each tab's address when a restored browser should
    /// reopen its tabs).
    #[serde(skip)]
    pub(crate) tabs: Vec<Tab>,
    /// The tab on screen, a place in `tabs`.
    #[serde(skip)]
    pub(crate) active: usize,
    /// The next tab's id.
    #[serde(skip)]
    next: u64,
    /// The address bar, as typed.
    #[serde(skip)]
    pub(crate) address: TextField,
    /// The window title last sent.
    #[serde(skip)]
    title: String,
}

/// One page.
pub(crate) struct Tab {
    /// Its piece's key (`page-<id>`), never reused.
    pub(crate) id: u64,
    /// The address the piece opened on; later ones go through `nav`.
    opened: String,
    nav: WebHandle,
    /// What the page last told: its address, title, loading, history.
    pub(crate) state: WebState,
}

impl View for Browser {
    const NAME: &'static str = "Browser";
    const DESCRIPTION: &'static str = "Web pages in tabs.";
    const ICON: &'static str = "icons/globe.svg";
    // `web`: the pages; `host`: the links the app opens here
    // (`host.route`) and the window's title (`host.title`)
    const CAPABILITIES: &'static [Capability] = &[Capability::Web, Capability::Host];

    fn attach(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        // a route is never empty, so every one is an address
        cx.follow::<HostRoute>((), |view, route, cx| match route {
            Ok(link) => {
                if let Some(url) = address(&link) {
                    view.open(url, cx)
                }
            }
            Err(refusal) => cx.log_refused("the route", &refusal),
        })
        .detach();
    }
}

/// What the address bar opens for `typed`: as typed when it names a scheme
/// (`https://…`, `http://localhost…`; the host refuses any other), else
/// `https://` in front of it. Nothing for a blank bar.
pub fn address(typed: &str) -> Option<String> {
    let typed = typed.trim();
    match typed {
        "" => None,
        typed if typed.contains("://") => Some(typed.to_owned()),
        typed => Some(format!("https://{typed}")),
    }
}

impl Browser {
    fn tab(&self) -> Option<&Tab> {
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
        let url = self
            .tab()
            .map_or(String::new(), |tab| tab.state.url.clone());
        self.address.reset(url);
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

    /// Enter in the address bar: the page on screen goes there, or the
    /// first tab opens on it. The bar keeps what was typed until the page
    /// says where it went.
    fn go(&mut self, cx: &mut Context<Self>) {
        let Some(url) = address(&self.address.text()) else {
            return;
        };
        match self.tab() {
            Some(tab) => {
                tab.nav.load(url);
                cx.notify();
            }
            None => self.open(url, cx),
        }
    }

    fn navigate(&mut self, command: fn(&WebHandle), cx: &mut Context<Self>) {
        if let Some(tab) = self.tab() {
            command(&tab.nav);
            cx.notify();
        }
    }

    /// The page of tab `id` says where it is.
    fn heard(&mut self, id: u64, state: &WebState, cx: &mut Context<Self>) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let tab = &mut self.tabs[index];
        let moved = tab.state.url != state.url;
        tab.state = state.clone();
        if index == self.active && moved {
            self.address.reset(state.url.clone());
        }
        self.settle_title(cx);
        cx.notify();
    }

    /// The window's title is the page's: its title, or its address before
    /// it has one. Sent when it changes.
    fn settle_title(&mut self, cx: &mut Context<Self>) {
        let title = self.tab().map_or(String::new(), |tab| label(&tab.state));
        if title != self.title {
            cx.host().notify::<HostTitle>(title.clone());
            self.title = title;
        }
    }
}

/// A tab's name: its page's title, else its address.
fn label(state: &WebState) -> String {
    match state.title.trim() {
        "" => state.url.clone(),
        title => title.to_owned(),
    }
}

impl Render for Browser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.global::<Theme>();
        div()
            .id("browser")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(design::text::BODY)
            .child(self.tab_bar(cx, &theme))
            .child(self.toolbar(cx, &theme))
            .child(self.pages(cx, &theme))
    }
}

impl Browser {
    fn tab_bar(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let tabs = self.tabs.iter().enumerate().map(|(index, tab)| {
            let active = index == self.active;
            let select = cx
                .listener(move |view: &mut Browser, _: &ClickEvent, _, cx| view.select(index, cx));
            design::item(
                design::tab(
                    format!("browser-tab-{}", tab.id),
                    label(&tab.state),
                    active,
                    theme,
                    select,
                )
                .h_full()
                .max_w(px(200.))
                .overflow_hidden(),
                Role::Tab,
                active,
            )
        });
        let count = self.tabs.len();
        div()
            .id("browser-tab-bar")
            .w_full()
            .h(design::size::ROW)
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                design::composite("browser-tabs", Role::TabList, "Tabs")
                    .orientation(design::Orientation::Horizontal)
                    .active(self.active, count)
                    .on_move(cx.processor(|view: &mut Browser, index: usize, _, cx| {
                        view.select(index, cx)
                    }))
                    .on_press(cx.processor(|view: &mut Browser, index: usize, _, cx| {
                        view.select(index, cx)
                    }))
                    .build()
                    .h_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .children(tabs),
            )
            .child(design::icon_button(
                "browser-new-tab",
                "+",
                "New tab",
                theme,
                cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                    view.open(HOME.to_owned(), cx)
                }),
            ))
            .when(count > 0, |bar| {
                bar.child(design::icon_button(
                    "browser-close-tab",
                    "×",
                    "Close tab",
                    theme,
                    cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.close(view.active, cx)
                    }),
                ))
            })
    }

    fn toolbar(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let state = self.tab().map(|tab| &tab.state);
        let can = |pick: fn(&WebState) -> bool| state.is_some_and(pick);
        let loading = can(|state| state.loading);
        let (reload, label): (fn(&WebHandle), _) = match loading {
            true => (WebHandle::stop, "Stop"),
            false => (WebHandle::reload, "Reload"),
        };
        div()
            .id("browser-toolbar")
            .w_full()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(theme.border)
            .child(
                design::button(
                    "browser-back",
                    "Back",
                    theme,
                    cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.navigate(WebHandle::back, cx)
                    }),
                )
                .kind(design::Kind::Quiet)
                .enabled(can(|state| state.can_go_back)),
            )
            .child(
                design::button(
                    "browser-forward",
                    "Forward",
                    theme,
                    cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.navigate(WebHandle::forward, cx)
                    }),
                )
                .kind(design::Kind::Quiet)
                .enabled(can(|state| state.can_go_forward)),
            )
            .child(
                design::button(
                    "browser-reload",
                    label,
                    theme,
                    cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.navigate(reload, cx)
                    }),
                )
                .kind(design::Kind::Quiet)
                .enabled(self.tab().is_some()),
            )
            .child(
                Input::new("browser-address", &self.address, "Address")
                    .flex_1()
                    .min_w_0()
                    .h(design::size::CONTROL)
                    .px_2()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .text_size(design::text::SECONDARY)
                    .placeholder("Type an address")
                    .on_submit(cx.listener(|view: &mut Browser, _: &(), _, cx| view.go(cx))),
            )
            .when(loading, |bar| {
                bar.child(
                    design::quiet("Loading…", theme)
                        .id("browser-loading")
                        .w(px(64.)),
                )
            })
    }

    /// Every tab's page, stacked; only the active one is visible.
    fn pages(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let pages = self.tabs.iter().enumerate().map(|(index, tab)| {
            let id = tab.id;
            web(format!("page-{id}"), tab.opened.clone())
                .track(&tab.nav)
                .on_state(
                    cx.listener(move |view: &mut Browser, state: &WebState, _, cx| {
                        view.heard(id, state, cx)
                    }),
                )
                .absolute()
                .inset_0()
                .when(index != self.active, |page| page.invisible())
        });
        div()
            .id("browser-pages")
            .flex_1()
            .min_h_0()
            .relative()
            .children(pages)
            .when(self.tabs.is_empty(), |pages| {
                pages.child(design::empty_state(
                    "browser-empty",
                    "No page open",
                    "Type an address above and press Enter.",
                    theme,
                ))
            })
    }
}

// Writes the wasm exports the host calls and the manifest it reads.
export_view!(Browser);

#[cfg(test)]
mod tests;
