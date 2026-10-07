//! What the view follows and asks: the session, the program's blocks, each
//! read of it, and the ops it submits. A read lands in its slot of the
//! view; a block reads again only the slots it wrote to, as the program
//! declares them.
use std::rc::Rc;

use ducktape_view_guest::Paged;
use ducktape_view_guest::host::Page;
use ducktape_view_guest::prelude::*;
use store::PageRequest;
use trade::{MAX_ROWS, Op, Order, ask};

use crate::Trade;
use crate::chart::Candles;

/// The prices a side the book is asked for.
pub(crate) const DEPTH: u32 = 15;
/// The trades the tape is asked for.
const TAPE: u32 = 32;
/// The rows a list asks for at a time.
const PAGE: u64 = 64;

impl Trade {
    /// Follows the session and the program's blocks for as long as the
    /// view runs.
    pub(crate) fn watch(&mut self, cx: &mut Context<Self>) {
        cx.follow::<HostSession>((), |view, session, cx| match session {
            Ok(next) => view.session_changed(next, cx),
            Err(refusal) => cx.log_refused("the session", &refusal),
        })
        .detach();
        // a block re-reads what it wrote to, as the program declares it
        cx.follow::<Changes<::trade::Trade>>((), |view, change, cx| match change {
            Ok(change) => view.changed(change.as_ref(), cx),
            Err(refusal) => cx.log_refused("trade's blocks", &refusal),
        })
        .detach();
    }

    fn session_changed(&mut self, next: Session, cx: &mut Context<Self>) {
        if next == self.session {
            return;
        }
        cx.notify();
        let prev = std::mem::replace(&mut self.session, next);
        let owner_changed = self.session.account != prev.account;
        if owner_changed {
            // the rows on screen are another account's
            (self.balances, self.open) = (Loadable::Idle, Loadable::Idle);
            (self.history, self.fills) = (None, None);
            self.refusal.clear();
            self.ticket.refusal.clear();
        }
        // a link that came back reads everything; else another account
        // reads what is an account's
        if self.session.connected && !prev.connected {
            self.refresh(cx);
        } else if owner_changed {
            self.read_owned(None, cx);
        }
    }

    /// Whether the reader may place and cancel: an op is an account's, and
    /// goes nowhere without a link.
    pub(crate) fn may_trade(&self) -> bool {
        self.session.connected && self.session.account.is_some()
    }

