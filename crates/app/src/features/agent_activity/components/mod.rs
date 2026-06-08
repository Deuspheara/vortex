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
use crate::tokens::{Tokens, activity_action_line_with_loading, element_key};

#[derive(Clone, Copy)]
pub struct ActivityRowVisual<'a> {
    pub row_key: &'static str,
    pub header_key: &'static str,
    pub item_id: &'a str,
    pub running: bool,
    pub show_loading: bool,
    pub animate: bool,
    pub group_pos: Option<ActivityGroupPos>,
}

pub fn activity_header_row_with_visual(
    visual: ActivityRowVisual<'_>,
    label: String,
    detail: Option<String>,
    trailing: AnyElement,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    activity_group_wrap(
        div()
            .id(element_key(visual.row_key, visual.item_id))
            .w_full()
            .flex()
            .flex_col()
            .child(timeline_row(
                element_key(visual.header_key, visual.item_id),
                activity_action_line_with_loading(
                    &label,
                    detail.as_deref(),
                    visual.running,
                    visual.show_loading,
                    visual.animate,
                    visual.item_id,
                )
                .into_any_element(),
                trailing,
                move |_, _, app: &mut App| on_toggle(app),
            )),
        visual.group_pos,
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
                .opacity(0.8)
        })
        .when(!mono, |el| {
            el.font_family(Tokens::ui_font_family())
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
                .opacity(0.76)
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
