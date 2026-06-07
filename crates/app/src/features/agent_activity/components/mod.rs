//! Shared thread activity row primitives (tool, reasoning, diff).

pub mod activity_step;
pub mod approval;
pub mod diff;
pub mod pending_action_bar;
pub mod reasoning;
pub mod tool_call;

use gpui::{AnyElement, App, IntoElement, div, prelude::*, px};

use crate::features::shell::state::ActivityGroupPos;
use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
use crate::tokens::{Tokens, activity_action_line, element_key};

/// Collapsible activity header (tool, reasoning).
pub fn activity_header_row(
    row_key: &'static str,
    header_key: &'static str,
    item_id: &str,
    label: String,
    detail: Option<String>,
    running: bool,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    trailing: AnyElement,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    activity_group_wrap(
        div()
            .id(element_key(row_key, item_id))
            .w_full()
            .flex()
            .flex_col()
            .child(timeline_row(
                element_key(header_key, item_id),
                activity_action_line(&label, detail.as_deref(), running, animate, item_id, 0)
                    .into_any_element(),
                trailing,
                move |_, _, app: &mut App| on_toggle(app),
            )),
        group_pos,
    )
}

/// Indented preview or output line under an activity header.
pub fn activity_output_line_row(line_id: &str, text: &str, mono: bool) -> impl IntoElement {
    div()
        .id(element_key("activity-output-line", line_id))
        .w_full()
        .min_w(px(0.0))
        .ml(Tokens::spacing_2())
        .pl(Tokens::spacing_2())
        .border_l_1()
        .border_color(Tokens::activity_detail_border())
        .when(mono, |el| {
            el.font_family("monospace")
                .text_size(Tokens::text_code())
                .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                .opacity(0.68)
        })
        .when(!mono, |el| {
            el.font_family(Tokens::ui_font_family())
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
                .opacity(0.64)
        })
        .text_color(Tokens::activity_detail_text())
        .overflow_hidden()
        .whitespace_nowrap()
        .child(text.to_string())
}

/// Truncation notice (message only).
#[allow(dead_code)]
pub fn activity_truncated_row(item_id: &str, message: String) -> impl IntoElement {
    div()
        .id(element_key("activity-truncated", item_id))
        .w_full()
        .min_w(px(0.0))
        .ml(Tokens::spacing_2())
        .pl(Tokens::spacing_2())
        .border_l_1()
        .border_color(Tokens::activity_detail_border())
        .font_family(Tokens::ui_font_family())
        .text_size(Tokens::text_xs())
        .text_color(Tokens::activity_meta_text())
        .child(message)
}

/// Truncation notice with trailing control (e.g. copy).
pub fn activity_truncated_row_with_trailing(
    item_id: &str,
    message: String,
    trailing: impl IntoElement,
) -> impl IntoElement {
    div()
        .id(element_key("activity-truncated", item_id))
        .w_full()
        .min_w(px(0.0))
        .ml(Tokens::spacing_2())
        .pl(Tokens::spacing_2())
        .border_l_1()
        .border_color(Tokens::activity_detail_border())
        .flex()
        .items_center()
        .justify_between()
        .gap(Tokens::spacing_2())
        .font_family(Tokens::ui_font_family())
        .child(
            div()
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::activity_meta_text())
                .child(message),
        )
        .child(trailing)
}
