//! A details page over the whole of its content (fading in and out), or beside it in a drawer
//! (sliding in and out). Both leave by the same rule: the app keeps what it closed while it is
//! `leaving` and drops it on `on_settled` (see `details_dock_leaving`).

use iced::Length;
use iced::widget::{container, space};

use crate::Element;
use crate::iad::layouts::anchor::Side;
use crate::iad::layouts::Effect;
use crate::iad::layouts::animation::{Easing, Spec};
use crate::iad::prelude::*;
use crate::iad::styles::container::window;

/// How long the details page takes to fade in or out.
const FADE: std::time::Duration = std::time::Duration::from_millis(280);

/// The value of the fade when it begins coming in: how much of the page shows on its first frame.
/// From nothing it is a blank window for a moment, which feels wrong; at half it is already the
/// page.
const STARTING_VALUE: f32 = 0.5;

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
    dock(base, pane, open, None)
}

/// [`details_dock`] that also animates out. `pane` is given while the details are open **and**
/// while they are leaving (the app keeps what it closed until `on_settled`); `open` says which.
/// `on_settled` is sent once the exit has finished and the app can drop what it kept.
pub fn details_dock_leaving<'a, M: Clone + 'a>(
    base: impl Into<Element<'a, M>>,
    pane: Option<Element<'a, M>>,
    open: bool,
    on_settled: M,
) -> Element<'a, M> {
    dock(base, pane, open, Some(on_settled))
}

fn dock<'a, M: Clone + 'a>(
    base: impl Into<Element<'a, M>>,
    pane: Option<Element<'a, M>>,
    open: bool,
    on_settled: Option<M>,
) -> Element<'a, M> {
    let wanted = pane.is_some();
    let surface: Element<'a, M> = match pane {
        Some(p) => container(p)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(window)
            .into(),
        None => space().into(),
    };
    let mut dock = Dock::new(base, surface)
        .open(open && wanted)
        // Given but not open: on its way out.
        .closing(wanted && !open)
        .side(Side::Right)
        .size(DockSize::FullScreen)
        .mode(DockMode::Overlay)
        // No dimming of what is behind the page while it fades.
        .scrim(false)
        // A page that appears, not a drawer that arrives: its whole surface, fill and contents,
        // fades in and out, ease-in-out cubic, to and from the window colour behind it.
        .motion(Effect::Fade, Effect::Fade)
        .starting_value(STARTING_VALUE)
        .backdrop(|t| t.semantic.window.bg)
        .spec(Spec::Timed(FADE, Easing::EaseInOutCubic));
    if let Some(m) = on_settled {
        dock = dock.on_settled(m);
    }
    dock.into()
}

/// How wide the details drawer is: a form, a little wider than comfortable to read in.
pub const DRAWER_WIDTH: f32 = 440.0;

/// `base` with `pane` in a drawer on the right, sliding in over it, or just `base` when `pane` is
/// `None`. A click outside the drawer sends `on_outside`.
pub fn details_drawer<'a, M: Clone + 'a>(
    base: impl Into<Element<'a, M>>,
    pane: Option<Element<'a, M>>,
    on_outside: M,
) -> Element<'a, M> {
    let open = pane.is_some();
    drawer(base, pane, open, on_outside, None)
}

/// [`details_drawer`] that also slides out; see [`details_dock_leaving`].
pub fn details_drawer_leaving<'a, M: Clone + 'a>(
    base: impl Into<Element<'a, M>>,
    pane: Option<Element<'a, M>>,
    open: bool,
    on_outside: M,
    on_settled: M,
) -> Element<'a, M> {
    drawer(base, pane, open, on_outside, Some(on_settled))
}

fn drawer<'a, M: Clone + 'a>(
    base: impl Into<Element<'a, M>>,
    pane: Option<Element<'a, M>>,
    open: bool,
    on_outside: M,
    on_settled: Option<M>,
) -> Element<'a, M> {
    let wanted = pane.is_some();
    let surface: Element<'a, M> = match pane {
        Some(p) => container(p)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(window)
            .into(),
        None => space().into(),
    };
    let mut dock = Dock::new(base, surface)
        .open(open && wanted)
        .closing(wanted && !open)
        .side(Side::Right)
        .size(DockSize::Fixed(DRAWER_WIDTH))
        .mode(DockMode::Overlay)
        .scrim(true)
        // A drawer arrives: it slides in from its edge and back out, the scrim fading with it.
        .motion(Effect::Slide, Effect::Slide)
        .on_outside(on_outside);
    if let Some(m) = on_settled {
        dock = dock.on_settled(m);
    }
    dock.into()
}
