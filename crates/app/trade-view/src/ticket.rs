//! The order ticket: the side, a limit or a market order, its price and
//! amount, what that comes to and what the account has free to pay it.
//! Nothing is sent that the program would refuse for a reason the ticket
//! can see: a price off the market's tick, an amount off the lot, an order
//! the free funds do not cover. A reader with no account, or no link,
//! reads the ticket and places nothing.
//!
//! This is the ticket's model: what it holds, the order it reads as
//! ([`Draft`]), what stands in that order's way, said in words, and the
//! presses that change it or send it. The trees draw it (`ui/ticket.rs`
//! is the desktop's) and press these verbs.
use serde::{Deserialize, Serialize};
use trade::{ASSETS, LOT, Level, Op, Side, UNIT};
use view::prelude::*;

use crate::{Column, Trade, format, words};

/// The shares of what the free funds afford that one press fills in.
pub(crate) const SHARES: [u64; 4] = [25, 50, 75, 100];

#[derive(Default, Serialize, Deserialize)]
pub struct Ticket {
    pub(crate) sell: bool,
    /// A market order: no price of its own, the best prices in the book.
    pub(crate) market: bool,
    pub(crate) price: TextField,
    pub(crate) amount: TextField,
    /// Why the program refused the last order, until the ticket changes.
    #[serde(skip)]
    pub(crate) refusal: String,
    /// An order is on its way: the button waits for its answer.
    #[serde(skip)]
    pub(crate) placing: bool,
}

impl Ticket {
    /// An empty ticket: another market's prices are not this one's.
    pub(crate) fn clear(&mut self) {
        self.price.reset("");
        self.amount.reset("");
        self.refusal.clear();
    }

    /// A field's text is the reader's: typing takes back what the program
    /// said of the last order.
    pub(crate) fn edited(&mut self) {
        self.refusal.clear();
    }
}

/// What stands between the ticket and an order.
pub(crate) enum Problem {
    /// The price is not a number on the market's tick.
    Price,
    /// The amount is not a number of whole lots.
    Amount,
    /// The free funds are short of what the order needs.
    Funds(u64),
}

/// The ticket as read: the order it would place and what that takes.
pub(crate) struct Draft {
    pub(crate) side: Side,
    /// The price typed; none for a market order, or while there is none.
    pub(crate) limit: Option<u64>,
    pub(crate) amount: Option<u64>,
    /// What the order comes to in the quote: exact at a limit, estimated
    /// from the book on screen at the market.
    pub(crate) total: Option<u64>,
    /// The asset the order spends and what the account has free of it.
    pub(crate) spends: u8,
    pub(crate) free: u64,
    pub(crate) problem: Option<Problem>,
}

impl Draft {
    /// `value` of the asset the order spends, written with its name:
    /// `25,280.00 USD`.
    pub(crate) fn funds(&self, value: u64) -> String {
        let spent = ASSETS[self.spends as usize].name;
        format!("{} {spent}", format::balance(self.spends, value))
    }
}

