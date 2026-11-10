//! The page for "nothing here".

use crate::Element;
use crate::iad::Icon;
use crate::iad::prelude::status_page;

pub fn empty<'a, M: 'a>(icon: Icon, title: &str, description: &str) -> Element<'a, M> {
    status_page()
        .icon(icon.view(64.0))
        .title(title)
        .description(description)
        .into()
}
