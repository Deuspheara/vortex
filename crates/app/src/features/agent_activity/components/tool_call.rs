//! Tool call row for the thread timeline.

use std::sync::Arc;

use gpui::{App, FontWeight, IntoElement, div, prelude::*, px};

use crate::features::agent_activity::components::{
    activity_output_line_row, activity_truncated_row_with_trailing,
};
use crate::shared::components::buttons::btn_copy_icon_arc;
use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};

use crate::features::shell::state::{
    ActivityGroupPos, AgentStatus, TOOL_OUTPUT_PREVIEW_BYTES, TOOL_OUTPUT_PREVIEW_LINES,
};
use crate::tokens::{Tokens, element_key};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolCallSummary {
    verb: String,
    primary: String,
    secondary: Option<String>,
    primary_mono: bool,
}

impl ToolCallSummary {
    fn new(
        verb: impl Into<String>,
        primary: impl Into<String>,
        secondary: Option<String>,
        primary_mono: bool,
    ) -> Self {
        Self {
            verb: verb.into(),
            primary: primary.into(),
            secondary,
            primary_mono,
        }
    }
}

fn clean_command(command: Option<&str>) -> Option<&str> {
    command
        .map(str::trim)
        .filter(|cmd| !cmd.is_empty() && *cmd != "{}" && !is_raw_json_fragment(cmd))
}

fn tool_call_summary(
    tool_name: &str,
    display_label: &str,
    command: Option<&str>,
    status: &AgentStatus,
) -> ToolCallSummary {
    if matches!(status, AgentStatus::WaitingApproval) {
        return ToolCallSummary::new(
            "Review",
            "approval request",
            clean_command(command).map(|cmd| trim_to_chars(cmd, 96)),
            false,
        );
    }

    let running = is_running(status);
    let command = clean_command(command);
    match tool_name {
        "read_file" | "open_node" => file_summary(
            if running { "Reading" } else { "Read" },
            command,
            display_label,
        ),
        "list_files" | "related_files" | "repo_map" => {
            list_summary(if running { "Listing" } else { "Listed" }, command)
        }
        "search_project" | "find_symbol" => search_summary(
            if running { "Searching" } else { "Searched" },
            command,
            display_label,
        ),
        "bash_virtual" => command_summary(
            if running { "Running" } else { "Ran" },
            command,
            Some("virtual shell"),
        ),
        "run_real_command" => command_summary(
            if running { "Running" } else { "Ran" },
            command,
            Some("real command"),
        ),
        "write_file" => file_summary(
            if running { "Writing" } else { "Wrote" },
            command,
            display_label,
        ),
        "edit_file" => file_summary(
            if running { "Editing" } else { "Edited" },
            command,
            display_label,
        ),
        "delete_file" => file_summary(
            if running { "Deleting" } else { "Deleted" },
            command,
            display_label,
        ),
        "apply_patch" | "propose_patch" => ToolCallSummary::new(
            if running { "Applying" } else { "Applied" },
            "patch",
            None,
            false,
        ),
        "fetch_url" | "web_fetch" | "web_extract" | "web_search" => web_summary(
            if running { "Fetching" } else { "Fetched" },
            command,
            display_label,
        ),
        "browser_snapshot" => ToolCallSummary::new(
            if running { "Capturing" } else { "Captured" },
            "browser snapshot",
            command.map(|cmd| trim_to_chars(cmd, 80)),
            false,
        ),
        "browser_screenshot" => ToolCallSummary::new(
            if running { "Capturing" } else { "Captured" },
            "screenshot",
            command.map(|cmd| trim_to_chars(cmd, 80)),
            false,
        ),
        name if name.starts_with("android.") || name.starts_with("android_cli.") => {
            android_summary(name, running, command, display_label)
        }
        _ => fallback_summary(display_label, command),
    }
}

fn file_summary(verb: &str, command: Option<&str>, display_label: &str) -> ToolCallSummary {
    let raw = command.unwrap_or_else(|| label_remainder(display_label).unwrap_or("file"));
    let (path, range) = split_line_range(raw);
    let primary = file_name(path).unwrap_or(path).to_string();
    let mut secondary = Vec::new();
    if let Some(range) = range {
        secondary.push(range.to_string());
    }
    if let Some(parent) = parent_path(path) {
        secondary.push(parent.to_string());
    }
    ToolCallSummary::new(verb, primary, join_secondary(secondary), false)
}

fn list_summary(verb: &str, command: Option<&str>) -> ToolCallSummary {
    let Some(command) = command else {
        return ToolCallSummary::new(verb, "files", None, false);
    };
    let (primary, secondary) = split_first_meta(command);
    ToolCallSummary::new(verb, primary, secondary, false)
}

