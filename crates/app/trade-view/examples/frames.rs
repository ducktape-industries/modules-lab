//! Writes the frames the view draws as fixtures for the app's
//! `--render-tree` (app `dev/screens`): `frames <dir>` leaves
//! `<dir>/manifest.json` and one `<name>.json` per screen. The app's real
//! renderer then draws them with no node behind it: the view runs here on
//! the tests' fake host and sample market, driven to each screen the way a
//! person would get to it.
use ducktape_view_guest::testing::TestAppContext;
use trade_view::Trade;

#[path = "../src/tests/sample.rs"]
mod sample;

use sample::{READER, World};

/// The two boards' panes.
const WIDE: (f32, f32) = (1280., 760.);
const PANE: (f32, f32) = (768., 500.);

/// One screen: its name, the pane it is drawn in and its theme, the market
/// behind it and the account seated, and what the reader did to get there.
struct Screen {
    name: &'static str,
    size: (f32, f32),
    dark: bool,
    world: fn() -> World,
    account: Option<u64>,
    drive: fn(&mut TestAppContext),
    /// The capture puts the pointer at 60% of the width, 165 px over the
    /// bottom, before it takes the picture.
    pointer: bool,
}

/// A pane the capture's pointer is on a candle in: the pane layout, wide
/// enough that 60% of it is left of the price axis. There it is on the
/// 76th candle, five pixels over the time axis.
const POINTED: (f32, f32) = (1000., 500.);

/// The boards' ticket: 0.1 BTC at 63,200.0, a quarter of what is free.
fn limit_buy(cx: &mut TestAppContext) {
    cx.simulate_input("trade-price", "63,200.0");
    cx.simulate_click("trade-share-25");
}

fn screens() -> Vec<Screen> {
    let board = |name, size, dark, drive| Screen {
        name,
        size,
        dark,
        world: sample::board,
        account: Some(READER),
        drive,
        pointer: false,
    };
    // the wide board: the pointer on the candle of Oct 6 08:00
    let hovered = |cx: &mut TestAppContext| {
        limit_buy(cx);
        cx.simulate_hover("candle-57", true);
    };
    vec![
        board("trade-wide-light", WIDE, false, hovered),
        board("trade-wide-dark", WIDE, true, hovered),
        board("trade-pane", PANE, false, |cx| {
            limit_buy(cx);
            cx.simulate_click("trade-balances");
        }),
        board("trade-market-sell", WIDE, false, |cx| {
            cx.simulate_click("trade-sell");
            cx.simulate_click("trade-at-market");
            cx.simulate_click("trade-share-50");
        }),
        board("trade-not-enough", WIDE, false, |cx| {
            cx.simulate_input("trade-price", "63,200.0");
            cx.simulate_input("trade-amount", "0.5000");
        }),
        Screen {
            account: None,
            ..board("trade-no-account", WIDE, false, |_| {})
        },
        board("trade-markets", WIDE, false, |cx| {
            cx.simulate_click("trade-market")
        }),
        Screen {
            world: sample::empty,
            ..board("trade-empty", WIDE, false, |_| {})
        },
        // the line on the candle under the pointer is the host's to paint:
        // the view only says which candle its readout tells
        Screen {
            pointer: true,
            ..board("trade-hover", POINTED, false, |cx| {
                cx.simulate_hover("candle-75", true)
            })
        },
    ]
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: frames <dir>"));
    std::fs::create_dir_all(&dir).expect("the fixtures directory");
    let mut manifest = Vec::new();
    for screen in screens() {
        let (mut cx, ..) = sample::opened::<Trade>((screen.world)(), screen.size, screen.account);
        cx.simulate_theme(screen.dark);
        (screen.drive)(&mut cx);
        cx.run_until_parked();
        let frame = serde_json::to_vec(&cx.whole_frame()).expect("a frame as JSON");
        std::fs::write(dir.join(format!("{}.json", screen.name)), frame).expect("the frame's file");
        manifest.push(serde_json::json!({
            "name": screen.name,
            "width": screen.size.0 as u32,
            "height": screen.size.1 as u32,
            "theme": if screen.dark { "dark" } else { "light" },
            "hover": screen.pointer,
        }));
    }
    let manifest = serde_json::to_vec_pretty(&manifest).expect("the manifest as JSON");
    std::fs::write(dir.join("manifest.json"), manifest).expect("the manifest's file");
}
