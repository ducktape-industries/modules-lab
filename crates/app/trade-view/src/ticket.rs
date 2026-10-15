//! The order ticket: the side, a limit or a market order, its price and
//! amount, what that comes to and what the account has free to pay it.
//! Nothing is sent that the program would refuse for a reason the ticket
//! can see: a price off the market's tick, an amount off the lot, an order
//! the free funds do not cover. A reader with no account, or no link,
//! reads the ticket and places nothing.
use serde::{Deserialize, Serialize};
use trade::{ASSETS, LOT, Level, Op, Side, UNIT};
use view::accesskit::Invalid;
use view::design::Orientation;
use view::prelude::*;
use view::{Div, FontWeight, Stateful, wire};

use crate::chart::sided;
use crate::{Trade, format, tabs};

/// The shares of what the free funds afford that one press fills in.
const SHARES: [u64; 4] = [25, 50, 75, 100];

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
}

/// What stands between the ticket and an order.
enum Problem {
    /// The price is not a number on the market's tick.
    Price,
    /// The amount is not a number of whole lots.
    Amount,
    /// The free funds are short of what the order needs.
    Funds(u64),
}

/// The ticket as read: the order it would place and what that takes.
struct Draft {
    side: Side,
    /// The price typed; none for a market order, or while there is none.
    limit: Option<u64>,
    amount: Option<u64>,
    /// What the order comes to in the quote: exact at a limit, estimated
    /// from the book on screen at the market.
    total: Option<u64>,
    /// The asset the order spends and what the account has free of it.
    spends: u8,
    free: u64,
    problem: Option<Problem>,
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

