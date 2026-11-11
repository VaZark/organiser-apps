//! Small pieces shared by the recipe page, the calendar and the fridge.

use iced::widget::{column, image, row};
use iced::{ContentFit, Length};
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use recipes_schema::photos;
use recipes_schema::shopping::{Item, Status};
use recipes_schema::units::pretty;

use crate::app::Message;

fn handle(name: &str) -> Option<image::Handle> {
    photos::path(&photos::dir(), name).map(image::Handle::from_path)
}

/// A picture across the full width at a fixed height, cropped to fill (a card's thumbnail).
pub fn photo<'a>(name: &str, height: f32) -> Element<'a, Message> {
    match handle(name) {
        Some(h) => image(h)
            .width(Length::Fill)
            .height(height)
            .content_fit(ContentFit::Cover)
            .border_radius(8)
            .into(),
        None => space().into(),
    }
}

/// A picture at a fixed height, whole.
pub fn whole<'a>(name: &str, height: f32) -> Element<'a, Message> {
    match handle(name) {
        Some(h) => image(h)
            .height(height)
            .content_fit(ContentFit::Contain)
            .border_radius(8)
            .into(),
        None => space().into(),
    }
}

/// A row of pictures to look at.
pub fn strip<'a>(names: &[String], height: f32) -> Element<'a, Message> {
    row(names.iter().map(|n| whole(n, height)))
        .spacing(8)
        .wrap()
        .into()
}

/// A row of pictures, each with a remove button, and a button to add more.
pub fn strip_edit<'a>(names: &[String], target: Option<i64>, height: f32) -> Element<'a, Message> {
    let mut r = row(names.iter().map(|n| {
        column![
            whole(n, height),
            flat_button(text("Remove")).on_press(Message::RemovePhoto(target, n.clone())),
        ]
        .spacing(4)
        .into()
    }))
    .spacing(8);
    r = r.push(
        flat_button(row![Icon::ListAdd.view(16.0), text("Add photos")].spacing(6))
            .on_press(Message::PickPhotos(target)),
    );
    r.wrap().into()
}

/// `100 g`, `2`, or nothing for a quantity that is not given.
pub fn amount(q: Option<f64>, unit: &str) -> String {
    match q {
        None => String::new(),
        Some(q) if unit.is_empty() => pretty(q),
        Some(q) => format!("{} {unit}", pretty(q)),
    }
}

/// What to get, one line: `pasta — 150 g more`.
pub fn item_line(i: &Item) -> String {
    let get = i
        .get
        .as_ref()
        .map(|(q, u)| amount(Some(*q), u))
        .unwrap_or_default();
    match i.status {
        Status::Enough => format!("{} — have it", i.name),
        Status::Missing if get.is_empty() => i.name.clone(),
        Status::Missing => format!("{} — {get}", i.name),
        Status::Short => format!("{} — {get} more", i.name),
        Status::Unit => format!("{} — {get} (you have it in another unit)", i.name),
    }
}
