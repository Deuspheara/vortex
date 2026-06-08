//! Tool call row for the thread timeline.

use std::sync::Arc;

use gpui::{App, FontWeight, IntoElement, MouseButton, div, prelude::*, px};

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
pub struct ToolCallSummary {
    verb: String,
    primary: String,
    secondary: Option<String>,
    primary_mono: bool,
    pub target_path: Option<String>,
    pub line_range: Option<ToolLineRange>,
    pub detail_rows: Vec<ToolCallDetailRow>,
    pub expandable: bool,
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
            target_path: None,
            line_range: None,
            detail_rows: Vec::new(),
            expandable: false,
        }
    }

    fn with_target(mut self, path: impl Into<String>, line_range: Option<ToolLineRange>) -> Self {
        self.target_path = Some(path.into());
        self.line_range = line_range;
        self
    }

    fn with_detail_rows(mut self, detail_rows: Vec<ToolCallDetailRow>) -> Self {
        self.expandable = !detail_rows.is_empty();
        self.detail_rows = detail_rows;
        self
    }
}

pub type ToolLineRange = (Option<u32>, Option<u32>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallDetailRow {
    File {
        verb: String,
        path: String,
        line_range: Option<ToolLineRange>,
        metadata: Option<String>,
    },
    Command {
        label: String,
        command: String,
        context: Option<String>,
    },
    Text {
        label: String,
        value: String,
        metadata: Option<String>,
    },
}

pub type OpenToolFileCallback = Arc<dyn Fn(String, Option<ToolLineRange>, &mut App) + 'static>;

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
        "list_files" | "related_files" | "repo_map" => list_summary(
            if running { "Listing" } else { "Listed" },
            command,
            display_label,
        ),
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

pub fn tool_call_detail_rows(
    tool_name: &str,
    display_label: &str,
    command: Option<&str>,
    status: &AgentStatus,
) -> Vec<ToolCallDetailRow> {
    tool_call_summary(tool_name, display_label, command, status).detail_rows
}

pub fn tool_call_detail_row_count(
    tool_name: &str,
    command: Option<&str>,
    status: &AgentStatus,
) -> usize {
    tool_call_summary(tool_name, "", command, status)
        .detail_rows
        .len()
}

fn file_summary(verb: &str, command: Option<&str>, display_label: &str) -> ToolCallSummary {
    let raw = command.unwrap_or_else(|| label_remainder(display_label).unwrap_or("file"));
    let (path, line_range, range_label) = split_line_range(raw);
    let primary = file_name(path).unwrap_or(path).to_string();
    let mut secondary = Vec::new();
    if let Some(range) = range_label.clone() {
        secondary.push(range);
    }
    if let Some(parent) = parent_path(path) {
        secondary.push(parent.to_string());
    }
    ToolCallSummary::new(verb, primary, join_secondary(secondary), false)
        .with_target(path.to_string(), line_range)
        .with_detail_rows(vec![ToolCallDetailRow::File {
            verb: verb.to_string(),
            path: path.to_string(),
            line_range,
            metadata: range_label,
        }])
}

fn list_summary(verb: &str, command: Option<&str>, display_label: &str) -> ToolCallSummary {
    let Some(command) = command else {
        return ToolCallSummary::new(verb, "files", None, false).with_detail_rows(vec![
            ToolCallDetailRow::Text {
                label: verb.to_string(),
                value: label_remainder(display_label)
                    .unwrap_or("files")
                    .to_string(),
                metadata: None,
            },
        ]);
    };
    let (primary, secondary) = split_first_meta(command);
    ToolCallSummary::new(verb, primary.clone(), secondary.clone(), false).with_detail_rows(vec![
        ToolCallDetailRow::Text {
            label: verb.to_string(),
            value: primary,
            metadata: secondary,
        },
    ])
}

fn search_summary(verb: &str, command: Option<&str>, display_label: &str) -> ToolCallSummary {
    let raw = command.unwrap_or_else(|| label_remainder(display_label).unwrap_or("project"));
    let (primary, secondary) = split_first_meta(raw);
    ToolCallSummary::new(
        verb,
        quote_if_plain(primary.clone()),
        secondary.clone(),
        false,
    )
    .with_detail_rows(vec![ToolCallDetailRow::Text {
        label: verb.to_string(),
        value: quote_if_plain(primary),
        metadata: secondary,
    }])
}

fn command_summary(verb: &str, command: Option<&str>, context: Option<&str>) -> ToolCallSummary {
    let command_preview = command
        .map(|cmd| trim_to_chars(cmd, 140))
        .unwrap_or_else(|| "command".to_string());
    ToolCallSummary::new(
        verb,
        command_preview.clone(),
        context.map(str::to_string),
        true,
    )
    .with_detail_rows(vec![ToolCallDetailRow::Command {
        label: if verb == "Ran" {
            "Command executed:".to_string()
        } else {
            "Command:".to_string()
        },
        command: command_preview,
        context: context.map(str::to_string),
    }])
}

fn web_summary(verb: &str, command: Option<&str>, display_label: &str) -> ToolCallSummary {
    let raw = command.unwrap_or_else(|| label_remainder(display_label).unwrap_or("URL"));
    let primary = url_host(raw).unwrap_or_else(|| trim_to_chars(raw, 96));
    let secondary = (primary != raw).then(|| trim_to_chars(raw, 96));
    ToolCallSummary::new(verb, primary.clone(), secondary.clone(), false).with_detail_rows(vec![
        ToolCallDetailRow::Text {
            label: verb.to_string(),
            value: primary,
            metadata: secondary,
        },
    ])
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
    .with_detail_rows(vec![ToolCallDetailRow::Text {
        label: verb.to_string(),
        value: command
            .map(|cmd| trim_to_chars(cmd, 96))
            .or_else(|| label_remainder(display_label).map(str::to_string))
            .unwrap_or_else(|| fallback_primary.to_string()),
        metadata: None,
    }])
}

fn fallback_summary(display_label: &str, command: Option<&str>) -> ToolCallSummary {
    let (verb, rest) = split_verb(display_label);
    let primary = rest
        .or_else(|| command.map(str::to_string))
        .unwrap_or_else(|| "tool".to_string());
    ToolCallSummary::new(verb.clone(), primary.clone(), None, false).with_detail_rows(vec![
        ToolCallDetailRow::Text {
            label: verb,
            value: primary,
            metadata: None,
        },
    ])
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

fn split_line_range(path: &str) -> (&str, Option<ToolLineRange>, Option<String>) {
    let Some((base, suffix)) = path.rsplit_once(':') else {
        return (path, None, None);
    };
    let Some(range) = parse_line_range(suffix) else {
        return (path, None, None);
    };
    (base, Some(range), Some(suffix.to_string()))
}

fn parse_line_range(value: &str) -> Option<ToolLineRange> {
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return None;
    }
    if let Some((start, end)) = value.split_once('-') {
        let start = parse_optional_line(start)?;
        let end = parse_optional_line(end)?;
        if start.is_none() && end.is_none() {
            None
        } else {
            Some((start, end))
        }
    } else {
        value
            .parse::<u32>()
            .ok()
            .map(|line| (Some(line), Some(line)))
    }
}

fn parse_optional_line(value: &str) -> Option<Option<u32>> {
    if value.is_empty() {
        Some(None)
    } else {
        value.parse::<u32>().ok().map(Some)
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
    expanded: bool,
    status: &AgentStatus,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    change_counts: Option<(usize, usize)>,
    on_open_file: Option<OpenToolFileCallback>,
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
                render_tool_summary(
                    item_id,
                    summary,
                    expanded,
                    is_running(status),
                    animate,
                    on_open_file,
                )
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
    expanded: bool,
    running: bool,
    animate: bool,
    on_open_file: Option<OpenToolFileCallback>,
) -> impl IntoElement {
    let primary = render_tool_primary(item_id, &summary, on_open_file);
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
        .child(primary)
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
        .when(summary.expandable, |el| {
            el.child(
                div()
                    .flex_shrink_0()
                    .text_size(Tokens::text_sm())
                    .line_height(Tokens::text_sm_leading_compact())
                    .text_color(Tokens::text_tertiary())
                    .opacity(0.78)
                    .child(if expanded { "⌄" } else { "›" }),
            )
        })
}

fn render_tool_primary(
    item_id: &str,
    summary: &ToolCallSummary,
    on_open_file: Option<OpenToolFileCallback>,
) -> gpui::AnyElement {
    if let (Some(path), Some(on_open_file)) = (summary.target_path.clone(), on_open_file) {
        return render_file_link(
            element_key("tool-primary-file", item_id),
            summary.primary.clone(),
            path,
            summary.line_range,
            on_open_file,
        );
    }

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
        .child(summary.primary.clone())
        .into_any_element()
}

fn render_file_link(
    id: impl Into<gpui::ElementId>,
    label: String,
    path: String,
    line_range: Option<ToolLineRange>,
    on_open_file: OpenToolFileCallback,
) -> gpui::AnyElement {
    let click_path = path.clone();
    div()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .flex()
        .items_center()
        .child(
            div()
                .id(id)
                .max_w_full()
                .min_w(px(0.0))
                .overflow_hidden()
                .truncate()
                .cursor_pointer()
                .border_b_1()
                .border_color(Tokens::info().opacity(0.72))
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
                .font_family(Tokens::ui_font_family())
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::info())
                .hover(|s| {
                    s.text_color(Tokens::accent_hover())
                        .border_color(Tokens::accent_hover())
                })
                .on_mouse_down(MouseButton::Left, move |_, _, app| {
                    app.stop_propagation();
                })
                .on_click(move |_, _, app| {
                    app.stop_propagation();
                    on_open_file(click_path.clone(), line_range, app);
                })
                .child(label),
        )
        .into_any_element()
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

pub fn render_tool_detail_line_row(
    item_id: &str,
    detail: ToolCallDetailRow,
    on_open_file: Option<OpenToolFileCallback>,
) -> impl IntoElement {
    div()
        .id(element_key("tool-detail-line", item_id))
        .w_full()
        .h(px(Tokens::ROW_HEIGHT_SM))
        .pl(Tokens::spacing_6())
        .pr(Tokens::spacing_1())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(render_tool_detail_content(item_id, detail, on_open_file))
}

fn render_tool_detail_content(
    item_id: &str,
    detail: ToolCallDetailRow,
    on_open_file: Option<OpenToolFileCallback>,
) -> gpui::AnyElement {
    match detail {
        ToolCallDetailRow::File {
            verb,
            path,
            line_range,
            metadata,
        } => div()
            .w_full()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(Tokens::spacing_2())
            .child(detail_label(verb))
            .child(render_file_link(
                element_key("tool-detail-file", item_id),
                file_name(&path).unwrap_or(&path).to_string(),
                path,
                line_range,
                on_open_file.unwrap_or_else(|| Arc::new(|_, _, _| {})),
            ))
            .when_some(metadata, |el, meta| el.child(detail_metadata(meta)))
            .into_any_element(),
        ToolCallDetailRow::Command {
            label,
            command,
            context,
        } => div()
            .w_full()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(Tokens::spacing_2())
            .child(detail_label(label))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .truncate()
                    .font_family("monospace")
                    .text_size(Tokens::text_code())
                    .line_height(Tokens::text_sm_leading_compact())
                    .text_color(Tokens::text_secondary())
                    .child(command),
            )
            .when_some(context, |el, context| el.child(detail_metadata(context)))
            .into_any_element(),
        ToolCallDetailRow::Text {
            label,
            value,
            metadata,
        } => div()
            .w_full()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(Tokens::spacing_2())
            .child(detail_label(label))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .truncate()
                    .text_size(Tokens::text_sm())
                    .line_height(Tokens::text_sm_leading_compact())
                    .font_family(Tokens::ui_font_family())
                    .text_color(Tokens::text_secondary())
                    .child(value),
            )
            .when_some(metadata, |el, meta| el.child(detail_metadata(meta)))
            .into_any_element(),
    }
}

