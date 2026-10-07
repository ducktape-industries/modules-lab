//! The chart: what its canvas carries, the candle under the pointer, and
//! the wheel, a drag and the arrow keys.
use ducktape_view_guest::wire::{CanvasCommand, CanvasShape, Node};
use gpui::{MouseButton, MouseDownEvent, MouseMoveEvent, ScrollDelta, ScrollWheelEvent, point, px};

use super::*;

/// The newest 1h bucket of the sample.
const HOUR: u64 = sample::NOW / 3_600_000;

/// What the chart's canvas carries.
fn canvas(cx: &TestAppContext) -> &[CanvasCommand] {
    let plot = cx.node("trade-plot").children().iter();
    let mut canvases = plot.filter_map(|node| match node {
        Node::Canvas { commands, .. } => Some(commands.as_slice()),
        _ => None,
    });
    canvases.next().expect("a canvas")
}

/// The rectangles the canvas carries whose left edge is in `[from, to)`:
/// each one's top and its size.
fn quads(cx: &TestAppContext, from: f32, to: f32) -> Vec<(f32, [f32; 2])> {
    let shapes = canvas(cx).iter().filter_map(|command| match command {
        CanvasCommand::Draw {
            shape: CanvasShape::Rectangle { position, size, .. },
            ..
        } => Some((*position, *size)),
        _ => None,
    });
    shapes
        .filter(|(position, _)| position[0] >= from && position[0] < to)
        .map(|(position, size)| (position[1], size))
        .collect()
}

/// The lines the canvas carries: the averages', and the closes' when the
/// chart is a line.
fn lines(cx: &TestAppContext) -> usize {
    let lines = canvas(cx).iter().filter(|command| {
        matches!(
            command,
            CanvasCommand::Draw {
                shape: CanvasShape::Line { .. },
                ..
            }
        )
    });
    lines.count()
}

/// The candles on screen: each has a slot that hears the pointer.
pub(super) fn slots(cx: &TestAppContext) -> usize {
    let plot = cx.node("trade-plot").children().iter();
    plot.filter(|node| node.key().is_some_and(|key| key.starts_with("candle-")))
        .count()
}

/// Every text laid over the plot.
fn labels(cx: &TestAppContext) -> Vec<String> {
    fn collect(node: &Node, out: &mut Vec<String>) {
        match node {
            Node::Text(text) => out.push(text.content.clone()),
            _ => node.children().iter().for_each(|child| collect(child, out)),
        }
    }
    let mut labels = Vec::new();
    collect(cx.node("trade-plot"), &mut labels);
    labels
}

/// What the readout says: `O`, the open, and so on to the volume.
fn readout(cx: &TestAppContext) -> Vec<String> {
    let texts = labels(cx);
    let open = texts
        .iter()
        .position(|text| text == "O")
        .expect("a readout");
    texts[open..open + 10].to_vec()
}

/// The wheel over the chart, `lines` away from the reader.
pub(super) fn wheel(cx: &mut TestAppContext, lines: f32) {
    cx.simulate_scroll_wheel(
        "trade-plot",
        ScrollWheelEvent {
            delta: ScrollDelta::Lines(point(0., lines)),
            ..Default::default()
        },
    );
}

fn key(cx: &mut TestAppContext, key: &str) {
    cx.simulate_focus("trade-plot");
    cx.simulate_key_down("trade-plot", key);
}

fn back(view: &Entity<Trade>) -> u64 {
    view.read(|trade| trade.chart.back)
}

/// How often the view asked for the candles before bucket `before`.
fn older(cx: &TestAppContext, before: u64) -> usize {
    let asked = asked(cx).into_iter();
    asked
        .filter(|query| matches!(query, Query::Candles { before: Some(at), .. } if *at == before))
        .count()
}

