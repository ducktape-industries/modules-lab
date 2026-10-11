//! The view on a fake host: each test drives the frame the way a person
//! would and reads what the host would get. Every frame it sends is held
//! to the host's sanitizer and accessibility audit.
use ducktape_view_guest::Entity;
use ducktape_view_guest::methods::{HostRoute, HostTitle};
use ducktape_view_guest::testing::{StreamSender, TestAppContext};
use ducktape_view_guest::wire::{Node, WebCommand, WebState};

use crate::{Browser, address};

struct Opened {
    cx: TestAppContext,
    view: Entity<Browser>,
    routes: StreamSender<HostRoute>,
}

fn opened() -> Opened {
    let mut cx = TestAppContext::new();
    let routes = cx.host().stream::<HostRoute>();
    let view = cx.open::<Browser>();
    cx.run_until_parked();
    Opened { cx, view, routes }
}

/// Types `typed` into the address bar and presses Enter.
fn enter(cx: &mut TestAppContext, typed: &str) {
    cx.simulate_input("browser-address", typed);
    cx.simulate_submit("browser-address");
}

/// The piece `key`'s first address and the commands its last frame carried.
fn piece(cx: &TestAppContext, key: &str) -> (String, Vec<WebCommand>) {
    match cx.node(key) {
        Node::Web { url, commands, .. } => (url.clone(), commands.clone()),
        other => panic!("{key} is no web piece: {other:?}"),
    }
}

fn hidden(cx: &TestAppContext, key: &str) -> bool {
    cx.style(key).visibility == Some(gpui::Visibility::Hidden)
}

fn disabled(cx: &TestAppContext, key: &str) -> bool {
    cx.interactivity(key).aria.disabled == Some(true)
}

impl Opened {
    fn tabs(&self) -> Vec<u64> {
        self.view
            .read(|view| view.tabs.iter().map(|tab| tab.id).collect())
    }
}

#[test]
fn an_address_without_a_scheme_opens_over_https() {
    assert_eq!(
        address("example.com").as_deref(),
        Some("https://example.com")
    );
    assert_eq!(address(" rust ").as_deref(), Some("https://rust"));
    assert_eq!(
        address("http://localhost:8000/a").as_deref(),
        Some("http://localhost:8000/a")
    );
    // a `://` past the start is no scheme
    assert_eq!(
        address("example.com/?next=https://example.org").as_deref(),
        Some("https://example.com/?next=https://example.org")
    );
    assert_eq!(address("   "), None);
    // through the bar: with no tab open, Enter opens the first on it
    let Opened { mut cx, .. } = opened();
    assert!(cx.find("page-1").is_none());
    enter(&mut cx, "example.com/a");
    assert_eq!(piece(&cx, "page-1").0, "https://example.com/a");
    assert_eq!(cx.host().requests::<HostTitle>(), ["https://example.com/a"]);
}

#[test]
fn enter_on_an_open_page_sends_it_one_load() {
    let Opened { mut cx, view, .. } = opened();
    enter(&mut cx, "https://example.com/");
    enter(&mut cx, "wikipedia.org");
    let (first, commands) = piece(&cx, "page-1");
    assert_eq!(first, "https://example.com/");
    assert_eq!(commands, [WebCommand::Load("https://wikipedia.org".into())]);
    // the next frame carries it no more: the page loads it once
    cx.simulate_web_state(
        "page-1",
        WebState {
            url: "https://www.wikipedia.org/".into(),
            title: "Wikipedia".into(),
            loading: true,
            ..WebState::default()
        },
    );
    assert!(piece(&cx, "page-1").1.is_empty());
    // the bar says where the page went
    assert_eq!(
        view.read(|view| view.address.text()),
        "https://www.wikipedia.org/"
    );
    assert!(cx.has_text("Loading…"));
    assert!(cx.has_text("Stop"));
    cx.simulate_click("browser-reload");
    assert_eq!(piece(&cx, "page-1").1, [WebCommand::Stop]);
}

