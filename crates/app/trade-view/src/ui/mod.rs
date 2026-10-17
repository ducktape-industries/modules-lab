//! The desktop's tree: what Trade draws under a pointer. From [`WIDE`] px
//! of the view's own width it is three columns, the chart's beside the
//! book's and, over the tape, the ticket's; under that the desk's pane,
//! the chart over the panel and one tabbed column beside them. Each region
//! draws itself: `chart.rs`, `book.rs` (the book and the tape),
//! `ticket.rs`, `orders.rs` (the panel). This file draws the frame, the
//! market's bar and its list of markets, the chart's tools and the pane's
//! column, and the menus that hang under their controls and take the keys
//! while they are open.
use std::rc::Rc;

use gpui::relative;
use trade::{MARKETS, SPANS};
use view::design::{Ladder, Orientation};
use view::prelude::*;
use view::{Anchor, AnchoredPositionMode, AnyElement, Div, Edges, FontWeight, Point, Stateful};

use crate::chart::SWITCHES;
use crate::{Column, Menu, Trade, format, look, words};

mod book;
mod chart;
mod orders;
/// The phone's tree, which the view's root picks under a finger. It reads
/// nothing of this one.
pub(crate) mod phone;
mod ticket;

/// The width from which the view is three columns.
const WIDE: f32 = 1100.;
/// The book's column and the ticket's, in the wide layout.
const BOOK_W: Pixels = px(240.);
const TICKET_W: Pixels = px(272.);
/// The pane layout's one column.
const COLUMN_W: Pixels = px(256.);

impl Menu {
    /// The control the menu hangs from, which takes the keys back.
    fn opener(self) -> &'static str {
        match self {
            Menu::Markets => "trade-market",
            Menu::Indicators => "trade-indicators",
            Menu::Step => "trade-step",
        }
    }

    /// The height of that control: how far under its top the menu hangs.
    fn drop(self) -> Pixels {
        match self {
            Menu::Markets => design::size::CONTROL,
            Menu::Indicators => design::size::ROW,
            Menu::Step => book::STEP_H,
        }
    }

    /// What takes the keys when the menu opens.
    fn entry(self) -> &'static str {
        match self {
            Menu::Markets => "trade-markets-search",
            Menu::Indicators => "trade-indicators-menu",
            Menu::Step => "trade-step-menu",
        }
    }
}

/// The desktop's frame: the market's bar over the columns.
pub(crate) fn render(view: &mut Trade, cx: &mut Context<Trade>) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let ladder = &design::POINTER;
    let wide = view.viewport.0 >= WIDE;
    let column = || {
        div()
            .flex_none()
            .flex()
            .flex_col()
            .min_h_0()
            .border_l_1()
            .border_color(theme.border)
    };
    let charted = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(view.tools(wide, cx, ladder, &theme))
        .child(view.chart(cx, &theme))
        .child(view.panel(wide, cx, ladder, &theme));
    let columns = div().flex_1().min_h_0().flex().child(charted);
    let bar = view.bar(wide, cx, &theme);
    let columns = match wide {
        true => columns
            .child(column().w(BOOK_W).child(view.book(
                view.viewport.1 - f32::from(BAR_H),
                cx,
                &theme,
            )))
            .child(
                column()
                    .w(TICKET_W)
                    .child(view.ticket(cx, ladder, &theme))
                    .child(view.tape(true, &theme)),
            ),
        false => columns.child(column().w(COLUMN_W).child(view.column(cx, ladder, &theme))),
    };
    div()
        .id("trade")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_size(design::text::BODY)
        .line_height(relative(1.3))
        .child(bar)
        .child(columns)
        .into_any_element()
}

/// The market bar's height, wide and in the pane.
const BAR_H: Pixels = px(48.);
const BAR_PANE_H: Pixels = px(40.);
/// The tools row's height, wide and in the pane.
const TOOLS_H: Pixels = px(40.);
const TOOLS_PANE_H: Pixels = px(34.);
/// A tab bar's height: the panel's and the pane column's.
const TABS_H: Pixels = px(32.);

impl Trade {
    fn open_menu(&mut self, menu: Menu, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = Some(menu);
        self.menu_cursor = 0;
        self.search.reset("");
        window.focus(menu.entry());
        cx.notify();
    }

