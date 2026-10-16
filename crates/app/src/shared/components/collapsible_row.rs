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
        .px(Tokens::spacing_0p5())
        .gap(Tokens::spacing_1p5())
        .rounded(Tokens::radius_xs())
        .hover(|s| s.bg(Tokens::surface_hover().opacity(0.35)))
        .cursor_pointer()
        .text_size(Tokens::text_sm())
        .line_height(Tokens::text_md_leading())
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

/// Keep grouped activity rows flat in the thread surface.
pub fn activity_group_wrap(
    content: impl IntoElement,
    _pos: Option<ActivityGroupPos>,
) -> impl IntoElement {
    div().w_full().child(content)
}

/// Expanded detail — indented plain text, no animation (avoids remount cost while scrolling).
#[allow(dead_code)]
pub fn timeline_body(
    _anim_id: impl Into<ElementId>,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .w_full()
        .pl(Tokens::spacing_7())
        .pr(Tokens::spacing_0p5())
        .pt(Tokens::spacing_1())
        .pb(Tokens::spacing_2())
        .font_family(Tokens::ui_font_family())
        .child(content)
}

/// Every transcript disclosure reserves the same trailing slot.
pub fn disclosure_chevron(expanded: bool) -> impl IntoElement {
    use crate::tokens::icons;
    use gpui_component::Icon;
    div()
        .size(Tokens::spacing_4())
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(if expanded {
                icons::CHEVRON_DOWN
            } else {
                icons::CHEVRON_RIGHT
            })
            .size(Tokens::spacing_3())
            .text_color(Tokens::text_tertiary()),
        )
}
