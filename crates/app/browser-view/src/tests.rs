//! The view on a fake host: each test drives the frame the way a person
//! would and reads what the host would get. Every frame it sends is held
//! to the host's sanitizer and accessibility audit.
use ducktape_view_guest::Entity;
use ducktape_view_guest::methods::{HostRoute, HostTitle, HostWidget};
use ducktape_view_guest::testing::{StreamSender, TestAppContext};
use ducktape_view_guest::wire::{Node, WebCommand, WebState, WidgetCommand};

use crate::{Browser, address, host_shown};

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

/// Presses the address field at rest, if it is, then types `typed` and
/// presses Enter.
fn enter(cx: &mut TestAppContext, typed: &str) {
    if !matches!(cx.node("browser-address"), Node::Field { .. }) {
        cx.simulate_click("browser-address");
    }
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

/// What assistive technology reads of `key`: its name and its value.
fn aria(cx: &TestAppContext, key: &str) -> (String, String) {
    let aria = &cx.interactivity(key).aria;
    let text = |text: &Option<_>| text.as_ref().map_or(String::new(), ToString::to_string);
    (text(&aria.label), text(&aria.value))
}

fn titled(url: &str, title: &str) -> WebState {
    WebState {
        url: url.into(),
        title: title.into(),
        ..WebState::default()
    }
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
    // through the field: with no tab open it is typing already, and
    // Enter opens the first tab on it
    let Opened { mut cx, .. } = opened();
    assert!(cx.find("page-1").is_none());
    assert!(matches!(cx.node("browser-address"), Node::Field { .. }));
    enter(&mut cx, "example.com/a");
    assert_eq!(piece(&cx, "page-1").0, "https://example.com/a");
    // an untitled page is named by its host
    assert_eq!(cx.host().requests::<HostTitle>(), ["example.com"]);
}

#[test]
fn the_field_at_rest_shows_the_host_and_pressed_the_whole_address() {
    let Opened { mut cx, view, .. } = opened();
    enter(&mut cx, "https://www.example.com/a/b");
    // at rest: the host, a lock for https, and the address for AT
    assert!(cx.has_text("example.com"));
    assert!(!cx.has_text("https://www.example.com/a/b"));
    assert_eq!(
        aria(&cx, "browser-address"),
        ("Address".into(), "https://www.example.com/a/b".into())
    );
    // pressed: the whole address, given the keyboard and selected
    cx.simulate_click("browser-address");
    assert!(matches!(cx.node("browser-address"), Node::Field { .. }));
    assert_eq!(
        view.read(|view| view.address.text()),
        "https://www.example.com/a/b"
    );
    let asked = cx.host().requests::<HostWidget>();
    assert!(
        matches!(
            asked.as_slice(),
            [
                ..,
                WidgetCommand::Focus { .. },
                WidgetCommand::SelectAll { .. }
            ]
        ),
        "{asked:?}"
    );
    // Escape drops what was typed and puts the host back
    cx.simulate_input("browser-address", "somewhere.else");
    cx.simulate_focus("browser-address");
    cx.simulate_key_down("browser-address", "escape");
    assert!(!matches!(cx.node("browser-address"), Node::Field { .. }));
    assert!(cx.has_text("example.com"));
    assert!(piece(&cx, "page-1").1.is_empty());
}

/// The field at rest shows `text` for `url`, `locked` or not.
fn shows(url: &str, text: &str, locked: bool) {
    assert_eq!(host_shown(url), (text.to_owned(), locked), "{url}");
}

#[test]
fn the_field_shows_the_real_host_or_the_whole_address() {
    shows("https://www.google.com/search?q=1", "google.com", true);
    shows("https://example.org#top", "example.org", true);
    // `www.` goes only where a dot stays
    shows("https://www.com/", "www.com", true);
    // a port the scheme does not imply stays; one it does goes
    shows("http://localhost:8000/a", "localhost:8000", false);
    shows("https://example.com:443/", "example.com", true);
    shows("https://example.com:8443/", "example.com:8443", true);
    shows("http://[::1]:8080/", "[::1]:8080", false);
    // an international name in its `xn--` form, typed either way
    shows(
        "https://xn--bcher-kva.example/",
        "xn--bcher-kva.example",
        true,
    );
    shows("https://bücher.example/", "xn--bcher-kva.example", true);
    // a long chain of names stays whole: the field cuts it from the start
    shows(
        "https://accounts.google.com.secure.login.evil.example/x",
        "accounts.google.com.secure.login.evil.example",
        true,
    );
    // what comes before an `@` is a user name, never the host: such an
    // address shows whole and unlocked
    shows(
        "https://google.com@evil.com/",
        "https://google.com@evil.com/",
        false,
    );
    shows(
        "https://user:pass@example.com/",
        "https://user:pass@example.com/",
        false,
    );
    // a backslash ends the host, as the page's engine reads it
    shows("https://evil.com\\@google.com/", "evil.com", true);
    // no scheme, no host, or none that reads: whole, unlocked
    shows("example.com", "example.com", false);
    shows("about:blank", "about:blank", false);
    shows("https://exa mple.com/", "https://exa mple.com/", false);
    shows("", "", false);
    // through the field: an address with a user name shows whole, cut
    // from the start, with no lock
    let mut opened = opened();
    enter(&mut opened.cx, "https://example.com/");
    assert!(opened.cx.find("browser-lock").is_some());
    opened.routes.send("https://google.com@evil.com/".into());
    opened.cx.run_until_parked();
    let cx = &opened.cx;
    assert!(cx.has_text("https://google.com@evil.com/"));
    assert!(cx.find("browser-lock").is_none());
    assert!(matches!(
        cx.style("browser-host").text.text_overflow,
        Some(gpui::TextOverflow::TruncateStart(_))
    ));
}

#[test]
fn enter_on_an_open_page_sends_it_one_load() {
    let Opened { mut cx, .. } = opened();
    enter(&mut cx, "https://example.com/");
    enter(&mut cx, "wikipedia.org");
    let (first, commands) = piece(&cx, "page-1");
    assert_eq!(first, "https://example.com/");
    assert_eq!(commands, [WebCommand::Load("https://wikipedia.org".into())]);
    // the next frame carries it no more: the page loads it once
    cx.simulate_web_state(
        "page-1",
        WebState {
            loading: true,
            ..titled("https://www.wikipedia.org/", "Wikipedia")
        },
    );
    assert!(piece(&cx, "page-1").1.is_empty());
    // the field says where the page went; while it loads, reload is stop
    // and a line runs along the field
    assert!(cx.has_text("wikipedia.org"));
    assert!(cx.find("browser-loading").is_some());
    assert_eq!(aria(&cx, "browser-stop").0, "Stop");
    assert!(cx.find("browser-reload").is_none());
    cx.simulate_click("browser-stop");
    assert_eq!(piece(&cx, "page-1").1, [WebCommand::Stop]);
    cx.simulate_web_state("page-1", titled("https://www.wikipedia.org/", "Wikipedia"));
    assert!(cx.find("browser-loading").is_none());
    // stop and reload are two elements: a stop press that arrives after the
    // load ended finds no stop, and never reloads
    assert!(cx.find("browser-stop").is_none());
    assert_eq!(aria(&cx, "browser-reload").0, "Reload");
    cx.simulate_click("browser-reload");
    assert_eq!(piece(&cx, "page-1").1, [WebCommand::Reload]);
}

#[test]
fn back_and_forward_follow_what_the_page_says() {
    let Opened { mut cx, .. } = opened();
    // nothing open: nothing to go back to
    assert!(disabled(&cx, "browser-back"));
    assert!(disabled(&cx, "browser-forward"));
    assert!(cx.find("browser-reload").is_none());
    enter(&mut cx, "example.com");
    assert!(disabled(&cx, "browser-back"));
    assert!(disabled(&cx, "browser-forward"));
    cx.simulate_web_state(
        "page-1",
        WebState {
            can_go_back: true,
            ..titled("https://example.com/next", "Next")
        },
    );
    assert!(!disabled(&cx, "browser-back"));
    assert!(disabled(&cx, "browser-forward"));
    // a link followed in the page moves the field and the window's title
    assert_eq!(aria(&cx, "browser-address").1, "https://example.com/next");
    assert_eq!(
        cx.host().requests::<HostTitle>().last().map(String::as_str),
        Some("Next")
    );
    cx.simulate_click("browser-back");
    assert_eq!(piece(&cx, "page-1").1, [WebCommand::Back]);
    cx.simulate_web_state(
        "page-1",
        WebState {
            can_go_forward: true,
            ..titled("https://example.com/", "Example")
        },
    );
    assert!(disabled(&cx, "browser-back"));
    assert!(!disabled(&cx, "browser-forward"));
    cx.simulate_click("browser-forward");
    assert_eq!(piece(&cx, "page-1").1, [WebCommand::Forward]);
}

#[test]
fn a_new_tab_gives_the_keys_to_its_address() {
    let Opened { mut cx, .. } = opened();
    enter(&mut cx, "https://example.com/");
    cx.simulate_click("browser-new-tab");
    assert_eq!(cx.focused().and_then(Node::key), Some("browser-address"));
    // typing goes straight to the new tab's page, not the first one's
    cx.simulate_input("browser-address", "wikipedia.org");
    cx.simulate_submit("browser-address");
    assert_eq!(
        piece(&cx, "page-2").1,
        [WebCommand::Load("https://wikipedia.org".into())]
    );
    assert!(piece(&cx, "page-1").1.is_empty());
}

#[test]
fn an_arrow_on_a_close_button_moves_the_tabs_and_the_keys_stay_seen() {
    let mut opened = opened();
    enter(&mut opened.cx, "example.com");
    opened.cx.simulate_click("browser-new-tab");
    let cx = &mut opened.cx;
    // the chosen tab's close button holds the keys
    cx.simulate_focus("browser-close-tab-2");
    cx.simulate_key_down("browser-close-tab-2", "left");
    assert!(!hidden(cx, "page-1") && hidden(cx, "page-2"));
    // that button is hidden now; the keys are on the tab list, not on it
    assert!(hidden(cx, "browser-close-tab-2"));
    assert_eq!(cx.focused().and_then(Node::key), Some("browser-tabs"));
    // the arrows go round: left from the first tab picks the last
    cx.simulate_key_down("browser-tabs", "left");
    assert!(!hidden(cx, "page-2") && hidden(cx, "page-1"));
}

#[test]
fn tabs_keep_their_pages_when_switched_and_closed() {
    let mut opened = opened();
    enter(&mut opened.cx, "example.com");
    // one tab: no tab bar, as Safari
    assert!(opened.cx.find("browser-tabs").is_none());
    opened.cx.simulate_click("browser-new-tab");
    opened.cx.simulate_click("browser-new-tab");
    let cx = &mut opened.cx;
    // a new tab gives the field the keys; Escape puts it back at rest
    cx.simulate_key_down("browser-address", "escape");
    assert!(cx.find("browser-tabs").is_some());
    // three pieces, the newest on screen, the others kept but hidden
    assert_eq!(piece(cx, "page-2").0, crate::HOME);
    assert!(hidden(cx, "page-1") && hidden(cx, "page-2") && !hidden(cx, "page-3"));
    // the chosen tab's close button shows; the others' on hover
    assert!(!hidden(cx, "browser-close-tab-3"));
    assert!(hidden(cx, "browser-close-tab-1") && hidden(cx, "browser-close-tab-2"));
    cx.simulate_web_state("page-1", titled("https://example.com/", "Example"));
    // a hidden tab's page still names its tab, and moves neither the field
    // nor the window's title
    assert!(cx.has_text("Example"));
    assert_eq!(aria(cx, "browser-address").1, crate::HOME);
    assert_eq!(
        cx.host().requests::<HostTitle>().last().map(String::as_str),
        Some("wikipedia.org")
    );
    cx.simulate_click("browser-tab-1");
    assert!(!hidden(cx, "page-1") && hidden(cx, "page-3"));
    assert_eq!(
        cx.host().requests::<HostTitle>().last().map(String::as_str),
        Some("Example")
    );
    // the arrows pick the tabs
    cx.simulate_focus("browser-tabs");
    cx.simulate_key_down("browser-tabs", "right");
    assert!(!hidden(cx, "page-2") && hidden(cx, "page-1"));
    // closing the middle tab closes its page alone
    cx.simulate_click("browser-close-tab-2");
    assert!(cx.find("page-2").is_none());
    assert_eq!(piece(cx, "page-1").0, "https://example.com");
    assert!(cx.find("page-3").is_some());
    assert_eq!(opened.tabs(), [1, 3]);
    // the tab after the closed one is on screen
    assert!(hidden(&opened.cx, "page-1") && !hidden(&opened.cx, "page-3"));
    // a new tab takes a fresh id: a closed tab's key never names another
    // page, so the host never hands the new tab an old one
    opened.cx.simulate_click("browser-new-tab");
    assert_eq!(opened.tabs(), [1, 3, 4]);
    assert_eq!(piece(&opened.cx, "page-4").0, crate::HOME);
    // closing the rightmost tab puts the one before it on screen
    opened.cx.simulate_click("browser-close-tab-4");
    assert!(hidden(&opened.cx, "page-1") && !hidden(&opened.cx, "page-3"));
    // a tab that is not the chosen one closes too, the chosen one stays
    opened.cx.simulate_click("browser-close-tab-1");
    assert_eq!(opened.tabs(), [3]);
    assert!(!hidden(&opened.cx, "page-3"));
    // one tab left: the bar goes
    assert!(opened.cx.find("browser-tabs").is_none());
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

#[test]
fn no_page_open_is_a_quiet_start() {
    let Opened { cx, .. } = opened();
    assert!(cx.has_text("No page open"));
    // the field holds the keyboard from the start
    assert!(
        cx.host()
            .requests::<HostWidget>()
            .iter()
            .any(|asked| matches!(asked, WidgetCommand::Focus { .. }))
    );
}
