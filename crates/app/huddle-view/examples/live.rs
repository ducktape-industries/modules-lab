//! `cargo run -p huddle-view --example live`: Huddle in the real app over
//! a fake node with the huddle program seated and Ada and Bob to invite.
//! The fake node has no live relay, so a call starts and rings but no track
//! comes up. The app must be a build with the camera and microphone (its
//! `av` feature, on by default): set `DUCKTAPE_APP` to it.
use view::rig;

fn main() {
    rig::run(huddle::PROGRAM, |net| {
        net.seat::<huddle::Huddle>(huddle::PROGRAM);
        net.person("ada");
        net.person("bob");
        Ok(())
    })
}