#[test]
fn the_chart_draws_the_candles_the_averages_and_the_axes() {
    let (cx, ..) = wide();
    // 704 px of plot at 8 px a candle
    assert_eq!(slots(&cx), 88);
    // a wick, a body and a volume bar in each candle's slot
    for slot in [1, 40, 87] {
        let (from, to) = (slot as f32 * 8., slot as f32 * 8. + 8.);
        assert_eq!(quads(&cx, from, to).len(), 3, "slot {slot}");
    }
    // two averages, a line from each candle to the next
    assert_eq!(lines(&cx), 2 * 87);
    // the newest candle is the one told, and the last price is on its tag
    assert_eq!(readout(&cx)[6..8], ["C", "63,589.1"]);
    let labels = labels(&cx);
    let told = labels
        .iter()
        .position(|text| text == "O")
        .expect("a readout");
    assert!(
        labels[..told].iter().any(|text| text == "63,589.1"),
        "{labels:?}"
    );
    // a time every twelve hours, a day where one starts
    shows(&cx, &["Oct 5", "12:00", "Oct 6", "Oct 7", "Oct 7 14:00"]);
    // round prices beside the rules
    shows(&cx, &["63,000", "66,000"]);
    let named = cx.interactivity("trade-plot").aria.label.clone();
    assert_eq!(
        named.as_deref(),
        Some("BTC/USD 1h candles, 88 on screen, last 63,589.1")
    );
}

#[test]
fn the_switches_take_the_averages_the_volume_and_the_candles_off() {
    let (mut cx, ..) = wide();
    let slot = |cx: &TestAppContext| quads(cx, 320., 328.).len();
    cx.simulate_click("trade-ma7");
    assert_eq!(lines(&cx), 87);
    assert!(!cx.has_text("MA7"), "the readout drops the average");
    cx.simulate_click("trade-ma25");
    assert_eq!(lines(&cx), 0);
    cx.simulate_click("trade-volume");
    assert_eq!(slot(&cx), 2, "a wick and a body");
    cx.simulate_click("Line");
    assert_eq!(slot(&cx), 0);
    assert_eq!(lines(&cx), 87, "the closes, joined");
    cx.simulate_click("Candles");
    cx.simulate_click("trade-volume");
    assert_eq!(slot(&cx), 3);
}

/// A bucket nothing traded in has no row: it is drawn flat at the close
/// before it, with no volume, and nothing is drawn before the first trade.
#[test]
fn a_bucket_with_no_trade_is_drawn_flat_at_the_close_before() {
    let mut world = sample::empty();
    let rows = vec![
        sample::hour(4, 1000, 1100, 900, 1050, 3),
        sample::hour(3, 1050, 1200, 1000, 1180, 2),
        sample::hour(0, 1180, 1250, 1170, 1210, 4),
    ];
    world.candles.insert((0, 3), rows);
    let (mut cx, ..) = opened(world, WIDE, Some(READER));
    assert_eq!(slots(&cx), 5, "from the first trade to the newest");
    assert!(cx.find("candle-82").is_none());
    // the two hours between: a wick and a body one pixel tall, at one height
    let close = quads(&cx, 84. * 8., 85. * 8.);
    for gap in [85, 86] {
        let flat = quads(&cx, gap as f32 * 8., gap as f32 * 8. + 8.);
        assert_eq!(flat.len(), 2, "no volume: {flat:?}");
        assert!(flat.iter().all(|(_, size)| size[1] == 1.), "{flat:?}");
        assert_eq!(flat[0].0, flat[1].0);
        // where the candle before closed: its body's top, as it rose
        assert_eq!(flat[1].0, close[1].0);
    }
    cx.simulate_hover("candle-86", true);
    assert_eq!(
        readout(&cx),
        [
            "O", "118.0", "H", "118.0", "L", "118.0", "C", "118.0", "V", "0.0000"
        ]
    );
    shows(&cx, &["Oct 7 13:00"]);
}

#[test]
fn the_candle_under_the_pointer_is_told_and_the_newest_when_none_is() {
    let (mut cx, ..) = wide();
    let newest = readout(&cx);
    cx.simulate_hover("candle-57", true);
    assert_eq!(
        readout(&cx),
        [
            "O", "64,589.7", "H", "64,780.6", "L", "64,553.7", "C", "64,553.7", "V", "232"
        ]
    );
    // its time: in the readout, and on the axis under it
    let stamps = labels(&cx)
        .iter()
        .filter(|text| *text == "Oct 6 08:00")
        .count();
    assert_eq!(stamps, 2, "{:?}", labels(&cx));
    // the pointer is in the next slot before it has left this one
    cx.simulate_hover("candle-58", true);
    let next = readout(&cx);
    cx.simulate_hover("candle-57", false);
    assert_eq!(readout(&cx), next);
    cx.simulate_hover("candle-58", false);
    assert_eq!(readout(&cx), newest);
}

