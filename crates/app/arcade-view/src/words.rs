//! What the view says, once: every sentence the phone's tree shows, and
//! each one the desktop's tree shows too (the view's name, a game's line,
//! the reader's best, a board being read, the line for a reader with no
//! account). A game's name is the program's ([`arcade::GAMES`]); a
//! sentence only the desktop says stays in its tree.
use view::prelude::*;

/// The view's name, and its first page's title.
pub(crate) const ARCADE: &str = "Arcade";

/// What each game is, in a line, in [`arcade::GAMES`]' order.
pub(crate) const BLURBS: [&str; 6] = [
    "Fill rows to clear them.",
    "Eat, grow, and keep off your tail.",
    "Slide equal tiles together.",
    "Clear the field without a mine.",
    "Eight walls, steel and falling items.",
    "Shoot the march down before it lands.",
];

/// The reader's best of a game, as the program keeps it: zero where none
/// is posted, none while the bests are being read.
pub(crate) fn best(best: Option<u64>) -> String {
    match best {
        Some(0) => "No score yet".to_owned(),
        Some(best) => format!("Your best {}", design::grouped(best)),
        None => "Your best \u{2014}".to_owned(),
    }
}

/// A reader with no account: nothing is posted, and no best is theirs.
pub(crate) const UNSIGNED: &str =
    "Scores go on the board from an account. Without one the games play and nothing is posted.";

/// A board being read.
pub(crate) const READING_BOARD: &str = "Reading the board\u{2026}";

/// An account on a board identity gives no name, called by its number.
pub(crate) fn unnamed(account: u64) -> String {
    format!("account {account}")
}