fn search_summary(verb: &str, command: Option<&str>, display_label: &str) -> ToolCallSummary {
    let raw = command.unwrap_or_else(|| label_remainder(display_label).unwrap_or("project"));
    let (primary, secondary) = split_first_meta(raw);
    ToolCallSummary::new(verb, quote_if_plain(primary), secondary, false)
}

fn command_summary(verb: &str, command: Option<&str>, context: Option<&str>) -> ToolCallSummary {
    ToolCallSummary::new(
        verb,
        command
            .map(|cmd| trim_to_chars(cmd, 140))
            .unwrap_or_else(|| "command".to_string()),
        context.map(str::to_string),
        true,
    )
}

fn web_summary(verb: &str, command: Option<&str>, display_label: &str) -> ToolCallSummary {
    let raw = command.unwrap_or_else(|| label_remainder(display_label).unwrap_or("URL"));
    let primary = url_host(raw).unwrap_or_else(|| trim_to_chars(raw, 96));
    let secondary = (primary != raw).then(|| trim_to_chars(raw, 96));
    ToolCallSummary::new(verb, primary, secondary, false)
}

fn android_summary(
    tool_name: &str,
    running: bool,
    command: Option<&str>,
    display_label: &str,
) -> ToolCallSummary {
    let primary = command
        .map(|cmd| trim_to_chars(cmd, 96))
        .or_else(|| label_remainder(display_label).map(str::to_string));
    let (verb, fallback_primary) = match tool_name {
        "android.observe" => (if running { "Observing" } else { "Observed" }, "emulator"),
        "android.read_logcat" => (if running { "Reading" } else { "Read" }, "logcat"),
        "android.tap_text" | "android.tap_resource_id" | "android.tap_point" => {
            (if running { "Tapping" } else { "Tapped" }, "target")
        }
        "android.type_text" => (if running { "Typing" } else { "Typed" }, "text"),
        "android.swipe" => (if running { "Swiping" } else { "Swiped" }, "screen"),
        "android.launch_app" => (if running { "Launching" } else { "Launched" }, "app"),
        "android.press_back" => (if running { "Pressing" } else { "Pressed" }, "Back"),
        "android.press_home" => (if running { "Pressing" } else { "Pressed" }, "Home"),
        "android.ensure_emulator" => (if running { "Preparing" } else { "Prepared" }, "emulator"),
        name if name.ends_with("docs_search") => (
            if running { "Searching" } else { "Searched" },
            "Android docs",
        ),
        name if name.ends_with("docs_fetch") => {
            (if running { "Fetching" } else { "Fetched" }, "Android docs")
        }
        _ => (if running { "Running" } else { "Ran" }, "Android tool"),
    };
    ToolCallSummary::new(
        verb,
        primary.unwrap_or_else(|| fallback_primary.to_string()),
        None,
        false,
    )
}

fn fallback_summary(display_label: &str, command: Option<&str>) -> ToolCallSummary {
    let (verb, rest) = split_verb(display_label);
    ToolCallSummary::new(
        verb,
        rest.or_else(|| command.map(str::to_string))
            .unwrap_or_else(|| "tool".to_string()),
        None,
        false,
    )
}

fn split_verb(label: &str) -> (String, Option<String>) {
    let trimmed = label.trim();
    if let Some((verb, rest)) = trimmed.split_once(' ') {
        let rest = rest.trim();
        (
            verb.to_string(),
            (!rest.is_empty()).then(|| rest.to_string()),
        )
    } else {
        (trimmed.to_string(), None)
    }
}

fn label_remainder(label: &str) -> Option<&str> {
    label
        .split_once(' ')
        .map(|(_, rest)| rest.trim())
        .filter(|rest| !rest.is_empty())
}

fn split_first_meta(value: &str) -> (String, Option<String>) {
    if let Some((primary, rest)) = value.split_once(" · ") {
        let rest = rest.trim();
        (
            primary.trim().to_string(),
            (!rest.is_empty()).then(|| rest.to_string()),
        )
    } else {
        (value.trim().to_string(), None)
    }
}

fn split_line_range(path: &str) -> (&str, Option<&str>) {
    let Some((base, suffix)) = path.rsplit_once(':') else {
        return (path, None);
    };
    let line_range = suffix.chars().all(|c| c.is_ascii_digit() || c == '-') && suffix.contains('-');
    if line_range {
        (base, Some(suffix))
    } else {
        (path, None)
    }
}

