//! `cargo run -p browser-view --example live`: Browser in the real app
//! over a fake node. A view-only entry: no program, nothing to seed. The
//! app must be a build with the web piece: set `DUCKTAPE_APP` to it.
use ducktape_view_guest::live;

fn main() {
    live::run("browser", |_| Ok(()))
}
