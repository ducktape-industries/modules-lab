//! Writes the frames the view draws as fixtures for the app's
//! `--render-tree` (app `dev/screens`): `frames <dir>` leaves
//! `<dir>/manifest.json` and one `<name>.json` per screen. The app's real
//! renderer then draws them with no node behind it.
use ducktape_view_guest::testing::TestAppContext;
use market_view::Market;

const SIZE: (u32, u32) = (1100, 640);

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: frames <dir>"));
    std::fs::create_dir_all(&dir).expect("the fixtures directory");
    let mut manifest = Vec::new();
    for (name, dark, hovered) in [
        ("market-light", false, None),
        ("market-dark", true, None),
        ("market-hover", true, Some("candle-80")),
    ] {
        let mut cx = TestAppContext::new();
        cx.open::<Market>();
        cx.simulate_theme(dark);
        // The header and the readout take what the chart does not.
        cx.simulate_measure("market-chart", SIZE.0 as f32 - 24., SIZE.1 as f32 - 94.);
        if let Some(slot) = hovered {
            cx.simulate_hover(slot, true);
        }
        cx.run_until_parked();
        let frame = serde_json::to_vec(&cx.whole_frame()).expect("a frame as JSON");
        std::fs::write(dir.join(format!("{name}.json")), frame).expect("the frame's file");
        manifest.push(serde_json::json!({
            "name": name,
            "width": SIZE.0,
            "height": SIZE.1,
            "theme": if dark { "dark" } else { "light" },
        }));
    }
    let manifest = serde_json::to_vec_pretty(&manifest).expect("the manifest as JSON");
    std::fs::write(dir.join("manifest.json"), manifest).expect("the manifest's file");
}
