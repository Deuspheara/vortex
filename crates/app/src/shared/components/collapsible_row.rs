//! Flat collapsible rows for the thread timeline.

use gpui::{AnyElement, ElementId, IntoElement, div, prelude::*, px};

use crate::features::shell::state::ActivityGroupPos;
use crate::tokens::Tokens;

/// Single-line timeline row — compact execution log style.
pub fn timeline_row(
    id: impl Into<ElementId>,
    label: AnyElement,
    trailing: AnyElement,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(Tokens::TOOL_ROW_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .px(Tokens::spacing_2())
        .gap(Tokens::spacing_2())
        .cursor_pointer()
        .text_color(Tokens::text_secondary())
        .on_click(on_click)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(label),
        )
        .child(div().flex_shrink_0().child(trailing))
}

/// Wrap activity rows in a shared left rail when part of a consecutive band.
pub fn activity_group_wrap(
    content: impl IntoElement,
    pos: Option<ActivityGroupPos>,
) -> impl IntoElement {
    match pos {
        None | Some(ActivityGroupPos::Single) => div().w_full().child(content),
        Some(_) => div().w_full().child(content),
    }
}

/// Expanded detail — indented plain text, no animation (avoids remount cost while scrolling).
#[allow(dead_code)]
pub fn timeline_body(
    _anim_id: impl Into<ElementId>,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .w_full()
        .ml(Tokens::spacing_3())
        .pl(Tokens::spacing_3())
        .pt(Tokens::spacing_1())
        .pb(Tokens::spacing_2())
        .border_l_1()
        .border_color(Tokens::activity_detail_border())
        .font_family(Tokens::ui_font_family())
        .child(content)
}
