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

/// Under the phone's first page's title.
pub(crate) const HUB_CAPTION: &str = "Boards of everyone's best scores";

/// The phone's honest line: it reads the boards, it plays nothing.
pub(crate) const PHONE_ONLY_READS: &str =
    "The games are played on a computer with a keyboard. On the phone you can read the boards.";

/// The head over the phone's list of games.
pub(crate) const GAMES: &str = "Games";

/// A game's row, as a screen reader names it: what it opens.
pub(crate) fn board_of(name: &str) -> String {
    format!("{name} board")
}

/// The head over a board's rows, and the start of its page's caption.
pub(crate) const TOP_TEN: &str = "Top ten";

/// Under a board's title: the ten, and what the game is.
pub(crate) fn board_caption(blurb: &str) -> String {
    format!("{TOP_TEN} \u{b7} {blurb}")
}

/// The reader's own row, marked, and the head over it outside the ten.
pub(crate) const YOU: &str = "You";

/// The reader's best outside the ten: the program gives no rank.
pub(crate) const NOT_IN_TOP_TEN: &str = "Not in the top ten";

/// A figure not known: a best not read yet, a rank the program does not
/// give.
pub(crate) const NOT_KNOWN: &str = "\u{2014}";

/// A board read with no account seated.
pub(crate) const OWN_BEST_NEEDS_AN_ACCOUNT: &str = "Your own best shows from an account.";

/// A board no one has posted to, and what fills it.
pub(crate) const NO_SCORES: &str = "No scores yet";
pub(crate) fn first_score(name: &str) -> String {
    format!("The first game of {name} played on a computer puts a score here.")
}