    fn draft(&self) -> Draft {
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
    fn affords(&self, draft: &Draft, share: u64) -> Option<u64> {
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

    /// The order the ticket holds, sent: once it is whole, covered, and
    /// the reader may trade.
    fn place(&mut self, cx: &mut Context<Self>) {
        let draft = self.draft();
        let priced = self.priced(&draft);
        let (Some(amount), None, true) = (draft.amount, &draft.problem, priced) else {
            return;
        };
        if self.ticket.placing {
            return;
        }
        let op = Op::Place {
            market: self.market as u8,
            side: draft.side,
            amount,
            limit: draft.limit,
        };
        self.submit(op, cx);
    }

    pub(crate) fn ticket(&self, cx: &mut Context<Self>, theme: &Theme) -> impl IntoElement + use<> {
        let theme = *theme;
        let ticket = &self.ticket;
        let (base, quote) = self.assets();
        let draft = self.draft();
        let trades = self.may_trade();
        let color = sided(draft.side, &theme);
        // Buy | Sell, the picked side filled in its colour
        let side = |id: &'static str, name: &'static str, sell: bool| {
            let picked = ticket.sell == sell;
            let fill = sided(if sell { Side::Sell } else { Side::Buy }, &theme);
            let segment = div()
                .id(id)
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .font_weight(FontWeight::MEDIUM)
                .map(|segment| match picked {
                    true => segment.bg(fill).text_color(theme.primary_foreground),
                    false => segment
                        .text_color(theme.muted)
                        .hover(move |style| style.text_color(theme.foreground)),
                })
                .aria_toggled(picked.into())
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| view.pick_side(sell, cx)))
                .child(name);
            design::item(segment, Role::RadioButton, picked)
        };
        let sides = design::composite("trade-side", Role::RadioGroup, "Side")
            .orientation(Orientation::Horizontal)
            .wrap()
            .active(usize::from(ticket.sell), 2)
            .on_move(
                cx.processor(|view: &mut Trade, side: usize, _, cx| view.pick_side(side == 1, cx)),
            )
            .build()
            .flex_none()
            .h(px(30.))
            .flex()
            .border_1()
            .border_color(theme.border_strong)
            .child(side("trade-buy", "Buy", false))
            .child(side("trade-sell", "Sell", true));
        let kinds = tabs(
            "trade-kind",
            "Order type",
            &theme,
            usize::from(ticket.market),
            [
                ("trade-limit".into(), "Limit".into(), None),
                ("trade-at-market".into(), "Market".into(), None),
            ],
            cx.processor(|view: &mut Trade, kind: usize, _, cx| {
                view.ticket.market = kind == 1;
                view.ticket.refusal.clear();
                cx.notify();
            }),
        )
        .h_full();
        let kinds = div()
            .flex_none()
            .h(design::size::CONTROL)
            .border_b_1()
            .border_color(theme.border)
            .child(kinds);
        // a field's text is the reader's: typing takes back what the
        // program said of the last order
        let typed = || {
            cx.listener(|view: &mut Trade, _: &wire::TextChange, _, _| view.ticket.refusal.clear())
        };
        let entered = || cx.listener(|view: &mut Trade, _: &(), _, cx| view.place(cx));
        // a field that holds no such number says so to who cannot see it
        let input =
            |id: &'static str, field: &TextField, name: &'static str, wrong: Option<String>| {
                let look = match (trades, &wrong) {
                    (false, _) => Look::Off,
                    (true, Some(_)) => Look::Bad,
                    (true, None) => Look::Plain,
                };
                let input = boxed(look, &theme, Input::new(id, field, name))
                    .disabled(!trades)
                    .on_change(typed())
                    .on_submit(entered());
                match wrong {
                    Some(wrong) => input.invalid(Invalid::True).description(wrong),
                    None => input,
                }
            };
        let wrong_price = matches!(draft.problem, Some(Problem::Price)).then(|| {
            let step = self.price(self.market().tick);
            format!("A price here is a number in steps of {step} {quote}.")
        });
        let wrong_amount = matches!(draft.problem, Some(Problem::Amount)).then(|| {
            let step = format::amount(LOT);
            format!("An amount is a number in steps of {step} {base}.")
        });
        let price = match ticket.market {
            true => shown("Best price in the book".into(), Look::Off, &theme).into_any_element(),
            false => {
                input("trade-price", &ticket.price, "Price", wrong_price.clone()).into_any_element()
            }
        };
        let amount = input(
            "trade-amount",
            &ticket.amount,
            "Amount",
            wrong_amount.clone(),
        );
        let mut column = div()
            .flex_none()
            .p(design::space::LG)
            .flex()
            .flex_col()
            .gap(design::space::MD)
            .child(sides)
            .child(kinds)
            .child(labelled(
                "Price",
                price,
                (!ticket.market).then_some(quote),
                &theme,
            ))
            .child(labelled("Amount", amount, Some(base), &theme));
        let named = match draft.side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        };
        let label = match draft.amount {
            Some(amount) => format!("{named} {} {base}", format::amount(amount)),
            None => format!("{named} {base}"),
        };
        if !trades {
            let needs = match self.session.account {
                None => "Placing an order needs an account. You are looking around without one",
                Some(_) => "Placing an order needs a link to the network. There is none now",
            };
            let note = format!("{needs}, so prices, the book and trades are all here to read.");
            return column
                .child(message(note, theme.surface, theme.muted))
                .child(go("trade-submit", label, None, &theme));
        }
        let shares = SHARES.map(|share| {
            let amount = self.affords(&draft, share);
            let picked = amount.is_some() && amount == draft.amount;
            div()
                .id(format!("trade-share-{share}"))
                .flex_1()
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .text_size(design::text::CAPTION)
                .map(|button| match picked {
                    true => button
                        .border_color(theme.foreground)
                        .text_color(theme.foreground),
                    false => button
                        .border_color(theme.border)
                        .text_color(theme.muted)
                        .hover(move |style| style.text_color(theme.foreground)),
                })
                .role(Role::Button)
                .aria_label(format!("{share}% of what is available"))
                .aria_toggled(picked.into())
                .focusable()
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    let amount = view.affords(&view.draft(), share);
                    view.ticket
                        .amount
                        .reset(amount.map(format::amount).unwrap_or_default());
                    view.ticket.refusal.clear();
                    cx.notify();
                }))
                .child(format!("{share}%"))
        });
        let short = matches!(draft.problem, Some(Problem::Funds(_)));
        let total = draft.total.map(format::total).unwrap_or_default();
        let total = match (ticket.market, short) {
            (_, true) => shown(total, Look::Bad, &theme),
            (true, _) => shown(total, Look::Off, &theme),
            (false, _) => shown(total, Look::Plain, &theme),
        };
        let total_name = if ticket.market {
            "Estimated total"
        } else {
            "Total"
        };
        column = column
            .child(div().flex().gap(design::space::XXS).children(shares))
            .child(labelled(total_name, total, Some(quote), &theme));
        let spent = ASSETS[draft.spends as usize].name;
        let funds = |value: u64| format!("{} {spent}", format::balance(draft.spends, value));
        let wrong = match &draft.problem {
            Some(Problem::Funds(needs)) => Some(format!(
                "This order needs {}. {} is available.",
                funds(*needs),
                funds(draft.free)
            )),
            Some(_) => wrong_price.or(wrong_amount),
            None => Some(ticket.refusal.clone()).filter(|refusal| !refusal.is_empty()),
        };
        // what is wrong takes the place of what is available
        column = match &wrong {
            Some(wrong) => column.child(message(wrong.clone(), theme.danger_soft, theme.danger)),
            None => column.child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .text_size(design::text::SECONDARY)
                    .text_color(theme.muted)
                    .child("Available")
                    .child(
                        design::mono(funds(draft.free))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground),
                    ),
            ),
        };
        let priced = self.priced(&draft);
        let ready = draft.amount.is_some() && draft.problem.is_none() && priced && !ticket.placing;
        let place: Press =
            Box::new(cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.place(cx)));
        column.child(go(
            "trade-submit",
            label,
            ready.then_some((color, place)),
            &theme,
        ))
    }

    fn pick_side(&mut self, sell: bool, cx: &mut Context<Self>) {
        self.ticket.sell = sell;
        self.ticket.refusal.clear();
        cx.notify();
    }
}

