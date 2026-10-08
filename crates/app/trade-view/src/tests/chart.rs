//! The chart: what its canvas carries, the candle under the pointer, and
//! the wheel, a drag and the arrow keys. The pointer is where the host says
//! it is, in the window's pixels; the chart is where `sample::chart_bounds`
//! puts it.
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

/// The candles on screen, as the plot's name says them.
pub(super) fn slots(cx: &TestAppContext) -> usize {
    let named = cx.interactivity("trade-plot").aria.label.clone();
    let named = named.expect("the plot is named");
    let (said, _) = named.split_once(" on screen").expect("its candles");
    let count = said.rsplit(' ').next().and_then(|count| count.parse().ok());
    count.expect("how many")
}

/// The pointer over the middle of slot `slot` of the wide board's chart:
/// 88 slots of 8 px across its 704 px.
fn over(cx: &mut TestAppContext, slot: usize) {
    sample::point(cx, WIDE, (slot as f32 * 8. + 4., 200.));
}

/// Where the line on the candle under the pointer is drawn, from the
/// plot's left edge: `None` while it is on no candle.
fn line(cx: &TestAppContext) -> Option<gpui::Length> {
    let line = &cx.node("trade-plot").children()[1];
    let style = &cx.styles()[line.style().expect("the line's style")];
    style.background.as_ref().and(style.inset.left)
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

/// The wheel over the chart in a pane of `size`, `lines` away from the
/// reader, with the pointer at `x` of the chart.
fn wheel_at(cx: &mut TestAppContext, size: (f32, f32), x: f32, lines: f32) {
    let (origin, _) = sample::chart_bounds(size);
    cx.simulate_scroll_wheel(
        "trade-plot",
        ScrollWheelEvent {
            position: point(px(origin.0 + x), px(origin.1 + 200.)),
            delta: ScrollDelta::Lines(point(0., lines)),
            ..Default::default()
        },
    );
}

/// The wheel over the wide board's price axis, right of the candles: the
/// zoom is about the plot's right edge.
pub(super) fn wheel(cx: &mut TestAppContext, lines: f32) {
    wheel_at(cx, WIDE, 704. + 32., lines);
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
    let newest = readout(&cx);
    over(&mut cx, 82);
    assert_eq!((line(&cx), readout(&cx)), (None, newest), "no candle there");
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
    over(&mut cx, 86);
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
    assert_eq!(line(&cx), None);
    over(&mut cx, 57);
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
    // the line is on the column its wick is drawn in
    assert_eq!(line(&cx), Some(px(460.).into()));
    over(&mut cx, 58);
    assert_ne!(readout(&cx), newest);
    assert_eq!(line(&cx), Some(px(468.).into()));
    // over the price axis, and under the plot on the time axis: no candle
    for off in [(720., 200.), (468., 500.)] {
        sample::point(&mut cx, WIDE, off);
        assert_eq!((line(&cx), readout(&cx)), (None, newest.clone()));
        over(&mut cx, 58);
    }
    // the pointer leaves the plot
    cx.simulate_hover("trade-plot", false);
    assert_eq!((line(&cx), readout(&cx)), (None, newest));
}

/// One listener on the plot finds the candle under the pointer, from where
/// the host says the chart is. A slot the pointer enters costs the view its
/// line, the readout and the time under the candle, and none of the canvas;
/// a move inside one slot costs it nothing.
#[test]
fn a_hover_sends_the_line_and_the_readout_and_none_of_the_chart() {
    let (mut cx, ..) = wide();
    let whole = cx.frame_bytes();
    let drawn = canvas(&cx).to_vec();
    let renders = cx.renders();
    over(&mut cx, 57);
    let sent: usize = cx.reports().iter().map(|tick| tick.bytes).sum();
    assert_eq!(canvas(&cx), drawn, "the canvas is as it was");
    assert!(
        sent < 512,
        "{sent} B crossed for a hover, of a {whole} B frame"
    );
    assert_eq!(cx.renders(), renders + 1);
    // the slot's first pixel, its last, and another height in it
    for inside in [(456., 200.), (463.9, 200.), (460., 20.)] {
        sample::point(&mut cx, WIDE, inside);
        let ticks = cx.reports().iter();
        assert_eq!(ticks.map(|tick| tick.patches).sum::<usize>(), 0);
    }
    assert_eq!(
        cx.renders(),
        renders + 1,
        "a move inside one slot draws nothing"
    );
    sample::point(&mut cx, WIDE, (464., 200.));
    assert_eq!(cx.renders(), renders + 2, "the next slot's first pixel");
    assert_eq!(line(&cx), Some(px(468.).into()));
}

/// Across the whole plot every slot the pointer enters is a small patch,
/// where an axis label gives way to the candle's time and comes back too:
/// the time axis is a layer of its own, so the plot's other children keep
/// their places and none is sent again for a label that came or went.
#[test]
fn a_sweep_across_the_plot_is_small_patches_all_the_way() {
    let (mut cx, ..) = wide();
    for slot in 0..88 {
        over(&mut cx, slot);
        let sent: usize = cx.reports().iter().map(|tick| tick.bytes).sum();
        let patches = cx.last_frame().patches.len();
        assert!(
            sent < 512 && patches <= 16,
            "slot {slot}: {sent} B in {patches} patches"
        );
    }
}

/// The wheel zooms about the pointer: the candle under it stays under it,
/// and the plot's right edge moves to make that so. At the live edge
/// there is no newer candle to bring in, so the edge stays.
#[test]
fn the_wheel_keeps_the_candle_under_the_pointer() {
    let (mut cx, view, _) = wide();
    // the pointer on the 45th of the 88 candles
    let x = 44. * 8. + 4.;
    over(&mut cx, 44);
    let told = readout(&cx);
    for closer in 1..=6 {
        wheel_at(&mut cx, WIDE, x, 1.);
        assert_eq!(readout(&cx), told, "{closer} notches closer");
    }
    assert_eq!(slots(&cx), 46);
    assert_eq!(back(&view), 20, "the newest candles are off the edge");
    for _ in 0..6 {
        wheel_at(&mut cx, WIDE, x, -1.);
        assert_eq!(readout(&cx), told);
    }
    assert_eq!((slots(&cx), back(&view)), (88, 0));
    wheel_at(&mut cx, WIDE, x, -1.);
    assert_eq!(back(&view), 0, "no newer candle than the newest");
    assert_ne!(readout(&cx), told, "an older candle came under the pointer");
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
    assert_eq!(slots(&cx), 300, "a pixel a candle: every hour there is");
    assert_eq!(back(&view), 0, "about the right edge");
}

/// Zoomed all the way out, the chart holds the most candles one canvas
/// carries, and reads page after page until it has them: a week of 15m
/// candles is on one screen, with no element of the frame per candle.
#[test]
fn zoomed_out_the_chart_reads_on_until_the_screen_is_full() {
    let tip = sample::NOW / 900_000;
    // more than a week of 15m candles, rising and falling by turns
    let rows = (0..800u64).rev().map(|back| {
        let open = 63_000 * UNIT + (back % 7) * 10 * UNIT;
        let close = 63_000 * UNIT + ((back + 3) % 7) * 10 * UNIT;
        trade::Candle {
            bucket: tip - back,
            open,
            high: open.max(close) + UNIT,
            low: open.min(close) - UNIT,
            close,
            volume: (1 + back % 5) * UNIT,
        }
    });
    let mut world = sample::board();
    world.candles.insert((0, 2), rows.collect());
    let pane = (1400., 760.);
    let (mut cx, ..): Opened = opened(world, pane, Some(READER));
    cx.simulate_click("15m");
    // the pointer on the plot, half way along its 824 px. 19 notches reach
    // the narrowest candle, and there is not one more: a notch that zooms
    // no further pans by nothing, and would ask for the page whatever the
    // others did
    for _ in 0..19 {
        wheel_at(&mut cx, pane, 412.5, -1.);
    }
    cx.run_until_parked();
    assert_eq!(slots(&cx), 720, "the most candles on screen");
    // no element per candle: the canvas, the line, and the labels
    let held = cx.node("trade-plot").children().len();
    assert!(held < 64, "{held} elements over 720 candles");
    // a slot is a little over a pixel wide, and the pointer still finds it
    sample::point(&mut cx, pane, (412.5, 200.));
    assert_eq!(line(&cx), Some(px(412.).into()));
    // the newest page and the two before it cover the screen
    assert_eq!(older(&cx, tip - 255), 1);
    assert_eq!(older(&cx, tip - 511), 1);
    assert_eq!(older(&cx, tip - 767), 0, "the screen is full");
    let commands = canvas(&cx).len();
    assert!(
        commands > 720 * 5 && commands <= 4096,
        "{commands} commands"
    );
}

/// The wheel out with the pointer on the plot, until the screen holds more
/// slots than there are candles held: the page before them is asked for
/// and its candles fill in. A zoom out about the pointer asks the chart to
/// pan to newer candles than there are; what asks for the page is where
/// the chart is, at the oldest candle held, and not which way it was asked
/// to go.
#[test]
fn zoomed_out_about_the_pointer_the_chart_reads_the_page_before() {
    let (mut cx, view, _) = wide();
    // the first page is the newest 256 of the sample's 300 hours
    assert_eq!(older(&cx, HOUR - 255), 0);
    for _ in 0..12 {
        wheel_at(&mut cx, WIDE, 352., -1.);
    }
    assert_eq!(older(&cx, HOUR - 255), 1, "the page before the oldest");
    assert_eq!(slots(&cx), 300, "every hour the program holds");
    assert_eq!(back(&view), 0);
}

/// At the oldest candle the program holds the chart can pan no further:
/// a zoom in about the pointer still keeps its candle, and a zoom out
/// that would pan past the oldest candle keeps that one in the first slot.
#[test]
fn the_wheel_at_the_oldest_candle_keeps_it_in_the_first_slot() {
    let (mut cx, view, _) = wide();
    for _ in 0..31 {
        key(&mut cx, "left");
    }
    assert_eq!(back(&view), 212, "the 88 that start at the first candle");
    let x = 44. * 8. + 4.;
    over(&mut cx, 44);
    let told = readout(&cx);
    wheel_at(&mut cx, WIDE, x, 1.);
    assert_eq!((back(&view), readout(&cx)), (216, told.clone()));
    wheel_at(&mut cx, WIDE, x, -1.);
    assert_eq!((back(&view), readout(&cx)), (212, told.clone()));
    // 97 slots: the pointer's candle would need the first slot before
    // the oldest candle
    wheel_at(&mut cx, WIDE, x, -1.);
    assert_eq!((slots(&cx), back(&view)), (97, 203));
    assert_ne!(readout(&cx), told, "a newer candle came under the pointer");
    assert_eq!(older(&cx, HOUR - 299), 0, "the program holds none older");
}

/// A chart that only moved in the window is drawn as it was: its new
/// origin is kept, for the pointer, and nothing renders. Under a resting
/// pointer the move puts another candle there, and that is drawn.
#[test]
fn a_chart_that_moved_is_drawn_again_only_for_another_candle_under_the_pointer() {
    let (mut cx, ..) = wide();
    let (origin, size) = sample::chart_bounds(WIDE);
    let renders = cx.renders();
    cx.simulate_bounds("trade-chart", (origin.0 + 16., origin.1), size);
    assert_eq!(cx.renders(), renders, "moved, with no pointer over it");
    // the pointer where slot 57 was: two slots further left of the chart
    over(&mut cx, 57);
    assert_eq!(line(&cx), Some(px(444.).into()), "placed by the new origin");
    let renders = cx.renders();
    cx.simulate_bounds("trade-chart", origin, size);
    assert_eq!(cx.renders(), renders + 1, "another candle is under it");
    assert_eq!(line(&cx), Some(px(460.).into()));
}

/// A plot whose width is not a whole number of slots: the slots end at its
/// right edge, where they are drawn, and the strip left of the first one
/// is no candle's. Nor is either axis.
#[test]
fn the_pointer_finds_a_slot_where_the_canvas_draws_it() {
    let pane = (1285., 760.);
    let (mut cx, ..) = opened(sample::board(), pane, Some(READER));
    assert_eq!(slots(&cx), 88, "709 px of plot at 8 px a candle");
    let mut at = |x: f32, y: f32| {
        sample::point(&mut cx, pane, (x, y));
        line(&cx)
    };
    assert_eq!(at(4.9, 200.), None, "left of the first slot");
    assert_eq!(at(5., 200.), Some(px(9.).into()), "its first pixel");
    assert_eq!(at(708.9, 200.), Some(px(705.).into()), "the plot's last");
    assert_eq!(at(709., 200.), None, "the price axis");
    assert_eq!(at(300., 489.9), Some(px(297.).into()), "its lowest pixel");
    assert_eq!(at(300., 490.), None, "the time axis");
    // the line is on the column the canvas draws that slot's wick in
    for wick in [9., 297., 705.] {
        let drawn = quads(&cx, wick, wick + 1.);
        assert!(drawn.iter().any(|(_, size)| size[0] == 1.), "{drawn:?}");
    }
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
    let (origin, _) = sample::chart_bounds(WIDE);
    let at = |x: f32| point(px(origin.0 + x), px(origin.1 + 200.));
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
    over(&mut cx, 3);
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

/// A tag never covers another, nor the readout: of two orders priced
/// close together the one nearer the last price is tagged and the other
/// keeps its line alone, and one along the plot's top keeps its price on
/// the axis and no name over the readout.
#[test]
fn an_orders_tag_gives_way_to_another_and_to_the_readout() {
    let mut world = sample::board();
    let sell = world.orders[1];
    let beside = trade::Order {
        id: 7,
        limit: Some(644_000 * (UNIT / 10)),
        amount: 300 * trade::LOT,
        ..sell
    };
    let top = trade::Order {
        id: 8,
        limit: Some(677_000 * (UNIT / 10)),
        amount: 700 * trade::LOT,
        ..sell
    };
    world.orders.splice(0..0, [top, beside]);
    let (cx, ..) = opened(world, WIDE, Some(READER));
    let labels = labels(&cx);
    let said = |text: &str| labels.iter().any(|label| label == text);
    assert!(said("Sell 0.1000") && said("64,352.0"), "{labels:?}");
    assert!(!said("Sell 0.0300") && !said("64,400.0"), "{labels:?}");
    assert!(!said("Sell 0.0700") && said("67,700.0"), "{labels:?}");
}