    /// Everything on screen, read again: after this reader's own op, a
    /// link that came back, or a mount.
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.changed(None, cx);
    }

    /// A block of the program landed: each read is asked again where the
    /// block wrote to a table it reads; `None`, a reopened link, asks all
    /// of them again.
    fn changed(&mut self, change: Option<&Change>, cx: &mut Context<Self>) {
        let market = self.market as u8;
        if touched(change, &ask::Stats) {
            self.read_stats(cx);
        }
        if touched(change, &self.candles_page(None)) {
            self.reread_candles(cx);
        }
        if touched(change, &self.book_ask()) {
            self.read_book(cx);
        }
        let limit = TAPE;
        if touched(change, &ask::Trades { market, limit }) {
            self.read_trades(cx);
        }
        self.read_owned(change, cx);
    }

    /// What is an account's, where `change` wrote to it: the balances, the
    /// open orders, and the two lists that page, which are built on their
    /// first read. With no account there is none of it.
    fn read_owned(&mut self, change: Option<&Change>, cx: &mut Context<Self>) {
        let Some(owner) = self.session.account else {
            return;
        };
        let host = cx.host();
        if touched(change, &ask::Balances { owner }) {
            let balances = host.query(ask::Balances { owner });
            cx.load(self, balances, |view| &mut view.balances);
        }
        let (open, page) = (false, PageRequest::default());
        if touched(change, &ask::Orders { owner, open, page }) {
            cx.load(self, open_orders(host.clone(), owner), |view| {
                &mut view.open
            });
            match &self.history {
                Some(history) => history.update(cx, |history, cx| history.reread(cx)),
                None => {
                    let host = host.clone();
                    let orders = move |after| {
                        let page = PageRequest::resume(after, PAGE);
                        paged(host.query(ask::Orders { owner, open, page }))
                    };
                    self.history = Some(cx.new(|cx| Paged::new(orders, cx)));
                }
            }
        }
        let page = PageRequest::default();
        if touched(change, &ask::Fills { owner, page }) {
            match &self.fills {
                Some(fills) => fills.update(cx, |fills, cx| fills.reread(cx)),
                None => {
                    let fills = move |after| {
                        let page = PageRequest::resume(after, PAGE);
                        paged(host.query(ask::Fills { owner, page }))
                    };
                    self.fills = Some(cx.new(|cx| Paged::new(fills, cx)));
                }
            }
        }
    }

    /// The market on screen changed: its chart, book and tape are read in
    /// place of the last market's.
    pub(crate) fn read_market(&mut self, cx: &mut Context<Self>) {
        (self.book, self.trades) = (Loadable::Idle, Loadable::Idle);
        self.read_candles(cx);
        self.read_book(cx);
        self.read_trades(cx);
    }

    fn read_stats(&mut self, cx: &mut Context<Self>) {
        cx.load(self, cx.host().query(ask::Stats), |view| &mut view.stats);
    }

    fn book_ask(&self) -> ask::Book {
        ask::Book {
            market: self.market as u8,
            step: crate::book::STEPS[self.step],
            depth: DEPTH,
        }
    }

    pub(crate) fn read_book(&mut self, cx: &mut Context<Self>) {
        cx.load(self, cx.host().query(self.book_ask()), |view| {
            &mut view.book
        });
    }

    fn read_trades(&mut self, cx: &mut Context<Self>) {
        let (market, limit) = (self.market as u8, TAPE);
        let trades = cx.host().query(ask::Trades { market, limit });
        cx.load(self, trades, |view| &mut view.trades);
    }

    /// One page of the candles on screen: the newest, or the ones before
    /// bucket `before`.
    fn candles_page(&self, before: Option<u64>) -> ask::Candles {
        ask::Candles {
            market: self.market as u8,
            span: self.chart.span as u8,
            before,
            limit: MAX_ROWS,
        }
    }

    /// The newest candles of the market and span on screen, in place of
    /// the ones shown: the chart starts over at its live edge.
    pub(crate) fn read_candles(&mut self, cx: &mut Context<Self>) {
        let chart = &mut self.chart;
        (chart.back, chart.hovered) = (0, None);
        (chart.rereading, chart.older) = (None, None);
        let page = cx.host().query(self.candles_page(None));
        self.chart.candles = Loadable::Loading(cx.land(page, |view, page, cx| {
            view.chart.candles = page.map(Candles::newest).into();
            cx.notify();
        }));
    }

    /// The newest page again, laid over the candles shown, which stay
    /// until it lands: the ones paged back to are older than any trade
    /// can move.
    fn reread_candles(&mut self, cx: &mut Context<Self>) {
        if self.chart.candles.ready().is_none() {
            return self.read_candles(cx);
        }
        let page = cx.host().query(self.candles_page(None));
        self.chart.rereading = Some(cx.land(page, |view, page, cx| {
            match (page, view.chart.candles.ready_mut()) {
                (Ok(page), Some(candles)) => {
                    let was = candles.tip();
                    if candles.lay(page) {
                        // a reader looking back stays on the candles they
                        // were on when a newer one opens
                        let opened = candles.tip().zip(was).map_or(0, |(now, was)| now - was);
                        if view.chart.back > 0 {
                            view.chart.back += opened;
                        }
                        cx.notify();
                    }
                }
                (Err(refusal), _) => cx.log_refused("the candles", &refusal),
                (Ok(_), None) => {}
            }
        }));
    }

    /// The page before the oldest candle held, once: the chart was panned
    /// back to it.
    pub(crate) fn read_older(&mut self, cx: &mut Context<Self>) {
        let held = self.chart.candles.ready().filter(|candles| !candles.whole);
        let Some(oldest) = held.and_then(Candles::oldest) else {
            return;
        };
        if self.chart.older.is_some() {
            return;
        }
        let page = cx.host().query(self.candles_page(Some(oldest)));
        self.chart.older = Some(cx.land(page, |view, page, cx| {
            view.chart.older = None;
            match (page, view.chart.candles.ready_mut()) {
                (Ok(page), Some(candles)) => {
                    candles.older(page);
                    cx.notify();
                }
                (Err(refusal), _) => cx.log_refused("older candles", &refusal),
                (Ok(_), None) => {}
            }
        }));
    }

    /// Another span: its candles are read in place of the last span's.
    pub(crate) fn pick_span(&mut self, span: usize, cx: &mut Context<Self>) {
        if span != self.chart.span {
            self.chart.span = span;
            self.read_candles(cx);
            cx.notify();
        }
    }

    /// One op to the program, from an account with a link: what it
    /// changed is read again. A refused order says why in the ticket, any
    /// other op over the panel.
    pub(crate) fn submit(&mut self, op: Op, cx: &mut Context<Self>) {
        if !self.may_trade() {
            return;
        }
        let placing = matches!(op, Op::Place { .. });
        let refusal: fn(&mut Trade) -> &mut String = match placing {
            true => |view| &mut view.ticket.refusal,
            false => |view| &mut view.refusal,
        };
        refusal(self).clear();
        self.ticket.placing |= placing;
        let submit = cx.host().ask::<Submit<::trade::Trade>>(op);
        cx.land(submit, move |view, result, cx| {
            cx.notify();
            if placing {
                view.ticket.placing = false;
            }
            match result {
                Ok(_) => view.refresh(cx),
                Err(refused) => *refusal(view) = refused.message,
            }
        })
        .detach();
    }
}

/// Whether `change` can have moved the answer to `read`; no change in
/// hand, a reopened link, moves every answer.
fn touched(change: Option<&Change>, read: &dyn program::Reads) -> bool {
    change.is_none_or(|change| change.touches::<::trade::Trade, _>(read))
}

/// A page of a list as [`Paged`] reads one: its rows and the cursor of the
/// page after.
async fn paged<T>(
    page: impl Future<Output = Result<store::PageResponse<T>, Error>>,
) -> Result<Page<T>, Error> {
    page.await.map(|page| (page.items, page.next))
}

/// Every open order of `owner`, newest first.
async fn open_orders(host: Host, owner: u64) -> Result<Rc<[Order]>, Error> {
    let orders = host.query_all(|after| ask::Orders {
        owner,
        open: true,
        page: PageRequest::resume(after, PAGE),
    });
    orders.await.map(Rc::from)
}