fn file_name(path: &str) -> Option<&str> {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
}

fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/')
        .map(|(parent, _)| parent)
        .filter(|parent| !parent.is_empty())
}

fn join_secondary(parts: Vec<String>) -> Option<String> {
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn quote_if_plain(value: String) -> String {
    let trimmed = value.trim();
    if trimmed.starts_with('"') || trimmed.starts_with('“') || trimmed.starts_with('`') {
        trimmed.to_string()
    } else {
        format!("\"{trimmed}\"")
    }
}

fn url_host(value: &str) -> Option<String> {
    let after_scheme = value.split_once("://").map(|(_, rest)| rest)?;
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme)
        .trim();
    (!host.is_empty()).then(|| host.to_string())
}

fn trim_to_chars(value: &str, max_chars: usize) -> String {
    let mut iter = value.chars();
    let trimmed = iter.by_ref().take(max_chars).collect::<String>();
    if iter.next().is_some() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

pub fn render_tool_header_row(
    item_id: &str,
    tool_name: &str,
    display_label: &str,
    command: Option<&str>,
    _expanded: bool,
    status: &AgentStatus,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    change_counts: Option<(usize, usize)>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let summary = tool_call_summary(tool_name, display_label, command, status);
    activity_group_wrap(
        div()
            .id(element_key("tool-row", item_id))
            .w_full()
            .flex()
            .flex_col()
            .child(timeline_row(
                element_key("tool-header", item_id),
                render_tool_summary(item_id, summary, is_running(status), animate)
                    .into_any_element(),
                change_counts_badge(item_id, change_counts).into_any_element(),
                move |_, _, app: &mut App| on_toggle(app),
            )),
        group_pos,
    )
}

fn render_tool_summary(
    item_id: &str,
    summary: ToolCallSummary,
    running: bool,
    animate: bool,
) -> impl IntoElement {
    div()
        .id(element_key("tool-summary", item_id))
        .w_full()
        .min_w(px(0.0))
        .h(px(Tokens::ROW_HEIGHT_SM))
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .when(running, |el| el.child(tool_loading_dots(item_id, animate)))
        .child(
            div()
                .flex_shrink_0()
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
                .font_family(Tokens::ui_font_family())
                .font_weight(FontWeight::MEDIUM)
                .text_color(gpui::rgb(0xffffff))
                .child(summary.verb),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .truncate()
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
                .font_weight(FontWeight::MEDIUM)
                .when(summary.primary_mono, |el| {
                    el.font_family("monospace")
                        .text_size(Tokens::text_code())
                        .text_color(Tokens::text_secondary())
                })
                .when(!summary.primary_mono, |el| {
                    el.font_family(Tokens::ui_font_family())
                        .text_color(Tokens::text_secondary())
                })
                .hover(|s| s.text_color(Tokens::text_primary()))
                .child(summary.primary),
        )
        .when_some(summary.secondary, |el, secondary| {
            el.child(
                div()
                    .max_w(px(300.0))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .truncate()
                    .text_size(Tokens::text_sm())
                    .line_height(Tokens::text_sm_leading_compact())
                    .font_family(Tokens::ui_font_family())
                    .text_color(Tokens::text_faint())
                    .opacity(0.78)
                    .hover(|s| s.text_color(Tokens::text_tertiary()).opacity(1.0))
                    .child(secondary),
            )
        })
}

fn tool_loading_dots(item_id: &str, animate: bool) -> gpui::AnyElement {
    let indicator = div()
        .id(element_key("tool-loading-dots", item_id))
        .w(px(24.0))
        .flex_shrink_0()
        .font_family("monospace")
        .text_size(Tokens::text_sm())
        .line_height(Tokens::text_sm_leading_compact())
        .text_color(Tokens::text_tertiary())
        .child("...");

    if animate {
        indicator.opacity(0.82).into_any_element()
    } else {
        indicator.opacity(0.62).into_any_element()
    }
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
        .gap(crate::tokens::Tokens::spacing_1())
        .px(crate::tokens::Tokens::spacing_1())
        .py(crate::tokens::Tokens::spacing_0p5())
        .rounded(crate::tokens::Tokens::radius_xs())
        .bg(crate::tokens::Tokens::surface_hover().opacity(0.4))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn completed() -> AgentStatus {
        AgentStatus::Completed
    }

    fn running() -> AgentStatus {
        AgentStatus::RunningTool
    }

    #[test]
    fn read_file_emphasizes_file_and_line_range() {
        let summary = tool_call_summary(
            "read_file",
            "Read build.gradle.kts",
            Some("android_todo/app/build.gradle.kts:73-120"),
            &completed(),
        );

        assert_eq!(
            summary,
            ToolCallSummary::new(
                "Read",
                "build.gradle.kts",
                Some("73-120 · android_todo/app".into()),
                false,
            )
        );
    }

    #[test]
    fn list_files_uses_path_and_metadata() {
        let summary = tool_call_summary(
            "list_files",
            "Listed android_todo",
            Some("android_todo/app · max 20"),
            &completed(),
        );

        assert_eq!(
            summary,
            ToolCallSummary::new("Listed", "android_todo/app", Some("max 20".into()), false)
        );
    }

    #[test]
    fn search_project_quotes_query_and_keeps_scope() {
        let summary = tool_call_summary(
            "search_project",
            "Searched for TODO",
            Some("TODO · @ crates/app · regex"),
            &completed(),
        );

        assert_eq!(
            summary,
            ToolCallSummary::new(
                "Searched",
                "\"TODO\"",
                Some("@ crates/app · regex".into()),
                false
            )
        );
    }

    #[test]
    fn shell_tools_use_monospace_command_preview() {
        let virtual_summary = tool_call_summary(
            "bash_virtual",
            "Ran command",
            Some("find android_todo -name \"*.kt\""),
            &completed(),
        );
        let real_summary = tool_call_summary(
            "run_real_command",
            "Running command",
            Some("cargo check -p app"),
            &running(),
        );

        assert_eq!(
            virtual_summary,
            ToolCallSummary::new(
                "Ran",
                "find android_todo -name \"*.kt\"",
                Some("virtual shell".into()),
                true,
            )
        );
        assert_eq!(
            real_summary,
            ToolCallSummary::new(
                "Running",
                "cargo check -p app",
                Some("real command".into()),
                true,
            )
        );
    }

    #[test]
    fn write_edit_delete_use_target_paths() {
        let wrote = tool_call_summary(
            "write_file",
            "Wrote Todo.kt",
            Some("android_todo/app/src/Todo.kt"),
            &completed(),
        );
        let edited = tool_call_summary(
            "edit_file",
            "Editing main.rs",
            Some("crates/app/src/main.rs"),
            &running(),
        );
        let deleted = tool_call_summary(
            "delete_file",
            "Deleted old.rs",
            Some("crates/app/src/old.rs"),
            &completed(),
        );

        assert_eq!(
            wrote,
            ToolCallSummary::new(
                "Wrote",
                "Todo.kt",
                Some("android_todo/app/src".into()),
                false
            )
        );
        assert_eq!(
            edited,
            ToolCallSummary::new("Editing", "main.rs", Some("crates/app/src".into()), false)
        );
        assert_eq!(
            deleted,
            ToolCallSummary::new("Deleted", "old.rs", Some("crates/app/src".into()), false)
        );
    }

    #[test]
    fn fetch_url_promotes_host() {
        let summary = tool_call_summary(
            "fetch_url",
            "Fetched URL",
            Some("https://docs.rs/gpui/latest/gpui/"),
            &completed(),
        );

        assert_eq!(
            summary,
            ToolCallSummary::new(
                "Fetched",
                "docs.rs",
                Some("https://docs.rs/gpui/latest/gpui/".into()),
                false,
            )
        );
    }

    #[test]
    fn unknown_tool_falls_back_to_display_label() {
        let summary = tool_call_summary(
            "custom_tool",
            "Processed workspace",
            Some("ignored"),
            &completed(),
        );

        assert_eq!(
            summary,
            ToolCallSummary::new("Processed", "workspace", None, false)
        );
    }

    #[test]
    fn raw_json_or_empty_command_falls_back_cleanly() {
        let empty = tool_call_summary("read_file", "Read file", Some("{}"), &completed());
        let json = tool_call_summary(
            "bash_virtual",
            "Ran command",
            Some("{\"cmd\":\"cargo check\"}"),
            &completed(),
        );

        assert_eq!(empty, ToolCallSummary::new("Read", "file", None, false));
        assert_eq!(
            json,
            ToolCallSummary::new("Ran", "command", Some("virtual shell".into()), true)
        );
    }

    #[test]
    fn long_command_is_truncated_without_panicking() {
        let command = "x".repeat(180);
        let summary =
            tool_call_summary("bash_virtual", "Ran command", Some(&command), &completed());

        assert_eq!(summary.verb, "Ran");
        assert!(summary.primary.ends_with('…'));
        assert!(summary.primary.chars().count() <= 141);
        assert!(summary.primary_mono);
    }
}
