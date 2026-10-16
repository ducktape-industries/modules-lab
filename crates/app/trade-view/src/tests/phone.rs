//! The phone's design (`ui/phone/`): under a finger the view draws the
//! phone's tree, picked once at the root.
use super::*;

/// The boards' market under a finger, 390 by 844.
fn phone() -> Opened {
    let (mut cx, view, world) = wide();
    cx.simulate_touch(true);
    cx.simulate_resize(390., 844.);
    cx.run_until_parked();
    (cx, view, world)
}

#[test]
fn a_finger_gets_the_phones_tree_and_a_pointer_the_desktops() {
    let (mut cx, ..) = phone();
    assert!(cx.find("trade-market").is_none(), "no desktop bar");
    assert!(cx.find("trade-plot").is_none(), "no desktop chart");
    shows(&cx, &["BTC/USD", "63,589.1"]);
    cx.simulate_touch(false);
    cx.simulate_resize(WIDE.0, WIDE.1);
    cx.run_until_parked();
    assert!(cx.find("trade-market").is_some());
}
