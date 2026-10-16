//! Shared thread activity row primitives (tool, reasoning, diff).

pub mod activity_step;
pub mod approval;
pub mod context_trace;
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

/// Flat preview line shown under a collapsed activity header.
pub fn activity_preview_line_row(line_id: &str, text: &str) -> impl IntoElement {
    div()
        .id(element_key("activity-preview-line", line_id))
        .w_full()
        .min_w(px(0.0))
        .px(Tokens::spacing_2())
        .font_family(Tokens::ui_font_family())
        .text_size(Tokens::text_sm())
        .line_height(Tokens::text_sm_leading())
        .text_color(Tokens::activity_detail_text())
        .overflow_hidden()
        .whitespace_nowrap()
        .child(text.to_string())
}

/// Indented output line inside an expanded activity body.
pub fn activity_output_line_row(line_id: &str, text: &str, mono: bool) -> impl IntoElement {
    div()
        .id(element_key("activity-output-line", line_id))
        .w_full()
        .min_w(px(0.0))
        .ml(Tokens::spacing_7())
        .pl(Tokens::spacing_3())
        .border_l_1()
        .border_color(Tokens::activity_detail_border())
        .when(mono, |el| {
            el.font_family(Tokens::code_font_family())
                .text_size(Tokens::text_code())
                .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                .bg(Tokens::code_bg())
        })
        .when(!mono, |el| {
            el.font_family(Tokens::ui_font_family())
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
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
        .ml(Tokens::spacing_7())
        .pl(Tokens::spacing_3())
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
        .ml(Tokens::spacing_7())
        .pl(Tokens::spacing_3())
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

/// One fixed icon column for every transcript activity row.
pub fn activity_icon(name: &str) -> impl IntoElement {
    use crate::tokens::icons;
    use gpui_component::Icon;
    let icon = match name {
        "reasoning" => icons::brain(),
        "codex_file_change" | "write_file" | "apply_patch" | "edit_file" => icons::file_edit(),
        "run_real_command" | "codex_command" | "bash_virtual" => icons::command(),
        "search_project" | "find_symbol" => Icon::new(icons::SEARCH),
        "list_files" | "repo_map" | "related_files" => Icon::new(icons::FOLDER),
        "web_search" | "web_fetch" | "codex_web_search" => Icon::new(icons::GLOBE),
        _ => Icon::new(icons::FILE_TEXT),
    };
    div()
        .size(Tokens::spacing_6())
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            icon.size(Tokens::spacing_4())
                .text_color(Tokens::text_tertiary()),
        )
}
