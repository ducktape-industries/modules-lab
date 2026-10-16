//! The phone's Browser (`wt/briefs/lab-phone/design/SPEC.md` §3, boards
//! 30 and 31; Expo has no browser, so this is a new screen in the Expo
//! kit's look, `design::TOUCH`). The view's root picks it once, when the
//! window's pointer is a finger; nothing under here asks again.
//!
//! A phone's build has no web view, so no page is drawn here and there
//! are no tabs: one screen says so, and hands an address to the phone's
//! own browser (`link.open`): the one typed, a link the app sent here, the
//! home page, or one handed before this session. The field takes the keys
//! only when pressed.
//!
//! Reads the address as typed, the links sent here, the addresses handed
//! out and the last one refused. Writes an address handed out
//! (`open_outside`, `open_typed`). It never reaches into the desktop's
//! tree.
use view::design::{Ladder, Nav};
use view::methods::refusal;
use view::prelude::*;
use view::wire::TextChange;
use view::{AnyElement, Div, FontWeight, Stateful, accesskit};

use crate::words;
use crate::{ADDRESS, Browser, HOME, address, host_shown};

/// The note's glyph, and a row's trailing one (SPEC §3.1).
const GLYPH: Pixels = px(18.);

/// The screen.
pub(crate) fn render(view: &Browser, cx: &mut Context<Browser>) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let ladder = &design::TOUCH;
    div()
        .id("browser")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_type(ladder.body)
        .child(
            // the view's one page: the host's capsule closes it
            design::page("browser-phone", words::BROWSER, Nav::Child, ladder, &theme)
                .caption(words::PAGES_OPEN_OUTSIDE)
                .child(
                    div()
                        .id("browser-phone-body")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(note(ladder, &theme))
                        .child(typed(view, cx, ladder, &theme))
                        .children(listing(
                            "browser-routed",
                            words::SENT_HERE,
                            &view.routed,
                            cx,
                            ladder,
                            &theme,
                        ))
                        .child(design::section_head(
                            "browser-home-head",
                            words::HOME_HEAD,
                            ladder,
                            &theme,
                        ))
                        .child(home(cx, ladder, &theme))
                        .children(listing(
                            "browser-opened",
                            words::OPENED_THIS_SESSION,
                            &view.handed,
                            cx,
                            ladder,
                            &theme,
                        )),
                ),
        )
        .into_any_element()
}

/// The honest line: a strip on the surface, the globe and the sentence.
// SDK gap: note strip, swap when feat/phone-kit lands
fn note(ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    div()
        .id("browser-no-web")
        .flex()
        .items_start()
        .gap(design::space::MD)
        .py(design::space::MD)
        .px(design::space::BLOCK)
        .bg(theme.surface)
        .border_b_1()
        .border_color(theme.border)
        .text_type(ladder.secondary)
        .text_color(theme.muted)
        .child(design::icon("globe", GLYPH))
        .child(div().flex_1().min_w_0().child(words::NO_WEB_VIEW))
}

/// The address field and Open, which hands what is typed to the phone's
/// browser; Enter in the field does too. Under them, the last address the
/// host would not open, until the address is edited.
fn typed(view: &Browser, cx: &mut Context<Browser>, ladder: &Ladder, theme: &Theme) -> Div {
    let input = Input::new(ADDRESS, &view.address, words::ADDRESS_LABEL)
        .placeholder(words::TYPE_AN_ADDRESS)
        .font_family(design::fonts::FAMILY_MONO)
        .on_change(cx.listener(|view: &mut Browser, _: &TextChange, _, cx| view.edited(cx)))
        .on_submit(cx.listener(|view: &mut Browser, _: &(), _, cx| view.open_typed(cx)));
    let opens = address(&view.address.text()).is_some();
    div()
        .p(design::space::BLOCK)
        .flex()
        .flex_col()
        .gap(design::space::SM)
        .child(design::field(input, ladder, theme).lead("globe"))
        .child(open(opens, cx, ladder, theme))
        .children(refused(view, cx, ladder, theme))
}

/// The address the host would not open, in the view's own words (the
/// host's are written for a developer), with Retry only where another ask
/// can pass: never for an address that is no web address, or a host that
/// opens none.
fn refused(
    view: &Browser,
    cx: &mut Context<Browser>,
    ladder: &Ladder,
    theme: &Theme,
) -> Option<Stateful<Div>> {
    let (url, why) = view.refused.as_ref()?;
    let (sentence, retries) = match why.code.as_str() {
        refusal::MALFORMED_REQUEST => (words::NOT_A_WEB_ADDRESS, false),
        refusal::UNKNOWN_REQUEST | refusal::UNDECLARED_CAPABILITY => (words::NO_LINKS_HERE, false),
        refusal::NEEDS_GESTURE => (words::NEEDS_A_PRESS, true),
        _ => (words::NOT_OPENED, true),
    };
    if !retries {
        return Some(lasting(sentence, ladder, theme));
    }
    let again = url.clone();
    Some(design::refused(
        "browser-open",
        sentence,
        ladder,
        theme,
        cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
            view.open_outside(again.clone(), cx)
        }),
    ))
}

