//! `cargo run -p huddle-view --example live`: Huddle in the real app over
//! a fake node. A view-only entry: no program, nothing to seed. The app
//! must be a build with the camera and microphone: set `DUCKTAPE_APP` to
//! one built with `--features av`.
use view::live;

fn main() {
    live::run("huddle", |_| Ok(()))
}