#[test]
fn back_and_forward_follow_what_the_page_says() {
    let Opened { mut cx, view, .. } = opened();
    // nothing open: nothing to go back to or reload
    assert!(disabled(&cx, "browser-back"));
    assert!(disabled(&cx, "browser-reload"));
    enter(&mut cx, "example.com");
    assert!(disabled(&cx, "browser-back"));
    assert!(disabled(&cx, "browser-forward"));
    assert!(!disabled(&cx, "browser-reload"));
    cx.simulate_web_state(
        "page-1",
        WebState {
            url: "https://example.com/next".into(),
            title: "Next".into(),
            can_go_back: true,
            ..WebState::default()
        },
    );
    assert!(!disabled(&cx, "browser-back"));
    assert!(disabled(&cx, "browser-forward"));
    // a link followed in the page moves the address bar and the tab's name
    assert!(cx.has_text("Next"));
    assert_eq!(
        view.read(|view| view.address.text()),
        "https://example.com/next"
    );
    cx.simulate_click("browser-back");
    assert_eq!(piece(&cx, "page-1").1, [WebCommand::Back]);
    cx.simulate_web_state(
        "page-1",
        WebState {
            url: "https://example.com/".into(),
            title: "Example".into(),
            can_go_forward: true,
            ..WebState::default()
        },
    );
    assert!(disabled(&cx, "browser-back"));
    assert!(!disabled(&cx, "browser-forward"));
    cx.simulate_click("browser-forward");
    assert_eq!(piece(&cx, "page-1").1, [WebCommand::Forward]);
}

#[test]
fn tabs_keep_their_pages_when_switched_and_closed() {
    let mut opened = opened();
    enter(&mut opened.cx, "example.com");
    opened.cx.simulate_click("browser-new-tab");
    opened.cx.simulate_click("browser-new-tab");
    let cx = &mut opened.cx;
    // three pieces, the newest on screen, the others kept but hidden
    assert_eq!(piece(cx, "page-2").0, crate::HOME);
    assert!(hidden(cx, "page-1") && hidden(cx, "page-2") && !hidden(cx, "page-3"));
    cx.simulate_web_state(
        "page-1",
        WebState {
            url: "https://example.com/".into(),
            title: "Example".into(),
            ..WebState::default()
        },
    );
    // a hidden tab's page still names its tab
    assert!(cx.has_text("Example"));
    cx.simulate_click("browser-tab-1");
    assert!(!hidden(cx, "page-1") && hidden(cx, "page-3"));
    assert_eq!(
        cx.host().requests::<HostTitle>().last().map(String::as_str),
        Some("Example")
    );
    // closing the middle tab closes its page alone
    cx.simulate_click("browser-tab-2");
    cx.simulate_click("browser-close-tab");
    assert!(cx.find("page-2").is_none());
    assert_eq!(piece(cx, "page-1").0, "https://example.com");
    assert!(cx.find("page-3").is_some());
    assert_eq!(opened.tabs(), [1, 3]);
    // the tab after the closed one is on screen
    assert!(hidden(&opened.cx, "page-1") && !hidden(&opened.cx, "page-3"));
    // the last tab closed: no page, and the bar opens the next
    opened.cx.simulate_click("browser-close-tab");
    opened.cx.simulate_click("browser-close-tab");
    assert!(opened.tabs().is_empty());
    assert!(opened.cx.has_text("No page open"));
}

#[test]
fn a_link_the_app_hands_over_opens_in_a_new_tab() {
    let mut opened = opened();
    enter(&mut opened.cx, "example.com");
    opened.routes.send("https://example.org/wiki?q=1".into());
    opened.cx.run_until_parked();
    assert_eq!(opened.tabs(), [1, 2]);
    let cx = &opened.cx;
    assert_eq!(piece(cx, "page-2").0, "https://example.org/wiki?q=1");
    assert!(hidden(cx, "page-1") && !hidden(cx, "page-2"));
    assert_eq!(piece(cx, "page-1").0, "https://example.com");
}