impl Trade {
    /// What the field `text` holds in steps of `step`: nothing while it
    /// is empty, and a refusal when it is not such a number.
    fn stepped(text: &str, step: u64) -> Result<Option<u64>, ()> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        match format::typed(text) {
            Some(value) if value > 0 && value.is_multiple_of(step) => Ok(Some(value)),
            _ => Err(()),
        }
    }

    pub(crate) fn draft(&self) -> Draft {
        let (market, ticket) = (self.market(), &self.ticket);
        let side = if ticket.sell { Side::Sell } else { Side::Buy };
        let spends = match side {
            Side::Buy => market.quote,
            Side::Sell => market.base,
        };
        let balances = self.balances.ready().map(Vec::as_slice).unwrap_or_default();
        let held = balances.iter().find(|balance| balance.asset == spends);
        let free = held.map_or(0, |balance| balance.free);
        let limit = match ticket.market {
            true => Ok(None),
            false => Self::stepped(&ticket.price.text(), market.tick),
        };
        let amount = Self::stepped(&ticket.amount.text(), LOT);
        let total = match (ticket.market, limit, amount) {
            (false, Ok(Some(limit)), Ok(Some(amount))) => Some(format::cost(limit, amount)),
            (true, _, Ok(Some(amount))) => self.estimate(side, amount),
            _ => None,
        };
        let needs = match side {
            Side::Buy => total,
            Side::Sell => amount.ok().flatten(),
        };
        let problem = match (&limit, &amount, needs) {
            (Err(()), ..) => Some(Problem::Price),
            (_, Err(()), _) => Some(Problem::Amount),
            (_, _, Some(needs)) if needs > free => Some(Problem::Funds(needs)),
            _ => None,
        };
        Draft {
            side,
            limit: limit.ok().flatten(),
            amount: amount.ok().flatten(),
            total,
            spends,
            free,
            problem,
        }
    }

    /// The side of the book an order of `side` trades against, best first.
    fn makers(&self, side: Side) -> &[Level] {
        let Some(book) = self.book.ready() else {
            return &[];
        };
        match side {
            Side::Buy => &book.asks,
            Side::Sell => &book.bids,
        }
    }

    /// What a market order of `amount` comes to against the book on
    /// screen, as far as that book goes; none while its other side is
    /// empty.
    // ponytail: the prices on screen only (`data::DEPTH` a side), so an
    // order deeper than those is estimated short, and `affords` likewise;
    // a `Quote` query answered by the program would be exact.
    fn estimate(&self, side: Side, amount: u64) -> Option<u64> {
        let makers = self.makers(side);
        let (mut left, mut total) = (amount, 0u64);
        for maker in makers {
            let taken = left.min(maker.amount);
            total = total.saturating_add(format::cost(maker.price, taken));
            left -= taken;
        }
        (!makers.is_empty()).then_some(total)
    }

    /// `share` percent of what the free funds afford, down to whole lots:
    /// of the base to sell, of what the quote buys at the limit, or at the
    /// book's asks from the best one up.
    pub(crate) fn affords(&self, draft: &Draft, share: u64) -> Option<u64> {
        let funds = (u128::from(draft.free) * u128::from(share) / 100) as u64;
        let bought = |funds: u64, price: u64| {
            u64::try_from(u128::from(funds) * u128::from(UNIT) / u128::from(price.max(1)))
                .unwrap_or(u64::MAX)
        };
        let amount = match (draft.side, self.ticket.market) {
            (Side::Sell, _) => funds,
            (Side::Buy, false) => bought(funds, draft.limit?),
            (Side::Buy, true) => {
                let (mut left, mut amount) = (funds, 0u64);
                for maker in self.makers(Side::Buy) {
                    let whole = format::cost(maker.price, maker.amount);
                    if left < whole {
                        amount = amount.saturating_add(bought(left, maker.price));
                        break;
                    }
                    (left, amount) = (left - whole, amount.saturating_add(maker.amount));
                }
                amount
            }
        };
        Some(amount - amount % LOT).filter(|amount| *amount > 0)
    }

    /// Whether the order has its price: the limit typed, or for a market
    /// order the book read, which its funds are checked against.
    fn priced(&self, draft: &Draft) -> bool {
        match self.ticket.market {
            true => self.book.ready().is_some(),
            false => draft.limit.is_some(),
        }
    }

    /// Whether the ticket's button places `draft`: it is whole, covered
    /// and priced, and no order is on its way.
    pub(crate) fn placeable(&self, draft: &Draft) -> bool {
        draft.amount.is_some()
            && draft.problem.is_none()
            && self.priced(draft)
            && !self.ticket.placing
    }

    /// The order the ticket holds, sent: once it is placeable, and the
    /// reader may trade.
    pub(crate) fn place(&mut self, cx: &mut Context<Self>) {
        let draft = self.draft();
        let (true, Some(amount)) = (self.placeable(&draft), draft.amount) else {
            return;
        };
        let op = Op::Place {
            market: self.market as u8,
            side: draft.side,
            amount,
            limit: draft.limit,
        };
        self.submit(op, cx);
    }

    pub(crate) fn pick_side(&mut self, sell: bool, cx: &mut Context<Self>) {
        self.ticket.sell = sell;
        self.ticket.refusal.clear();
        cx.notify();
    }

    /// A limit order, or with `market` one at the book's best prices.
    pub(crate) fn pick_kind(&mut self, market: bool, cx: &mut Context<Self>) {
        self.ticket.market = market;
        self.ticket.refusal.clear();
        cx.notify();
    }

    /// The amount `share` percent of the free funds affords, in the
    /// amount's field; nothing when they afford no lot.
    pub(crate) fn take_share(&mut self, share: u64, cx: &mut Context<Self>) {
        let amount = self.affords(&self.draft(), share);
        self.ticket
            .amount
            .reset(amount.map(format::amount).unwrap_or_default());
        self.ticket.refusal.clear();
        cx.notify();
    }

    /// A book row's price, pressed: the ticket takes it as a limit, and
    /// the pane's column turns to the ticket.
    pub(crate) fn take_price(&mut self, price: u64, cx: &mut Context<Self>) {
        let price = self.price(price);
        self.ticket.price.reset(price);
        self.ticket.market = false;
        self.ticket.refusal.clear();
        self.column = Column::Order;
        cx.notify();
    }

    /// What is wrong with the price typed, said: it is off the tick.
    pub(crate) fn price_problem(&self, draft: &Draft) -> Option<String> {
        let (_, quote) = self.assets();
        let step = || self.price(self.market().tick);
        matches!(draft.problem, Some(Problem::Price)).then(|| words::price_step(&step(), quote))
    }

    /// What is wrong with the amount typed, said: it is off the lot.
    pub(crate) fn amount_problem(&self, draft: &Draft) -> Option<String> {
        let (base, _) = self.assets();
        let step = || format::amount(LOT);
        matches!(draft.problem, Some(Problem::Amount)).then(|| words::amount_step(&step(), base))
    }

    /// What stands between the ticket and its order, said: the free funds
    /// short of it, a field's problem, or else why the program refused the
    /// last order. `None` while nothing does.
    pub(crate) fn ticket_problem(&self, draft: &Draft) -> Option<String> {
        match &draft.problem {
            Some(Problem::Funds(needs)) => Some(words::short_of(
                &draft.funds(*needs),
                &draft.funds(draft.free),
            )),
            Some(_) => self.price_problem(draft).or(self.amount_problem(draft)),
            None => Some(self.ticket.refusal.clone()).filter(|refusal| !refusal.is_empty()),
        }
    }

    /// What the ticket's button says: the side, and the amount once one
    /// is typed.
    pub(crate) fn place_label(&self, draft: &Draft) -> String {
        let (base, _) = self.assets();
        words::place(draft.side, draft.amount.map(format::amount), base)
    }

    /// Why the reader places nothing, while they may not trade: no
    /// account, or no link.
    pub(crate) fn why_not_trading(&self) -> Option<String> {
        let signed_in = self.session.account.is_some();
        (!self.may_trade()).then(|| words::cannot_place(signed_in))
    }
}