    /// Closes the open menu and gives the keys back to its control.
    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.menu.take() {
            window.focus(menu.opener());
            cx.notify();
        }
    }

    /// A market picked from the list: the list closes, and the market is
    /// the one on screen.
    fn take_market(&mut self, market: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menu(window, cx);
        self.pick_market(market, cx);
    }

    /// The markets the search holds: every one while it holds nothing.
    fn found_markets(&self) -> Vec<usize> {
        let wanted = self.search.text().trim().to_lowercase();
        (0..MARKETS.len())
            .filter(|market| MARKETS[*market].name.to_lowercase().contains(&wanted))
            .collect()
    }

    /// Item `index` of the open menu, pressed.
    fn pick(&mut self, menu: Menu, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        match menu {
            Menu::Markets => {
                if let Some(market) = self.found_markets().get(index) {
                    self.take_market(*market, window, cx);
                }
            }
            // a set of switches: the menu stays open over them
            Menu::Indicators => {
                self.chart.switch(index);
                cx.notify();
            }
            Menu::Step => {
                self.close_menu(window, cx);
                self.pick_step(index, cx);
            }
        }
    }
}

impl Trade {
    /// The market's bar: its picker, the practice tag, the last price and
    /// how far it is from the price a day before, and the day's high, low
    /// and volume. The pane has no room for the volume.
    fn bar(&self, wide: bool, cx: &mut Context<Self>, theme: &Theme) -> impl IntoElement + use<> {
        let stats = self.market_stats().filter(|stats| stats.last > 0);
        let moved = stats.and_then(|stats| format::change(stats.last, stats.before));
        let tone = look::moved(moved, theme.foreground, theme);
        let last = design::mono(stats.map_or("—".into(), |stats| self.price(stats.last)))
            .text_size(if wide { px(18.) } else { design::text::TITLE })
            .font_weight(FontWeight::MEDIUM)
            .text_color(tone);
        let moved = design::mono(moved.map_or("—".into(), format::percent)).text_color(tone);
        let fact = |label: &'static str, value: Option<String>| {
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .text_size(design::text::CAPTION)
                .text_color(theme.muted)
                .child(label)
                .child(
                    design::mono(value.unwrap_or("—".into()))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground),
                )
        };
        let practice = div()
            .px(px(5.))
            .py(px(3.))
            .border_1()
            .border_color(theme.border)
            .font_family(design::fonts::FAMILY_MONO)
            .text_size(px(10.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.muted)
            .child("PRACTICE");
        let bar = div()
            .flex_none()
            .h(if wide { BAR_H } else { BAR_PANE_H })
            .px(design::space::LG)
            .flex()
            .items_center()
            .gap(if wide { px(20.) } else { design::space::BLOCK })
            .border_b_1()
            .border_color(theme.border)
            .child(self.market_picker(cx, theme))
            .child(practice)
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(design::space::SM)
                    .child(last)
                    .child(moved),
            );
        // a refusal takes the facts' place: the prices are not known
        if let Some(refusal) = self.stats.failed() {
            return bar.child(design::quiet(refusal.message.clone(), theme));
        }
        // a day nobody traded in has no high and no low to tell
        let traded = stats.filter(|stats| stats.high > 0);
        bar.when(!wide, |bar| bar.child(div().flex_1()))
            .child(fact(
                words::HIGH_24H,
                traded.map(|stats| self.price(stats.high)),
            ))
            .child(fact(
                words::LOW_24H,
                traded.map(|stats| self.price(stats.low)),
            ))
            .when(wide, |bar| {
                bar.child(fact(words::VOLUME_24H, self.day_volume()))
            })
    }

    /// The market's name, which opens the list of markets under it: a
    /// search, and each market with its last price and its day.
    fn market_picker(&self, cx: &mut Context<Self>, theme: &Theme) -> impl IntoElement + use<> {
        let open = self.menu == Some(Menu::Markets);
        let opener = chip("trade-market", theme)
            .h(design::size::CONTROL)
            .pl(design::space::MD)
            .gap(design::space::SM)
            .aria_expanded(open)
            .text_size(design::text::SECTION)
            .font_weight(FontWeight::MEDIUM)
            .aria_label(format!("Market: {}", self.market().name))
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.open_menu(Menu::Markets, window, cx)
            }))
            .child(self.market().name)
            .child(chevron(open, theme));
        let list = open.then(|| self.markets(cx, theme));
        self.popover(Menu::Markets, words::MARKETS, opener, list, false, cx)
    }

    /// The open list of markets.
    fn markets(&self, cx: &mut Context<Self>, theme: &Theme) -> AnyElement {
        let theme = *theme;
        let found = self.found_markets();
        let active = self.menu_cursor.min(found.len().saturating_sub(1));
        let cell = |width: Pixels| div().w(width).flex_none().flex().justify_end();
        let rows = found.iter().enumerate().map(|(index, market)| {
            let (market, name) = (*market, MARKETS[*market].name);
            let stats = self.stats.ready().and_then(|stats| stats.get(market));
            let stats = stats.filter(|stats| stats.last > 0);
            let moved = stats.and_then(|stats| format::change(stats.last, stats.before));
            let decimals = format::decimals(MARKETS[market].tick);
            let row = div()
                .id(format!("market-{name}"))
                .h(design::size::CONTROL)
                .px(design::space::MD)
                .flex()
                .items_center()
                .text_size(design::text::SECONDARY)
                .when(market == self.market, |row| row.bg(theme.surface_raised))
                .hover(move |style| style.bg(theme.surface))
                .aria_selected(market == self.market)
                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                    view.take_market(market, window, cx)
                }))
                .child(div().flex_1().font_weight(FontWeight::MEDIUM).child(name))
                .child(cell(px(88.)).child(design::mono(
                    stats.map_or("—".into(), |stats| format::fixed(stats.last, decimals)),
                )))
                .child(
                    cell(px(64.)).child(
                        design::mono(moved.map_or("—".into(), format::percent))
                            .text_color(look::moved(moved, theme.muted, &theme)),
                    ),
                );
            design::item(row, Role::ListBoxOption, index == active)
        });
        let head = div()
            .h(px(22.))
            .px(design::space::MD)
            .flex()
            .items_center()
            .text_size(design::text::CAPTION)
            .text_color(theme.muted)
            .child(div().flex_1().child("Market"))
            .child(cell(px(88.)).child("Price"))
            .child(cell(px(64.)).child("24h"));
        let search = Input::new("trade-markets-search", &self.search, "Search markets")
            .h(design::size::CONTROL)
            .w_full()
            .px_2()
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(design::text::SECONDARY)
            .placeholder("Search markets")
            // Enter takes the first market found
            .on_submit(
                cx.listener(|view, _: &(), window, cx| view.pick(Menu::Markets, 0, window, cx)),
            );
        let list = design::composite("trade-markets-list", Role::ListBox, words::MARKETS)
            .active(active, found.len())
            .on_move(cx.processor(|view: &mut Trade, index: usize, _, cx| {
                view.menu_cursor = index;
                cx.notify();
            }))
            .on_press(cx.processor(|view: &mut Trade, index: usize, window, cx| {
                view.pick(Menu::Markets, index, window, cx)
            }))
            .build()
            .flex()
            .flex_col()
            .children(rows)
            .when(found.is_empty(), |list| {
                list.child(
                    design::quiet("No market by that name", &theme)
                        .px(design::space::MD)
                        .py(design::space::XS),
                )
            });
        let frame = frame("trade-markets-frame", &theme, cx)
            .w(px(320.))
            .py(design::space::SM)
            .child(
                div()
                    .px(design::space::SM)
                    .pb(design::space::SM)
                    .child(search),
            )
            .child(head)
            .child(list);
        hung(frame, false, Menu::Markets.drop())
    }

    /// The tools over the chart: the candle span, and what the chart
    /// draws. The pane folds the second into one menu.
    fn tools(
        &self,
        wide: bool,
        cx: &mut Context<Self>,
        ladder: &Ladder,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        let spans = design::segmented(
            "trade-spans",
            words::CANDLE_SPAN,
            ladder,
            theme,
            Some(self.chart.span),
            SPANS
                .iter()
                .map(|span| (span.name.into(), span.name.into())),
            cx.processor(|view: &mut Trade, span: usize, _, cx| view.pick_span(span, cx)),
        );
        let tools = div()
            .flex_none()
            .h(if wide { TOOLS_H } else { TOOLS_PANE_H })
            .px(design::space::LG)
            .flex()
            .items_center()
            .gap(design::space::SM)
            .border_b_1()
            .border_color(theme.border)
            .child(spans);
        if !wide {
            return tools
                .child(div().flex_1())
                .child(self.indicators(cx, theme));
        }
        let toggle = |id: &'static str, index: usize, mark: Option<Hsla>, on: bool| {
            chip(id, theme)
                .h(design::size::CONTROL)
                .when(!on, |chip| chip.text_color(theme.muted))
                .aria_toggled(on.into())
                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                    view.pick(Menu::Indicators, index, window, cx)
                }))
                .children(mark.map(|mark| div().w(px(10.)).h(px(2.)).bg(mark)))
                .child(SWITCHES[index])
        };
        let chart = &self.chart;
        let style = design::segmented(
            "trade-style",
            "Chart style",
            ladder,
            theme,
            Some(usize::from(chart.line)),
            [
                ("Candles".into(), "Candles".into()),
                (words::LINE.into(), words::LINE.into()),
            ],
            cx.processor(|view: &mut Trade, line: usize, _, cx| {
                view.chart.line = line == 1;
                cx.notify();
            }),
        );
        tools
            .child(toggle("trade-ma7", 0, Some(theme.muted), chart.ma7))
            .child(toggle("trade-ma25", 1, Some(theme.warning), chart.ma25))
            .child(toggle("trade-volume", 2, None, chart.volume))
            .child(div().flex_1())
            .child(style)
    }

    /// The pane's menu of what the chart draws.
    fn indicators(&self, cx: &mut Context<Self>, theme: &Theme) -> impl IntoElement + use<> {
        let open = self.menu == Some(Menu::Indicators);
        let opener = chip("trade-indicators", theme)
            .h(design::size::ROW)
            .aria_expanded(open)
            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                view.open_menu(Menu::Indicators, window, cx)
            }))
            .child("Indicators")
            .child(chevron(open, theme));
        let menu = open.then(|| {
            let items = SWITCHES.iter().zip(self.chart.switches());
            let items = items.map(|(name, on)| ((*name).into(), on)).collect();
            self.menu_list(
                Menu::Indicators,
                "Indicators",
                Role::MenuItemCheckBox,
                items,
                cx,
                theme,
            )
        });
        self.popover(Menu::Indicators, "Indicators", opener, menu, true, cx)
    }

    /// The pane layout's column: the ticket, the book or the tape, by tab.
    fn column(
        &self,
        cx: &mut Context<Self>,
        ladder: &Ladder,
        theme: &Theme,
    ) -> impl IntoElement + use<> {
        const TABS: [(Column, &str, &str); 3] = [
            (Column::Order, "trade-tab-order", "Order"),
            (Column::Book, "trade-tab-book", words::BOOK),
            (Column::Trades, "trade-tab-trades", words::TRADES),
        ];
        let picked = TABS.iter().position(|(tab, ..)| *tab == self.column);
        let bar = tabs(
            "trade-column-tabs",
            "Order, book or trades",
            ladder,
            theme,
            picked.unwrap_or_default(),
            TABS.map(|(_, id, name)| (id.into(), name.into(), None)),
            cx.processor(|view: &mut Trade, tab: usize, _, cx| {
                view.column = TABS[tab].0;
                cx.notify();
            }),
        )
        .h_full();
        let bar = div()
            .flex_none()
            .h(TABS_H)
            .px(design::space::LG)
            .border_b_1()
            .border_color(theme.border)
            .child(bar);
        let body = match self.column {
            Column::Order => self.ticket(cx, ladder, theme).into_any_element(),
            Column::Book => {
                let height = self.viewport.1 - f32::from(BAR_PANE_H + TABS_H);
                self.book(height, cx, theme).into_any_element()
            }
            Column::Trades => self.tape(false, theme).into_any_element(),
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(bar)
            .child(body)
    }

    /// `opener` with `panel` hanging under it while `menu` is open, from
    /// its right edge when `right`: a dialog, so Tab stays in it, and
    /// Escape or a press outside closes it.
    fn popover(
        &self,
        menu: Menu,
        label: &'static str,
        opener: impl IntoElement,
        panel: Option<AnyElement>,
        right: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let close = cx.listener(|view, _: &(), window, cx| view.close_menu(window, cx));
        let overlay = modal_overlay(format!("{}-layer", menu.opener()), label, opener, panel)
            .flex_col()
            .on_dismiss(close);
        div().relative().flex_none().child(match right {
            true => overlay.items_end(),
            false => overlay.items_start(),
        })
    }

    /// The open menu of `items`, each a name and whether it is on: one Tab
    /// stop whose ↑ ↓ walk them and whose Enter or Space presses one.
    fn menu_list(
        &self,
        menu: Menu,
        label: &'static str,
        role: Role,
        items: Vec<(SharedString, bool)>,
        cx: &mut Context<Self>,
        theme: &Theme,
    ) -> AnyElement {
        let theme = *theme;
        let count = items.len();
        let active = self.menu_cursor.min(count.saturating_sub(1));
        let lines = items.into_iter().enumerate().map(|(index, (name, on))| {
            let line = div()
                .id(format!("{}-{index}", menu.entry()))
                .h(design::size::ROW)
                .px(design::space::LG)
                .flex()
                .items_center()
                .gap(design::space::SM)
                .text_size(design::text::SECONDARY)
                .whitespace_nowrap()
                .hover(move |style| style.bg(theme.surface))
                .aria_toggled(on.into())
                .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                    view.pick(menu, index, window, cx)
                }))
                .child(
                    div()
                        .w(design::space::LG)
                        .text_color(theme.muted)
                        .child(if on { "✓" } else { "" }),
                )
                .child(name);
            design::item(line, role, index == active)
        });
        let list = design::composite(menu.entry(), Role::Menu, label)
            .active(active, count)
            .on_move(cx.processor(|view: &mut Trade, index: usize, _, cx| {
                view.menu_cursor = index;
                cx.notify();
            }))
            .on_press(
                cx.processor(move |view: &mut Trade, index: usize, window, cx| {
                    view.pick(menu, index, window, cx)
                }),
            )
            .build()
            .flex()
            .flex_col()
            .children(lines);
        let frame = frame(format!("{}-frame", menu.entry()), &theme, cx)
            .min_w(px(132.))
            .py(design::space::XXS)
            .child(list);
        hung(frame, true, menu.drop())
    }
}

