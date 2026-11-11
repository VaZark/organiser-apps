//! One modal flow for every window size: a dialog over a wide window, a full-screen sheet over a
//! narrow one. The sheet slides up, or fades in when `fade` is set.

use iced::Size;
use iced::widget::space;

use crate::Element;
use crate::components::panel::panel;
use crate::components::tab_scope::tab_scope;
use crate::iad::prelude::*;

/// Show `modal` (its content and the message that closes it) over `base`, or just `base`.
pub fn present<'a, M: Clone + 'a>(
    base: Element<'a, M>,
    modal: Option<(Element<'a, M>, M)>,
    narrow: bool,
    viewport: Size,
    window_radius: f32,
    fade: bool,
) -> Element<'a, M> {
    let (content, close) = match modal {
        Some((content, close)) => (
            // A wide dialog is an overlay with its own focus scope; the narrow full-screen sheet is
            // a dock, which has none, so Tab is given its scope here.
            Some(panel(
                if narrow { tab_scope(content) } else { content },
                narrow,
                viewport,
                window_radius,
            )),
            Some(close),
        ),
        None => (None, None),
    };
    if narrow {
        let open = content.is_some();
        Dock::new(base, content.unwrap_or_else(|| space().into()))
            .open(open)
            .scope(DockScope::Window)
            .window_radius(window_radius)
            .size(DockSize::FullScreen)
            .mode(DockMode::Overlay)
            // The full-screen sheet slides up, or, with `fade`, rises out of the window colour.
            .fade(fade)
            .into()
    } else {
        present_dialog(base, content, close, 0.0)
    }
}
