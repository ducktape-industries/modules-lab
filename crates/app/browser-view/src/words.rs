//! What the view says, once: every sentence the phone's screen shows, and
//! each one the desktop's shows too (the view's name, the address field's
//! name and what it says while empty). A sentence only the desktop says
//! stays in its tree.

/// The view's name, and the phone's page title.
pub(crate) const BROWSER: &str = "Browser";

/// The address field, as a screen reader names it.
pub(crate) const ADDRESS_LABEL: &str = "Address";

/// The address field while nothing is typed.
pub(crate) const TYPE_AN_ADDRESS: &str = "Type an address";

/// Under the phone's page title.
pub(crate) const PAGES_OPEN_OUTSIDE: &str = "Pages open in your phone\u{2019}s browser";

/// The phone's honest line: its build has no web view.
pub(crate) const NO_WEB_VIEW: &str = "This build has no web view. An address you open here is \
     handed to the phone\u{2019}s own browser.";

/// What hands the typed address to the phone's browser.
pub(crate) const OPEN_IN_BROWSER: &str = "Open in browser";

/// An address the host would not open because it is no web address:
/// another try cannot open it.
pub(crate) const NOT_A_WEB_ADDRESS: &str =
    "Only a web address (https://\u{2026}) opens in the phone\u{2019}s browser.";

/// The host hands no address to the phone's browser at all.
pub(crate) const NO_LINKS_HERE: &str = "This app hands no address to the phone\u{2019}s browser.";

/// The host took the address without a press, as it opens one only from
/// one; Retry is that press.
pub(crate) const NEEDS_A_PRESS: &str =
    "The phone\u{2019}s browser opens an address only from a press: press Retry.";

/// Any other refusal of an address, which another try may pass.
pub(crate) const NOT_OPENED: &str = "The phone\u{2019}s browser did not open it.";

/// The head over the home page's row.
pub(crate) const HOME_HEAD: &str = "Home";

/// The home page's name.
pub(crate) const HOME_NAME: &str = "Wikipedia";

/// The head over the links the app sent here, each waiting for a press.
pub(crate) const SENT_HERE: &str = "Sent here";

/// The head over the addresses handed out this session.
pub(crate) const OPENED_THIS_SESSION: &str = "Opened this session";
