//! The desktop's tree: what Arcade draws under a pointer, on the pointer's
//! sizes. The hub (`hub.rs`), a game being played (`play.rs`) or Tetris
//! versus (`versus.rs`), whichever is open. Under a finger the root draws
//! the phone's screens (`phone/`) instead.
use gpui::relative;
use view::AnyElement;
use view::prelude::*;

use crate::Arcade;

/// The phone's tree, which the view's root picks under a finger. It reads
/// nothing of this one.
pub(crate) mod phone;

pub(crate) fn render(
    view: &mut Arcade,
    window: &mut Window,
    cx: &mut Context<Arcade>,
) -> AnyElement {
    let theme = *cx.global::<Theme>();
    let ladder = &design::POINTER;
    let screen = match (view.versus.is_some(), view.play.is_some()) {
        (true, _) => view
            .versus_screen(window, cx, ladder, &theme)
            .into_any_element(),
        (false, true) => view.play_screen(cx, ladder, &theme).into_any_element(),
        (false, false) => view.hub(cx, &theme).into_any_element(),
    };
    div()
        .id("arcade")
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.foreground)
        .text_size(design::text::BODY)
        .line_height(relative(1.3))
        .child(screen)
        .into_any_element()
}
