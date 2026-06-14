//! Flattened row manifest for `v_virtual_list`.
//!
//! Each [`ThreadItem`] becomes one or more lightweight [`RowRef`] rows.
//! Section spacing lives on header rows only; output lines are tight.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{Pixels, Size, px, size};

use crate::features::agent_activity::components::tool_call::tool_call_detail_row_count;
use crate::features::chat::layout::{
    self, APPROVAL_H, DIFF_FILE_H, HEADER_H, LINE_H, PLAN_STATUS_H, REASONING_BODY_MAX_H,
    RUN_ERROR_TITLE_H, SECTION_HEADER_H, TRUNCATED_H, USER_SEE_MORE_H,
};
use crate::features::shell::state::{
    AgentStatus, ChoiceMeta, ChoiceOption, DiffFileSummary, ThreadItem, USER_MESSAGE_PREVIEW_LINES,
    first_user_message_ix, project_timeline, should_emit_thread_item, user_message_truncatable,
};
use crate::shared::components::markdown_preview::{
    LINE_LEADING, MarkdownBlock, estimate_markdown_height, parse_markdown_blocks_shared_streaming,
};
use crate::shared::state::TranscriptMode;
use crate::tokens::Tokens;

/// Matches `todo_row` height in [`crate::features::todos::components::todo_list`].
pub const TODO_ROW_H: f32 = Tokens::ROW_HEIGHT_SM;

pub const TOOL_OUTPUT_PREVIEW_LINES: usize = 80;
pub const TOOL_OUTPUT_PREVIEW_BYTES: usize = 16_000;
pub const REASONING_OUTPUT_PREVIEW_LINES: usize = 40;
/// Faint preview lines shown under the header while reasoning is streaming.
pub const REASONING_STREAMING_PREVIEW_LINES: usize = 2;

/// Position of an activity item within a consecutive run (reasoning / tools / diffs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityGroupPos {
    Single,
    First,
    Middle,
    Last,
}

pub fn activity_group_pos(item_ix: usize, items: &[ThreadItem]) -> Option<ActivityGroupPos> {
    let item = items.get(item_ix)?;
    if !item.is_agent_activity() {
        return None;
    }
    let prev = item_ix > 0 && items[item_ix - 1].is_agent_activity();
    let next = item_ix + 1 < items.len() && items[item_ix + 1].is_agent_activity();
    Some(match (prev, next) {
        (false, false) => ActivityGroupPos::Single,
        (false, true) => ActivityGroupPos::First,
        (true, true) => ActivityGroupPos::Middle,
        (true, false) => ActivityGroupPos::Last,
    })
}

/// Section gap above a header row (exported for streaming height reuse in `ThreadView`).
pub fn row_top_gap(row: RowRef, prev_row: Option<RowRef>, items: &[ThreadItem]) -> f32 {
    top_gap(row, prev_row, items)
}

fn top_gap(row: RowRef, prev_row: Option<RowRef>, items: &[ThreadItem]) -> f32 {
    if matches!(row, RowRef::EndSpacer) {
        return 0.0;
    }
    let item_ix = row.item_ix().expect("content row") as usize;
    let item = &items[item_ix];

    if !row.is_header() {
        return 0.0;
    }

    if prev_row.is_none() {
        return 0.0;
    }

    if item.is_agent_activity() {
        if let Some(prev) = prev_row {
            if prev.item_ix() == Some(item_ix as u32) {
                return 0.0;
            }
            if prev_row_item_is_activity(prev, items) {
                return layout::activity_inner_gap();
            }
            if prev_row_is_assistant(prev, items) {
                return layout::post_assistant_activity_gap();
            }
        }
        return layout::activity_band_gap();
    }

    if let Some(prev) = prev_row {
        if prev_row_item_is_activity(prev, items) {
            return layout::turn_gap();
        }
    }
    layout::turn_gap()
}

fn prev_row_item_is_activity(prev: RowRef, items: &[ThreadItem]) -> bool {
    prev.item_ix()
        .and_then(|ix| items.get(ix as usize))
        .is_some_and(ThreadItem::is_agent_activity)
}

