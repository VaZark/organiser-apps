//! A details page that fades in over the whole of its content, at every window width.

use iced::Length;
use iced::widget::{container, space};

use crate::Element;
use crate::iad::layouts::anchor::Side;
use crate::iad::prelude::*;
use crate::iad::styles::container::window;

/// The widest the content of a details page should be: a form across a wide window is hard to read.
pub const READABLE_WIDTH: f32 = 640.0;

/// `base` with `pane` over all of it, fading in, or just `base` when `pane` is `None`.
///
/// The pane paints the window colour itself. iad draws an overlay on its own lighter surface,
/// which looked like the background changing as the details opened. The surface is always built so
/// that it can fade out as well as in. The pane's own content should be limited to
/// [`READABLE_WIDTH`], which the page then centres.
pub fn details_dock<'a, M: Clone + 'a>(
    base: impl Into<Element<'a, M>>,
    pane: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let open = pane.is_some();
    let surface: Element<'a, M> = match pane {
        Some(p) => container(p)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(window)
            .into(),
        None => space().into(),
    };
    Dock::new(base, surface)
        .open(open)
        .side(Side::Right)
        .size(DockSize::FullScreen)
        .mode(DockMode::Overlay)
        // No dimming of what is behind the page while it fades in.
        .scrim(false)
        // A page that appears, not a drawer that arrives.
        .fade(true)
        .into()
}