/// A refusal another try cannot pass: the kit's refused notice with no
/// Retry.
// SDK gap: refused notice without an action, swap when feat/phone-kit lands
fn lasting(sentence: &'static str, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    div()
        .id("browser-open-refused")
        .py(design::space::MD)
        .px(design::space::BLOCK)
        .bg(theme.danger_soft)
        .text_type(ladder.secondary)
        .text_color(theme.danger)
        .role(Role::Alert)
        .aria_live(accesskit::Live::Assertive)
        .child(sentence)
}

/// Open: the primary button, with the glyph of a page that opens outside.
/// It cannot act on an empty field, and says so.
// SDK gap: button with a leading glyph, swap when feat/phone-kit lands
fn open(opens: bool, cx: &mut Context<Browser>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    // (ground, words) as the kit's primary button, and as it is disabled
    let (ground, tone) = match opens {
        true => (theme.primary, theme.primary_foreground),
        false => (theme.surface_raised, theme.faint),
    };
    let face = div()
        .id("browser-open")
        .w_full()
        .h(px(design::height::touch::PRIMARY as f32))
        .flex()
        .items_center()
        .justify_center()
        .gap(design::space::SM)
        .border_1()
        .border_color(ground)
        .bg(ground)
        .text_color(tone)
        .text_type(ladder.label)
        .font_weight(FontWeight::MEDIUM)
        .role(Role::Button)
        .aria_label(words::OPEN_IN_BROWSER)
        .child(design::icon("external-link", GLYPH))
        .child(words::OPEN_IN_BROWSER);
    match opens {
        true => design::pressed(face, ladder)
            .focusable()
            .on_click(cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| view.open_typed(cx))),
        false => face.aria_disabled(true),
    }
}

/// The home page's row: pressed, it opens in the phone's browser.
fn home(cx: &mut Context<Browser>, ladder: &Ladder, theme: &Theme) -> Stateful<Div> {
    outside(
        "browser-home",
        ("globe", words::HOME_NAME.to_owned(), host_shown(HOME).0),
        ladder,
        theme,
    )
    .child(design::icon("external-link", GLYPH).text_color(theme.muted))
    .on_click(cx.listener(|view: &mut Browser, _: &ClickEvent, _, cx| {
        view.open_outside(HOME.to_owned(), cx)
    }))
}

/// `urls` under the head `{id}-head` saying `head`, newest first, each a
/// row (`{id}-{index}`) that hands it to the phone's browser; nothing while
/// there are none.
fn listing(
    id: &str,
    head: &'static str,
    urls: &[String],
    cx: &mut Context<Browser>,
    ladder: &Ladder,
    theme: &Theme,
) -> Vec<AnyElement> {
    if urls.is_empty() {
        return Vec::new();
    }
    let head = design::section_head(format!("{id}-head"), head, ladder, theme);
    let rows = urls.iter().enumerate().map(|(index, url)| {
        let again = url.clone();
        outside(
            format!("{id}-{index}"),
            ("external-link", host_shown(url).0, url.clone()),
            ladder,
            theme,
        )
        .on_click(
            cx.listener(move |view: &mut Browser, _: &ClickEvent, _, cx| {
                view.open_outside(again.clone(), cx)
            }),
        )
        .into_any_element()
    });
    std::iter::once(head.into_any_element())
        .chain(rows)
        .collect()
}

/// A row that opens an address outside: its glyph, its name over its
/// caption; the caller adds the press and what trails it.
fn outside(
    id: impl Into<ElementId>,
    (glyph, name, caption): (&str, String, String),
    ladder: &Ladder,
    theme: &Theme,
) -> Stateful<Div> {
    design::pressed(design::row(id, ladder, theme), ladder)
        .min_h(ladder.row_tall)
        .role(Role::Button)
        .aria_label(name.clone())
        .focusable()
        .child(design::lead(
            design::icon(glyph, ladder.icon_row).text_color(theme.muted),
        ))
        .child(
            design::row_words(design::row_title(name, ladder, theme))
                .child(design::row_caption(caption, ladder, theme)),
        )
}
