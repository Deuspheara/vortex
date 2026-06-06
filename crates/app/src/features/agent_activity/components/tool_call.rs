//! Tool call row for the thread timeline.

use std::sync::Arc;

use gpui::{App, IntoElement, div, prelude::*};

use crate::features::agent_activity::components::{
    activity_header_row, activity_output_line_row, activity_truncated_row_with_trailing,
};
use crate::shared::components::buttons::btn_copy_icon_arc;

use crate::features::shell::state::{
    ActivityGroupPos, AgentStatus, TOOL_OUTPUT_PREVIEW_BYTES, TOOL_OUTPUT_PREVIEW_LINES,
};
use crate::tokens::element_key;

fn is_running(status: &AgentStatus) -> bool {
    matches!(status, AgentStatus::RunningTool)
}

fn is_raw_json_fragment(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('{')
        || t.starts_with('[')
        || t.starts_with('"')
        || t.contains("\":")
        || t.contains("<|")
}

fn show_command_detail(command: Option<&str>, label: &str) -> Option<String> {
    command.and_then(|cmd| {
        let cmd = cmd.trim();
        if cmd.is_empty() || cmd == "{}" || is_raw_json_fragment(cmd) || label.contains(cmd) {
            return None;
        }
        Some(if cmd.len() > 120 {
            format!("{}…", &cmd[..117])
        } else {
            cmd.to_string()
        })
    })
}

pub fn render_tool_header_row(
    item_id: &str,
    display_label: &str,
    command: Option<&str>,
    _expanded: bool,
    status: &AgentStatus,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    change_counts: Option<(usize, usize)>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let running = is_running(status);
    let waiting = matches!(status, AgentStatus::WaitingApproval);
    let label = if waiting {
        "Waiting for your approval".to_string()
    } else {
        display_label.to_string()
    };
    let detail = if waiting {
        Some(match command {
            Some(command) if !command.trim().is_empty() => format!("Review request: {command}"),
            _ => "Review this request to continue the run.".to_string(),
        })
    } else {
        show_command_detail(command, &label)
    };

    activity_header_row(
        "tool-row",
        "tool-header",
        item_id,
        label,
        detail,
        running,
        animate,
        group_pos,
        change_counts_badge(item_id, change_counts).into_any_element(),
        on_toggle,
    )
}

fn change_counts_badge(item_id: &str, change_counts: Option<(usize, usize)>) -> impl IntoElement {
    let Some((added, removed)) =
        change_counts.filter(|(added, removed)| *added > 0 || *removed > 0)
    else {
        return div().into_any_element();
    };

    div()
        .id(element_key("tool-change-counts", item_id))
        .flex()
        .items_center()
        .gap(crate::tokens::Tokens::spacing_2())
        .px(crate::tokens::Tokens::spacing_2())
        .py(crate::tokens::Tokens::spacing_0p5())
        .rounded(crate::tokens::Tokens::radius_full())
        .border_1()
        .border_color(crate::tokens::Tokens::border_subtle())
        .bg(crate::tokens::Tokens::surface().opacity(0.35))
        .child(
            div()
                .text_size(crate::tokens::Tokens::text_xs())
                .text_color(crate::tokens::Tokens::text_secondary())
                .child(format!("+{added}")),
        )
        .child(
            div()
                .text_size(crate::tokens::Tokens::text_xs())
                .text_color(crate::tokens::Tokens::danger())
                .child(format!("-{removed}")),
        )
        .into_any_element()
}

pub fn render_tool_output_line_row(item_id: &str, text: &str) -> impl IntoElement {
    activity_output_line_row(item_id, text, true)
}

pub fn render_tool_output_truncated_row(
    item_id: &str,
    total_lines: usize,
    full_output: Arc<str>,
) -> impl IntoElement {
    let preview_kb = TOOL_OUTPUT_PREVIEW_BYTES / 1024;
    let message = format!(
        "Showing the first {} lines / {} KB of {total_lines} lines. Copy the full output if you need more detail.",
        TOOL_OUTPUT_PREVIEW_LINES, preview_kb
    );
    activity_truncated_row_with_trailing(
        item_id,
        message,
        btn_copy_icon_arc(
            element_key("copy-tool-output-full", item_id),
            full_output,
            "Copy full output",
        ),
    )
}
