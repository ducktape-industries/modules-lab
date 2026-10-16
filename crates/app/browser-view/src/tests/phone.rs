//! The phone's screen (`ui/phone/`), as the phone boards draw it: the
//! field and Open, the home row, the addresses handed out this session,
//! under a finger at 390 wide. A phone's build has no web view: an
//! address goes to the phone's own browser (`link.open`), which only a
//! phone shows opening. The sweep holds every state at 360, 390 and 430.
use view::methods::LinkOpen;

use super::*;
use crate::{HOME, words};

/// The view open under a finger, 390 by 844.
fn phone() -> Opened {
    let mut opened = opened();
    opened.cx.simulate_touch(true);
    opened.cx.simulate_resize(390., 844.);
    opened
}

/// What the view handed to the phone's browser, in order.
fn handed(cx: &TestAppContext) -> Vec<String> {
    cx.host().requests::<LinkOpen>()
}

/// `typed` in the field, then Open pressed.
fn open_typed(cx: &mut TestAppContext, typed: &str) {
    cx.simulate_input("browser-address", typed);
    cx.simulate_click("browser-open");
}

#[test]
fn an_empty_field_disables_open() {
    let Opened { mut cx, .. } = phone();
    shows_all(
        &cx,
        &[
            words::BROWSER,
            words::PAGES_OPEN_OUTSIDE,
            words::NO_WEB_VIEW,
            words::OPEN_IN_BROWSER,
        ],
    );
    assert!(disabled(&cx, "browser-open"));
    assert!(cx.interactivity("browser-open").on_click.is_none());
    cx.simulate_input("browser-address", "   ");
    assert!(disabled(&cx, "browser-open"), "a blank field opens nothing");
}

#[test]
fn a_bare_address_opens_over_https_in_the_phones_browser_and_stays_typed() {
    let Opened { mut cx, .. } = phone();
    open_typed(&mut cx, "wikipedia.org/wiki/Duck");
    assert_eq!(handed(&cx), ["https://wikipedia.org/wiki/Duck"]);
    // the field keeps it, so a typo is mended rather than typed again
    match cx.node("browser-address") {
        Node::Field { .. } => {}
        other => panic!("the address is a field: {other:?}"),
    }
    shows_all(&cx, &[words::OPENED_THIS_SESSION, "wikipedia.org"]);
    // Enter in the field opens it too
    cx.simulate_submit("browser-address");
    assert_eq!(handed(&cx).len(), 2);
}

#[test]
fn home_opens_the_desktops_home_page() {
    let Opened { mut cx, .. } = phone();
    shows_all(&cx, &[words::HOME, words::HOME_NAME, "wikipedia.org"]);
    cx.simulate_click("browser-home");
    assert_eq!(handed(&cx), [HOME]);
}

#[test]
fn an_address_opened_again_is_listed_once_newest_first() {
    let Opened { mut cx, view, .. } = phone();
    open_typed(&mut cx, "ducktape.industries");
    cx.simulate_click("browser-home");
    cx.simulate_click("browser-opened-1");
    assert_eq!(
        view.read(|browser| browser.handed.clone()),
        ["https://ducktape.industries", HOME]
    );
    assert!(cx.find("browser-opened-2").is_none());
}

#[test]
fn the_phone_places_no_page_and_gives_the_keys_to_nothing() {
    let mut opened = phone();
    open_typed(&mut opened.cx, "example.com");
    assert!(opened.view.read(|browser| browser.tabs.is_empty()));
    // a view opened under a finger hears it before its first frame: its
    // field takes no keys, so no keyboard rises over the screen
    let focus = |cx: &TestAppContext| {
        cx.host()
            .requests::<HostWidget>()
            .iter()
            .filter(|asked| matches!(asked, WidgetCommand::Focus { .. }))
            .count()
    };
    let before = focus(&opened.cx);
    let bytes = opened.cx.snapshot().expect("a snapshot");
    opened.cx.restore::<Browser>(&bytes).expect("restored");
    opened.cx.run_until_parked();
    assert!(opened.cx.find("browser-no-web").is_some());
    assert_eq!(focus(&opened.cx), before);
}

/// The sweep: every board's state, every frame audited and fitted at each
/// phone width.
#[test]
fn sweep() {
    let Opened { mut cx, .. } = phone();
    view::testing::sweep(&mut cx, |cx| {
        // board 30: the field empty, then typed; board 31: opened
        cx.simulate_input("browser-address", "wikipedia.org/wiki/Duck");
        cx.simulate_click("browser-open");
        cx.simulate_click("browser-home");
    });
}

#[track_caller]
fn shows_all(cx: &TestAppContext, texts: &[&str]) {
    for text in texts {
        assert!(cx.has_text(text), "no {text:?} in {:?}", cx.texts());
    }
}
