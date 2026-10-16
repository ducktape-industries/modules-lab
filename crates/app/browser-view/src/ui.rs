//! The desktop's tree: what Browser draws under a pointer, on the
//! pointer's sizes. Laid out as Safari's "Separate" tabs, in the product's
//! own square material: one toolbar, then the tab bar, then the page. The
//! toolbar holds back and forward, the address field centred, and a
//! new-tab button. At rest the field shows the page's host (`host_shown`;
//! a lock for `https`) with reload, stop while the page loads, at its
//! right end, and a line along its bottom while it loads; pressed, it is
//! the whole address, selected, to type over. Enter opens what it says: a
//! bare word, or anything without `://`, gets `https://` in front; Escape
//! or a press outside puts the host back. The tab bar shows from two tabs
//! up, the tabs sharing its width, each with a close button on hover. A
//! link the app hands the view (`host.route`: the app sends `https://`
//! links here when this view is the one that declares `Web`) opens in a
//! new tab.
//!
//! Under a finger the root draws the phone's screen (`phone/`) instead.
use view::design::Ladder;
use view::prelude::*;
use view::{AnyElement, Div, FontWeight, Stateful, Svg};

use crate::words;
use crate::{ADDRESS, Browser, HOME, focus_address, host_shown, label};

/// The phone's tree, which the view's root picks under a finger. It reads
/// nothing of this one.
pub(crate) mod phone;

/// The address field's widest.
const FIELD_WIDTH: Pixels = px(560.);

/// The tab bar's height, the toolbar's hairline over it included, and a
/// tab title's inset each side, clear of its close button.
const TAB_BAR: Pixels = px(31.);
const TAB_INSET: Pixels = px(30.);

/// The toolbar, the tab bar from two tabs up, and the pages; on the first
/// frame, the keys to the address field; each link the app sent here, in a
/// new tab, the newest on screen.
pub(crate) fn render(
    view: &mut Browser,
    window: &mut Window,
    cx: &mut Context<Browser>,
) -> AnyElement {
    if std::mem::take(&mut view.keys_to_address) {
        focus_address(window);
    }
    for url in std::mem::take(&mut view.routed).into_iter().rev() {
        view.open(url, cx);
    }
    let theme = *cx.global::<Theme>();
    let ladder = &design::POINTER;
    div()
        .id("browser")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_size(design::text::BODY)
        .child(view.toolbar(cx, ladder, &theme))
        .when(view.tabs.len() > 1, |browser| {
            browser.child(view.tab_bar(cx, ladder, &theme))
        })
        .child(view.pages(cx, &theme))
        .into_any_element()
}

/// One of the app's bundled Lucide icons, `size` square, in the text
/// colour around it.
fn glyph(path: &'static str, size: Pixels) -> Svg {
    svg().path(path).size(size).flex_none()
}

/// A glyph pressed for what `name` says: muted, in ink under the pointer,
/// at least [`Ladder::hit`](design::Ladder) each way. The browser's own,
/// not `design::icon_button`, whose builder lets a caller only style it:
/// the field's reload and a tab's close sit over controls that take
/// presses too, so each consumes its own, and an unchosen tab's leaves the Tab
/// order, which needs the element itself.
fn icon_button(
    id: impl Into<ElementId>,
    glyph: impl IntoElement,
    name: &'static str,
    ladder: &Ladder,
    theme: &Theme,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let theme = *theme;
    div()
        .id(id)
        .min_w(ladder.hit)
        .min_h(ladder.hit)
        .flex()
        .items_center()
        .justify_center()
        .px_1()
        .text_color(theme.muted)
        .cursor_pointer()
        .hover(move |style| style.text_color(theme.foreground))
        .role(Role::Button)
        .aria_label(name)
        .focusable()
        .on_click(click)
        .child(glyph)
}