/// The line on the hovered candle is the host's to paint: its slot's
/// hover style, carried once. So a hover costs the view the readout and the
/// time under the candle, and none of the chart.
#[test]
fn a_hover_sends_the_readout_and_none_of_the_chart() {
    let (mut cx, ..) = wide();
    let slot = cx.interactivity("candle-57");
    let group = slot.group.clone().expect("the slot is a group");
    let Some(line) = cx.node("candle-57").children().first() else {
        panic!("the slot holds its line");
    };
    let style = line
        .interactivity()
        .and_then(|line| line.group_hover.as_ref());
    assert_eq!(style.map(|style| &style.group), Some(&group));
    let whole = cx.frame_bytes();
    let drawn = canvas(&cx).to_vec();
    let renders = cx.renders();
    cx.simulate_hover("candle-57", true);
    let sent: usize = cx.reports().iter().map(|tick| tick.bytes).sum();
    assert_eq!(canvas(&cx), drawn, "the canvas is as it was");
    assert!(
        sent < 512,
        "{sent} B crossed for a hover, of a {whole} B frame"
    );
    cx.simulate_hover("candle-58", true);
    cx.simulate_hover("candle-57", false);
    assert_eq!(
        cx.renders(),
        renders + 2,
        "leaving for the next slot draws nothing"
    );
}

#[test]
fn the_wheel_zooms_between_the_widest_and_the_narrowest_candle() {
    let (mut cx, view, _) = wide();
    wheel(&mut cx, 1.);
    assert_eq!(slots(&cx), 79);
    wheel(&mut cx, -1.);
    assert_eq!(slots(&cx), 88);
    for _ in 0..40 {
        wheel(&mut cx, 1.);
    }
    assert_eq!(slots(&cx), 17, "40 px a candle");
    for _ in 0..60 {
        wheel(&mut cx, -1.);
    }
    assert_eq!(slots(&cx), 234, "3 px a candle");
    assert_eq!(back(&view), 0, "about the right edge");
}

/// The arrow keys, for a reader without a pointer: ← → pan a tenth of the
/// candles on screen, ↑ ↓ zoom.
#[test]
fn the_arrow_keys_pan_and_zoom() {
    let (mut cx, view, _) = wide();
    let live = readout(&cx);
    key(&mut cx, "left");
    assert_eq!(back(&view), 8);
    assert_ne!(readout(&cx), live, "eight candles older");
    key(&mut cx, "right");
    key(&mut cx, "right");
    assert_eq!(back(&view), 0, "no newer than the newest candle");
    assert_eq!(readout(&cx), live);
    key(&mut cx, "up");
    assert_eq!(slots(&cx), 70, "10 px a candle");
    key(&mut cx, "down");
    assert_eq!(slots(&cx), 88);
}

#[test]
fn a_drag_pans_by_whole_candles() {
    let (mut cx, view, _) = wide();
    let at = |x: f32| point(px(x), px(200.));
    let moved = |x: f32, pressed: Option<MouseButton>| MouseMoveEvent {
        position: at(x),
        pressed_button: pressed,
        ..Default::default()
    };
    cx.simulate_mouse_down(
        "trade-plot",
        MouseDownEvent {
            button: MouseButton::Left,
            position: at(300.),
            ..Default::default()
        },
    );
    // to the right: the candles follow the pointer, older ones come in
    cx.simulate_mouse_move("trade-plot", moved(383., Some(MouseButton::Left)));
    assert_eq!(back(&view), 10);
    // what was left of the distance adds up
    cx.simulate_mouse_move("trade-plot", moved(388., Some(MouseButton::Left)));
    assert_eq!(back(&view), 11);
    cx.simulate_mouse_move("trade-plot", moved(340., Some(MouseButton::Left)));
    assert_eq!(back(&view), 5);
    // the button came up outside the window: the next move lets go
    cx.simulate_mouse_move("trade-plot", moved(600., None));
    cx.simulate_mouse_move("trade-plot", moved(700., Some(MouseButton::Left)));
    assert_eq!(back(&view), 5);
}