/// A small bordered control of a toolbar: a switch, or what opens a menu.
fn chip(id: &'static str, theme: &Theme) -> Stateful<Div> {
    let theme = *theme;
    div()
        .id(id)
        .px(design::space::SM)
        .flex()
        .flex_none()
        .items_center()
        .gap(design::space::XS)
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.background)
        .text_size(design::text::SECONDARY)
        .whitespace_nowrap()
        .hover(move |style| style.bg(theme.surface))
        .role(Role::Button)
        .focusable()
}

/// The mark beside what opens a menu.
fn chevron(open: bool, theme: &Theme) -> Div {
    div()
        .text_color(theme.muted)
        .child(if open { "⌃" } else { "⌄" })
}

/// The box a menu or a list hangs in. A press outside it closes it.
fn frame(id: impl Into<ElementId>, theme: &Theme, cx: &mut Context<Trade>) -> Stateful<Div> {
    let close = cx.listener(|view, _: &MouseDownEvent, window, cx| view.close_menu(window, cx));
    div()
        .id(id)
        .flex()
        .flex_col()
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.background)
        .shadow_lg()
        .occlude()
        .on_mouse_down_out(close)
}

/// `frame` hung under its control, `drop` under the control's top and from
/// its right edge when `right`, kept inside the window.
fn hung(frame: impl IntoElement, right: bool, drop: Pixels) -> AnyElement {
    let anchor = if right {
        Anchor::TopRight
    } else {
        Anchor::TopLeft
    };
    deferred(
        anchored()
            .anchor(anchor)
            .position_mode(AnchoredPositionMode::Local)
            .position(Point {
                x: px(0.),
                y: px(0.),
            })
            .offset(Point {
                x: px(0.),
                y: drop + design::space::XXS,
            })
            .snap_to_window_with_margin(Edges::all(design::space::SM))
            .child(frame),
    )
    .into_any_element()
}

