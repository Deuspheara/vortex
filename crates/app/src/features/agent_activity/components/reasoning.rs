//! Reasoning step row for the thread timeline.

use gpui::{App, IntoElement};

use crate::features::agent_activity::components::{
    activity_output_line_row, activity_preview_line_row,
};
use crate::features::shell::state::{
    ActivityGroupPos, AgentStatus, REASONING_OUTPUT_PREVIEW_LINES,
};

fn is_running(status: &AgentStatus) -> bool {
    matches!(status, AgentStatus::Thinking)
}

fn reasoning_preview(summary: &str) -> Option<String> {
    summary
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| {
            let mut chars = line.chars();
            let preview = chars.by_ref().take(96).collect::<String>();
            if chars.next().is_some() {
                format!("{preview}…")
            } else {
                preview
            }
        })
}

pub fn render_reasoning_header_row(
    item_id: &str,
    _title: &str,
    summary: &str,
    expanded: bool,
    status: &AgentStatus,
    _animate: bool,
    group_pos: Option<ActivityGroupPos>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
    use crate::tokens::{Tokens, element_key};
    use gpui::{div, prelude::*, px};
    let running = is_running(status);
    let label = if running { "Thinking" } else { "Thought" };
    let preview = reasoning_preview(summary).map(|s| s.replace("**", ""));
    activity_group_wrap(
        timeline_row(
            element_key("reasoning-header", item_id),
            div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(Tokens::spacing_1p5())
                .child(super::activity_icon("reasoning"))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(Tokens::text_sm())
                        .font_family(Tokens::ui_font_family())
                        .text_color(Tokens::text_secondary())
                        .child(label),
                )
                .when_some(preview, |el, text| {
                    el.child(
                        div()
                            .min_w(px(0.0))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_size(Tokens::text_sm())
                            .text_color(Tokens::text_secondary())
                            .child(text),
                    )
                })
                .into_any_element(),
            crate::shared::components::collapsible_row::disclosure_chevron(expanded)
                .into_any_element(),
            move |_, _, app| on_toggle(app),
        ),
        group_pos,
    )
}

pub fn render_reasoning_preview_line_row(item_id: &str, text: &str) -> impl IntoElement {
    activity_preview_line_row(item_id, text)
}

#[allow(dead_code)]
pub fn render_reasoning_output_line_row(item_id: &str, text: &str) -> impl IntoElement {
    activity_output_line_row(item_id, text, false)
}

#[allow(dead_code)]
pub fn render_reasoning_output_truncated_row(
    item_id: &str,
    total_lines: usize,
) -> impl IntoElement {
    let message = format!(
        "Showing the first {} of {total_lines} lines. Expand for the full recap.",
        REASONING_OUTPUT_PREVIEW_LINES
    );
    crate::features::agent_activity::components::activity_truncated_row(item_id, message)
}
