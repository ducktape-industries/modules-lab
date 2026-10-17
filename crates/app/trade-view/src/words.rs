//! What the view says, once: every sentence, status word and problem the
//! phone's tree shows, and each one the desktop's tree shows too (the
//! ticket, the book and the tape, the reader's orders and balances, the
//! chart's switches, a write's settlement). A word only the desktop says
//! (its menus, its tables' heads) stays in its tree.
use trade::{OrderState, Side};

/// The view's name: the phone's first page's title.
pub(crate) const TRADE: &str = "Trade";
/// Under it.
pub(crate) const SPOT_MARKETS: &str = "Spot markets for practice funds";
/// The markets, and a market's line under its name.
pub(crate) const MARKETS: &str = "Markets";
pub(crate) const PRACTICE_SPOT: &str = "Practice \u{b7} spot";

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

// The market's day.
pub(crate) const HIGH_24H: &str = "24h high";
pub(crate) const LOW_24H: &str = "24h low";
pub(crate) const VOLUME_24H: &str = "24h volume";

// The chart's tools and what it draws.
pub(crate) const CANDLE_SPAN: &str = "Candle span";
pub(crate) const CHART: &str = "Chart";
pub(crate) const CHART_SETTINGS: &str = "Chart settings";
pub(crate) const MA7: &str = "MA 7";
pub(crate) const MA25: &str = "MA 25";
pub(crate) const VOLUME: &str = "Volume";
pub(crate) const LINE: &str = "Line";
/// The line's switch, where it stands alone on a row.
pub(crate) const LINE_INSTEAD: &str = "Line instead of candles";

// The ticket's fields and figures.
pub(crate) const ORDER_TICKET: &str = "Order ticket";
pub(crate) const SIDE: &str = "Side";
pub(crate) const ORDER_TYPE: &str = "Order type";
pub(crate) const PRICE: &str = "Price";
pub(crate) const AMOUNT: &str = "Amount";
pub(crate) const TOTAL: &str = "Total";
pub(crate) const ESTIMATED_TOTAL: &str = "Estimated total";
pub(crate) const AVAILABLE: &str = "Available";
/// A market order's price, which is not the reader's to type.
pub(crate) const BEST_PRICE: &str = "Best price in the book";

/// The ticket's sheet under its title: the market, its last price, and
/// whose funds these are.
pub(crate) fn ticket_line(market: &str, last: &str) -> String {
    format!("{market} \u{b7} last {last} \u{b7} practice funds")
}

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

/// A market order with nothing on the book's other side to trade against.
pub(crate) fn no_makers(side: Side) -> &'static str {
    match side {
        Side::Buy => "The book has no asks: a market order has nothing to buy.",
        Side::Sell => "The book has no bids: a market order has nothing to sell to.",
    }
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

// A write on its way, and one that may have landed.
pub(crate) const SENDING: &str = "Sending\u{2026}";
pub(crate) const CANCELLING: &str = "Cancelling\u{2026}";
pub(crate) const IN_DOUBT: &str = "No answer came back: it may have gone through.";

// The book and the tape.
pub(crate) const BOOK: &str = "Book";
pub(crate) const ORDER_BOOK: &str = "Order book";
pub(crate) const TRADES: &str = "Trades";
pub(crate) const SPREAD: &str = "Spread";
pub(crate) const TIME: &str = "Time";
pub(crate) const PRICE_STEP: &str = "Price step";

/// The price step's control, named with the step it is at.
pub(crate) fn step_named(step: &str) -> String {
    format!("{PRICE_STEP}: {step}")
}
/// What the phone's market page shows under its chart, as a reader names
/// the choice.
pub(crate) const BOOK_TRADES_ORDERS: &str = "Book, trades or orders";
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

/// What a row of the book is, at a step written in `quote`.
pub(crate) fn per_row(quote: &str) -> String {
    format!("{quote} per row of the book")
}

