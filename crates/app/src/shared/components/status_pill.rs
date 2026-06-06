//! Compact status pill for top bar and composer footer.

use gpui::{FontWeight, IntoElement, div, prelude::*};

use crate::tokens::{Tokens, element_key};

pub fn status_pill(id: &str, label: &str) -> impl IntoElement {
    div()
        .id(element_key("status-pill", id))
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_0p5())
        .rounded(Tokens::radius_full())
        .bg(Tokens::surface_hover())
        .border_1()
        .border_color(Tokens::border_subtle())
        .text_size(Tokens::text_xs())
        .font_weight(FontWeight::MEDIUM)
        .text_color(Tokens::text_tertiary())
        .child(label.to_string())
}
