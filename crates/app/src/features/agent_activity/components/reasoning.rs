//! Reasoning step row for the thread timeline.

use gpui::{App, IntoElement};

use crate::features::agent_activity::components::{
    ActivityRowVisual, activity_header_row_with_visual, activity_output_line_row,
    activity_preview_line_row,
};
use crate::features::shell::state::{
    ActivityGroupPos, AgentStatus, REASONING_OUTPUT_PREVIEW_LINES,
};

fn is_running(status: &AgentStatus) -> bool {
    matches!(status, AgentStatus::Thinking)
}

fn is_generic_reasoning_title(title: &str) -> bool {
    matches!(title, "Thinking" | "thinking" | "Current step")
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

fn reasoning_header_copy(title: &str, summary: &str, running: bool) -> (String, Option<String>) {
    let preview = reasoning_preview(summary);
    if running {
        return match preview {
            Some(detail) => ("Current step".into(), Some(detail)),
            None if is_generic_reasoning_title(title) => ("Planning".into(), None),
            None => (title.to_string(), None),
        };
    }

    let label = if is_generic_reasoning_title(title) {
        "Reasoning".to_string()
    } else {
        title.to_string()
    };
    let detail = preview.filter(|detail| detail != &label);
    (label, detail)
}

pub fn render_reasoning_header_row(
    item_id: &str,
    title: &str,
    summary: &str,
    _expanded: bool,
    status: &AgentStatus,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let running = is_running(status);
    let (label, detail) = reasoning_header_copy(title, summary, running);

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
        label,
        detail,
        gpui::div().into_any_element(),
        on_toggle,
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
