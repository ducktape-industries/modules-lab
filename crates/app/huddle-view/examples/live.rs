//! `cargo run -p huddle-view --example live`: Huddle in the real app over
//! a fake node. A view-only entry: no program, nothing to seed. The app
//! must be a build with the camera and microphone (its `av` feature, on by
//! default): set `DUCKTAPE_APP` to it.
use view::rig;

fn main() {
    rig::run("huddle", |_| Ok(()))
}