fn prev_row_is_assistant(prev: RowRef, items: &[ThreadItem]) -> bool {
    prev.item_ix().is_some_and(|ix| {
        matches!(
            items.get(ix as usize),
            Some(ThreadItem::AssistantMessage { .. })
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowRef {
    UserMessage { item_ix: u32 },
    SubagentHeader { item_ix: u32 },
    SubagentBody { item_ix: u32 },
    ReasoningHeader { item_ix: u32 },
    ReasoningPreviewLine { item_ix: u32, line_ix: u16 },
    ReasoningBody { item_ix: u32 },
    ToolHeader { item_ix: u32 },
    ToolDetailLine { item_ix: u32, line_ix: u16 },
    ToolOutputLine { item_ix: u32, line_ix: u16 },
    ToolOutputTruncated { item_ix: u32 },
    DiffHeader { item_ix: u32 },
    DiffFileLine { item_ix: u32, file_ix: u16 },
    AssistantMessage { item_ix: u32 },
    Approval { item_ix: u32 },
    RunError { item_ix: u32 },
    ChoiceRequest { item_ix: u32 },
    PlanStatus { item_ix: u32 },
    EndSpacer,
}

impl RowRef {
    pub fn item_ix(&self) -> Option<u32> {
        match self {
            RowRef::EndSpacer => None,
            RowRef::UserMessage { item_ix }
            | RowRef::SubagentHeader { item_ix }
            | RowRef::SubagentBody { item_ix }
            | RowRef::ReasoningHeader { item_ix }
            | RowRef::ReasoningPreviewLine { item_ix, .. }
            | RowRef::ReasoningBody { item_ix }
            | RowRef::ToolHeader { item_ix }
            | RowRef::ToolDetailLine { item_ix, .. }
            | RowRef::ToolOutputLine { item_ix, .. }
            | RowRef::ToolOutputTruncated { item_ix }
            | RowRef::DiffHeader { item_ix }
            | RowRef::DiffFileLine { item_ix, .. }
            | RowRef::AssistantMessage { item_ix }
            | RowRef::Approval { item_ix }
            | RowRef::RunError { item_ix }
            | RowRef::ChoiceRequest { item_ix }
            | RowRef::PlanStatus { item_ix } => Some(*item_ix),
        }
    }

    pub fn is_header(&self) -> bool {
        matches!(
            self,
            RowRef::UserMessage { .. }
                | RowRef::SubagentHeader { .. }
                | RowRef::ReasoningHeader { .. }
                | RowRef::ToolHeader { .. }
                | RowRef::DiffHeader { .. }
                | RowRef::AssistantMessage { .. }
                | RowRef::Approval { .. }
                | RowRef::RunError { .. }
                | RowRef::ChoiceRequest { .. }
                | RowRef::PlanStatus { .. }
        )
    }
}

/// Body height for an assistant row from pre-parsed blocks (no re-parse).
pub fn assistant_content_height(blocks: &[MarkdownBlock], streaming: bool, has_text: bool) -> f32 {
    let mut h = layout::assistant_body_pt() + estimate_markdown_height(blocks, false);
    if streaming && has_text {
        h += layout::assistant_streaming_extra();
    }
    h
}

pub fn assistant_actions_height() -> f32 {
    Tokens::ROW_HEIGHT_SM
}

pub fn assistant_accessory_height(_item_ix: usize, _items: &[ThreadItem]) -> f32 {
    assistant_actions_height() + f32::from(Tokens::spacing_1())
}

pub fn assistant_row_height_from_blocks(
    item_ix: usize,
    items: &[ThreadItem],
    blocks: &[MarkdownBlock],
    streaming: bool,
    has_text: bool,
) -> f32 {
    assistant_content_height(blocks, streaming, has_text)
        + assistant_accessory_height(item_ix, items)
}

pub fn row_height_with_collapsed(
    row: RowRef,
    prev_row: Option<RowRef>,
    items: &[ThreadItem],
    _collapsed: &HashSet<String>,
) -> f32 {
    let gap = top_gap(row, prev_row, items);
    match row {
        RowRef::EndSpacer => Tokens::THREAD_END_SCROLL_PADDING,
        RowRef::UserMessage { item_ix } => {
            let Some(ThreadItem::UserMessage {
                text,
                attachments,
                expanded,
                ..
            }) = items.get(item_ix as usize)
            else {
                return HEADER_H + gap;
            };
            let is_initial = first_user_message_ix(items) == Some(item_ix as usize);
            let truncatable = is_initial && user_message_truncatable(text);
            let collapsed = truncatable && !expanded;
            let lines = if collapsed {
                USER_MESSAGE_PREVIEW_LINES
            } else {
                text.lines().count().max(text.len().div_ceil(55)).max(1)
            };
            let attachments_h = if attachments.is_empty() {
                0.0
            } else {
                f32::from(Tokens::spacing_1()) + Tokens::ATTACHMENT_PREVIEW_SIZE
            };
            let label_h = f32::from(Tokens::text_sm_leading_compact());
            let stack_gap = f32::from(Tokens::spacing_1());
            let top_inset = f32::from(Tokens::spacing_1());
            let see_more = if truncatable {
                stack_gap + USER_SEE_MORE_H
            } else {
                0.0
            };
            top_inset
                + label_h
                + stack_gap
                + lines as f32 * LINE_LEADING
                + see_more
                + attachments_h
                + gap
        }
        RowRef::ReasoningHeader { .. }
        | RowRef::SubagentHeader { .. }
        | RowRef::ToolHeader { .. }
        | RowRef::DiffHeader { .. } => HEADER_H + gap,
        RowRef::PlanStatus { .. } => PLAN_STATUS_H + gap,
        RowRef::SubagentBody { item_ix } => {
            let Some(ThreadItem::SubagentRun { summary, .. }) = items.get(item_ix as usize) else {
                return HEADER_H + gap;
            };
            subagent_body_height(item_ix as usize, summary, items)
        }
        RowRef::ReasoningBody { item_ix } => {
            let Some(ThreadItem::ReasoningStep { summary, .. }) = items.get(item_ix as usize)
            else {
                return HEADER_H + gap;
            };
            layout::assistant_body_pt()
                + estimate_assistant_height(summary).min(REASONING_BODY_MAX_H)
                + gap
        }
        RowRef::ToolDetailLine { .. } => Tokens::ROW_HEIGHT_SM,
        RowRef::ReasoningPreviewLine { item_ix, line_ix }
        | RowRef::ToolOutputLine { item_ix, line_ix } => {
            line_height_for(items, item_ix, line_ix as usize)
        }
        RowRef::ToolOutputTruncated { .. } => TRUNCATED_H,
        RowRef::DiffFileLine { .. } => DIFF_FILE_H,
        RowRef::AssistantMessage { item_ix } => {
            let Some(ThreadItem::AssistantMessage {
                markdown,
                streaming,
                ..
            }) = items.get(item_ix as usize)
            else {
                return HEADER_H + gap;
            };
            let sanitized = crate::agent::text::sanitize_assistant_text(markdown);
            let blocks = parse_markdown_blocks_shared_streaming(&sanitized, *streaming);
            assistant_row_height_from_blocks(
                item_ix as usize,
                items,
                blocks.as_ref(),
                *streaming,
                !sanitized.is_empty(),
            ) + gap
        }
        RowRef::Approval { .. } => APPROVAL_H + gap,
        RowRef::RunError { item_ix } => {
            let message = match items.get(item_ix as usize) {
                Some(ThreadItem::RunError { message, .. }) => message.as_str(),
                _ => "",
            };
            estimate_run_error_height(message) + gap
        }
        RowRef::ChoiceRequest { item_ix } => match items.get(item_ix as usize) {
            Some(ThreadItem::ChoiceRequest {
                prompt,
                options,
                meta,
                resolved,
                ..
            }) => estimate_choice_height(prompt, options, meta, *resolved) + gap,
            _ => SECTION_HEADER_H + gap,
        },
    }
}

#[allow(dead_code)]
pub fn row_height(row: RowRef, prev_row: Option<RowRef>, items: &[ThreadItem]) -> f32 {
    row_height_with_collapsed(row, prev_row, items, &HashSet::new())
}

fn line_height_for(items: &[ThreadItem], item_ix: u32, line_ix: usize) -> f32 {
    let Some(item) = items.get(item_ix as usize) else {
        return LINE_H;
    };

    if let ThreadItem::ReasoningStep {
        summary: _,
        expanded: false,
        status,
        ..
    } = item
    {
        if matches!(status, AgentStatus::Thinking) && line_ix < REASONING_STREAMING_PREVIEW_LINES {
            return LINE_H;
        }
    }

    match item {
        ThreadItem::ToolCall {
            output: Some(_),
            expanded: true,
            ..
        } if line_ix < TOOL_OUTPUT_PREVIEW_LINES => LINE_H,
        ThreadItem::ReasoningStep { expanded: true, .. }
            if line_ix < REASONING_OUTPUT_PREVIEW_LINES =>
        {
            LINE_H
        }
        _ => LINE_H,
    }
}

fn estimate_assistant_height(markdown: &str) -> f32 {
    let sanitized = crate::agent::text::sanitize_assistant_text(markdown);
    let blocks = parse_markdown_blocks_shared_streaming(&sanitized, false);
    estimate_markdown_height(blocks.as_ref(), false)
}

fn estimate_wrapped_text_height(text: &str, chars_per_line: usize, line_height: f32) -> f32 {
    let explicit_lines = text.lines().count().max(1);
    let wrapped_lines = text
        .lines()
        .map(|line| line.chars().count().div_ceil(chars_per_line).max(1))
        .sum::<usize>()
        .max(explicit_lines);
    wrapped_lines as f32 * line_height
}

fn estimate_choice_height(
    prompt: &str,
    options: &[ChoiceOption],
    meta: &ChoiceMeta,
    resolved: bool,
) -> f32 {
    let card_py = f32::from(Tokens::spacing_2());
    let card_gap = f32::from(Tokens::spacing_2());
    let title_gap = f32::from(Tokens::spacing_0p5());

    let summary = meta.summary.as_deref().unwrap_or("Decision needed");
    let summary_h =
        estimate_wrapped_text_height(summary, 60, f32::from(Tokens::text_sm_leading_compact()));
    let prompt_h = estimate_wrapped_text_height(prompt, 44, LINE_LEADING);
    let reason_h = meta
        .blocking_reason
        .as_deref()
        .map(|reason| {
            title_gap
                + estimate_wrapped_text_height(
                    reason,
                    62,
                    f32::from(Tokens::text_sm_leading_compact()),
                )
        })
        .unwrap_or(0.0);
    let text_stack_h = summary_h + title_gap + prompt_h + reason_h;
    let header_h = text_stack_h
        .max(if resolved { 0.0 } else { Tokens::ROW_HEIGHT_SM })
        .max(Tokens::ROW_HEIGHT_SM);

    let mut child_count = 1usize;
    let mut h = card_py * 2.0 + header_h;
    if !options.is_empty() {
        for option in options {
            child_count += 1;
            let label_h = estimate_wrapped_text_height(
                &option.label,
                58,
                f32::from(Tokens::text_sm_leading()),
            )
            .max(Tokens::ROW_HEIGHT_SM - f32::from(Tokens::spacing_1()) * 2.0);
            let mut option_h = f32::from(Tokens::spacing_1()) * 2.0 + label_h;
            if let Some(desc) = &option.description {
                option_h += f32::from(Tokens::spacing_0p5())
                    + estimate_wrapped_text_height(
                        desc,
                        70,
                        f32::from(Tokens::text_sm_leading_compact()),
                    );
            }
            h += option_h;
        }
    }
    if meta.allow_custom {
        child_count += 1;
        h += f32::from(Tokens::text_sm_leading_compact());
    }
    h + child_count.saturating_sub(1) as f32 * card_gap + 2.0
}

fn estimate_run_error_height(message: &str) -> f32 {
    let msg_lines = message.chars().count().div_ceil(55).max(1) as f32;
    let content_h = RUN_ERROR_TITLE_H + layout::run_error_inner_gap() + msg_lines * LINE_LEADING;
    layout::run_error_py()
        + content_h.max(14.0)
        + layout::run_error_py()
        + layout::assistant_body_pt()
}

pub fn reasoning_preview(summary: &str) -> String {
    let first = summary.lines().next().unwrap_or(summary);
    first.chars().take(120).collect()
}

/// Last non-empty lines shown as a faint preview while reasoning streams.
pub fn reasoning_preview_lines(summary: &str, max_lines: usize) -> Vec<String> {
    let lines: Vec<&str> = summary.lines().filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let start = lines.len().saturating_sub(max_lines);
    lines[start..]
        .iter()
        .map(|line| line.chars().take(200).collect())
        .collect()
}

/// Single-line preview for change detection while streaming.
pub fn reasoning_streaming_preview(summary: &str) -> String {
    reasoning_preview_lines(summary, REASONING_STREAMING_PREVIEW_LINES).join(" ")
}

#[allow(dead_code)]
pub fn build_manifest(items: &[ThreadItem]) -> Vec<RowRef> {
    build_manifest_with_transcript(items, TranscriptMode::Normal)
}

pub fn build_manifest_with_transcript(items: &[ThreadItem], mode: TranscriptMode) -> Vec<RowRef> {
    build_manifest_with_collapsed_and_transcript(items, &HashSet::new(), mode)
}

#[allow(dead_code)]
pub fn build_manifest_with_collapsed(
    items: &[ThreadItem],
    collapsed: &HashSet<String>,
) -> Vec<RowRef> {
    build_manifest_with_collapsed_and_transcript(items, collapsed, TranscriptMode::Normal)
}

pub fn build_manifest_with_collapsed_and_transcript(
    items: &[ThreadItem],
    _collapsed: &HashSet<String>,
    mode: TranscriptMode,
) -> Vec<RowRef> {
    let timeline = project_timeline(items, mode);
    let mut manifest = Vec::new();

    for event in &timeline {
        let item_ix = event.item_ix as usize;
        if !should_emit_thread_item(
            items.get(item_ix).expect("timeline item"),
            mode,
            item_ix,
            items,
        ) {
            continue;
        }
        manifest.extend(row_refs_for_item_with_mode(
            event.item_ix,
            items.get(item_ix).expect("item"),
            mode,
            items,
        ));
    }
    manifest.push(RowRef::EndSpacer);
    manifest
}

#[allow(dead_code)]
pub fn section_label_for_row(_row: RowRef) -> Option<&'static str> {
    None
}

pub fn row_sizes_for_manifest(manifest: &[RowRef], items: &[ThreadItem]) -> Vec<Size<Pixels>> {
    manifest
        .iter()
        .enumerate()
        .map(|(ix, &row)| row_size(row, manifest.get(ix.wrapping_sub(1)).copied(), items))
        .collect()
}

#[allow(dead_code)]
pub fn row_sizes_for_manifest_with_collapsed(
    manifest: &[RowRef],
    items: &[ThreadItem],
    collapsed: &HashSet<String>,
) -> Vec<Size<Pixels>> {
    manifest
        .iter()
        .enumerate()
        .map(|(ix, &row)| {
            row_size_with_collapsed(
                row,
                manifest.get(ix.wrapping_sub(1)).copied(),
                items,
                collapsed,
            )
        })
        .collect()
}

#[allow(dead_code)]
pub fn build_header_index(manifest: &[RowRef]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (ix, row) in manifest.iter().enumerate() {
        if row.is_header() {
            if let Some(item_ix) = row.item_ix() {
                map.insert(format!("item-{item_ix}"), ix);
            }
        }
    }
    map
}

#[allow(dead_code)]
pub fn header_manifest_ix(manifest: &[RowRef], item_ix: u32) -> Option<usize> {
    manifest
        .iter()
        .position(|row| row.is_header() && row.item_ix() == Some(item_ix))
}

pub fn row_size(row: RowRef, prev_row: Option<RowRef>, items: &[ThreadItem]) -> Size<Pixels> {
    row_size_with_collapsed(row, prev_row, items, &HashSet::new())
}

pub fn row_size_with_collapsed(
    row: RowRef,
    prev_row: Option<RowRef>,
    items: &[ThreadItem],
    collapsed: &HashSet<String>,
) -> Size<Pixels> {
    size(
        px(Tokens::THREAD_MAX_WIDTH),
        px(row_height_with_collapsed(row, prev_row, items, collapsed)),
    )
}

pub fn row_sizes_for_rows(
    rows: &[RowRef],
    items: &[ThreadItem],
    prev_before_first: Option<RowRef>,
) -> Vec<Size<Pixels>> {
    rows.iter()
        .enumerate()
        .map(|(i, &row)| {
            let prev = if i == 0 {
                prev_before_first
            } else {
                Some(rows[i - 1])
            };
            row_size(row, prev, items)
        })
        .collect()
}

pub fn manifest_span(manifest: &[RowRef], header_ix: usize) -> (usize, usize) {
    let row = &manifest[header_ix];
    if matches!(row, RowRef::EndSpacer) {
        return (header_ix, header_ix + 1);
    }
    let item_ix = row.item_ix().expect("header row");
    let mut end = header_ix + 1;
    while end < manifest.len() {
        match manifest[end] {
            RowRef::EndSpacer => break,
            r if r.item_ix() == Some(item_ix) && !r.is_header() => end += 1,
            _ => break,
        }
    }
    (header_ix, end)
}

#[allow(dead_code)]
pub fn row_refs_for_item(item_ix: u32, item: &ThreadItem, items: &[ThreadItem]) -> Vec<RowRef> {
    row_refs_for_item_with_mode(item_ix, item, TranscriptMode::Normal, items)
}

pub fn row_refs_for_item_with_mode(
    item_ix: u32,
    item: &ThreadItem,
    mode: TranscriptMode,
    all_items: &[ThreadItem],
) -> Vec<RowRef> {
    if !should_emit_thread_item(item, mode, item_ix as usize, all_items) {
        return Vec::new();
    }
    match item {
        ThreadItem::UserMessage { .. } => vec![RowRef::UserMessage { item_ix }],
        ThreadItem::SubagentRun { expanded, .. } => subagent_refs(item_ix, *expanded),
        ThreadItem::ReasoningStep {
            summary,
            expanded,
            status,
            ..
        } => reasoning_refs_with_mode(item_ix, summary, *expanded, status, mode),
        ThreadItem::ToolCall {
            tool_name,
            command,
            output,
            expanded,
            status,
            ..
        } => tool_refs_with_mode(
            item_ix,
            tool_name,
            command.as_deref(),
            output.as_deref(),
            *expanded,
            status,
            mode,
        ),
        ThreadItem::DiffSummary {
            files, expanded, ..
        } => diff_refs(item_ix, files, *expanded),
        ThreadItem::AssistantMessage { .. } => vec![RowRef::AssistantMessage { item_ix }],
        ThreadItem::ApprovalRequest { .. } => vec![RowRef::Approval { item_ix }],
        ThreadItem::RunError { .. } => vec![RowRef::RunError { item_ix }],
        ThreadItem::ChoiceRequest { .. } => vec![RowRef::ChoiceRequest { item_ix }],
        ThreadItem::PlanStatus { .. } => vec![RowRef::PlanStatus { item_ix }],
        ThreadItem::ContextTrace { .. } => vec![],
        ThreadItem::TodoList { .. } => vec![],
    }
}

fn subagent_refs(item_ix: u32, expanded: bool) -> Vec<RowRef> {
    let mut rows = vec![RowRef::SubagentHeader { item_ix }];
    if expanded {
        rows.push(RowRef::SubagentBody { item_ix });
    }
    rows
}

fn subagent_body_height(item_ix: usize, summary: &str, items: &[ThreadItem]) -> f32 {
    let summary_text = if summary.trim().is_empty() {
        "Investigating task in child run."
    } else {
        summary
    };
    let summary_lines = summary_text.chars().count().div_ceil(72).clamp(1, 3) as f32;
    let meta_lines = if matches!(items.get(item_ix), Some(ThreadItem::SubagentRun { .. })) {
        2.0
    } else {
        1.0
    };
    f32::from(Tokens::spacing_1())
        + f32::from(Tokens::spacing_2())
        + summary_lines * f32::from(Tokens::text_sm_leading_compact())
        + f32::from(Tokens::spacing_1())
        + meta_lines * f32::from(Tokens::text_sm_leading_compact())
}

fn reasoning_refs_with_mode(
    item_ix: u32,
    summary: &str,
    expanded: bool,
    status: &AgentStatus,
    mode: TranscriptMode,
) -> Vec<RowRef> {
    if !mode.shows_reasoning_rows() {
        return Vec::new();
    }
    reasoning_refs(item_ix, summary, expanded, status)
}

fn reasoning_refs(
    item_ix: u32,
    summary: &str,
    expanded: bool,
    status: &AgentStatus,
) -> Vec<RowRef> {
    if summary.trim().is_empty() && !matches!(status, AgentStatus::Thinking) {
        return Vec::new();
    }
    let mut rows = vec![RowRef::ReasoningHeader { item_ix }];
    if expanded {
        if !summary.trim().is_empty() {
            rows.push(RowRef::ReasoningBody { item_ix });
        }
        return rows;
    }
    if matches!(status, AgentStatus::Thinking) && !summary.trim().is_empty() {
        let preview = reasoning_preview_lines(summary, REASONING_STREAMING_PREVIEW_LINES);
        for (line_ix, _) in preview.iter().enumerate() {
            rows.push(RowRef::ReasoningPreviewLine {
                item_ix,
                line_ix: line_ix as u16,
            });
        }
    }
    rows
}

fn tool_refs_with_mode(
    item_ix: u32,
    tool_name: &str,
    command: Option<&str>,
    output: Option<&str>,
    expanded: bool,
    status: &AgentStatus,
    mode: TranscriptMode,
) -> Vec<RowRef> {
    let mut rows = vec![RowRef::ToolHeader { item_ix }];
    if !expanded {
        return rows;
    }

    if output.is_some() {
        tool_output_rows(item_ix, output, status, &mut rows);
        return rows;
    }

    if mode.shows_tool_output_rows() {
        for line_ix in 0..tool_call_detail_row_count(tool_name, command, status) {
            rows.push(RowRef::ToolDetailLine {
                item_ix,
                line_ix: line_ix as u16,
            });
        }
    }

    rows
}

#[allow(dead_code)]
fn tool_refs(
    item_ix: u32,
    output: Option<&str>,
    expanded: bool,
    status: &AgentStatus,
) -> Vec<RowRef> {
    let mut rows = vec![RowRef::ToolHeader { item_ix }];
    if !expanded {
        return rows;
    }
    tool_output_rows(item_ix, output, status, &mut rows);
    rows
}

fn tool_output_rows(
    item_ix: u32,
    output: Option<&str>,
    _status: &AgentStatus,
    rows: &mut Vec<RowRef>,
) {
    match output {
        Some(out) => {
            let (preview_lines, _, truncated, _) = build_tool_output_preview(out);
            for line_ix in 0..preview_lines.len() {
                rows.push(RowRef::ToolOutputLine {
                    item_ix,
                    line_ix: line_ix as u16,
                });
            }
            if truncated {
                rows.push(RowRef::ToolOutputTruncated { item_ix });
            }
        }
        None => {}
    }
}

fn diff_refs(item_ix: u32, files: &[DiffFileSummary], expanded: bool) -> Vec<RowRef> {
    let mut rows = vec![RowRef::DiffHeader { item_ix }];
    if !expanded {
        return rows;
    }
    for (file_ix, _) in files.iter().enumerate() {
        rows.push(RowRef::DiffFileLine {
            item_ix,
            file_ix: file_ix as u16,
        });
    }
    rows
}

pub fn build_tool_output_preview(output: &str) -> (Arc<[Arc<str>]>, usize, bool, Arc<str>) {
    let full = Arc::from(output);
    let total_lines = output.lines().count();
    let mut preview_lines: Vec<Arc<str>> = Vec::new();
    let mut byte_count = 0usize;
    let mut truncated = false;

    for line in output.lines() {
        if preview_lines.len() >= TOOL_OUTPUT_PREVIEW_LINES {
            truncated = true;
            break;
        }
        let line_bytes = line.len() + 1;
        if byte_count + line_bytes > TOOL_OUTPUT_PREVIEW_BYTES && !preview_lines.is_empty() {
            truncated = true;
            break;
        }
        byte_count += line_bytes;
        preview_lines.push(Arc::from(line));
    }

    if total_lines > preview_lines.len() {
        truncated = true;
    }

    (Arc::from(preview_lines), total_lines, truncated, full)
}

#[allow(dead_code)]
pub fn tool_output_line_text(item: &ThreadItem, line_ix: usize) -> Option<Arc<str>> {
    let ThreadItem::ToolCall {
        output: Some(out),
        expanded: true,
        ..
    } = item
    else {
        return None;
    };
    if line_ix == 0 && out.is_empty() {
        return Some(Arc::from("…"));
    }
    out.lines().nth(line_ix).map(Arc::from)
}

pub fn reasoning_preview_line_text(item: &ThreadItem, line_ix: usize) -> Option<Arc<str>> {
    let ThreadItem::ReasoningStep {
        summary,
        expanded: false,
        status,
        ..
    } = item
    else {
        return None;
    };
    if !matches!(status, AgentStatus::Thinking) {
        return None;
    }
    reasoning_preview_lines(summary, REASONING_STREAMING_PREVIEW_LINES)
        .get(line_ix)
        .map(|line| Arc::from(line.as_str()))
}

pub fn collapsed_item_header_changed(new: &ThreadItem, old: &ThreadItem) -> bool {
    if new.is_expanded() || old.is_expanded() {
        return new.is_expanded() != old.is_expanded() || new != old;
    }
    match (new, old) {
        (
            ThreadItem::ToolCall {
                tool_name,
                command,
                status,
                output,
                ..
            },
            ThreadItem::ToolCall {
                tool_name: old_name,
                command: old_cmd,
                status: old_st,
                output: old_out,
                ..
            },
        ) => {
            tool_name != old_name
                || command != old_cmd
                || status != old_st
                || output.is_some() != old_out.is_some()
        }
        (
            ThreadItem::ReasoningStep {
                title,
                summary,
                status,
                ..
            },
            ThreadItem::ReasoningStep {
                title: old_title,
                summary: old_summary,
                status: old_st,
                ..
            },
        ) => {
            let preview = |s: &str, st: &AgentStatus| {
                if matches!(st, AgentStatus::Thinking) {
                    reasoning_streaming_preview(s)
                } else {
                    reasoning_preview(s)
                }
            };
            title != old_title
                || status != old_st
                || preview(summary, status) != preview(old_summary, old_st)
        }
        (
            ThreadItem::DiffSummary {
                files_changed,
                additions,
                deletions,
                ..
            },
            ThreadItem::DiffSummary {
                files_changed: fc,
                additions: a,
                deletions: d,
                ..
            },
        ) => files_changed != fc || additions != a || deletions != d,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::shell::state::{PlanExecutionState, PlanProgressCounts};

    #[test]
    fn subagent_emits_single_header_row() {
        let item = ThreadItem::SubagentRun {
            id: "subagent-1".into(),
            task: "Inspect rendering".into(),
            model: "fast-model".into(),
            summary: String::new(),
            expanded: true,
            status: AgentStatus::RunningTool,
            child_run_id: "child-1".into(),
            parent_call_id: "call-1".into(),
        };
        let refs = row_refs_for_item_with_mode(
            0,
            &item,
            TranscriptMode::Verbose,
            std::slice::from_ref(&item),
        );
        assert_eq!(
            refs,
            vec![
                RowRef::SubagentHeader { item_ix: 0 },
                RowRef::SubagentBody { item_ix: 0 }
            ]
        );
        assert!(row_height(refs[0], None, &[item]) > 0.0);
    }

    #[test]
    fn child_activity_keeps_own_rows() {
        let items = vec![
            ThreadItem::SubagentRun {
                id: "subagent-1".into(),
                task: "Inspect rendering".into(),
                model: "fast-model".into(),
                summary: String::new(),
                expanded: true,
                status: AgentStatus::RunningTool,
                child_run_id: "child-1".into(),
                parent_call_id: "call-1".into(),
            },
            ThreadItem::ReasoningStep {
                id: "reason-child".into(),
                title: "Thinking".into(),
                summary: "Checking rows".into(),
                expanded: false,
                status: AgentStatus::Thinking,
                depth: 1,
                parent_call_id: Some("call-1".into()),
            },
        ];
        let manifest = build_manifest_with_transcript(&items, TranscriptMode::Verbose);
        assert!(manifest.contains(&RowRef::SubagentHeader { item_ix: 0 }));
        assert!(manifest.contains(&RowRef::SubagentBody { item_ix: 0 }));
        assert!(manifest.contains(&RowRef::ReasoningHeader { item_ix: 1 }));
    }

    #[test]
    fn choice_request_height_accounts_for_prompt_options_and_custom_hint() {
        let item = ThreadItem::ChoiceRequest {
            id: "choice-1".into(),
            prompt: "What would you like to focus on first for this portfolio app, given the layout, visual style, and implementation constraints?".into(),
            options: vec![
                ChoiceOption {
                    id: "design".into(),
                    label: "Start with UI/UX design concepts".into(),
                    description: Some("Explore typography, layout direction, color, interaction patterns, and responsive structure before implementation.".into()),
                    recommended: true,
                },
                ChoiceOption {
                    id: "features".into(),
                    label: "Define core features such as project showcase, resume builder, and contact form".into(),
                    description: Some("Prioritize the actual portfolio sections and behavior before investing in visual polish.".into()),
                    recommended: false,
                },
                ChoiceOption {
                    id: "stack".into(),
                    label: "Choose tech stack including Rust, WebAssembly, or CSS frameworks".into(),
                    description: Some("Lock implementation technology before building screens.".into()),
                    recommended: false,
                },
            ],
            meta: ChoiceMeta {
                summary: Some("Portfolio App Requirements".into()),
                recommended_option_id: Some("design".into()),
                allow_custom: true,
                blocking_reason: Some("The agent needs this decision before continuing.".into()),
            },
            selected: None,
            resolved: false,
        };
        let items = vec![item];
        let height = row_height(RowRef::ChoiceRequest { item_ix: 0 }, None, &items);
        assert!(height > 300.0, "choice card height was {height}");
    }

    #[test]
    fn assistant_markdown_height_accounts_for_lists_tables_code_and_actions() {
        let markdown = r##"
Portfolio app plan for HTML/JS implementation.

1. Create index.html with:
   - Responsive navigation bar
   - Hero section with CTA
   - Project grid

| File | Purpose |
| --- | --- |
| index.html | Semantic document structure with multiple sections |
| style.css | Responsive styling and motion |

```js
const year = new Date().getFullYear();
document.querySelector("#year").textContent = year;
```
"##;
        let item = ThreadItem::AssistantMessage {
            id: "assistant-1".into(),
            markdown: markdown.into(),
            streaming: false,
            depth: 0,
            parent_call_id: None,
        };
        let items = vec![item];
        let height = row_height(RowRef::AssistantMessage { item_ix: 0 }, None, &items);
        assert!(height > 260.0, "assistant row height was {height}");
    }

    #[test]
    fn plan_status_uses_compact_activity_row_height() {
        let item = ThreadItem::PlanStatus {
            id: "plan-1".into(),
            state: PlanExecutionState::NotStarted,
            summary: "Ready to implement".into(),
            counts: PlanProgressCounts {
                pending: 3,
                in_progress: 0,
                completed: 0,
                cancelled: 0,
            },
            source_conversation_id: None,
        };
        let items = vec![item];
        assert_eq!(
            row_height(RowRef::PlanStatus { item_ix: 0 }, None, &items),
            Tokens::TOOL_ROW_HEIGHT
        );
    }

    #[test]
    fn expanded_tool_output_rows_include_truncation_notice() {
        let output = (0..(TOOL_OUTPUT_PREVIEW_LINES + 8))
            .map(|ix| format!("line {ix}"))
            .collect::<Vec<_>>()
            .join("\n");
        let item = ThreadItem::ToolCall {
            id: "tool-1".into(),
            tool_name: "list_files".into(),
            command: Some("ls".into()),
            output: Some(output),
            expanded: true,
            status: AgentStatus::Completed,
            depth: 0,
            parent_call_id: None,
        };
        let refs = row_refs_for_item_with_mode(
            0,
            &item,
            TranscriptMode::Verbose,
            std::slice::from_ref(&item),
        );
        assert_eq!(refs.first(), Some(&RowRef::ToolHeader { item_ix: 0 }));
        assert!(refs.contains(&RowRef::ToolOutputTruncated { item_ix: 0 }));
        assert_eq!(
            row_height(RowRef::ToolOutputTruncated { item_ix: 0 }, None, &[item]),
            TRUNCATED_H
        );
    }

    #[test]
    fn expanded_tool_in_normal_mode_shows_output_not_summary_detail() {
        let item = ThreadItem::ToolCall {
            id: "tool-1".into(),
            tool_name: "search_project".into(),
            command: Some("in **/*.kt @ android_todo".into()),
            output: Some("app/src/MainActivity.kt\napp/src/TodoRepository.kt".into()),
            expanded: true,
            status: AgentStatus::Completed,
            depth: 0,
            parent_call_id: None,
        };

        let refs = row_refs_for_item_with_mode(
            0,
            &item,
            TranscriptMode::Normal,
            std::slice::from_ref(&item),
        );

        assert_eq!(refs.first(), Some(&RowRef::ToolHeader { item_ix: 0 }));
        assert!(refs.contains(&RowRef::ToolOutputLine {
            item_ix: 0,
            line_ix: 0
        }));
        assert!(
            !refs
                .iter()
                .any(|row| matches!(row, RowRef::ToolDetailLine { .. }))
        );
    }
}