/// How a field's box is drawn.
#[derive(Clone, Copy)]
enum Look {
    Plain,
    /// Not the reader's to fill: a market order's price, a ticket that
    /// places nothing.
    Off,
    /// The figure in it is what is wrong.
    Bad,
}

/// A field's box around `element`: the control's height, figures in mono
/// that end at the unit beside them.
fn boxed<E: Styled>(look: Look, theme: &Theme, element: E) -> E {
    let (edge, ground, ink) = match look {
        Look::Plain => (theme.border_strong, theme.background, theme.foreground),
        Look::Off => (theme.border, theme.surface, theme.muted),
        Look::Bad => (theme.danger, theme.background, theme.foreground),
    };
    element
        .h(design::size::CONTROL)
        .w_full()
        .pl(design::space::SM)
        // the unit sits over the box's right end
        .pr(px(46.))
        .border_1()
        .border_color(edge)
        .bg(ground)
        .text_color(ink)
        .font_family(design::fonts::FAMILY_MONO)
        .text_size(design::text::MONO)
        .text_right()
}

/// A figure the ticket works out, in a field's box: not the reader's to
/// type.
fn shown(text: String, look: Look, theme: &Theme) -> Div {
    boxed(look, theme, div())
        .flex()
        .items_center()
        .justify_end()
        .whitespace_nowrap()
        .child(text)
}

/// A field under its name, the unit of its figure at its right end.
fn labelled(
    name: &'static str,
    field: impl IntoElement,
    unit: Option<&'static str>,
    theme: &Theme,
) -> Div {
    let unit = unit.map(|unit| {
        div()
            .absolute()
            .right(design::space::SM)
            .top_0()
            .h_full()
            .w(px(30.))
            .flex()
            .items_center()
            .child(unit)
    });
    div()
        .flex()
        .flex_col()
        .gap(design::space::XXS)
        .text_size(design::text::CAPTION)
        .text_color(theme.muted)
        .child(name)
        .child(div().relative().child(field).children(unit))
}

/// A few words in the ticket, on their own ground: a note, or what is
/// wrong with the order.
fn message(text: String, ground: Hsla, ink: Hsla) -> Div {
    div()
        .px(design::space::SM)
        .py(px(7.))
        .bg(ground)
        .text_color(ink)
        .text_size(design::text::SECONDARY)
        .child(text)
}

/// What a press on the ticket's button does.
type Press = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// The ticket's button: filled in its side's colour with a press to hear,
/// greyed out and saying so with none.
fn go(
    id: &'static str,
    label: String,
    press: Option<(Hsla, Press)>,
    theme: &Theme,
) -> Stateful<Div> {
    let button = div()
        .id(id)
        .flex_none()
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .font_weight(FontWeight::MEDIUM)
        .role(Role::Button)
        .child(label);
    match press {
        Some((color, press)) => {
            design::focus_shown_on_ink(button.bg(color).text_color(theme.primary_foreground), theme)
                .focusable()
                .on_click(press)
        }
        None => button
            .bg(theme.surface_raised)
            .text_color(theme.muted)
            .aria_disabled(true),
    }
}