/// A glyph button of the toolbar, [`icon_button`] at the control's size,
/// in ink, with a soft fill on hover; one that cannot act is faint, says
/// so, and answers nothing.
fn glyph_button(
    id: &'static str,
    icon: &'static str,
    name: &'static str,
    enabled: bool,
    ladder: &Ladder,
    theme: &Theme,
    click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let theme = *theme;
    let icon = glyph(icon, px(16.));
    let button = match enabled {
        true => icon_button(id, icon, name, ladder, &theme, click)
            .text_color(theme.foreground)
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
    button.size(design::size::CONTROL)
}

impl Browser {
    /// Back and forward, the address field centred, new tab, on the
    /// window's ground with a hairline under it (the tab bar's, when it
    /// shows).
    fn toolbar(&self, cx: &mut Context<Self>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
        let can = |pick: fn(&WebState) -> bool| self.tab().is_some_and(|tab| pick(&tab.state));
        // the field is centred in the bar: the two sides grow alike from
        // one width, two buttons' (back and forward)
        let side = || {
            div()
                .flex_1()
                .min_w(design::size::CONTROL * 2.)
                .flex()
                .items_center()
        };
        div()
            .id("browser-toolbar")
            .w_full()
            .flex()
            .items_center()
            .gap(design::space::LG)
            .p(design::space::SM)
            .when(self.tabs.len() < 2, |bar| {
                bar.border_b_1().border_color(theme.border)
            })
            .child(
                side()
                    .child(glyph_button(
                        "browser-back",
                        "icons/chevron-left.svg",
                        "Back",
                        can(|state| state.can_go_back),
                        ladder,
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
                        ladder,
                        theme,
                        cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
                            view.navigate(WebHandle::forward, cx)
                        }),
                    )),
            )
            .child(self.field(cx, ladder, theme))
            .child(
                side().justify_end().child(
                    glyph_button(
                        "browser-new-tab",
                        "icons/plus.svg",
                        "New tab",
                        true,
                        ladder,
                        theme,
                        // typing replaces the home page's address at once
                        cx.listener(|view: &mut Browser, _: &ClickEvent, window, cx| {
                            view.open(HOME.to_owned(), cx);
                            view.edit(window, cx);
                        }),
                    )
                    .text_color(theme.muted),
                ),
            )
    }

    /// The address field: at rest the host, centred, with reload or stop
    /// at its right end; typing, the whole address on the window's ground,
    /// in the host's ink focus ring.
    fn field(&self, cx: &mut Context<Self>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
        let theme = *theme;
        let field = div()
            .id("browser-field")
            .flex_basis(FIELD_WIDTH)
            .min_w_0()
            .h(design::size::CONTROL)
            .relative();
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
                    .aria_label(words::ADDRESS_LABEL)
                    .consumes_keys(["escape"])
                    .on_key_down(
                        cx.listener(|view: &mut Browser, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "escape" {
                                view.rest(cx)
                            }
                        }),
                    )
                    .child(
                        Input::new(ADDRESS, &self.address, words::ADDRESS_LABEL)
                            .size_full()
                            .px(design::space::MD)
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.background)
                            .text_size(design::text::BODY)
                            .placeholder(words::TYPE_AN_ADDRESS)
                            .on_submit(
                                cx.listener(|view: &mut Browser, _: &(), _, cx| view.go(cx)),
                            ),
                    );
            }
        };
        let state = &tab.state;
        // Stop and Reload are two elements, not one that changes its press:
        // a press routed a frame late reaches the one that was pressed, or
        // nothing once it is gone, never the other
        let (reload, id, icon, size, name): (fn(&WebHandle), _, _, _, _) = match state.loading {
            true => (
                WebHandle::stop,
                "browser-stop",
                "icons/x.svg",
                px(13.),
                "Stop",
            ),
            false => (
                WebHandle::reload,
                "browser-reload",
                "icons/rotate-cw.svg",
                px(14.),
                "Reload",
            ),
        };
        let (host, locked) = host_shown(&state.url);
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
            .cursor_text()
            .role(Role::Button)
            .aria_label(words::ADDRESS_LABEL)
            .aria_value(state.url.clone())
            .focusable()
            .on_click(
                cx.listener(|view: &mut Browser, _: &ClickEvent, window, cx| view.edit(window, cx)),
            )
            .when(locked, |shown| {
                shown.child(
                    glyph("icons/lock.svg", px(11.))
                        .id("browser-lock")
                        .text_color(theme.muted),
                )
            })
            // cut from the start: the name a host is registered under is
            // its right end, and stays in view
            .child(
                div()
                    .id("browser-host")
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_start()
                    .child(host),
            );
        field
            .bg(theme.surface_raised)
            .child(shown)
            .child(
                icon_button(
                    id,
                    glyph(icon, size),
                    name,
                    ladder,
                    &theme,
                    cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.navigate(reload, cx)
                    }),
                )
                .absolute()
                .top_0()
                .right_0()
                .size(design::size::CONTROL)
                // its press is not the field's under it
                .consumes_click(),
            )
            .when(state.loading, |field| {
                field.child(
                    div()
                        .id("browser-loading")
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(2.))
                        .bg(theme.foreground),
                )
            })
    }

    /// The tabs, sharing the bar's width; each closes from a button at its
    /// left, shown on hover and on the chosen one. The chosen tab is the
    /// window's ground and runs into the page; the others sit on the bar
    /// over its hairline.
    fn tab_bar(&self, cx: &mut Context<Self>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
        let theme = *theme;
        // a hovered tab: one step lighter than the bar
        let lighter = Hsla {
            l: theme.surface.l + 0.023,
            ..theme.surface
        };
        let tabs = self.tabs.iter().enumerate().map(|(index, tab)| {
            let active = index == self.active;
            let group = format!("browser-tab-group-{}", tab.id);
            let title = div()
                .id(format!("browser-tab-{}", tab.id))
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .px(TAB_INSET)
                .text_size(design::text::SECONDARY)
                .map(|title| match active {
                    true => title
                        .text_color(theme.foreground)
                        .font_weight(FontWeight::MEDIUM),
                    false => title.text_color(theme.muted),
                })
                .hover(move |style| style.text_color(theme.foreground))
                .aria_selected(active)
                .on_click(
                    cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                        view.select(index, cx)
                    }),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        // a title is cut at its end, a host at its start
                        .when(tab.state.title.trim().is_empty(), |host| {
                            host.text_ellipsis_start()
                        })
                        .child(label(&tab.state)),
                );
            div()
                .id(format!("browser-tab-slot-{}", tab.id))
                .group(group.clone())
                .flex_1()
                .min_w_0()
                .h_full()
                .relative()
                .map(|slot| match active {
                    true => slot.bg(theme.background),
                    false => slot
                        .bg(theme.surface)
                        .hover(move |style| style.bg(lighter))
                        .border_b_1(),
                })
                .border_color(theme.border)
                .when(index > 0, |slot| slot.border_l_1())
                .child(design::item(title, Role::Tab, active))
                .child(
                    icon_button(
                        format!("browser-close-tab-{}", tab.id),
                        glyph("icons/x.svg", px(11.)),
                        "Close tab",
                        ladder,
                        &theme,
                        cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                            view.close(index, cx)
                        }),
                    )
                    .absolute()
                    // its glyph centred 15 px in, as an 18 px box 6 px from
                    // the edge would put it; the press target stays 24
                    .top(px(3.))
                    .left(px(3.))
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
            .wrap()
            // an arrow pressed on a close button moves the tab list: the
            // keys go back to the list, as that button may now be hidden
            .on_move(
                cx.processor(|view: &mut Browser, index: usize, window, cx| {
                    view.select(index, cx);
                    window.focus("browser-tabs");
                }),
            )
            .on_press(
                cx.processor(|view: &mut Browser, index: usize, _, cx| view.select(index, cx)),
            )
            .build()
            .w_full()
            .h(TAB_BAR)
            .flex()
            // the toolbar's hairline above, and the window's edges: lines
            // the focus ring inks, its own inset shadow being under the
            // tabs' fills (the line under the bar is the tabs')
            .border_t_1()
            .border_x_1()
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
            // no page: a quiet note, a little above the middle
            .when(self.tabs.is_empty(), |pages| {
                pages
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(design::space::XS)
                    .pb(design::space::XL * 2.)
                    .child(
                        div()
                            .text_size(design::text::TITLE)
                            .font_weight(FontWeight::MEDIUM)
                            .child("No page open"),
                    )
                    .child(
                        div()
                            .text_color(theme.muted)
                            .child("Type an address above and press Enter."),
                    )
            })
    }
}