/// A row of tabs, each its id, its name and a count beside it: one Tab
/// stop whose ← → open the next, wrapping at the ends.
fn tabs(
    id: &'static str,
    label: &'static str,
    ladder: &Ladder,
    theme: &Theme,
    picked: usize,
    names: impl IntoIterator<Item = (ElementId, SharedString, Option<usize>)>,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let on_pick = Rc::new(on_pick);
    let names: Vec<_> = names.into_iter().collect();
    let count = names.len();
    let moved = on_pick.clone();
    let tabs = names
        .into_iter()
        .enumerate()
        .map(|(index, (id, name, count))| {
            let pick = on_pick.clone();
            let click =
                move |_: &ClickEvent, window: &mut Window, app: &mut App| pick(index, window, app);
            let tab = design::tab(id, name, index == picked, ladder, theme, click)
                .h_full()
                .px_0()
                .gap(px(5.))
                .whitespace_nowrap()
                .children(count.map(|count| {
                    design::mono(count.to_string())
                        .text_size(design::text::CAPTION)
                        .text_color(theme.muted)
                }));
            design::item(tab, Role::Tab, index == picked)
        });
    design::composite(id, Role::TabList, label)
        .orientation(Orientation::Horizontal)
        .wrap()
        .active(picked, count)
        .on_move(move |index, window, app| moved(index, window, app))
        .build()
        .flex()
        .flex_none()
        .items_center()
        .gap(design::space::BLOCK)
        .children(tabs)
}

/// One calm line in place of rows there are none of, or of rows the
/// program refused to give.
fn nothing(text: impl Into<SharedString>, theme: &Theme) -> Div {
    design::quiet(text, theme)
        .px(design::space::LG)
        .py(design::space::SM)
}
