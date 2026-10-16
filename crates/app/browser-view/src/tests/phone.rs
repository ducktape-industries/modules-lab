//! The phone's screen (`ui/phone/`), as the phone boards draw it: the
//! field and Open, the links the app sent here, the home row, the
//! addresses handed out this session, a refused one, under a finger at 390
//! wide. A phone's build has no web view: an address goes to the phone's
//! own browser (`link.open`), which only a phone shows opening. The sweep
//! holds every state at 360, 390 and 430.
use view::host::Error;
use view::methods::LinkOpen;

use super::*;
use crate::{HOME, words};

/// The app's refusal of a link it does not open (its kernel's words).
const NOT_OPENED: &str = "`link.open` opens a duck:// or https:// link";

/// The view open under a finger, 390 by 844, on a host that opens a link
/// as the app does: a `duck://` or `https://` one, else refused.
fn phone() -> Opened {
    let mut opened = opened();
    opened.cx.host().handle::<LinkOpen>(|link| {
        let openable = ["duck://", "https://"].iter().any(|scheme| {
            link.strip_prefix(scheme)
                .is_some_and(|rest| !rest.is_empty())
        });
        match openable {
            true => Ok(()),
            false => Err(Error::new("malformed_request", NOT_OPENED)),
        }
    });
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
    shows_all(&cx, &[words::HOME_HEAD, words::HOME_NAME, "wikipedia.org"]);
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

/// The host hands on `https://` alone; the phone's browser upgrades an
/// `http://` address anyway, so the view sends it as `https://`.
#[test]
fn an_http_address_goes_to_the_phones_browser_as_https() {
    let Opened { mut cx, view, .. } = phone();
    open_typed(&mut cx, "http://example.com/a?q=1");
    assert_eq!(handed(&cx), ["https://example.com/a?q=1"]);
    assert!(cx.find("browser-open-refused").is_none());
    assert_eq!(
        view.read(|browser| browser.handed.clone()),
        ["https://example.com/a?q=1"]
    );
}

/// No host sentence on screen: the refusal is the view's own, it offers
/// no Retry that cannot pass, and editing the address ends it.
#[test]
fn an_address_that_is_no_web_address_says_so_offers_no_retry_and_goes_on_an_edit() {
    let Opened { mut cx, view, .. } = phone();
    open_typed(&mut cx, "ftp://example.com");
    assert_eq!(handed(&cx), ["ftp://example.com"]);
    assert!(cx.find("browser-open-refused").is_some());
    shows_all(&cx, &[words::NOT_A_WEB_ADDRESS]);
    assert!(!cx.has_text(NOT_OPENED), "the host's words stay off screen");
    assert!(
        cx.find("browser-open-retry").is_none(),
        "another try cannot pass"
    );
    assert!(view.read(|browser| browser.handed.is_empty()));
    cx.simulate_input("browser-address", "example.com");
    assert!(cx.find("browser-open-refused").is_none());
}

/// A refusal another ask can pass offers Retry, which asks again; an
/// address that opens ends it.
#[test]
fn a_refusal_another_try_can_pass_offers_retry() {
    let Opened { mut cx, view, .. } = phone();
    cx.host().handle::<LinkOpen>(|_| {
        Err(Error::new(
            "needs_gesture",
            "`link.open` needs a press or key",
        ))
    });
    open_typed(&mut cx, "example.com");
    shows_all(&cx, &[words::NEEDS_A_PRESS]);
    cx.simulate_click("browser-open-retry");
    assert_eq!(handed(&cx), ["https://example.com", "https://example.com"]);
    cx.host().handle::<LinkOpen>(|_| Ok(()));
    cx.simulate_click("browser-open-retry");
    assert!(cx.find("browser-open-refused").is_none());
    assert_eq!(
        view.read(|browser| browser.handed.clone()),
        ["https://example.com"]
    );
}

/// The phone's browser opens a link only from a press, so a link the app
/// sends here waits on the screen for one, and no tab hides it.
#[test]
fn a_link_the_app_sends_waits_for_a_press_and_opens_no_tab() {
    let Opened {
        mut cx,
        view,
        routes,
    } = phone();
    let titles = cx.host().requests::<HostTitle>().len();
    routes.send("https://example.org/wiki?q=1".into());
    cx.run_until_parked();
    assert!(view.read(|browser| browser.tabs.is_empty()));
    assert_eq!(cx.host().requests::<HostTitle>().len(), titles);
    assert!(handed(&cx).is_empty(), "nothing opens without a press");
    shows_all(&cx, &[words::SENT_HERE, "example.org"]);
    cx.simulate_click("browser-routed-0");
    assert_eq!(handed(&cx), ["https://example.org/wiki?q=1"]);
    assert!(
        cx.find("browser-routed-0").is_none(),
        "opened, it waits no more"
    );
    assert!(!cx.has_text(words::SENT_HERE));
    assert!(cx.find("browser-opened-0").is_some());
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
    let Opened { mut cx, routes, .. } = phone();
    view::testing::sweep(&mut cx, |cx| {
        // board 30: the field empty, then typed; board 31: opened; then a
        // refused address, and a link sent here
        cx.simulate_input("browser-address", "wikipedia.org/wiki/Duck");
        cx.simulate_click("browser-open");
        cx.simulate_click("browser-home");
        cx.simulate_input("browser-address", "ftp://example.com");
        cx.simulate_click("browser-open");
        routes.send("https://example.org/".into());
        cx.run_until_parked();
    });
}

#[track_caller]
fn shows_all(cx: &TestAppContext, texts: &[&str]) {
    for text in texts {
        assert!(cx.has_text(text), "no {text:?} in {:?}", cx.texts());
    }
}
