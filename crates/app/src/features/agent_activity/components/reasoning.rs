//! Reasoning step row for the thread timeline.

use gpui::{App, IntoElement};

use crate::features::agent_activity::components::{
    ActivityRowVisual, activity_header_row_with_visual, activity_output_line_row,
};
use crate::features::shell::state::{
    ActivityGroupPos, AgentStatus, REASONING_OUTPUT_PREVIEW_LINES,
};

fn is_running(status: &AgentStatus) -> bool {
    matches!(status, AgentStatus::Thinking)
}

fn reasoning_title(title: &str, running: bool) -> String {
    if running {
        return title.to_string();
    }
    match title {
        "Thinking" | "thinking" => "Reasoning".into(),
        other => other.to_string(),
    }
}

pub fn render_reasoning_header_row(
    item_id: &str,
    title: &str,
    _summary: &str,
    _expanded: bool,
    status: &AgentStatus,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let running = is_running(status);
    let title_owned = reasoning_title(title, running);

    activity_header_row_with_visual(
        ActivityRowVisual {
            row_key: "reasoning-row",
            header_key: "reasoning-header",
            item_id,
            running,
            show_loading: false,
            animate,
            group_pos,
        },
        title_owned,
        None,
        gpui::div().into_any_element(),
        on_toggle,
    )
}

pub fn render_reasoning_preview_line_row(item_id: &str, text: &str) -> impl IntoElement {
    activity_output_line_row(item_id, text, false)
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
