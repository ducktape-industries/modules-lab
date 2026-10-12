//! Browser: web pages in tabs, each page drawn by the host's browser engine
//! (the SDK's `web` piece, which only a view declaring `Capability::Web`
//! may place).
//!
//! Laid out as Safari's "Separate" tabs: one toolbar, then the tab bar,
//! then the page. The toolbar holds back and forward, the address field
//! centred, and a new-tab button. At rest the field shows the page's host
//! (a lock for `https`) with reload, stop while the page loads, at its
//! right end, and a line along its bottom while it loads; pressed, it is
//! the whole address, selected, to type over. Enter opens what it says: a
//! bare word, or anything without `://`, gets `https://` in front; Escape
//! or a press outside puts the host back. The tab bar shows from two tabs
//! up, the tabs sharing its width, each with a close button on hover. A
//! link the app hands the view (`host.route`: the app sends `https://`
//! links here when this view is the one that declares `Web`) opens in a
//! new tab.
//!
//! Every tab keeps its page: all pieces stay in the tree, stacked in one
//! box, the inactive ones invisible, so switching tabs never reloads one. A
//! piece is keyed by its tab's id, never its place, so closing a tab
//! leaves the others' pages where they are.
use ducktape_view_guest::prelude::*;
use ducktape_view_guest::wire::{ElementIdWire, WidgetCommand};
use ducktape_view_guest::{Div, Stateful, Svg};
use serde::{Deserialize, Serialize};

/// What the new-tab button opens.
pub const HOME: &str = "https://www.wikipedia.org/";

/// The address field's id, at rest (a button) and typing (the field).
const ADDRESS: &str = "browser-address";

/// The address field's and the glyph buttons' corners. Safari's, where the
/// product's controls are square (`design::radius::CONTROL`): the owner
/// asked for a Safari-like browser.
const ROUND: Pixels = px(8.);

/// The address field's widest, as Safari's.
const FIELD_WIDTH: Pixels = px(640.);

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
    editing: bool,
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

    fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // no tab open: typing goes to the address field at once
        focus_address(window);
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

/// What the address field shows of `url` at rest: its host, `www.`
/// dropped (`https://www.google.com/search?q=1` is `google.com`).
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host)
}

/// Gives the keyboard to the address field with its text selected, so what
/// is typed replaces it.
fn focus_address(window: &mut Window) {
    window.focus(ADDRESS);
    window.dispatch(WidgetCommand::SelectAll {
        target: vec![ElementIdWire::Name(ADDRESS.into())],
    });
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
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    fn rest(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            cx.notify();
        }
    }

    /// Enter in the address field: the page on screen goes there, or the
    /// first tab opens on it. The field shows the page's host again, which
    /// moves when the page says where it went.
    fn go(&mut self, cx: &mut Context<Self>) {
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

    fn navigate(&mut self, command: fn(&WebHandle), cx: &mut Context<Self>) {
        if let Some(tab) = self.tab() {
            command(&tab.nav);
            cx.notify();
        }
    }

    /// The page of tab `id` says where it is.
    fn heard(&mut self, id: u64, state: &WebState, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return;
        };
        tab.state = state.clone();
        self.settle_title(cx);
        cx.notify();
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
fn label(state: &WebState) -> String {
    match state.title.trim() {
        "" => host(&state.url).to_owned(),
        title => title.to_owned(),
    }
}

/// One of the app's bundled Lucide icons, `size` square, in the text
/// colour around it.
fn glyph(path: &'static str, size: Pixels) -> Svg {
    svg().path(path).size(size).flex_none()
}

/// A glyph button of the toolbar, `design::icon_button` at the control's
/// height with a soft fill on hover; one that cannot act is faint, says
/// so, and answers nothing.
fn glyph_button(
    id: &'static str,
    icon: &'static str,
    name: &'static str,
    enabled: bool,
    theme: &Theme,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let theme = *theme;
    let icon = glyph(icon, px(18.));
    let button = match enabled {
        true => design::icon_button(id, icon, name, &theme, click)
            .hover(move |style| style.text_color(theme.foreground).bg(theme.surface_raised)),
        false => div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme.faint)
            .role(Role::Button)
            .aria_label(name)
            .aria_disabled(true)
            .child(icon),
    };
    button.size(design::size::CONTROL).rounded(ROUND)
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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .bg(theme.surface)
                    .child(self.toolbar(cx, &theme))
                    .map(|chrome| match self.tabs.len() > 1 {
                        true => chrome.child(self.tab_bar(cx, &theme)),
                        // the hairline the tab bar draws otherwise
                        false => chrome.border_b_1().border_color(theme.border),
                    }),
            )
            .child(self.pages(cx, &theme))
    }
}

