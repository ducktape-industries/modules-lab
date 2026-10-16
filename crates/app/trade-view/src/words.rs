//! What the view says, once: the sentences, status words and problems of
//! the ticket, the book, the tape and the reader's orders, which both trees
//! show. A word only the desktop's chrome says (its bar's facts, its menus,
//! its tables' heads) stays in its tree.
use trade::{OrderState, Side};

/// The sides of an order.
pub(crate) const BUY: &str = "Buy";
pub(crate) const SELL: &str = "Sell";

/// A side's name.
pub(crate) fn side(side: Side) -> &'static str {
    match side {
        Side::Buy => BUY,
        Side::Sell => SELL,
    }
}

/// The two kinds of order: at a price of its own, or at the book's best.
pub(crate) const LIMIT: &str = "Limit";
pub(crate) const MARKET: &str = "Market";

/// An order's kind, by whether it has a limit.
pub(crate) fn kind(limited: bool) -> &'static str {
    match limited {
        true => LIMIT,
        false => MARKET,
    }
}

/// Where an order stands.
pub(crate) fn state(state: OrderState) -> &'static str {
    match state {
        OrderState::Open => "Open",
        OrderState::Filled => "Filled",
        OrderState::Cancelled => "Cancelled",
    }
}

/// Whose price a fill traded at: the resting order's, or the one that
/// took it.
pub(crate) fn fill_role(maker: bool) -> &'static str {
    match maker {
        true => "Maker",
        false => "Taker",
    }
}

// The ticket's fields and figures.
pub(crate) const PRICE: &str = "Price";
pub(crate) const AMOUNT: &str = "Amount";
pub(crate) const TOTAL: &str = "Total";
pub(crate) const ESTIMATED_TOTAL: &str = "Estimated total";
pub(crate) const AVAILABLE: &str = "Available";
/// A market order's price, which is not the reader's to type.
pub(crate) const BEST_PRICE: &str = "Best price in the book";

/// A price off the market's tick: `step` written in `quote`.
pub(crate) fn price_step(step: &str, quote: &str) -> String {
    format!("A price here is a number in steps of {step} {quote}.")
}

/// An amount off the lot: `step` written in `base`.
pub(crate) fn amount_step(step: &str, base: &str) -> String {
    format!("An amount is a number in steps of {step} {base}.")
}

/// An order the free funds do not cover: both sums with their asset.
pub(crate) fn short_of(needs: &str, free: &str) -> String {
    format!("This order needs {needs}. {free} is available.")
}

/// Why a reader places nothing: they have no account, or one and no link.
pub(crate) fn cannot_place(signed_in: bool) -> String {
    let needs = match signed_in {
        false => "Placing an order needs an account. You are looking around without one",
        true => "Placing an order needs a link to the network. There is none now",
    };
    format!("{needs}, so prices, the book and trades are all here to read.")
}

/// The ticket's button: `Buy 0.1000 BTC`, or `Buy BTC` before an amount.
pub(crate) fn place(side: Side, amount: Option<String>, base: &str) -> String {
    let side = self::side(side);
    match amount {
        Some(amount) => format!("{side} {amount} {base}"),
        None => format!("{side} {base}"),
    }
}

/// A share button, as a reader names it.
pub(crate) fn share(share: u64) -> String {
    format!("{share}% of what is available")
}

/// An op that may have landed: no answer came back for it.
pub(crate) const IN_DOUBT: &str = "No answer came back: it may have gone through.";

// The book and the tape.
pub(crate) const ORDER_BOOK: &str = "Order book";
pub(crate) const TRADES: &str = "Trades";
pub(crate) const SPREAD: &str = "Spread";
pub(crate) const TIME: &str = "Time";
pub(crate) const EMPTY_BOOK: &str = "No orders in the book";
pub(crate) const NO_TRADES: &str = "No trades yet";

/// A column of prices, in `quote`.
pub(crate) fn price_in(quote: &str) -> String {
    format!("Price ({quote})")
}

/// A column of amounts, in `base`.
pub(crate) fn amount_in(base: &str) -> String {
    format!("Amount ({base})")
}

/// The bids' share of what the book's rows hold, and the asks'.
pub(crate) fn bids_share(share: u32) -> String {
    format!("Bids {share}%")
}
pub(crate) fn asks_share(share: u32) -> String {
    format!("{share}% Asks")
}

// The reader's orders, fills and balances.
pub(crate) const OPEN_ORDERS: &str = "Open orders";
pub(crate) const HISTORY: &str = "History";
pub(crate) const FILLS: &str = "Fills";
pub(crate) const BALANCES: &str = "Balances";
pub(crate) const CANCEL: &str = "Cancel";
pub(crate) const CANCEL_ALL: &str = "Cancel all";
pub(crate) const GET_FUNDS: &str = "Get practice funds";
pub(crate) const NO_OPEN_ORDERS: &str = "No open orders";
pub(crate) const NO_ORDERS: &str = "No orders yet";
pub(crate) const NO_FILLS: &str = "No fills yet";
pub(crate) const NO_FUNDS: &str = "No funds yet. Get practice funds to place an order.";
/// A reader with no account, where their orders would be.
pub(crate) const SIGNED_OUT: &str = "Orders, fills and balances are an account's.";
/// A row of a list whose page is on its way.
pub(crate) const READING: &str = "Reading\u{2026}";
