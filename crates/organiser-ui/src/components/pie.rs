//! A small pie showing how much is done: a ring, filled clockwise from the top.

use iced::mouse::Cursor;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Color, Length, Point, Radians, Rectangle, Renderer};

use crate::Element;
use crate::iad::Theme;

struct Pie {
    /// 0.0..=1.0.
    done: f32,
    color: Option<Color>,
}

impl<M> canvas::Program<M, Theme> for Pie {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let c = frame.center();
        let r = bounds.width.min(bounds.height) / 2.0 - 1.0;
        let fg = theme.semantic.window.fg;
        let fill = self.color.unwrap_or(theme.semantic.accent.bg);
        frame.stroke(
            &Path::circle(c, r),
            Stroke::default()
                .with_width(1.5)
                .with_color(Color { a: 0.35, ..fg }),
        );
        let done = self.done.clamp(0.0, 1.0);
        if done >= 0.999 {
            frame.fill(&Path::circle(c, r), fill);
        } else if done > 0.0 {
            let top = -std::f32::consts::FRAC_PI_2;
            let wedge = Path::new(|b| {
                b.move_to(c);
                b.line_to(Point::new(c.x, c.y - r));
                b.arc(canvas::path::Arc {
                    center: c,
                    radius: r,
                    start_angle: Radians(top),
                    end_angle: Radians(top + done * std::f32::consts::TAU),
                });
                b.close();
            });
            frame.fill(&wedge, fill);
        }
        vec![frame.into_geometry()]
    }
}

/// A pie `size` across, `done` of the way round (0.0 to 1.0), in `color` or the accent.
pub fn pie<'a, M: 'a>(done: f32, size: f32, color: Option<Color>) -> Element<'a, M> {
    iced::widget::canvas(Pie { done, color })
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}