fn detail_label(label: String) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .text_size(Tokens::text_sm())
        .line_height(Tokens::text_sm_leading_compact())
        .font_family(Tokens::ui_font_family())
        .text_color(Tokens::text_faint())
        .child(label)
}

fn detail_metadata(metadata: String) -> impl IntoElement {
    div()
        .max_w(px(240.0))
        .min_w(px(0.0))
        .overflow_hidden()
        .truncate()
        .text_size(Tokens::text_sm())
        .line_height(Tokens::text_sm_leading_compact())
        .font_family(Tokens::ui_font_family())
        .text_color(Tokens::text_faint())
        .opacity(0.78)
        .child(metadata)
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

    fn assert_visible_summary(
        summary: &ToolCallSummary,
        verb: &str,
        primary: &str,
        secondary: Option<&str>,
        primary_mono: bool,
    ) {
        assert_eq!(summary.verb, verb);
        assert_eq!(summary.primary, primary);
        assert_eq!(summary.secondary.as_deref(), secondary);
        assert_eq!(summary.primary_mono, primary_mono);
    }

    #[test]
    fn read_file_emphasizes_file_and_line_range() {
        let summary = tool_call_summary(
            "read_file",
            "Read build.gradle.kts",
            Some("android_todo/app/build.gradle.kts:73-120"),
            &completed(),
        );

        assert_visible_summary(
            &summary,
            "Read",
            "build.gradle.kts",
            Some("73-120 · android_todo/app"),
            false,
        );
        assert_eq!(
            summary.target_path.as_deref(),
            Some("android_todo/app/build.gradle.kts")
        );
        assert_eq!(summary.line_range, Some((Some(73), Some(120))));
        assert!(matches!(
            summary.detail_rows.first(),
            Some(ToolCallDetailRow::File { path, .. })
                if path == "android_todo/app/build.gradle.kts"
        ));
    }

    #[test]
    fn list_files_uses_path_and_metadata() {
        let summary = tool_call_summary(
            "list_files",
            "Listed android_todo",
            Some("android_todo/app · max 20"),
            &completed(),
        );

        assert_visible_summary(
            &summary,
            "Listed",
            "android_todo/app",
            Some("max 20"),
            false,
        );
        assert!(matches!(
            summary.detail_rows.first(),
            Some(ToolCallDetailRow::Text { value, metadata, .. })
                if value == "android_todo/app" && metadata.as_deref() == Some("max 20")
        ));
    }

    #[test]
    fn search_project_quotes_query_and_keeps_scope() {
        let summary = tool_call_summary(
            "search_project",
            "Searched for TODO",
            Some("TODO · @ crates/app · regex"),
            &completed(),
        );

        assert_visible_summary(
            &summary,
            "Searched",
            "\"TODO\"",
            Some("@ crates/app · regex"),
            false,
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

        assert_visible_summary(
            &virtual_summary,
            "Ran",
            "find android_todo -name \"*.kt\"",
            Some("virtual shell"),
            true,
        );
        assert!(matches!(
            virtual_summary.detail_rows.first(),
            Some(ToolCallDetailRow::Command { command, context, .. })
                if command == "find android_todo -name \"*.kt\""
                    && context.as_deref() == Some("virtual shell")
        ));
        assert_visible_summary(
            &real_summary,
            "Running",
            "cargo check -p app",
            Some("real command"),
            true,
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

        assert_visible_summary(
            &wrote,
            "Wrote",
            "Todo.kt",
            Some("android_todo/app/src"),
            false,
        );
        assert_eq!(
            wrote.target_path.as_deref(),
            Some("android_todo/app/src/Todo.kt")
        );
        assert_visible_summary(&edited, "Editing", "main.rs", Some("crates/app/src"), false);
        assert_eq!(
            edited.target_path.as_deref(),
            Some("crates/app/src/main.rs")
        );
        assert_visible_summary(&deleted, "Deleted", "old.rs", Some("crates/app/src"), false);
        assert_eq!(
            deleted.target_path.as_deref(),
            Some("crates/app/src/old.rs")
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

        assert_visible_summary(
            &summary,
            "Fetched",
            "docs.rs",
            Some("https://docs.rs/gpui/latest/gpui/"),
            false,
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

        assert_visible_summary(&summary, "Processed", "workspace", None, false);
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

        assert_visible_summary(&empty, "Read", "file", None, false);
        assert_visible_summary(&json, "Ran", "command", Some("virtual shell"), true);
    }

    #[test]
    fn line_range_parsing_supports_single_and_open_ranges() {
        assert_eq!(
            split_line_range("src/main.rs:42").1,
            Some((Some(42), Some(42)))
        );
        assert_eq!(
            split_line_range("src/main.rs:10-").1,
            Some((Some(10), None))
        );
        assert_eq!(
            split_line_range("src/main.rs:-20").1,
            Some((None, Some(20)))
        );
        assert_eq!(split_line_range("src/main.rs:not-a-line").1, None);
    }

    #[test]
    fn detail_row_count_tracks_expanded_rows() {
        assert_eq!(
            tool_call_detail_row_count("read_file", Some("src/main.rs:1-2"), &completed()),
            1
        );
        assert_eq!(
            tool_call_detail_row_count("bash_virtual", Some("cargo test"), &completed()),
            1
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