impl Browser {
    /// Back and forward, the address field centred, new tab.
    fn toolbar(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let can = |pick: fn(&WebState) -> bool| self.tab().is_some_and(|tab| pick(&tab.state));
        // the field is centred in the bar: the two sides grow alike from
        // one width, two buttons' (back and forward)
        let side = || {
            div()
                .flex_1()
                .min_w(design::size::CONTROL * 2. + design::space::XXS)
                .flex()
                .items_center()
                .gap(design::space::XXS)
        };
        div()
            .id("browser-toolbar")
            .w_full()
            .flex()
            .items_center()
            .gap(design::space::SM)
            .px(design::space::LG)
            .py(design::space::LG)
            .child(
                side()
                    .child(glyph_button(
                        "browser-back",
                        "icons/chevron-left.svg",
                        "Back",
                        can(|state| state.can_go_back),
                        theme,
                        cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                            view.navigate(WebHandle::back, cx)
                        }),
                    ))
                    .child(glyph_button(
                        "browser-forward",
                        "icons/chevron-right.svg",
                        "Forward",
                        can(|state| state.can_go_forward),
                        theme,
                        cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                            view.navigate(WebHandle::forward, cx)
                        }),
                    )),
            )
            .child(self.field(cx, theme))
            .child(side().justify_end().child(glyph_button(
                "browser-new-tab",
                "icons/plus.svg",
                "New tab",
                true,
                theme,
                cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                    view.open(HOME.to_owned(), cx)
                }),
            )))
    }

    /// The address field: at rest the host, centred, with reload or stop
    /// at its right end; typing, the whole address.
    fn field(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let theme = *theme;
        let field = div()
            .id("browser-field")
            .flex_basis(FIELD_WIDTH)
            .min_w_0()
            .h(design::size::CONTROL)
            .relative()
            .rounded(ROUND)
            .bg(theme.surface_raised);
        let tab = match self.tab() {
            Some(tab) if !self.editing => tab,
            // typing: the field itself, Enter opens, Escape or a press
            // outside puts the host back
            _ => {
                return field
                    .on_mouse_down_out(
                        cx.listener(|view: &mut Browser, _: &MouseDownEvent, _, cx| view.rest(cx)),
                    )
                    // the field's Escape passes up to here
                    .role(Role::Group)
                    .aria_label("Address")
                    .consumes_keys(["escape"])
                    .on_key_down(
                        cx.listener(|view: &mut Browser, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "escape" {
                                view.rest(cx)
                            }
                        }),
                    )
                    .child(
                        Input::new(ADDRESS, &self.address, "Address")
                            .size_full()
                            .px(design::space::MD)
                            .rounded(ROUND)
                            .bg(theme.surface_raised)
                            .border_1()
                            .border_color(theme.surface_raised)
                            .text_size(design::text::BODY)
                            .placeholder("Type an address")
                            .on_submit(
                                cx.listener(|view: &mut Browser, _: &(), _, cx| view.go(cx)),
                            ),
                    );
            }
        };
        let state = &tab.state;
        let (reload, icon, name): (fn(&WebHandle), _, _) = match state.loading {
            true => (WebHandle::stop, "icons/x.svg", "Stop"),
            false => (WebHandle::reload, "icons/rotate-cw.svg", "Reload"),
        };
        // the host is centred in the whole field, clear of reload on the
        // right and as clear on the left
        let shown = div()
            .id(ADDRESS)
            .absolute()
            .inset_0()
            .px(design::size::CONTROL)
            .flex()
            .items_center()
            .justify_center()
            .gap(design::space::XXS)
            .rounded(ROUND)
            .cursor_text()
            .role(Role::Button)
            .aria_label("Address")
            .aria_value(state.url.clone())
            .focusable()
            .on_click(
                cx.listener(|view: &mut Browser, _: &ClickEvent, window, cx| view.edit(window, cx)),
            )
            .when(state.url.starts_with("https://"), |shown| {
                shown.child(glyph("icons/lock.svg", px(11.)).text_color(theme.muted))
            })
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(host(&state.url).to_owned()),
            );
        field
            .child(shown)
            .child(
                design::icon_button(
                    "browser-reload",
                    glyph(icon, px(14.)),
                    name,
                    &theme,
                    cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.navigate(reload, cx)
                    }),
                )
                .absolute()
                .top(px(2.))
                .right(px(2.))
                .rounded(ROUND)
                // its press is not the field's under it
                .consumes_click(),
            )
            .when(state.loading, |field| {
                field.child(
                    div()
                        .id("browser-loading")
                        .absolute()
                        .bottom_0()
                        // clear of the round corners, which do not clip it
                        .left(ROUND)
                        .right(ROUND)
                        .h(px(2.))
                        .bg(theme.accent),
                )
            })
    }

    /// The tabs, sharing the bar's width; each closes from a button at its
    /// left, shown on hover and on the chosen one.
    fn tab_bar(&self, cx: &mut Context<Self>, theme: &Theme) -> Stateful<Div> {
        let theme = *theme;
        let tabs = self.tabs.iter().enumerate().map(|(index, tab)| {
            let active = index == self.active;
            let group = format!("browser-tab-group-{}", tab.id);
            let title = div()
                .id(format!("browser-tab-{}", tab.id))
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .px(design::size::CONTROL)
                .text_size(design::text::SECONDARY)
                .text_color(if active {
                    theme.foreground
                } else {
                    theme.muted
                })
                .hover(move |style| style.text_color(theme.foreground))
                .aria_selected(active)
                .on_click(
                    cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.select(index, cx)
                    }),
                )
                .child(div().min_w_0().truncate().child(label(&tab.state)));
            div()
                .id(format!("browser-tab-slot-{}", tab.id))
                .group(group.clone())
                .flex_1()
                .min_w_0()
                .h_full()
                .relative()
                .bg(if active {
                    theme.background
                } else {
                    theme.surface_raised
                })
                .when(index > 0, |slot| {
                    slot.border_l_1().border_color(theme.border)
                })
                .child(design::item(title, Role::Tab, active))
                .child(
                    design::icon_button(
                        format!("browser-close-tab-{}", tab.id),
                        glyph("icons/x.svg", px(12.)),
                        "Close tab",
                        &theme,
                        cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                            view.close(index, cx)
                        }),
                    )
                    .absolute()
                    .top(px(2.))
                    .left(px(2.))
                    .rounded(ROUND)
                    .hover(move |style| style.text_color(theme.foreground).bg(theme.surface_raised))
                    // its press is the button's, not the tab's under it,
                    // and its Enter not the tab list's around it
                    .consumes_click()
                    .consumes_keys(["enter", "space"])
                    .when(!active, |close| {
                        close
                            .tab_stop(false)
                            .invisible()
                            .group_hover(group, |style| style.visible())
                    }),
                )
        });
        design::composite("browser-tabs", Role::TabList, "Tabs")
            .orientation(design::Orientation::Horizontal)
            .active(self.active, self.tabs.len())
            .on_move(cx.processor(|view: &mut Browser, index: usize, _, cx| view.select(index, cx)))
            .on_press(
                cx.processor(|view: &mut Browser, index: usize, _, cx| view.select(index, cx)),
            )
            .build()
            .w_full()
            .h(design::size::CONTROL)
            .flex()
            // hairlines round the bar, which the focus ring inks: the ring's
            // own inset shadow is under the tabs' fills
            .border_1()
            .border_color(theme.border)
            .children(tabs)
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
                pages.flex().items_center().justify_center().child(
                    design::empty_state(
                        "browser-empty",
                        "No page open",
                        "Type an address above and press Enter.",
                        theme,
                    )
                    .items_center()
                    .text_center(),
                )
            })
    }
}

// Writes the wasm exports the host calls and the manifest it reads.
export_view!(Browser);

#[cfg(test)]
mod tests;