// The reader's orders, fills and balances.
pub(crate) const ORDERS: &str = "Orders";
pub(crate) const OPEN: &str = "Open";
pub(crate) const OPEN_ORDERS: &str = "Open orders";
pub(crate) const HISTORY: &str = "History";
pub(crate) const ORDER_HISTORY: &str = "Order history";
pub(crate) const FILLS: &str = "Fills";
/// The phone's orders page's lists, as a reader names the choice.
pub(crate) const ORDER_LISTS: &str = "Open orders, history or fills";
pub(crate) const BALANCES: &str = "Balances";
pub(crate) const EVERY_MARKET: &str = "Every market";
pub(crate) const YOUR_ACCOUNT: &str = "Your account";
pub(crate) const PRACTICE_FUNDS: &str = "Practice funds";
pub(crate) const ASSETS: &str = "Assets";
pub(crate) const CANCEL: &str = "Cancel";
pub(crate) const CANCEL_ALL: &str = "Cancel all";
pub(crate) const GET_FUNDS: &str = "Get practice funds";
/// Get practice funds while the grant is on its way.
pub(crate) const GETTING_FUNDS: &str = "Getting practice funds\u{2026}";
/// The grant landed.
pub(crate) const FUNDED: &str = "Practice funds topped up to the grant.";
/// Get practice funds pressed with every asset at its grant: nothing went.
pub(crate) const AT_GRANT: &str = "Every asset was already at its grant: nothing to add.";
pub(crate) const NO_OPEN_ORDERS: &str = "No open orders";
pub(crate) const NO_ORDERS: &str = "No orders yet";
pub(crate) const NO_FILLS: &str = "No fills yet";
/// An account with nothing: what is missing, then what fills it.
pub(crate) const NO_FUNDS: &str = "No funds yet";
pub(crate) const FUNDS_TO_ORDER: &str = "Get practice funds to place an order.";
/// A reader with no account, where their orders would be.
pub(crate) const SIGNED_OUT: &str = "Orders, fills and balances are an account's.";
/// A reader with no account, where their account's rows would be.
pub(crate) const NO_ACCOUNT: &str = "Orders and balances are an account's. You are looking around \
    without one, so prices, the book and trades are all here to read.";
/// A reader with no link to a node.
pub(crate) const NO_LINK: &str =
    "Not connected to a node: prices are the last read. Orders are not sent.";
/// A row of a list whose page is on its way.
pub(crate) const READING: &str = "Reading\u{2026}";

/// An account with nothing, in one line, where there is no room for two.
pub(crate) fn no_funds() -> String {
    format!("{NO_FUNDS}. {FUNDS_TO_ORDER}")
}

/// The open orders' row under its title: how many rest.
pub(crate) fn resting(count: usize) -> String {
    match count {
        0 => "None".into(),
        count => format!("{count} resting \u{b7} Cancel all from the list"),
    }
}

/// The open orders' head, counted.
pub(crate) fn open_orders(count: usize) -> String {
    format!("{OPEN_ORDERS} \u{b7} {count}")
}

/// What an order rests at: `Limit 62,635.0`, or `Market`.
pub(crate) fn limit(price: Option<String>) -> String {
    match price {
        Some(price) => format!("{LIMIT} {price}"),
        None => MARKET.into(),
    }
}

/// How much of an order filled.
pub(crate) fn filled(percent: u128) -> String {
    format!("{percent}% filled")
}

/// What an order's fills came to a unit, or a dash before any.
pub(crate) fn average(price: Option<String>) -> String {
    format!("avg {}", price.as_deref().unwrap_or("\u{2014}"))
}

/// What an asset's funds hold in resting orders.
pub(crate) fn in_orders(held: &str) -> String {
    format!("In orders {held}")
}

/// Under a balance's figure.
pub(crate) const FREE: &str = "available";

/// What a grant does: each asset's grant, written.
pub(crate) fn grants(grants: &[String]) -> String {
    format!(
        "Tops each asset up to its grant: {}. Never lowers a balance.",
        grants.join(", ")
    )
}

/// The market an order or a fill is in, after what it is.
pub(crate) fn in_market(market: &str) -> String {
    format!("\u{b7} {market}")
}

/// Parts of a line, joined as the kit joins them.
pub(crate) fn joined(parts: &[impl std::borrow::Borrow<str>]) -> String {
    parts.join(" \u{b7} ")
}