/// Panned back to the oldest candle held, the chart asks for the page
/// before it, once, and no further once the program has no older candle.
#[test]
fn panned_to_the_oldest_candle_the_chart_asks_for_the_page_before() {
    let (mut cx, view, _) = wide();
    // the first page is the newest 256 of the sample's 300 hours
    let oldest = HOUR - 255;
    for _ in 0..20 {
        key(&mut cx, "left");
    }
    assert_eq!(back(&view), 160);
    assert_eq!(older(&cx, oldest), 0, "eight candles to go");
    key(&mut cx, "left");
    assert_eq!(back(&view), 168, "the oldest candle is the first on screen");
    assert_eq!(older(&cx, oldest), 1);
    // the 44 before them landed: the chart pans on into them, to the end
    for _ in 0..10 {
        key(&mut cx, "left");
    }
    assert_eq!(back(&view), 212);
    assert_eq!(older(&cx, oldest), 1);
    assert_eq!(older(&cx, HOUR - 299), 0, "a short page is the last");
    // the newest of the 88 that start at the program's first candle
    shows(&cx, &["Sep 28 18:00"]);
}

#[test]
fn another_span_starts_over_at_its_newest_candle() {
    let (mut cx, view, world) = wide();
    key(&mut cx, "left");
    cx.simulate_hover("candle-3", true);
    let day = sample::NOW / 86_400_000;
    let rows = (0..3).rev().map(|back| trade::Candle {
        bucket: day - back,
        open: 63_000 * UNIT,
        high: 64_000 * UNIT,
        low: 62_000 * UNIT,
        close: 63_500 * UNIT,
        volume: 5_000 * UNIT,
    });
    world.borrow_mut().candles.insert((0, 5), rows.collect());
    cx.simulate_click("1D");
    assert_eq!(back(&view), 0);
    assert_eq!(slots(&cx), 3);
    // a day's candle is told by its day
    shows(&cx, &["Oct 7", "5,000"]);
    assert_eq!(readout(&cx)[..2], ["O", "63,000.0"]);
}

/// A block's candle lands in the chart on screen: the newest page is laid
/// over the candles held, and a reader looking back stays on the candles
/// they were on.
#[test]
fn a_block_that_traded_moves_the_last_candle_and_not_the_reader() {
    let (mut cx, view, world) = wide();
    let changes = cx.host().stream::<Changes<trade::Trade>>();
    let traded = |world: &Shared, bucket: u64, close: u64| {
        let mut world = world.borrow_mut();
        let candles = world.candles.get_mut(&(0, 3)).expect("the 1h candles");
        match candles.last_mut().filter(|last| last.bucket == bucket) {
            Some(last) => last.close = close,
            None => candles.push(trade::Candle {
                bucket,
                open: close,
                high: close,
                low: close,
                close,
                volume: UNIT,
            }),
        }
        world.stats[0].last = close;
        Some(Change {
            height: 2,
            keys: vec![state::CANDLES.key(&(0u8, 3u8, bucket))],
        })
    };
    key(&mut cx, "left");
    let looked_at = readout(&cx);
    changes.send(traded(&world, HOUR, 64_000 * UNIT));
    cx.run_until_parked();
    // the bar's last price and the chart's tag both say it
    let said = cx.texts().iter().filter(|text| *text == "64,000.0").count();
    assert_eq!(said, 2, "{:?}", cx.texts());
    assert_eq!((back(&view), readout(&cx)), (8, looked_at.clone()));
    // the next hour opens: the candles looked at are one further back
    changes.send(traded(&world, HOUR + 1, 64_100 * UNIT));
    cx.run_until_parked();
    shows(&cx, &["64,100.0"]);
    assert_eq!((back(&view), readout(&cx)), (9, looked_at));
    // at the live edge the chart follows the newest candle
    key(&mut cx, "right");
    key(&mut cx, "right");
    changes.send(traded(&world, HOUR + 2, 64_200 * UNIT));
    cx.run_until_parked();
    assert_eq!(back(&view), 0);
    assert_eq!(readout(&cx)[6..8], ["C", "64,200.0"]);
}
