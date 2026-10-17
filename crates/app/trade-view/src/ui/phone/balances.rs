//! The reader's balances on the phone (SPEC §2.6, board 26): each asset
//! with what is free of it and what its resting orders hold, then Get
//! practice funds and what a grant does; an account with nothing says so
//! over the same button.
use trade::{ASSETS, Op};
use view::AnyElement;
use view::design::space;
use view::prelude::*;
use view::{Div, FontWeight, Stateful};

use super::parts::{quiet, refusal, refused, scroller};
use super::{LADDER, back};
use crate::{Trade, format, words};

pub(super) fn page(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> AnyElement {
    let body = match (&view.session.account, &view.balances, view.balances.ready()) {
        (None, ..) => quiet("trade-signed-out", words::SIGNED_OUT, theme).into_any_element(),
        (_, Loadable::Failed(failed), _) => {
            let retry = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.refresh(cx));
            refused("trade-balances", failed, theme, retry).into_any_element()
        }
        (_, _, Some(balances)) if balances.iter().all(|it| it.free == 0 && it.held == 0) => {
            let empty = design::empty_state(
                "trade-no-funds",
                "wallet",
                words::NO_FUNDS,
                words::FUNDS_TO_ORDER,
                LADDER,
                theme,
            );
            let fund = fund(view, cx, theme).small();
            empty
                .child(div().mt(space::SM).child(fund))
                .into_any_element()
        }
        (_, _, Some(balances)) => {
            let rows = balances.iter().map(|balance| asset_row(balance, theme));
            div()
                .flex()
                .flex_col()
                .child(design::section_head(
                    "trade-assets-head",
                    words::ASSETS,
                    LADDER,
                    theme,
                ))
                .children(rows)
                .child(grant(view, cx, theme))
                .into_any_element()
        }
        _ => div().into_any_element(),
    };
    design::page(
        "trade-balances",
        words::BALANCES,
        back(view, cx),
        LADDER,
        theme,
    )
    .caption(words::PRACTICE_FUNDS)
    .child(
        scroller("trade-balances-page")
            .children(refusal(view.refused(|op| *op == Op::Fund), theme))
            .child(body),
    )
    .into_any_element()
}

/// An asset's row: its name over what its resting orders hold, and what
/// is free of it.
fn asset_row(balance: &trade::Balance, theme: &Theme) -> Stateful<Div> {
    let name = Trade::asset_name(balance.asset);
    let held = format::balance(balance.asset, balance.held);
    let free = div()
        .flex_none()
        .flex()
        .flex_col()
        .items_end()
        .child(
            div()
                .font_family(design::fonts::FAMILY_MONO)
                .text_type(LADDER.body)
                .font_weight(FontWeight::MEDIUM)
                .child(format::balance(balance.asset, balance.free)),
        )
        .child(
            div()
                .text_type(LADDER.caption)
                .text_color(theme.muted)
                .child(words::FREE),
        );
    design::row(("trade-asset", balance.asset as usize), LADDER, theme)
        .min_h(LADDER.row_tall)
        .child(
            design::row_words(design::row_title(name, LADDER, theme)).child(design::row_caption(
                words::in_orders(&held),
                LADDER,
                theme,
            )),
        )
        .child(free)
}

/// Get practice funds, the whole width, and what a grant does.
fn grant(view: &Trade, cx: &mut Context<Trade>, theme: &Theme) -> Div {
    let grants = ASSETS.map(|asset| format!("{} {}", format::trimmed(asset.grant), asset.name));
    div()
        .p(space::BLOCK)
        .flex()
        .flex_col()
        .gap(space::LG)
        .child(fund(view, cx, theme))
        .child(
            div()
                .text_type(LADDER.caption)
                .text_color(theme.muted)
                .child(words::grants(&grants)),
        )
}

/// Get practice funds: off while the reader may not, waiting while the
/// grant is on its way.
fn fund(
    view: &Trade,
    cx: &mut Context<Trade>,
    theme: &Theme,
) -> design::Button<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static> {
    let fund = cx.listener(|view: &mut Trade, _: &ClickEvent, _, cx| view.submit(Op::Fund, cx));
    design::button("trade-fund", words::GET_FUNDS, LADDER, theme, fund)
        .enabled(view.may_trade())
        .busy(view.is_sending(&Op::Fund))
}
