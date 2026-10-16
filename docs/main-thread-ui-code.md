# Main Thread UI Code

This document collects the Rust source for the main thread UI components and directly referenced support files from `docs/main-thread-ui-report.md`. It is intended as a portable source bundle for reworking the thread UI in another project.

Generated from the current workspace files.

## Included files

### Core thread UI files

- `crates/app/src/features/chat/layout.rs`
- `crates/app/src/features/chat/manifest.rs`
- `crates/app/src/features/chat/state.rs`
- `crates/app/src/features/chat/components/mod.rs`
- `crates/app/src/features/chat/components/message.rs`
- `crates/app/src/features/chat/components/choice_card.rs`
- `crates/app/src/features/chat/components/error_card.rs`
- `crates/app/src/features/chat/components/context_chip.rs`
- `crates/app/src/features/chat/thread_view/mod.rs`
- `crates/app/src/features/chat/thread_view/render.rs`
- `crates/app/src/features/chat/thread_view/rows.rs`
- `crates/app/src/features/chat/thread_view/caches.rs`
- `crates/app/src/features/chat/thread_view/scroll.rs`
- `crates/app/src/features/chat/thread_view/sync.rs`

### Agent activity components

- `crates/app/src/features/agent_activity/components/mod.rs`
- `crates/app/src/features/agent_activity/components/activity_step.rs`
- `crates/app/src/features/agent_activity/components/reasoning.rs`
- `crates/app/src/features/agent_activity/components/tool_call.rs`
- `crates/app/src/features/agent_activity/components/diff.rs`
- `crates/app/src/features/agent_activity/components/context_trace.rs`
- `crates/app/src/features/agent_activity/components/approval.rs`
- `crates/app/src/features/agent_activity/components/pending_action_bar.rs`

### Adjacent thread UI components

- `crates/app/src/features/composer/components/mode_chips.rs`
- `crates/app/src/features/todos/components/mod.rs`
- `crates/app/src/features/todos/components/todo_list.rs`
- `crates/app/src/features/todos/mod.rs`

### Thread schemas and state

- `crates/app/src/features/shell/state.rs`
- `crates/app/src/shared/state/transcript.rs`
- `crates/agent_protocol/src/event.rs`
- `crates/agent_protocol/src/mode.rs`
- `crates/agent_protocol/src/command.rs`

### Thread integration and reducer glue

- `crates/app/src/window/render.rs`
- `crates/app/src/window/types.rs`
- `crates/app/src/window/bridge/thread_bridge.rs`
- `crates/app/src/window/bridge/subagent_bridge.rs`
- `crates/app/src/window/orchestration/agent_run.rs`
- `crates/app/src/agent/reducer.rs`

---

## Core thread UI files

### `crates/app/src/features/chat/layout.rs`

```rust
//! Shared thread layout constants — gaps and heights for manifest + components.

use crate::shared::components::markdown_preview::LINE_LEADING;
use crate::tokens::Tokens;

#[inline]
fn px(v: gpui::Pixels) -> f32 {
    v.into()
}

/// Space before a new conversational turn — compact but readable.
pub fn turn_gap() -> f32 {
    px(Tokens::spacing_1p5())
}

/// Space before the first row in an agent activity band.
pub fn activity_band_gap() -> f32 {
    px(Tokens::spacing_1())
}

/// Tight stack between consecutive activity headers — `Tokens::spacing_0p5()`.
pub fn activity_inner_gap() -> f32 {
    px(Tokens::spacing_0p5())
}

/// Space between assistant prose and the following tool/reasoning band.
pub fn post_assistant_activity_gap() -> f32 {
    px(Tokens::spacing_1())
}

/// Vertical padding inside user message bubble (both sides) — `Tokens::spacing_4()`.
pub fn user_bubble_py() -> f32 {
    px(Tokens::spacing_4())
}

/// Height of truncated-output notice rows — `Tokens::ROW_HEIGHT_XS`.
pub const TRUNCATED_H: f32 = Tokens::ROW_HEIGHT_XS;

/// Vertical padding inside run-error row (both sides) — `Tokens::spacing_4()`.
pub fn run_error_py() -> f32 {
    px(Tokens::spacing_4())
}

/// Title line height in run-error estimate — `LINE_LEADING`.
pub const RUN_ERROR_TITLE_H: f32 = LINE_LEADING;

/// Gap between error title and message — `Tokens::spacing_0p5()`.
pub fn run_error_inner_gap() -> f32 {
    px(Tokens::spacing_0p5())
}

/// Max expanded reasoning body height.
pub const REASONING_BODY_MAX_H: f32 = 480.0;

/// Vertical chrome around an expanded activity body (`timeline_body`).
pub fn activity_body_chrome_h() -> f32 {
    px(Tokens::spacing_1()) + px(Tokens::spacing_2())
}

/// Section header row height — `Tokens::ROW_HEIGHT_SM`.
pub const SECTION_HEADER_H: f32 = Tokens::ROW_HEIGHT_SM;

/// Gap between timeline sections — `Tokens::spacing_2()`.
pub fn section_gap() -> f32 {
    px(Tokens::spacing_2())
}

/// "See more" control height under collapsed user messages.
pub const USER_SEE_MORE_H: f32 = 22.0;

/// Activity header row height — `Tokens::TOOL_ROW_HEIGHT`.
pub const HEADER_H: f32 = Tokens::TOOL_ROW_HEIGHT;

/// Compact plan status row height — `Tokens::TOOL_ROW_HEIGHT`.
pub const PLAN_STATUS_H: f32 = Tokens::TOOL_ROW_HEIGHT;

/// Monospace output line height — `Tokens::DIFF_LINE_HEIGHT`.
pub const LINE_H: f32 = Tokens::DIFF_LINE_HEIGHT;

/// Diff file stat line height — `Tokens::ROW_HEIGHT_SM`.
pub const DIFF_FILE_H: f32 = Tokens::ROW_HEIGHT_SM;

/// Approval row height — `Tokens::ROW_HEIGHT_SM`.
pub const APPROVAL_H: f32 = Tokens::ROW_HEIGHT_SM;

/// Top padding on assistant markdown body.
pub fn assistant_body_pt() -> f32 {
    0.0
}

/// Extra height for streaming cursor under assistant body — `Tokens::spacing_2()`.
pub fn assistant_streaming_extra() -> f32 {
    px(Tokens::spacing_2())
}

```

### `crates/app/src/features/chat/manifest.rs`

````rust
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
    ContextTraceHeader { item_ix: u32 },
    ContextTraceLine { item_ix: u32, line_ix: u16 },
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
            | RowRef::ContextTraceHeader { item_ix }
            | RowRef::ContextTraceLine { item_ix, .. }
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
                | RowRef::ContextTraceHeader { .. }
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
        | RowRef::DiffHeader { .. }
        | RowRef::ContextTraceHeader { .. } => HEADER_H + gap,
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
                + layout::activity_body_chrome_h()
                + gap
        }
        RowRef::ToolDetailLine { .. } => Tokens::ROW_HEIGHT_SM,
        RowRef::ReasoningPreviewLine { item_ix, line_ix }
        | RowRef::ToolOutputLine { item_ix, line_ix } => {
            line_height_for(items, item_ix, line_ix as usize)
        }
        RowRef::ToolOutputTruncated { .. } => TRUNCATED_H,
        RowRef::DiffFileLine { .. } => DIFF_FILE_H,
        RowRef::ContextTraceLine { .. } => Tokens::ROW_HEIGHT_MD,
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
        ThreadItem::ContextTrace {
            entries, expanded, ..
        } => context_trace_refs(item_ix, entries.len(), *expanded),
        ThreadItem::TodoList { .. } => vec![],
    }
}

fn context_trace_refs(item_ix: u32, entry_count: usize, expanded: bool) -> Vec<RowRef> {
    if entry_count == 0 {
        return Vec::new();
    }
    let mut rows = vec![RowRef::ContextTraceHeader { item_ix }];
    if expanded {
        rows.extend((0..entry_count.min(u16::MAX as usize)).map(|line_ix| {
            RowRef::ContextTraceLine {
                item_ix,
                line_ix: line_ix as u16,
            }
        }));
    }
    rows
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

````

### `crates/app/src/features/chat/state.rs`

```rust
//! Timeline projection — normalized events from thread items.

use crate::features::agent_activity::state::{ActivityPhase, phase_for_tool_name};
use crate::features::shell::state::{AgentStatus, ThreadItem};
use crate::shared::state::TranscriptMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TimelineEventKind {
    UserMessage,
    AssistantMessage,
    FinalSummary,
    ReasoningNote,
    FileRead,
    FileSearch,
    FileEdit,
    CommandRun,
    SubagentRun,
    DiffSummary,
    ApprovalRequest,
    ChoiceRequest,
    Error,
    ContextTrace,
    TodoUpdate,
    PlanCreated,
    PlanUpdated,
    PlanningStatus,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TimelineEvent {
    pub id: String,
    pub item_ix: u32,
    pub kind: TimelineEventKind,
    pub label: String,
    pub detail: Option<String>,
    pub phase: ActivityPhase,
    pub status: AgentStatus,
    pub artifact_id: Option<String>,
    pub additions: Option<usize>,
    pub deletions: Option<usize>,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct TaskViewModel {
    pub goal: String,
    pub timeline: Vec<TimelineEvent>,
}

pub fn project_timeline(items: &[ThreadItem], mode: TranscriptMode) -> Vec<TimelineEvent> {
    let mut events = Vec::new();
    for (item_ix, item) in items.iter().enumerate() {
        if let Some(event) = project_item(item_ix as u32, item, mode, items) {
            events.push(event);
        }
    }
    events
}

pub fn build_task_view(title: &str, items: &[ThreadItem], mode: TranscriptMode) -> TaskViewModel {
    let goal = if title.is_empty() {
        items
            .iter()
            .find_map(|item| {
                if let ThreadItem::UserMessage { text, .. } = item {
                    Some(
                        text.lines()
                            .next()
                            .unwrap_or(text)
                            .chars()
                            .take(80)
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "New task".into())
    } else {
        title.to_string()
    };
    TaskViewModel {
        goal,
        timeline: project_timeline(items, mode),
    }
}

fn project_item(
    item_ix: u32,
    item: &ThreadItem,
    mode: TranscriptMode,
    all_items: &[ThreadItem],
) -> Option<TimelineEvent> {
    match item {
        ThreadItem::UserMessage { id, text, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::UserMessage,
            label: text
                .lines()
                .next()
                .unwrap_or(text)
                .chars()
                .take(80)
                .collect(),
            detail: None,
            phase: ActivityPhase::Explore,
            status: AgentStatus::Completed,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::AssistantMessage { id, streaming, .. } => {
            let is_final =
                crate::features::chat::state::is_final_assistant(item_ix as usize, all_items);
            let kind = if is_final && mode == TranscriptMode::Summary {
                TimelineEventKind::FinalSummary
            } else {
                TimelineEventKind::AssistantMessage
            };
            Some(TimelineEvent {
                id: id.clone(),
                item_ix,
                kind,
                label: "Assistant".into(),
                detail: None,
                phase: ActivityPhase::Review,
                status: if *streaming {
                    AgentStatus::Thinking
                } else {
                    AgentStatus::Completed
                },
                artifact_id: None,
                additions: None,
                deletions: None,
            })
        }
        ThreadItem::PlanStatus { .. } => None,
        ThreadItem::SubagentRun {
            id,
            task,
            model,
            status,
            ..
        } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::SubagentRun,
            label: format!("Subagent: {task}"),
            detail: Some(model.clone()),
            phase: ActivityPhase::Run,
            status: status.clone(),
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::ReasoningStep {
            id,
            title,
            summary,
            status,
            ..
        } => {
            if !mode.shows_reasoning_rows() {
                return None;
            }
            let preview = summary
                .lines()
                .next()
                .unwrap_or(title)
                .chars()
                .take(80)
                .collect();
            Some(TimelineEvent {
                id: id.clone(),
                item_ix,
                kind: TimelineEventKind::ReasoningNote,
                label: title.clone(),
                detail: Some(preview),
                phase: ActivityPhase::Explore,
                status: status.clone(),
                artifact_id: None,
                additions: None,
                deletions: None,
            })
        }
        ThreadItem::ToolCall {
            id,
            tool_name,
            command,
            status,
            ..
        } => {
            let phase = phase_for_tool_name(tool_name);
            let kind = tool_kind_from_name(tool_name);
            Some(TimelineEvent {
                id: id.clone(),
                item_ix,
                kind,
                label: tool_name.clone(),
                detail: command.clone(),
                phase,
                status: status.clone(),
                artifact_id: Some(format!("tool-{id}")),
                additions: None,
                deletions: None,
            })
        }
        ThreadItem::DiffSummary {
            id,
            files_changed,
            additions,
            deletions,
            ..
        } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::FileEdit,
            label: format!("{files_changed} files changed"),
            detail: Some(format!("+{additions} −{deletions}")),
            phase: ActivityPhase::Edit,
            status: AgentStatus::Completed,
            artifact_id: Some(format!("diff-{id}")),
            additions: Some(*additions),
            deletions: Some(*deletions),
        }),
        ThreadItem::ApprovalRequest { id, title, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::ApprovalRequest,
            label: title.clone(),
            detail: None,
            phase: ActivityPhase::Review,
            status: AgentStatus::WaitingApproval,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::RunError { id, message, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::Error,
            label: message
                .lines()
                .next()
                .unwrap_or(message)
                .chars()
                .take(80)
                .collect(),
            detail: Some(message.clone()),
            phase: ActivityPhase::Run,
            status: AgentStatus::Failed,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::ChoiceRequest { id, prompt, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::ChoiceRequest,
            label: prompt.chars().take(80).collect(),
            detail: None,
            phase: ActivityPhase::Review,
            status: AgentStatus::WaitingApproval,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::ContextTrace { id, entries, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::ContextTrace,
            label: "Context".into(),
            detail: Some(format!("{} items", entries.len())),
            phase: ActivityPhase::Explore,
            status: AgentStatus::Completed,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::TodoList { .. } => None,
    }
}

fn tool_kind_from_name(name: &str) -> TimelineEventKind {
    match name {
        "read_file" | "Read" => TimelineEventKind::FileRead,
        "search" | "grep" | "glob_file_search" | "codebase_search" => TimelineEventKind::FileSearch,
        "propose_patch" | "apply_patch" | "edit_file" | "write" => TimelineEventKind::FileEdit,
        "bash_virtual" | "run_real_command" | "RunCommand" | "shell" => {
            TimelineEventKind::CommandRun
        }
        _ => TimelineEventKind::CommandRun,
    }
}

pub fn should_emit_thread_item(
    item: &ThreadItem,
    mode: TranscriptMode,
    item_ix: usize,
    items: &[ThreadItem],
) -> bool {
    match mode {
        TranscriptMode::Summary => match item {
            ThreadItem::UserMessage { .. }
            | ThreadItem::RunError { .. }
            | ThreadItem::ApprovalRequest { .. }
            | ThreadItem::ChoiceRequest { .. } => true,
            ThreadItem::AssistantMessage {
                streaming: true, ..
            } => true,
            ThreadItem::AssistantMessage { .. } => is_final_assistant(item_ix, items),
            ThreadItem::ReasoningStep { status, .. } => {
                matches!(status, AgentStatus::Thinking)
            }
            ThreadItem::ToolCall { status, .. } => matches!(
                status,
                AgentStatus::RunningTool | AgentStatus::WaitingApproval
            ),
            ThreadItem::DiffSummary { .. }
            | ThreadItem::SubagentRun { .. }
            | ThreadItem::ContextTrace { .. }
            | ThreadItem::TodoList { .. }
            | ThreadItem::PlanStatus { .. } => false,
        },
        _ => match item {
            ThreadItem::ReasoningStep { status, .. } => {
                mode.shows_reasoning_rows() || matches!(status, AgentStatus::Thinking)
            }
            ThreadItem::TodoList { .. } => false,
            _ => true,
        },
    }
}

/// Last non-streaming assistant in the thread (ignoring trailing todo lists).
pub fn is_final_assistant(item_ix: usize, items: &[ThreadItem]) -> bool {
    match items.get(item_ix) {
        Some(ThreadItem::AssistantMessage {
            streaming: false, ..
        }) => items.get(item_ix + 1..).is_none_or(|rest| {
            rest.iter()
                .all(|i| matches!(i, ThreadItem::TodoList { .. }))
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_msg(id: &str) -> ThreadItem {
        ThreadItem::UserMessage {
            id: id.into(),
            text: "hi".into(),
            attachments: Vec::new(),
            expanded: false,
        }
    }

    fn assistant(id: &str, streaming: bool) -> ThreadItem {
        ThreadItem::AssistantMessage {
            id: id.into(),
            markdown: String::new(),
            streaming,
            depth: 0,
            parent_call_id: None,
        }
    }

    fn thinking() -> ThreadItem {
        ThreadItem::ReasoningStep {
            id: "r1".into(),
            title: "Thinking".into(),
            summary: String::new(),
            expanded: false,
            status: AgentStatus::Thinking,
            depth: 0,
            parent_call_id: None,
        }
    }

    #[test]
    fn summary_shows_streaming_assistant() {
        let items = vec![user_msg("u1"), assistant("a1", true)];
        assert!(should_emit_thread_item(
            &items[1],
            TranscriptMode::Summary,
            1,
            &items,
        ));
    }

    #[test]
    fn summary_hides_completed_intermediate_assistant() {
        let items = vec![
            user_msg("u1"),
            assistant("a1", false),
            assistant("a2", true),
        ];
        assert!(!should_emit_thread_item(
            &items[1],
            TranscriptMode::Summary,
            1,
            &items,
        ));
    }

    #[test]
    fn normal_shows_thinking_row() {
        let items = vec![user_msg("u1"), thinking()];
        assert!(should_emit_thread_item(
            &items[1],
            TranscriptMode::Normal,
            1,
            &items,
        ));
    }
}

```

### `crates/app/src/features/chat/components/mod.rs`

```rust
pub mod choice_card;
pub mod context_chip;
pub mod error_card;
pub mod message;

```

### `crates/app/src/features/chat/components/message.rs`

```rust
//! Stateless message components — full-width, minimal chrome.

use std::sync::Arc;

use crate::features::shell::state::{MessageAttachment, USER_MESSAGE_PREVIEW_LINES};
use crate::shared::components::buttons::btn_copy_icon_arc;
use crate::shared::components::markdown_preview::LINE_LEADING;
use crate::tokens::{Tokens, braille_spinner, element_key};
use gpui::{
    App, Image, ImageFormat, IntoElement, ObjectFit, StyledImage, div, img, prelude::*, px,
};

/// User turn — flatter Zed-like row with just a subtle accent wash.
pub fn user_message(
    text: &str,
    attachments: &[MessageAttachment],
    id: &str,
    collapsed: bool,
    show_see_more: bool,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let text = text.to_string();
    let copy_text = std::sync::Arc::<str>::from(text.as_str());
    let preview_max_h = px(USER_MESSAGE_PREVIEW_LINES as f32 * LINE_LEADING);
    div()
        .id(element_key("user-message", id))
        .w_full()
        .flex()
        .items_start()
        .gap(Tokens::spacing_2())
        .pt(Tokens::spacing_1())
        .child(
            div()
                .mt(Tokens::spacing_0p5())
                .w(px(2.0))
                .h(px(18.0))
                .rounded(Tokens::radius_full())
                .bg(Tokens::accent().opacity(0.8)),
        )
        .child(
            div()
                .max_w(px(Tokens::THREAD_MAX_WIDTH * 0.88))
                .relative()
                .flex()
                .flex_col()
                .items_start()
                .gap(Tokens::spacing_1())
                .min_w(px(0.0))
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .line_height(Tokens::text_sm_leading_compact())
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(Tokens::text_tertiary())
                        .child("You"),
                )
                .child(
                    div()
                        .w_full()
                        .pr(Tokens::spacing_6())
                        .text_size(Tokens::text_md())
                        .line_height(Tokens::text_md_leading())
                        .text_color(Tokens::text_primary())
                        .when(collapsed, |el| el.max_h(preview_max_h).overflow_hidden())
                        .child(text.clone()),
                )
                .when(!attachments.is_empty(), |el| {
                    el.child(div().flex().flex_wrap().gap(Tokens::spacing_1()).children(
                        attachments.iter().map(|attachment| {
                            div()
                                .w(Tokens::attachment_preview_size())
                                .h(Tokens::attachment_preview_size())
                                .overflow_hidden()
                                .rounded(Tokens::radius_sm())
                                .border_1()
                                .border_color(Tokens::border_subtle())
                                .bg(Tokens::surface_active())
                                .child(render_message_attachment_preview(attachment))
                        }),
                    ))
                })
                .when(show_see_more, |el| {
                    el.child(
                        div()
                            .id(element_key("user-message-see-more", id))
                            .text_size(Tokens::text_xs())
                            .line_height(Tokens::text_sm_leading_compact())
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(Tokens::accent())
                            .cursor_pointer()
                            .hover(|s| s.text_color(Tokens::accent().opacity(0.8)))
                            .child(if collapsed { "See more" } else { "See less" })
                            .on_click(move |_, _, app: &mut gpui::App| on_toggle(app)),
                    )
                })
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .right(px(0.0))
                        .opacity(0.35)
                        .hover(|s| s.opacity(1.0))
                        .child(btn_copy_icon_arc(
                            element_key("copy-user", id),
                            copy_text,
                            "Copy message",
                        )),
                ),
        )
}

/// Subtle stream indicator at the end of an in-progress assistant reply.
#[allow(dead_code)]
pub fn streaming_cursor(animate: bool) -> impl IntoElement {
    div()
        .mt(Tokens::spacing_0p5())
        .text_size(Tokens::text_sm())
        .text_color(Tokens::text_faint())
        .opacity(0.6)
        .child(braille_spinner("streaming-cursor", animate))
}

/// Plain-text body for a streaming assistant reply — no markdown parse.
pub fn streaming_assistant_plain(text: &str) -> impl IntoElement {
    div()
        .w_full()
        .font_family(Tokens::ui_font_family())
        .text_size(Tokens::text_md())
        .line_height(Tokens::text_md_leading())
        .text_color(Tokens::text_primary())
        .child(text.to_string())
}

/// Static cursor for streaming — avoids braille animation repainting at display rate.
pub fn static_streaming_cursor() -> impl IntoElement {
    div()
        .mt(Tokens::spacing_0p5())
        .text_size(Tokens::text_sm())
        .text_color(Tokens::text_faint())
        .opacity(0.6)
        .child("▍")
}

fn render_message_attachment_preview(attachment: &MessageAttachment) -> gpui::AnyElement {
    match &attachment.preview {
        crate::features::shell::state::MessageAttachmentPreview::File(path) => img(path.clone())
            .w_full()
            .h_full()
            .object_fit(ObjectFit::Cover)
            .with_fallback(render_message_attachment_fallback)
            .into_any_element(),
        crate::features::shell::state::MessageAttachmentPreview::Bytes { mime_type, bytes } => {
            ImageFormat::from_mime_type(mime_type)
                .map(|format| Arc::new(Image::from_bytes(format, bytes.clone())))
                .map(|image| {
                    img(image)
                        .w_full()
                        .h_full()
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(render_message_attachment_fallback)
                        .into_any_element()
                })
                .unwrap_or_else(render_message_attachment_fallback)
        }
    }
}

fn render_message_attachment_fallback() -> gpui::AnyElement {
    div()
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(Tokens::surface_active())
        .text_color(Tokens::text_tertiary())
        .child("IMG")
        .into_any_element()
}

```

### `crates/app/src/features/chat/components/choice_card.rs`

```rust
use gpui::{FontWeight, IntoElement, div, prelude::*};
use gpui_component::Icon;

use crate::features::shell::state::{ChoiceMeta, ChoiceOption};
use crate::shared::components::buttons::btn_deny;
use crate::tokens::{Tokens, element_key, icons};

pub fn choice_card(
    id: &str,
    prompt: &str,
    options: &[ChoiceOption],
    meta: &ChoiceMeta,
    selected: Option<&str>,
    resolved: bool,
    on_select: impl Fn(String, &mut gpui::App) + Clone + 'static,
    on_cancel: impl Fn(&mut gpui::App) + Clone + 'static,
) -> impl IntoElement {
    let cancel = on_cancel.clone();
    div()
        .id(element_key("choice-card", id))
        .w_full()
        .px(Tokens::spacing_2p5())
        .py(Tokens::spacing_2())
        .rounded(Tokens::radius_md())
        .bg(Tokens::surface())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .flex()
                .items_start()
                .gap(Tokens::spacing_2())
                .child(Icon::new(icons::QUESTION).size(Tokens::text_sm()))
                .child(
                    div()
                        .flex_1()
                        .min_w(gpui::px(0.0))
                        .flex()
                        .flex_col()
                        .gap(Tokens::spacing_0p5())
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_tertiary())
                                .child(
                                    meta.summary
                                        .clone()
                                        .unwrap_or_else(|| "Decision needed".into()),
                                ),
                        )
                        .child(
                            div()
                                .text_size(Tokens::text_sm())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_primary())
                                .child(prompt.to_string()),
                        )
                        .when_some(meta.blocking_reason.clone(), |el, reason| {
                            el.child(
                                div()
                                    .text_size(Tokens::text_xs())
                                    .text_color(Tokens::text_faint())
                                    .child(reason),
                            )
                        }),
                )
                .when(!resolved, |el| {
                    el.child(
                        btn_deny(element_key("choice-cancel", id), "Cancel run").on_click(
                            move |_, _, app: &mut gpui::App| {
                                cancel(app);
                            },
                        ),
                    )
                }),
        )
        .children(options.iter().map(|option| {
            let option_id = option.id.clone();
            let is_selected = selected == Some(option_id.as_str());
            let recommended = option.recommended
                || meta
                    .recommended_option_id
                    .as_ref()
                    .is_some_and(|id| id == &option.id);
            let cb = on_select.clone();
            div()
                .id(element_key("choice-option", &format!("{id}-{}", option.id)))
                .w_full()
                .px(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .rounded(Tokens::radius_sm())
                .border_1()
                .border_color(if is_selected {
                    Tokens::accent()
                } else {
                    Tokens::border_subtle()
                })
                .when(!resolved, |el| {
                    el.cursor_pointer().hover(|s| s.bg(Tokens::surface_hover()))
                })
                .when(is_selected, |el| el.bg(Tokens::surface_active()))
                .on_click(move |_, _, app: &mut gpui::App| {
                    if !resolved {
                        cb(option_id.clone(), app);
                    }
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .child(
                            div()
                                .flex_1()
                                .min_w(gpui::px(0.0))
                                .text_size(Tokens::text_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Tokens::text_primary())
                                .child(option.label.clone()),
                        )
                        .when(recommended, |el| {
                            el.child(
                                div()
                                    .px(Tokens::spacing_1p5())
                                    .py(Tokens::spacing_0p5())
                                    .rounded(Tokens::radius_full())
                                    .bg(Tokens::accent().opacity(0.12))
                                    .text_size(Tokens::text_xs())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Tokens::accent())
                                    .child("Recommended"),
                            )
                        }),
                )
                .when_some(option.description.clone(), |el, description| {
                    el.child(
                        div()
                            .pt(Tokens::spacing_0p5())
                            .text_size(Tokens::text_xs())
                            .text_color(Tokens::text_faint())
                            .child(description),
                    )
                })
                .into_any_element()
        }))
        .when(meta.allow_custom, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .child("Custom responses can be sent from the composer."),
            )
        })
}

```

### `crates/app/src/features/chat/components/error_card.rs`

```rust
//! Flat error row for run / provider failures.

use crate::shared::components::buttons::{btn_ghost_label, btn_outline};
use crate::tokens::{Tokens, icons};
use gpui::{FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

pub struct ErrorCardProps {
    pub title: String,
    pub message: String,
    pub retryable: bool,
    pub on_open_settings: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_retry: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
}

pub fn error_card(props: ErrorCardProps) -> impl IntoElement {
    div()
        .id("error-card")
        .w_full()
        .max_w(px(Tokens::THREAD_MAX_WIDTH))
        .py(Tokens::spacing_2())
        .child(
            div()
                .w_full()
                .border_l_2()
                .border_color(Tokens::warning())
                .pl(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(Tokens::spacing_2())
                        .child(
                            Icon::new(icons::TRIANGLE_ALERT)
                                .size(Tokens::text_sm())
                                .text_color(Tokens::warning()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(Tokens::spacing_0p5())
                                .child(
                                    div()
                                        .text_size(Tokens::text_sm())
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Tokens::text_primary())
                                        .child(props.title),
                                )
                                .child(
                                    div()
                                        .text_size(Tokens::text_xs())
                                        .text_color(Tokens::text_secondary())
                                        .child(props.message),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .when_some(props.on_open_settings, |el, cb| {
                            el.child(
                                btn_ghost_label("error-open-settings", "Open settings")
                                    .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                            )
                        })
                        .when(props.retryable, |el| {
                            el.when_some(props.on_retry, |el, cb| {
                                el.child(
                                    btn_outline("error-retry", "Retry")
                                        .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                                )
                            })
                        }),
                ),
        )
}

```

### `crates/app/src/features/chat/components/context_chip.rs`

```rust
use gpui::{FontWeight, IntoElement, div, prelude::*, px};

use crate::features::shell::state::ChipKind;
use crate::features::shell::state::ContextChip;
use crate::tokens::Tokens;

/// Renders a subtle context pill aligned with the app surface palette.
pub fn context_chip(chip: &ContextChip) -> impl IntoElement {
    let label = chip.label.clone();
    let dot = chip_dot(&chip.kind);

    div()
        .h(px(Tokens::ROW_HEIGHT_XS))
        .px(Tokens::spacing_2())
        .rounded(Tokens::radius_full())
        .bg(Tokens::surface_hover())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded(Tokens::radius_full())
                .bg(dot),
        )
        .child(
            div()
                .text_size(Tokens::text_xs())
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::text_secondary())
                .child(label),
        )
}

fn chip_dot(kind: &ChipKind) -> gpui::Hsla {
    match kind {
        ChipKind::Repo => Tokens::text_tertiary(),
        ChipKind::File => Tokens::text_tertiary(),
        ChipKind::Branch => Tokens::accent(),
        ChipKind::Tool => Tokens::accent(),
    }
}

```

### `crates/app/src/features/chat/thread_view/mod.rs`

```rust
//! Thread surface — flattened rows + `gpui_component::v_virtual_list`.
//! Streaming assistant text renders inline in the list (same scroll as the rest of the feed).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Context, Entity, IntoElement, Render, ScrollStrategy, Timer, Window, div, prelude::*, px,
};
use gpui_component::VirtualListScrollHandle;
use gpui_component::v_virtual_list;

use crate::features::agent_activity::components::approval::thread_approval_row;
use crate::features::agent_activity::components::context_trace::{
    render_context_trace_header_row, render_context_trace_line_row,
};
use crate::features::agent_activity::components::diff::{
    render_diff_file_line_row, render_diff_header_row,
};
use crate::features::agent_activity::components::reasoning::{
    render_reasoning_header_row, render_reasoning_preview_line_row,
};
use crate::features::agent_activity::components::tool_call::{
    OpenToolFileCallback, render_tool_detail_line_row, render_tool_header_row,
    render_tool_output_line_row, render_tool_output_truncated_row, tool_call_detail_rows,
};
use crate::features::chat::components::choice_card::choice_card;
use crate::features::chat::components::error_card::{ErrorCardProps, error_card};
use crate::features::chat::components::message::user_message;
use crate::features::chat::manifest::{
    RowRef, activity_group_pos, assistant_accessory_height, assistant_actions_height,
    assistant_row_height_from_blocks, build_manifest_with_transcript, build_tool_output_preview,
    collapsed_item_header_changed, manifest_span, reasoning_preview_line_text,
    row_height_with_collapsed, row_refs_for_item_with_mode, row_size, row_sizes_for_manifest,
    row_sizes_for_rows, row_top_gap,
};
use crate::features::composer::layout::render_composer_fade;
use crate::features::shell::state::{ConversationId, ThreadItem};
use crate::shared::components::collapsible_row::activity_group_wrap;
use crate::shared::components::markdown_preview::{
    LINE_LEADING, MarkdownBlock, markdown_preview_blocks_thread_shared,
    markdown_preview_thread_streaming, parse_markdown_blocks_shared_streaming,
};
use crate::shared::components::streaming_markdown::{SealedBlockCache, streaming_assistant_body};
use crate::shared::state::TranscriptMode;
use crate::tokens::{Tokens, element_key};
use crate::ui::agent_window::AgentWindow;
use crate::ui::thread_update::{ThreadAction, ThreadEffect};

const MOTION_PAUSE_MS: u64 = 250;
const ASSISTANT_STREAM_SYNC_MS: u64 = 33;
/// Frame-aligned coalescing window for the streaming tail. Tokens that arrive within
/// one display frame are flushed together (one commit per frame). At ~8 ms this targets
/// 120 Hz on fast displays; GPUI further coalesces the resulting `notify()`s to the display
/// refresh and the backpressure path widens the batch if patch cost climbs.
const STREAM_FRAME_SYNC_MS: u64 = 8;
/// Fallback batch when render cost is elevated (≈30 fps).
const STREAM_BATCH_SLOW_MS: u64 = 33;
/// Backpressure batch when render falls behind (≈15 fps).
const STREAM_BATCH_BACKPRESSURE_MS: u64 = 66;
/// Only grow the virtual-list shell when height increases by at least one line.
const STREAMING_HEIGHT_EPSILON: f32 = LINE_LEADING * 0.5;

pub(crate) struct SealedMarkdownParseResult {
    item_id: String,
    sealed_end: usize,
    content_hash: u64,
    blocks: Arc<[MarkdownBlock]>,
    height: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AssistantActionProjection {
    pub can_retry: bool,
    pub can_open_diff: bool,
    pub can_approve: bool,
}

mod caches;
mod render;
mod rows;
mod scroll;
mod sync;

pub(crate) use caches::{MarkdownCacheEntry, ToolOutputPreview};
pub(crate) use rows::{render_empty_thread_state, tail_signature};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadChrome {
    Main,
    Embedded,
}

pub struct ThreadView {
    agent: Entity<AgentWindow>,
    conversation_id: Option<ConversationId>,
    chrome: ThreadChrome,
    items: Vec<ThreadItem>,
    manifest: Vec<RowRef>,
    row_sizes: Rc<Vec<gpui::Size<gpui::Pixels>>>,
    item_index: HashMap<String, usize>,
    header_manifest_index: Vec<Option<usize>>,
    markdown_cache: HashMap<String, MarkdownCacheEntry>,
    tool_output_cache: HashMap<String, ToolOutputPreview>,
    scroll_handle: VirtualListScrollHandle,
    stick_to_bottom: bool,
    pending_scroll_bottom: bool,
    pending_scroll_item_id: Option<String>,
    motion_paused: bool,
    last_scroll_activity: Option<Instant>,
    pending_items: Option<Vec<ThreadItem>>,
    pending_item_updates: HashMap<String, ThreadItem>,
    pending_streaming_row_patches: HashSet<String>,
    pending_mutated_item_patches: HashSet<String>,
    last_stream_sync: Option<Instant>,
    stream_debounce_scheduled: bool,
    item_update_debounce_scheduled: bool,
    last_tail_signature: u64,
    user_scrolled_up: bool,
    last_scroll_offset: Option<gpui::Point<gpui::Pixels>>,
    approval_active: bool,
    assistant_actions: AssistantActionProjection,
    sealed_blocks: HashMap<String, SealedBlockCache>,
    pending_sealed_parses: HashMap<String, u64>,
    sealed_parse_tx: flume::Sender<SealedMarkdownParseResult>,
    last_stream_patch: Option<Instant>,
    stream_batch_ms: u64,
    transcript_mode: TranscriptMode,
    composer_overlay_bar_height: f32,
    composer_input_expanded: bool,
    composer_has_attachments: bool,
    composer_has_error: bool,
}

impl ThreadView {
    pub fn new(
        agent: Entity<AgentWindow>,
        conversation_id: Option<ConversationId>,
        items: Vec<ThreadItem>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (sealed_parse_tx, sealed_parse_rx) = flume::unbounded();
        let mut view = Self {
            agent,
            conversation_id,
            chrome: ThreadChrome::Main,
            items: Vec::new(),
            manifest: Vec::new(),
            row_sizes: Rc::new(Vec::new()),
            item_index: HashMap::new(),
            header_manifest_index: Vec::new(),
            markdown_cache: HashMap::new(),
            tool_output_cache: HashMap::new(),
            scroll_handle: VirtualListScrollHandle::new(),
            stick_to_bottom: true,
            pending_scroll_bottom: true,
            pending_scroll_item_id: None,
            motion_paused: false,
            last_scroll_activity: None,
            pending_items: None,
            pending_item_updates: HashMap::new(),
            pending_streaming_row_patches: HashSet::new(),
            pending_mutated_item_patches: HashSet::new(),
            last_stream_sync: None,
            stream_debounce_scheduled: false,
            item_update_debounce_scheduled: false,
            last_tail_signature: 0,
            user_scrolled_up: false,
            last_scroll_offset: None,
            approval_active: false,
            assistant_actions: AssistantActionProjection::default(),
            sealed_blocks: HashMap::new(),
            pending_sealed_parses: HashMap::new(),
            sealed_parse_tx,
            last_stream_patch: None,
            stream_batch_ms: STREAM_FRAME_SYNC_MS,
            transcript_mode: TranscriptMode::default(),
            composer_overlay_bar_height: 0.0,
            composer_input_expanded: false,
            composer_has_attachments: false,
            composer_has_error: false,
        };
        view.start_sealed_parse_listener(sealed_parse_rx, cx);
        if let Some(cid) = view.conversation_id.clone() {
            view.sync(cid, items, false, cx);
        }
        view
    }

    pub fn new_embedded(
        agent: Entity<AgentWindow>,
        items: Vec<ThreadItem>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (sealed_parse_tx, sealed_parse_rx) = flume::unbounded();
        let mut view = Self {
            agent,
            conversation_id: None,
            chrome: ThreadChrome::Embedded,
            items: Vec::new(),
            manifest: Vec::new(),
            row_sizes: Rc::new(Vec::new()),
            item_index: HashMap::new(),
            header_manifest_index: Vec::new(),
            markdown_cache: HashMap::new(),
            tool_output_cache: HashMap::new(),
            scroll_handle: VirtualListScrollHandle::new(),
            stick_to_bottom: true,
            pending_scroll_bottom: true,
            pending_scroll_item_id: None,
            motion_paused: false,
            last_scroll_activity: None,
            pending_items: None,
            pending_item_updates: HashMap::new(),
            pending_streaming_row_patches: HashSet::new(),
            pending_mutated_item_patches: HashSet::new(),
            last_stream_sync: None,
            stream_debounce_scheduled: false,
            item_update_debounce_scheduled: false,
            last_tail_signature: 0,
            user_scrolled_up: false,
            last_scroll_offset: None,
            approval_active: false,
            assistant_actions: AssistantActionProjection::default(),
            sealed_blocks: HashMap::new(),
            pending_sealed_parses: HashMap::new(),
            sealed_parse_tx,
            last_stream_patch: None,
            stream_batch_ms: STREAM_FRAME_SYNC_MS,
            transcript_mode: TranscriptMode::default(),
            composer_overlay_bar_height: 0.0,
            composer_input_expanded: false,
            composer_has_attachments: false,
            composer_has_error: false,
        };
        view.start_sealed_parse_listener(sealed_parse_rx, cx);
        view.rebuild_all(items);
        cx.notify();
        view
    }

    fn start_sealed_parse_listener(
        &mut self,
        rx: flume::Receiver<SealedMarkdownParseResult>,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_weak, cx| {
            while let Ok(result) = rx.recv_async().await {
                entity
                    .update(cx, |view, cx| {
                        view.handle_sealed_parse_result(result, cx);
                    })
                    .ok();
            }
        })
        .detach();
    }

    pub fn set_conversation(
        &mut self,
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        cx: &mut Context<Self>,
    ) {
        self.dispatch(
            ThreadAction::SetConversation {
                conversation_id,
                items,
            },
            cx,
        );
    }

    pub fn sync(
        &mut self,
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        run_active: bool,
        cx: &mut Context<Self>,
    ) {
        self.dispatch(
            ThreadAction::Sync {
                conversation_id,
                items,
                immediate: false,
                run_active,
            },
            cx,
        );
    }

    pub fn sync_live(
        &mut self,
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        run_active: bool,
        cx: &mut Context<Self>,
    ) {
        self.dispatch(
            ThreadAction::Sync {
                conversation_id,
                items,
                immediate: true,
                run_active,
            },
            cx,
        );
    }

    pub fn set_transcript_mode(&mut self, mode: TranscriptMode) {
        self.transcript_mode = mode;
    }

    pub fn set_assistant_actions(
        &mut self,
        actions: AssistantActionProjection,
        cx: &mut Context<Self>,
    ) {
        if self.assistant_actions != actions {
            self.assistant_actions = actions;
            cx.notify();
        }
    }

    pub fn set_approval_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.dispatch(ThreadAction::SetApprovalActive(active), cx);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.apply_thread_effect(ThreadEffect::ScrollToBottom);
    }

    pub fn reveal_item(&mut self, item_id: &str) {
        self.pending_scroll_item_id = Some(item_id.to_string());
    }

    pub fn push_item(&mut self, item: ThreadItem, cx: &mut Context<Self>) {
        self.dispatch(ThreadAction::PushItem(item), cx);
    }

    pub fn update_item(&mut self, item: ThreadItem, cx: &mut Context<Self>) {
        self.dispatch(ThreadAction::UpdateItem(item), cx);
    }

    /// Append sanitized text to a streaming assistant row without cloning the full item.
    pub fn append_assistant_delta(&mut self, item_id: &str, chunk: &str, cx: &mut Context<Self>) {
        if chunk.is_empty() {
            return;
        }
        let Some(&item_ix) = self.item_index.get(item_id) else {
            return;
        };
        let Some(ThreadItem::AssistantMessage { markdown, .. }) = self.items.get_mut(item_ix)
        else {
            return;
        };
        markdown.push_str(chunk);
        let inserted = self.ensure_manifest_rows_for_item(item_ix);
        self.pending_streaming_row_patches
            .insert(item_id.to_string());
        let mut effects = vec![ThreadEffect::ScheduleItemUpdate];
        if inserted && self.stick_to_bottom && !self.user_scrolled_up {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        self.apply_thread_effects(effects, cx);
    }

    pub fn append_reasoning_delta(&mut self, item_id: &str, chunk: &str, cx: &mut Context<Self>) {
        if chunk.is_empty() {
            return;
        }
        let Some(&item_ix) = self.item_index.get(item_id) else {
            return;
        };
        let Some(ThreadItem::ReasoningStep { summary, .. }) = self.items.get_mut(item_ix) else {
            return;
        };
        summary.push_str(chunk);
        let inserted = self.ensure_manifest_rows_for_item(item_ix);
        self.pending_mutated_item_patches
            .insert(item_id.to_string());
        let mut effects = vec![ThreadEffect::ScheduleItemUpdate];
        if inserted && self.stick_to_bottom && !self.user_scrolled_up {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        self.apply_thread_effects(effects, cx);
    }

    pub fn append_tool_output_delta(
        &mut self,
        item_id: &str,
        prefix: &str,
        chunk: &str,
        cx: &mut Context<Self>,
    ) {
        if prefix.is_empty() && chunk.is_empty() {
            return;
        }
        let Some(&item_ix) = self.item_index.get(item_id) else {
            return;
        };
        let Some(ThreadItem::ToolCall { output, .. }) = self.items.get_mut(item_ix) else {
            return;
        };
        let output = output.get_or_insert_with(String::new);
        output.push_str(prefix);
        output.push_str(chunk);
        let inserted = self.ensure_manifest_rows_for_item(item_ix);
        self.pending_mutated_item_patches
            .insert(item_id.to_string());
        let mut effects = vec![ThreadEffect::ScheduleItemUpdate];
        if inserted && self.stick_to_bottom && !self.user_scrolled_up {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        self.apply_thread_effects(effects, cx);
    }

    pub fn refresh_item(&mut self, item: ThreadItem, cx: &mut Context<Self>) {
        self.dispatch(ThreadAction::RefreshItem(item), cx);
    }

    pub fn cancel_pending_item(&mut self, item_id: &str) {
        self.pending_item_updates.remove(item_id);
    }
}

/// Embeds the thread view entity in the chat column.
pub fn render_thread(thread_view: Entity<ThreadView>) -> impl IntoElement {
    thread_view
}

```

### `crates/app/src/features/chat/thread_view/render.rs`

```rust
//! Thread view GPUI render.

use super::*;

impl Render for ThreadView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::shared::render_profile::span("ThreadView::render");
        self.clear_motion_pause_if_idle();
        if self.chrome == ThreadChrome::Main {
            self.sync_composer_footer(cx);
        }
        self.handle_scroll_if_changed();

        if self.pending_scroll_bottom && self.stick_to_bottom && !self.user_scrolled_up {
            let last = self.manifest.len().saturating_sub(1);
            if last > 0 {
                self.scroll_handle
                    .scroll_to_item(last, ScrollStrategy::Bottom);
            }
            self.pending_scroll_bottom = false;
        }
        if let Some(item_id) = self.pending_scroll_item_id.take() {
            if let Some(row_ix) = self.manifest.iter().position(|row| {
                row.item_ix()
                    .and_then(|ix| self.items.get(ix as usize))
                    .is_some_and(|item| item.id() == item_id)
            }) {
                self.scroll_handle
                    .scroll_to_item(row_ix, ScrollStrategy::Top);
            }
        }

        let row_sizes = Rc::clone(&self.row_sizes);
        let entity = cx.entity();

        div()
            .id("thread-surface")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .w_full()
            .font_family(Tokens::ui_font_family())
            .when(self.chrome == ThreadChrome::Main, |el| {
                el.px(Tokens::thread_padding_x())
                    .pt(Tokens::thread_padding_top())
            })
            .when(self.chrome == ThreadChrome::Embedded, |el| {
                el.px(Tokens::spacing_3()).pt(Tokens::spacing_3())
            })
            .child(
                div()
                    .id("thread-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(if self.items.is_empty() {
                        render_empty_thread_state().into_any_element()
                    } else {
                        div()
                            .id("thread-list")
                            .w_full()
                            .when(self.chrome == ThreadChrome::Main, |el| {
                                el.max_w(px(Tokens::THREAD_MAX_WIDTH))
                            })
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .flex_col()
                            .justify_start()
                            .child(
                                v_virtual_list(
                                    entity.clone(),
                                    "thread-virtual-list",
                                    row_sizes,
                                    |this, range, _window, cx| this.render_visible(range, cx),
                                )
                                .track_scroll(&self.scroll_handle)
                                .flex_1()
                                .min_h(px(0.0))
                                .w_full(),
                            )
                            .into_any_element()
                    }),
            )
            .when(self.chrome == ThreadChrome::Main, |el| {
                el.child(render_composer_fade(self.composer_thread_inset_px()))
            })
    }
}

```

### `crates/app/src/features/chat/thread_view/rows.rs`

```rust
//! Thread row rendering

use super::*;
use crate::features::shell::state::ActivityGroupPos;
use crate::shared::components::collapsible_row::timeline_body;

impl ThreadView {
    pub(crate) fn render_visible(
        &mut self,
        range: std::ops::Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let start = std::time::Instant::now();
        let row_count = range.len() as u64;
        let approval_active = self.approval_active;
        let animate = self.animate();
        let agent = self.agent.clone();
        let sizes = Rc::clone(&self.row_sizes);

        let rows = range
            .map(|row_ix| {
                let row_h = sizes
                    .get(row_ix)
                    .map(|s| s.height)
                    .unwrap_or(px(Tokens::TOOL_ROW_HEIGHT));
                match self.manifest.get(row_ix).copied() {
                    Some(row_ref) => {
                        let prev = self.manifest.get(row_ix.wrapping_sub(1)).copied();
                        let gap = row_top_gap(row_ref, prev, &self.items);
                        self.render_row(row_ref, row_h, gap, approval_active, animate, &agent, cx)
                    }
                    None => div().w_full().h(row_h).into_any_element(),
                }
            })
            .collect();
        crate::shared::render_profile::record(
            "ThreadView::render_visible",
            start.elapsed(),
            row_count,
        );
        rows
    }

    pub(crate) fn render_row(
        &mut self,
        row_ref: RowRef,
        row_h: gpui::Pixels,
        top_gap: f32,
        approval_active: bool,
        animate: bool,
        agent: &Entity<AgentWindow>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if matches!(row_ref, RowRef::EndSpacer) {
            return div()
                .id("thread-end-spacer")
                .w_full()
                .h(row_h)
                .into_any_element();
        }

        let item_ix = row_ref.item_ix().expect("content row") as usize;
        let item = self.items.get(item_ix);
        let depth = item.and_then(item_depth).unwrap_or(0);
        let row_id = stable_row_id(row_ref, item);
        let group_pos = activity_group_pos(item_ix, &self.items);

        let content = match row_ref {
            RowRef::UserMessage { .. } => {
                let Some(ThreadItem::UserMessage {
                    text,
                    attachments,
                    id,
                    expanded,
                }) = item
                else {
                    return div().into_any_element();
                };
                let is_initial = crate::features::shell::state::first_user_message_ix(&self.items)
                    == Some(item_ix);
                let truncatable =
                    is_initial && crate::features::shell::state::user_message_truncatable(text);
                let collapsed = truncatable && !expanded;
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                user_message(
                    text,
                    attachments,
                    id,
                    collapsed,
                    truncatable,
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::SubagentHeader { .. } => {
                let Some(ThreadItem::SubagentRun {
                    id,
                    task,
                    model: _,
                    summary: _,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
                let agent_select = agent.clone();
                let _profile =
                    crate::shared::render_profile::span("ThreadView::subagent_header_vm");
                let (activity_summary, is_selected) = agent.read_with(cx, |window, _| {
                    let projection = window.subagent_transcripts.get(id);
                    let summary = projection
                        .map(|projection| projection.activity_summary())
                        .unwrap_or_else(|| "No child activity yet".to_string());
                    let selected = window
                        .inspector_tabs
                        .active()
                        .is_some_and(|tab| matches!(&tab.kind, crate::features::shell::state::InspectorTabKind::Subagent(active_id) if active_id == id));
                    (summary, selected)
                });
                render_subagent_summary_row(
                    id,
                    task.clone(),
                    status,
                    &activity_summary,
                    is_selected,
                    group_pos,
                    animate,
                    move |app: &mut gpui::App| {
                        agent_select.update(app, |view, cx| {
                            view.select_subagent_tab(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::SubagentBody { .. } => {
                let Some(ThreadItem::SubagentRun { id, summary, .. }) = item else {
                    return div().into_any_element();
                };
                let _profile = crate::shared::render_profile::span("ThreadView::subagent_body_vm");
                let (activity_summary, last_event) = agent.read_with(cx, |window, _| {
                    let projection = window.subagent_transcripts.get(id);
                    (
                        projection
                            .map(|projection| projection.activity_summary())
                            .unwrap_or_else(|| "No child activity yet".to_string()),
                        projection.and_then(|projection| projection.last_event_label.clone()),
                    )
                });
                render_subagent_detail_row(
                    id,
                    summary,
                    &activity_summary,
                    last_event.as_deref(),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ReasoningHeader { .. } => {
                let Some(ThreadItem::ReasoningStep {
                    id,
                    title,
                    summary,
                    expanded,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                render_reasoning_header_row(
                    id,
                    title,
                    summary,
                    *expanded,
                    status,
                    animate,
                    group_pos,
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::ReasoningBody { .. } => {
                let Some(ThreadItem::ReasoningStep {
                    id,
                    summary,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let streaming =
                    matches!(status, crate::features::shell::state::AgentStatus::Thinking);
                activity_group_wrap(
                    timeline_body(
                        element_key("reasoning-body-wrap", id),
                        div()
                            .id(element_key("reasoning-body", id))
                            .w_full()
                            .pr(Tokens::spacing_1())
                            .max_h(px(crate::features::chat::layout::REASONING_BODY_MAX_H))
                            .overflow_y_scroll()
                            .child(markdown_preview_thread_streaming(summary, streaming)),
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ReasoningPreviewLine { line_ix, .. } => {
                let Some(item) = item else {
                    return div().into_any_element();
                };
                let text = reasoning_preview_line_text(item, line_ix as usize)
                    .unwrap_or_else(|| Arc::from(""));
                activity_group_wrap(
                    render_reasoning_preview_line_row(
                        &format!("reason-{item_ix}-preview-{line_ix}"),
                        &text,
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ToolHeader { .. } => {
                let Some(ThreadItem::ToolCall {
                    id,
                    tool_name,
                    command,
                    output,
                    expanded,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let preview = if matches!(
                    status,
                    crate::features::shell::state::AgentStatus::WaitingApproval
                ) {
                    output.as_deref().or(command.as_deref())
                } else {
                    command.as_deref()
                };
                let running = matches!(
                    status,
                    crate::features::shell::state::AgentStatus::RunningTool
                );
                let _profile = crate::shared::render_profile::span("ThreadView::tool_header_vm");
                let (change_counts, display_label) = agent.read_with(cx, |window, _| {
                    (
                        window.live_edit_change_counts(tool_name, running),
                        window.tool_row_label(tool_name, preview, running),
                    )
                });
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                let agent_select = agent.clone();
                let agent_open = agent.clone();
                let on_open_file: OpenToolFileCallback =
                    Arc::new(move |path, line_range, app: &mut gpui::App| {
                        agent_open.update(app, |view, cx| {
                            view.open_file_in_external_editor(&path, line_range, cx);
                        });
                    });
                render_tool_header_row(
                    id,
                    tool_name,
                    &display_label,
                    preview,
                    *expanded,
                    status,
                    animate,
                    group_pos,
                    change_counts,
                    Some(on_open_file),
                    move |app: &mut gpui::App| {
                        agent_select.update(app, |view, cx| {
                            view.select_tool_artifact(&toggle_id, cx);
                        });
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::ToolDetailLine { line_ix, .. } => {
                let Some(ThreadItem::ToolCall {
                    id: _,
                    tool_name,
                    command,
                    output,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let preview = if matches!(
                    status,
                    crate::features::shell::state::AgentStatus::WaitingApproval
                ) {
                    output.as_deref().or(command.as_deref())
                } else {
                    command.as_deref()
                };
                let running = matches!(
                    status,
                    crate::features::shell::state::AgentStatus::RunningTool
                );
                let display_label = agent.read_with(cx, |window, _| {
                    window.tool_row_label(tool_name, preview, running)
                });
                let details = tool_call_detail_rows(tool_name, &display_label, preview, status);
                let Some(detail) = details.get(line_ix as usize).cloned() else {
                    return div().into_any_element();
                };
                let agent_open = agent.clone();
                let on_open_file: OpenToolFileCallback =
                    Arc::new(move |path, line_range, app: &mut gpui::App| {
                        agent_open.update(app, |view, cx| {
                            view.open_file_in_external_editor(&path, line_range, cx);
                        });
                    });
                activity_group_wrap(
                    render_tool_detail_line_row(
                        &format!("tool-{item_ix}-detail-{line_ix}"),
                        detail,
                        Some(on_open_file),
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ToolOutputLine { line_ix, .. } => {
                let (id, out) = {
                    let Some(ThreadItem::ToolCall {
                        id,
                        output: Some(out),
                        expanded: true,
                        ..
                    }) = item
                    else {
                        return div().into_any_element();
                    };
                    (id.clone(), out.clone())
                };
                let preview = self.cached_tool_output_preview(&id, &out);
                let text = preview
                    .lines
                    .get(line_ix as usize)
                    .cloned()
                    .unwrap_or_else(|| Arc::from(""));
                activity_group_wrap(
                    render_tool_output_line_row(&format!("tool-{item_ix}-line-{line_ix}"), &text),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ToolOutputTruncated { .. } => {
                let (id, out) = {
                    let Some(ThreadItem::ToolCall {
                        id,
                        output: Some(out),
                        ..
                    }) = item
                    else {
                        return div().into_any_element();
                    };
                    (id.clone(), out.clone())
                };
                let preview = self.cached_tool_output_preview(&id, &out);
                activity_group_wrap(
                    render_tool_output_truncated_row(&id, preview.total_lines, preview.full),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::DiffHeader { .. } => {
                let Some(ThreadItem::DiffSummary {
                    id,
                    files_changed,
                    additions,
                    deletions,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                let agent_review = agent.clone();
                render_diff_header_row(
                    id,
                    *files_changed,
                    *additions,
                    *deletions,
                    group_pos,
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                    move |app: &mut gpui::App| {
                        agent_review.update(app, |view, cx| {
                            view.open_diff_panel(cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::DiffFileLine { file_ix, .. } => {
                let Some(ThreadItem::DiffSummary { id, files, .. }) = item else {
                    return div().into_any_element();
                };
                let Some(file) = files.get(file_ix as usize) else {
                    return div().into_any_element();
                };
                activity_group_wrap(
                    render_diff_file_line_row(
                        &format!("{id}-file-{file_ix}"),
                        &file.path,
                        file.added,
                        file.removed,
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ContextTraceHeader { .. } => {
                let Some(ThreadItem::ContextTrace {
                    id,
                    entries,
                    expanded,
                }) = item
                else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                render_context_trace_header_row(
                    id,
                    entries,
                    *expanded,
                    animate,
                    group_pos,
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.select_inspector_view(
                                crate::features::shell::state::InspectorView::Context,
                                cx,
                            );
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::ContextTraceLine { line_ix, .. } => {
                let Some(ThreadItem::ContextTrace { id, entries, .. }) = item else {
                    return div().into_any_element();
                };
                let Some(entry) = entries.get(line_ix as usize) else {
                    return div().into_any_element();
                };
                activity_group_wrap(
                    render_context_trace_line_row(&format!("{id}-context-{line_ix}"), entry),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::AssistantMessage { .. } => self.render_assistant(item_ix, cx),
            RowRef::Approval { .. } => {
                let Some(ThreadItem::ApprovalRequest {
                    title,
                    risk,
                    resolved,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                thread_approval_row(title, risk, *resolved, approval_active).into_any_element()
            }
            RowRef::RunError { .. } => {
                let Some(ThreadItem::RunError {
                    message, retryable, ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let title = message.lines().next().unwrap_or(message).to_string();
                let msg = message.clone();
                let retry = *retryable;
                let settings_entity = agent.clone();
                error_card(ErrorCardProps {
                    title,
                    message: msg,
                    retryable: retry,
                    on_open_settings: Some(Box::new(move |app| {
                        settings_entity.update(app, |view, cx| view.open_settings(cx));
                    })),
                    on_retry: None,
                })
                .into_any_element()
            }
            RowRef::ChoiceRequest { .. } => {
                let Some(ThreadItem::ChoiceRequest {
                    id,
                    prompt,
                    options,
                    meta,
                    selected,
                    resolved,
                }) = item
                else {
                    return div().into_any_element();
                };
                let choice_id = id.clone();
                let entity = agent.clone();
                let cancel_entity = agent.clone();
                choice_card(
                    id,
                    prompt,
                    options,
                    meta,
                    selected.as_deref(),
                    *resolved,
                    move |option_id, app| {
                        entity.update(app, |view, cx| {
                            view.submit_choice(&choice_id, &option_id, cx);
                        });
                    },
                    move |app| {
                        cancel_entity.update(app, |view, cx| {
                            view.cancel_active_run(cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::PlanStatus { .. } => {
                let Some(ThreadItem::PlanStatus {
                    id,
                    state,
                    summary,
                    counts,
                    source_conversation_id,
                }) = item
                else {
                    return div().into_any_element();
                };
                let agent_open = agent.clone();
                render_plan_status_row(
                    id,
                    state,
                    summary,
                    counts.summary(),
                    source_conversation_id.as_ref().map(|id| id.0.as_str()),
                    group_pos,
                    move |app: &mut gpui::App| {
                        agent_open.update(app, |view, cx| {
                            view.select_inspector_view(
                                crate::features::shell::state::InspectorView::Plan,
                                cx,
                            );
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::EndSpacer => unreachable!("handled above"),
        };

        div()
            .id(element_key("thread-row-shell", &row_id))
            .w_full()
            .h(row_h)
            .when(depth > 0, |el| el.pl(Tokens::tree_indent(depth as u32)))
            .when(top_gap > 0.0, |el| el.pt(px(top_gap)))
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_start()
            .child(content)
            .into_any_element()
    }

    pub(crate) fn render_assistant(
        &mut self,
        item_ix: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let _profile = crate::shared::render_profile::span("ThreadView::render_assistant");
        let Some(ThreadItem::AssistantMessage {
            id,
            markdown,
            streaming,
            ..
        }) = self.items.get(item_ix)
        else {
            return div().into_any_element();
        };
        if *streaming {
            let id = id.clone();
            let jump_to_latest = self.user_scrolled_up.then(|| {
                let entity = cx.entity();
                let button_id = id.clone();
                div()
                    .id(element_key("stream-scroll-pill", &button_id))
                    .w_full()
                    .pt(Tokens::spacing_1())
                    .flex()
                    .justify_start()
                    .child(
                        div()
                            .id(element_key("stream-scroll-pill-btn", &button_id))
                            .px(Tokens::spacing_2())
                            .py(Tokens::spacing_0p5())
                            .rounded(Tokens::radius_sm())
                            .bg(Tokens::surface_hover())
                            .text_size(Tokens::text_xs())
                            .text_color(Tokens::text_tertiary())
                            .cursor_pointer()
                            .hover(|s| s.bg(Tokens::surface_active()))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |view, cx| {
                                    view.stick_to_bottom = true;
                                    view.user_scrolled_up = false;
                                    view.pending_scroll_bottom = true;
                                    cx.notify();
                                });
                            })
                            .child("Jump to latest ↓"),
                    )
                    .into_any_element()
            });

            let sealed = self.sealed_blocks.get(&id).cloned().unwrap_or_default();
            let show_cursor = markdown.len() > 2;
            let actions_projection = self.assistant_actions;
            let actions = assistant_action_row(
                &id,
                true,
                cx.entity().clone(),
                self.agent.clone(),
                actions_projection.can_retry,
                actions_projection.can_open_diff,
                actions_projection.can_approve,
            );
            return div()
                .id(element_key("assistant-segment", &id))
                .w_full()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(streaming_assistant_body(
                    &id,
                    markdown,
                    &sealed,
                    show_cursor,
                ))
                .when_some(jump_to_latest, |el, button| el.child(button))
                .child(actions)
                .into_any_element();
        }

        let id = id.clone();
        let markdown = markdown.clone();
        self.sealed_blocks.remove(&id);

        let blocks = self.cached_markdown_blocks(&id, &markdown, false);
        let actions_projection = self.assistant_actions;
        let actions = assistant_action_row(
            &id,
            false,
            cx.entity().clone(),
            self.agent.clone(),
            actions_projection.can_retry,
            actions_projection.can_open_diff,
            actions_projection.can_approve,
        );
        div()
            .id(element_key("assistant-segment", &id))
            .w_full()
            .flex()
            .flex_col()
            .gap(Tokens::spacing_0p5())
            .text_size(Tokens::text_md())
            .line_height(Tokens::text_md_leading())
            .child(markdown_preview_blocks_thread_shared(blocks, false))
            .child(actions)
            .into_any_element()
    }
}

fn assistant_action_row(
    item_id: &str,
    streaming: bool,
    thread: Entity<ThreadView>,
    agent: Entity<AgentWindow>,
    can_retry: bool,
    can_open_diff: bool,
    can_approve: bool,
) -> impl IntoElement {
    use crate::shared::components::buttons::btn_icon_sm;
    use crate::tokens::icons;

    div()
        .id(element_key("assistant-actions", item_id))
        .w_full()
        .h(px(Tokens::ROW_HEIGHT_SM))
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .opacity(if streaming { 0.40 } else { 0.20 })
        .hover(|s| s.opacity(0.58))
        .child({
            let copy_thread = thread.clone();
            let copy_item_id = item_id.to_string();
            btn_icon_sm(element_key("assistant-copy", item_id), icons::COPY)
                .tooltip("Copy reply")
                .on_click(move |_, _, app: &mut gpui::App| {
                    copy_thread.update(app, |view, cx| {
                        if let Some(markdown) = view.items.iter().find_map(|item| {
                            if let ThreadItem::AssistantMessage { id, markdown, .. } = item {
                                (id == &copy_item_id).then(|| markdown.clone())
                            } else {
                                None
                            }
                        }) {
                            cx.write_to_clipboard(markdown.into());
                        }
                    });
                })
        })
        .child(
            btn_icon_sm(element_key("assistant-helpful", item_id), icons::CHECK).tooltip("Helpful"),
        )
        .child(
            btn_icon_sm(element_key("assistant-unhelpful", item_id), icons::X_MARK)
                .tooltip("Needs work"),
        )
        .when(can_retry, |el| {
            let retry_agent = agent.clone();
            el.child(
                btn_icon_sm(element_key("assistant-retry", item_id), icons::ARROW_UP)
                    .tooltip("Retry last turn")
                    .when(!streaming, |button| {
                        button.on_click(move |_, _, app: &mut gpui::App| {
                            retry_agent.update(app, |view, cx| {
                                view.retry_last_user_turn(cx);
                            });
                        })
                    }),
            )
        })
        .when(can_open_diff, |el| {
            let diff_agent = agent.clone();
            el.child(
                btn_icon_sm(
                    element_key("assistant-open-diff", item_id),
                    icons::GIT_COMPARE,
                )
                .tooltip("Open changes")
                .on_click(move |_, _, app: &mut gpui::App| {
                    diff_agent.update(app, |view, cx| {
                        view.open_diff_panel(cx);
                    });
                }),
            )
        })
        .when(can_approve, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child("Approval pending"),
            )
        })
}

pub(crate) fn render_empty_thread_state() -> impl IntoElement {
    div()
        .id("thread-empty-state")
        .flex_1()
        .w_full()
        .max_w(px(Tokens::THREAD_MAX_WIDTH))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .max_w(px(Tokens::THREAD_EMPTY_COPY_WIDTH))
                .flex()
                .flex_col()
                .items_start()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .text_size(Tokens::text_xl())
                        .line_height(Tokens::text_xl_leading())
                        .text_color(Tokens::text_primary())
                        .child("Start a thread"),
                )
                .child(
                    div()
                        .text_size(Tokens::text_md())
                        .line_height(Tokens::text_md_leading())
                        .text_color(Tokens::text_tertiary())
                        .child(
                            "Ask for a code change, inspect a file, or describe the task. The thread stays continuous once the first turn starts.",
                        ),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading())
                        .text_color(Tokens::text_faint())
                        .child("Tool runs, reasoning, and diff summaries will appear inline beneath the reply."),
                ),
        )
}

fn render_subagent_summary_row(
    item_id: &str,
    task: String,
    status: &crate::features::shell::state::AgentStatus,
    activity_summary: &str,
    is_selected: bool,
    group_pos: Option<ActivityGroupPos>,
    animate: bool,
    on_open: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
    use crate::tokens::{activity_action_line_with_loading, icons};
    use gpui_component::Icon;

    let on_open = Rc::new(on_open);
    let on_open_button = on_open.clone();
    let on_open_row = on_open.clone();
    let status_label = status_label(status);
    let running = matches!(
        status,
        crate::features::shell::state::AgentStatus::RunningTool
            | crate::features::shell::state::AgentStatus::Thinking
    );
    let title = format!("Subagent {}", status_label.to_ascii_lowercase());
    let detail = if activity_summary.trim().is_empty() {
        Some(task)
    } else {
        Some(activity_summary.to_string())
    };
    let open_button = div()
        .id(element_key("subagent-open-tab", item_id))
        .h(px(Tokens::ROW_HEIGHT_XS))
        .px(Tokens::spacing_1())
        .flex()
        .items_center()
        .justify_center()
        .rounded(Tokens::radius_xs())
        .cursor_pointer()
        .text_color(Tokens::text_tertiary())
        .hover(|s| s.text_color(Tokens::text_primary()))
        .on_click(move |_, _, app| on_open_button(app))
        .child(
            Icon::new(icons::EXTERNAL_LINK)
                .size(px(13.0))
                .text_color(Tokens::text_tertiary()),
        );

    activity_group_wrap(
        div()
            .id(element_key("subagent-shell", item_id))
            .w_full()
            .when(is_selected, |el| {
                el.bg(Tokens::surface_hover().blend(Tokens::accent().opacity(0.05)))
                    .rounded(Tokens::radius_xs())
            })
            .child(timeline_row(
                element_key("subagent-header", item_id),
                div()
                    .min_w(px(0.0))
                    .child(
                        div().min_w(px(0.0)).overflow_hidden().child(
                            activity_action_line_with_loading(
                                &title,
                                detail.as_deref(),
                                running,
                                false,
                                animate,
                                item_id,
                            )
                            .into_any_element(),
                        ),
                    )
                    .into_any_element(),
                open_button.into_any_element(),
                move |_, _, app: &mut gpui::App| on_open_row(app),
            )),
        group_pos,
    )
}

fn render_subagent_detail_row(
    item_id: &str,
    summary: &str,
    activity_summary: &str,
    last_event: Option<&str>,
    group_pos: Option<ActivityGroupPos>,
) -> impl IntoElement {
    let summary_excerpt = summary
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    activity_group_wrap(
        timeline_body(
            element_key("subagent-body-wrap", item_id),
            div()
                .id(element_key("subagent-body", item_id))
                .w_full()
                .pr(Tokens::spacing_1())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading_compact())
                        .text_color(Tokens::activity_detail_text())
                        .child(if summary_excerpt.trim().is_empty() {
                            "Investigating task in child run.".to_string()
                        } else {
                            summary_excerpt
                        }),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .line_height(Tokens::text_sm_leading_compact())
                        .text_color(Tokens::activity_meta_text())
                        .child(format!("Transcript · {activity_summary}")),
                )
                .when_some(last_event, |el, last| {
                    el.child(
                        div()
                            .text_size(Tokens::text_xs())
                            .line_height(Tokens::text_sm_leading_compact())
                            .text_color(Tokens::text_faint())
                            .child(format!("Last update · {last}")),
                    )
                }),
        ),
        group_pos,
    )
}

fn render_plan_status_row(
    item_id: &str,
    state: &crate::features::shell::state::PlanExecutionState,
    summary: &str,
    counts_summary: String,
    source_conversation_id: Option<&str>,
    group_pos: Option<ActivityGroupPos>,
    on_open: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
    use crate::tokens::activity_action_line_with_loading;

    let meta = match source_conversation_id {
        Some(source) => format!("{counts_summary} · Source: {source}"),
        None => counts_summary,
    };

    activity_group_wrap(
        div()
            .id(element_key("plan-status-row", item_id))
            .child(timeline_row(
                element_key("plan-status-header", item_id),
                activity_action_line_with_loading(
                    &format!("Plan · {}", state.label()),
                    (!summary.trim().is_empty()).then_some(summary),
                    false,
                    false,
                    false,
                    item_id,
                )
                .into_any_element(),
                div()
                    .id(element_key("plan-status-meta", item_id))
                    .max_w(px(220.0))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .truncate()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .hover(|s| s.text_color(Tokens::text_secondary()))
                    .child(meta)
                    .into_any_element(),
                move |_, _, app: &mut gpui::App| on_open(app),
            )),
        group_pos,
    )
}

fn status_label(status: &crate::features::shell::state::AgentStatus) -> &'static str {
    match status {
        crate::features::shell::state::AgentStatus::Thinking
        | crate::features::shell::state::AgentStatus::RunningTool => "Running",
        crate::features::shell::state::AgentStatus::WaitingApproval => "Waiting approval",
        crate::features::shell::state::AgentStatus::Completed => "Completed",
        crate::features::shell::state::AgentStatus::Failed => "Failed",
        crate::features::shell::state::AgentStatus::Idle => "Idle",
    }
}

pub(crate) fn stable_row_id(row_ref: RowRef, item: Option<&ThreadItem>) -> String {
    let item_id = item.map(ThreadItem::id).unwrap_or("end");
    match row_ref {
        RowRef::UserMessage { .. } => format!("user-{item_id}"),
        RowRef::SubagentHeader { .. } => format!("subagent-{item_id}"),
        RowRef::SubagentBody { .. } => format!("subagent-body-{item_id}"),
        RowRef::ReasoningHeader { .. } => format!("reasoning-header-{item_id}"),
        RowRef::ReasoningPreviewLine { line_ix, .. } => {
            format!("reasoning-preview-{item_id}-{line_ix}")
        }
        RowRef::ReasoningBody { .. } => format!("reasoning-body-{item_id}"),
        RowRef::ToolHeader { .. } => format!("tool-header-{item_id}"),
        RowRef::ToolDetailLine { line_ix, .. } => format!("tool-detail-{item_id}-{line_ix}"),
        RowRef::ToolOutputLine { line_ix, .. } => format!("tool-line-{item_id}-{line_ix}"),
        RowRef::ToolOutputTruncated { .. } => format!("tool-truncated-{item_id}"),
        RowRef::DiffHeader { .. } => format!("diff-header-{item_id}"),
        RowRef::DiffFileLine { file_ix, .. } => format!("diff-file-{item_id}-{file_ix}"),
        RowRef::ContextTraceHeader { .. } => format!("context-trace-header-{item_id}"),
        RowRef::ContextTraceLine { line_ix, .. } => {
            format!("context-trace-line-{item_id}-{line_ix}")
        }
        RowRef::AssistantMessage { .. } => format!("assistant-{item_id}"),
        RowRef::Approval { .. } => format!("approval-{item_id}"),
        RowRef::RunError { .. } => format!("run-error-{item_id}"),
        RowRef::ChoiceRequest { .. } => format!("choice-{item_id}"),
        RowRef::PlanStatus { .. } => format!("plan-status-{item_id}"),
        RowRef::EndSpacer => "thread-end-spacer".to_string(),
    }
}

fn item_depth(item: &ThreadItem) -> Option<u8> {
    match item {
        ThreadItem::AssistantMessage { depth, .. }
        | ThreadItem::ReasoningStep { depth, .. }
        | ThreadItem::ToolCall { depth, .. }
        | ThreadItem::DiffSummary { depth, .. } => Some(*depth),
        ThreadItem::SubagentRun { .. } => Some(0),
        ThreadItem::PlanStatus { .. } => Some(0),
        _ => None,
    }
}

pub(crate) fn tail_signature(items: &[ThreadItem]) -> u64 {
    let mut sig = items.len() as u64;
    for item in items.iter().rev() {
        match item {
            ThreadItem::AssistantMessage {
                markdown,
                streaming,
                ..
            } => {
                sig = sig.wrapping_mul(31).wrapping_add(markdown.len() as u64);
                sig = sig.wrapping_add(if *streaming { 1 } else { 0 });
                break;
            }
            ThreadItem::ToolCall { output, .. } => {
                sig = sig
                    .wrapping_mul(31)
                    .wrapping_add(output.as_ref().map(|s| s.len()).unwrap_or(0) as u64);
                break;
            }
            ThreadItem::ReasoningStep { summary, .. } => {
                sig = sig.wrapping_mul(31).wrapping_add(summary.len() as u64);
                break;
            }
            _ => {}
        }
    }
    sig
}

```

### `crates/app/src/features/chat/thread_view/caches.rs`

````rust
//! Thread view caches.

use super::*;

#[derive(Clone)]
pub(crate) struct ToolOutputPreview {
    pub(crate) source: String,
    pub(crate) lines: Arc<[Arc<str>]>,
    pub(crate) total_lines: usize,
    pub(crate) full: Arc<str>,
}

pub(crate) struct MarkdownCacheEntry {
    content_hash: u64,
    streaming: bool,
    _has_text: bool,
    _char_count: usize,
    blocks: Arc<[MarkdownBlock]>,
}

pub(crate) fn live_tail_height(tail: &str) -> f32 {
    if tail.trim().is_empty() {
        return 0.0;
    }
    if tail
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("```"))
    {
        let lines = tail.lines().count().saturating_sub(1).max(1) as f32;
        return 18.0 + 8.0 + lines * Tokens::DIFF_LINE_HEIGHT;
    }
    let visual_lines = tail
        .lines()
        .map(|line| line.chars().count().div_ceil(58).max(1))
        .sum::<usize>()
        .max(1) as f32;
    visual_lines * LINE_LEADING
}

pub(crate) fn markdown_content_hash(source: &str, streaming: bool) -> u64 {
    let hash = blake3::hash(source.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bytes) ^ ((u64::from(streaming)) << 63)
}

impl ThreadView {
    pub(crate) fn streaming_assistant_height_estimate_cached(
        &self,
        item_id: &str,
        sanitized: &str,
        gap: f32,
    ) -> f32 {
        let sealed = self.sealed_blocks.get(item_id);
        let sealed_height = sealed.map(|sealed| sealed.height).unwrap_or(0.0);
        let tail = if let Some(sealed) = sealed {
            sanitized.get(sealed.sealed_end..).unwrap_or_default()
        } else {
            crate::shared::components::streaming_markdown::split_at_seal_boundary(sanitized).1
        };
        let mut height = crate::features::chat::layout::assistant_body_pt()
            + sealed_height
            + live_tail_height(tail);
        if !sanitized.is_empty() {
            height += crate::features::chat::layout::assistant_streaming_extra();
        }
        let accessory_height = self
            .item_index
            .get(item_id)
            .copied()
            .map(|item_ix| assistant_accessory_height(item_ix, &self.items))
            .unwrap_or_else(assistant_actions_height);
        height + accessory_height + gap
    }

    pub(crate) fn record_stream_patch_cost(&mut self, elapsed: Duration) {
        self.last_stream_patch = Some(Instant::now());
        self.stream_batch_ms = if elapsed > Duration::from_millis(12) {
            STREAM_BATCH_BACKPRESSURE_MS
        } else if elapsed > Duration::from_millis(6) {
            STREAM_BATCH_SLOW_MS
        } else {
            STREAM_FRAME_SYNC_MS
        };
    }

    pub(crate) fn flush_sealed_blocks_sanitized(&mut self, item_id: &str, display: &str) {
        let _profile = crate::shared::render_profile::span("flush_sealed_blocks_sanitized");
        let cached_sealed_end = self
            .sealed_blocks
            .get(item_id)
            .map(|cached| cached.sealed_end)
            .unwrap_or(0);
        let (sealed, sealed_len) =
            if cached_sealed_end > 0 && display.is_char_boundary(cached_sealed_end) {
                let tail = display.get(cached_sealed_end..).unwrap_or_default();
                let (newly_sealed, _) =
                    crate::shared::components::streaming_markdown::split_at_seal_boundary(tail);
                let sealed_len = cached_sealed_end + newly_sealed.len();
                (&display[..sealed_len], sealed_len)
            } else {
                let (sealed, _) =
                    crate::shared::components::streaming_markdown::split_at_seal_boundary(display);
                (sealed, sealed.len())
            };
        if sealed_len == 0 {
            return;
        }
        if self
            .sealed_blocks
            .get(item_id)
            .is_some_and(|cached| cached.sealed_end >= sealed_len)
        {
            return;
        }

        let content_hash = markdown_content_hash(sealed, false);
        if self
            .pending_sealed_parses
            .get(item_id)
            .is_some_and(|pending_hash| *pending_hash == content_hash)
        {
            return;
        }

        self.pending_sealed_parses
            .insert(item_id.to_string(), content_hash);
        let tx = self.sealed_parse_tx.clone();
        let item_id = item_id.to_string();
        let sealed = sealed.to_string();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let blocks = parse_markdown_blocks_shared_streaming(&sealed, false);
            let height = crate::shared::components::markdown_preview::estimate_markdown_height(
                &blocks, false,
            );
            crate::shared::render_profile::record(
                "sealed_markdown_parse_worker",
                start.elapsed(),
                blocks.len() as u64,
            );
            let _ = tx.send(SealedMarkdownParseResult {
                item_id,
                sealed_end: sealed_len,
                content_hash,
                blocks,
                height,
            });
        });
    }

    pub(crate) fn handle_sealed_parse_result(
        &mut self,
        result: SealedMarkdownParseResult,
        cx: &mut Context<Self>,
    ) {
        if !self
            .pending_sealed_parses
            .get(&result.item_id)
            .is_some_and(|pending_hash| *pending_hash == result.content_hash)
        {
            return;
        }
        self.pending_sealed_parses.remove(&result.item_id);
        if self
            .sealed_blocks
            .get(&result.item_id)
            .is_some_and(|cached| {
                cached.sealed_end >= result.sealed_end && cached.content_hash == result.content_hash
            })
        {
            return;
        }
        self.sealed_blocks.insert(
            result.item_id.clone(),
            SealedBlockCache {
                sealed_end: result.sealed_end,
                blocks: result.blocks,
                content_hash: result.content_hash,
                height: result.height,
            },
        );

        let Some(&item_ix) = self.item_index.get(&result.item_id) else {
            cx.notify();
            return;
        };
        let Some(header_ix) = self.header_ix_for_item(item_ix as u32) else {
            cx.notify();
            return;
        };
        let (start, end) = manifest_span(&self.manifest, header_ix);
        if end - start == 1 {
            let effects = self.patch_streaming_assistant_row(item_ix, start);
            self.apply_thread_effects(effects, cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn cached_markdown_blocks(
        &mut self,
        id: &str,
        markdown: &str,
        streaming: bool,
    ) -> Arc<[MarkdownBlock]> {
        let display = crate::agent::text::sanitize_assistant_text(markdown);
        let hash = markdown_content_hash(&display, streaming);
        if let Some(entry) = self.markdown_cache.get(id) {
            if entry.content_hash == hash && entry.streaming == streaming {
                crate::shared::render_profile::record(
                    "cached_markdown_blocks_hit",
                    Duration::ZERO,
                    1,
                );
                return Arc::clone(&entry.blocks);
            }
        }
        let _profile = crate::shared::render_profile::span("cached_markdown_blocks_miss");
        let blocks = parse_markdown_blocks_shared_streaming(&display, streaming);
        let char_count = display.chars().count();
        self.markdown_cache.insert(
            id.to_string(),
            MarkdownCacheEntry {
                content_hash: hash,
                streaming,
                _has_text: !display.is_empty(),
                _char_count: char_count,
                blocks: Arc::clone(&blocks),
            },
        );
        blocks
    }

    pub(crate) fn cached_tool_output_preview(
        &mut self,
        id: &str,
        output: &str,
    ) -> ToolOutputPreview {
        if let Some(cached) = self.tool_output_cache.get(id) {
            if cached.source == output {
                return cached.clone();
            }
        }

        let (lines, total_lines, _truncated, full) = build_tool_output_preview(output);
        let preview = ToolOutputPreview {
            source: output.to_string(),
            lines,
            total_lines,
            full,
        };
        self.tool_output_cache
            .insert(id.to_string(), preview.clone());
        preview
    }
}

````

### `crates/app/src/features/chat/thread_view/scroll.rs`

```rust
//! Thread scroll behavior

use super::*;
use crate::features::composer::components::composer_pill::composer_input_needs_stacked_layout;

impl ThreadView {
    pub(crate) fn handle_scroll_if_changed(&mut self) {
        let offset = self.scroll_handle.base_handle().offset();
        if self.last_scroll_offset == Some(offset) {
            return;
        }
        self.last_scroll_offset = Some(offset);
        self.motion_paused = true;
        self.last_scroll_activity = Some(Instant::now());
        if offset.y < px(-8.0) {
            self.user_scrolled_up = true;
            self.stick_to_bottom = false;
            self.pending_scroll_bottom = false;
        } else {
            self.user_scrolled_up = false;
            self.stick_to_bottom = true;
        }
    }

    pub(crate) fn clear_motion_pause_if_idle(&mut self) {
        if !self.motion_paused {
            return;
        }
        if let Some(t) = self.last_scroll_activity {
            if t.elapsed() >= Duration::from_millis(MOTION_PAUSE_MS) {
                self.motion_paused = false;
                self.last_scroll_activity = None;
            }
        }
    }

    pub(crate) fn animate(&self) -> bool {
        !self.motion_paused
    }

    pub(crate) fn end_spacer_height(&self) -> f32 {
        Tokens::composer_stack_total(
            self.composer_overlay_bar_height,
            self.composer_input_expanded,
            self.composer_has_attachments,
            self.composer_has_error,
        )
    }

    pub(crate) fn composer_thread_inset_px(&self) -> f32 {
        Tokens::composer_thread_inset_px(
            self.composer_overlay_bar_height,
            self.composer_input_expanded,
            self.composer_has_attachments,
            self.composer_has_error,
        )
    }

    pub(crate) fn refresh_end_spacer_size(&mut self) {
        let Some(ix) = self
            .manifest
            .iter()
            .rposition(|row| matches!(row, RowRef::EndSpacer))
        else {
            return;
        };
        let height = px(self.end_spacer_height());
        self.mutate_row_sizes(|sizes| {
            if ix < sizes.len() {
                sizes[ix].height = height;
            }
        });
    }

    /// Keep end-spacer height in sync with the live composer footer (pill + mode chips + overlays).
    pub(crate) fn sync_composer_footer(&mut self, cx: &Context<Self>) {
        let agent = self.agent.read(cx);
        let overlay = agent.composer_overlay_bar_height();
        let has_attachments = !agent.pending_image_attachments.is_empty();
        let has_error = agent.composer_error.is_some();
        let expanded = composer_input_needs_stacked_layout(&agent.input_state.read(cx).value())
            || has_attachments
            || has_error;
        if (overlay - self.composer_overlay_bar_height).abs() <= 0.5
            && expanded == self.composer_input_expanded
            && has_attachments == self.composer_has_attachments
            && has_error == self.composer_has_error
        {
            return;
        }
        self.composer_overlay_bar_height = overlay;
        self.composer_input_expanded = expanded;
        self.composer_has_attachments = has_attachments;
        self.composer_has_error = has_error;
        self.refresh_end_spacer_size();
        if self.stick_to_bottom && !self.user_scrolled_up {
            self.pending_scroll_bottom = true;
        }
    }
}

```

### `crates/app/src/features/chat/thread_view/sync.rs`

```rust
//! Thread sync and state machine

use super::*;

impl ThreadView {
    pub(crate) fn set_conversation_inner(
        &mut self,
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        self.conversation_id = Some(conversation_id.clone());
        self.stick_to_bottom = true;
        self.pending_scroll_bottom = true;
        self.pending_items = None;
        self.pending_item_updates.clear();
        self.pending_streaming_row_patches.clear();
        self.pending_mutated_item_patches.clear();
        self.last_stream_sync = None;
        self.markdown_cache.clear();
        self.tool_output_cache.clear();
        self.item_index.clear();
        self.header_manifest_index.clear();
        self.user_scrolled_up = false;
        self.last_scroll_offset = None;
        self.sealed_blocks.clear();
        self.pending_sealed_parses.clear();
        self.sync_inner(conversation_id, items, cx, true, false)
    }

    pub(crate) fn push_item_inner(&mut self, item: ThreadItem) -> Vec<ThreadEffect> {
        let item_ix = self.items.len() as u32;
        self.item_index
            .insert(item.id().to_string(), item_ix as usize);
        self.items.push(item);
        let new_refs = row_refs_for_item_with_mode(
            item_ix,
            self.items.last().expect("just pushed"),
            self.transcript_mode,
            &self.items,
        );
        let new_len = new_refs.len();

        let insert_at = self.manifest.len().saturating_sub(1);
        let prev_before = self.manifest.get(insert_at.wrapping_sub(1)).copied();
        let new_sizes = row_sizes_for_rows(&new_refs, &self.items, prev_before);
        let header_ix = new_refs
            .iter()
            .position(|row| row.is_header() && row.item_ix() == Some(item_ix))
            .map(|offset| insert_at + offset);

        self.manifest.splice(insert_at..insert_at, new_refs);
        self.set_header_manifest_ix(item_ix as usize, header_ix);

        let size_insert = self.row_sizes.len().saturating_sub(1);
        self.mutate_row_sizes(|sizes| {
            sizes.splice(size_insert..size_insert, new_sizes);
        });
        self.ensure_end_spacer();
        self.refresh_end_spacer_size();
        self.refresh_row_size_at(size_insert + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());
        let mut effects = vec![ThreadEffect::Notify];
        if self.stick_to_bottom && !self.user_scrolled_up {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        effects
    }

    /// Insert manifest rows when an item exists in `items` but was omitted (e.g. transcript filter drift).
    pub(crate) fn ensure_manifest_rows_for_item(&mut self, item_ix: usize) -> bool {
        if self.header_ix_for_item(item_ix as u32).is_some() {
            return false;
        }
        let Some(item) = self.items.get(item_ix).cloned() else {
            return false;
        };
        let new_refs =
            row_refs_for_item_with_mode(item_ix as u32, &item, self.transcript_mode, &self.items);
        if new_refs.is_empty() {
            return false;
        }
        let new_len = new_refs.len();
        let insert_at = self.manifest.len().saturating_sub(1);
        let prev_before = self.manifest.get(insert_at.wrapping_sub(1)).copied();
        let new_sizes = row_sizes_for_rows(&new_refs, &self.items, prev_before);
        let header_ix = new_refs
            .iter()
            .position(|row| row.is_header() && row.item_ix() == Some(item_ix as u32))
            .map(|offset| insert_at + offset);

        self.manifest.splice(insert_at..insert_at, new_refs);
        self.set_header_manifest_ix(item_ix, header_ix);

        let size_insert = self.row_sizes.len().saturating_sub(1);
        self.mutate_row_sizes(|sizes| {
            sizes.splice(size_insert..size_insert, new_sizes);
        });
        self.ensure_end_spacer();
        self.refresh_row_size_at(size_insert + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());
        true
    }

    pub(crate) fn mutate_row_sizes(
        &mut self,
        update: impl FnOnce(&mut Vec<gpui::Size<gpui::Pixels>>),
    ) {
        update(Rc::make_mut(&mut self.row_sizes));
    }

    pub(crate) fn rebuild_header_manifest_index(&mut self) {
        self.header_manifest_index = vec![None; self.items.len()];
        for (row_ix, row) in self.manifest.iter().copied().enumerate() {
            if row.is_header() {
                if let Some(item_ix) = row.item_ix().map(|ix| ix as usize) {
                    if item_ix < self.header_manifest_index.len() {
                        self.header_manifest_index[item_ix] = Some(row_ix);
                    }
                }
            }
        }
    }

    pub(crate) fn header_ix_for_item(&self, item_ix: u32) -> Option<usize> {
        self.header_manifest_index
            .get(item_ix as usize)
            .and_then(|ix| *ix)
    }

    pub(crate) fn set_header_manifest_ix(&mut self, item_ix: usize, row_ix: Option<usize>) {
        if self.header_manifest_index.len() <= item_ix {
            self.header_manifest_index.resize(item_ix + 1, None);
        }
        self.header_manifest_index[item_ix] = row_ix;
    }

    pub(crate) fn shift_header_manifest_indices_after(
        &mut self,
        item_ix: usize,
        start: usize,
        delta: isize,
    ) {
        if delta == 0 {
            return;
        }

        for (ix, header_ix) in self.header_manifest_index.iter_mut().enumerate() {
            if ix == item_ix {
                continue;
            }
            let Some(current) = *header_ix else {
                continue;
            };
            if current > start {
                *header_ix = Some(current.saturating_add_signed(delta));
            }
        }
    }

    pub(crate) fn replace_manifest_span_for_item(
        &mut self,
        item_ix: usize,
        start: usize,
        end: usize,
        new_refs: Vec<RowRef>,
        new_sizes: Vec<gpui::Size<gpui::Pixels>>,
    ) -> usize {
        let old_len = end.saturating_sub(start);
        let new_len = new_refs.len();
        let header_ix = new_refs
            .iter()
            .position(|row| row.is_header() && row.item_ix() == Some(item_ix as u32))
            .map(|offset| start + offset);

        self.manifest.splice(start..end, new_refs);
        self.mutate_row_sizes(|sizes| {
            sizes.splice(start..end, new_sizes);
        });
        self.set_header_manifest_ix(item_ix, header_ix);
        self.shift_header_manifest_indices_after(
            item_ix,
            start,
            new_len as isize - old_len as isize,
        );
        new_len
    }

    pub(crate) fn refresh_row_size_at(&mut self, ix: usize) {
        if ix >= self.manifest.len() {
            return;
        }
        let row = self.manifest[ix];
        let prev = self.manifest.get(ix.wrapping_sub(1)).copied();
        let next = row_size(row, prev, &self.items);
        self.mutate_row_sizes(|sizes| {
            if ix < sizes.len() {
                sizes[ix] = next;
            }
        });
    }

    pub(crate) fn row_height_for_patch(&mut self, row: RowRef, prev: Option<RowRef>) -> f32 {
        let _profile = crate::shared::render_profile::span("row_height_with_collapsed");
        if matches!(row, RowRef::EndSpacer) {
            return self.end_spacer_height();
        }
        if let RowRef::AssistantMessage { item_ix } = row {
            let assistant_data: Option<(String, String)> =
                self.items.get(item_ix as usize).and_then(|item| {
                    if let ThreadItem::AssistantMessage {
                        id,
                        markdown,
                        streaming: true,
                        ..
                    } = item
                    {
                        let sanitized = crate::agent::text::sanitize_assistant_text(markdown);
                        Some((id.clone(), sanitized))
                    } else {
                        None
                    }
                });
            if let Some((id, sanitized)) = assistant_data {
                let blocks = self.cached_markdown_blocks(&id, &sanitized, true);
                let gap = row_top_gap(row, prev, &self.items);
                return assistant_row_height_from_blocks(
                    item_ix as usize,
                    &self.items,
                    blocks.as_ref(),
                    true,
                    !sanitized.is_empty(),
                ) + gap;
            }
        }
        row_height_with_collapsed(row, prev, &self.items, &std::collections::HashSet::new())
    }

    pub(crate) fn row_sizes_for_patch(
        &mut self,
        rows: &[RowRef],
        prev_before_first: Option<RowRef>,
    ) -> Vec<gpui::Size<gpui::Pixels>> {
        let _profile = crate::shared::render_profile::span("row_sizes_for_patch");
        rows.iter()
            .enumerate()
            .map(|(i, &row)| {
                let prev = if i == 0 {
                    prev_before_first
                } else {
                    Some(rows[i - 1])
                };
                gpui::size(
                    gpui::px(Tokens::THREAD_MAX_WIDTH),
                    gpui::px(self.row_height_for_patch(row, prev)),
                )
            })
            .collect()
    }

    pub(crate) fn ensure_end_spacer(&mut self) {
        if self
            .manifest
            .last()
            .is_some_and(|row| matches!(row, RowRef::EndSpacer))
        {
            return;
        }
        self.manifest.push(RowRef::EndSpacer);
        let end_spacer = gpui::size(
            gpui::px(Tokens::THREAD_MAX_WIDTH),
            gpui::px(self.end_spacer_height()),
        );
        self.mutate_row_sizes(|sizes| {
            sizes.push(end_spacer);
        });
    }

    pub(crate) fn update_item_inner(
        &mut self,
        item: ThreadItem,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        let debounced = matches!(
            &item,
            ThreadItem::ToolCall {
                status: crate::features::shell::state::AgentStatus::RunningTool,
                ..
            } | ThreadItem::AssistantMessage {
                streaming: true,
                ..
            } | ThreadItem::ReasoningStep {
                status: crate::features::shell::state::AgentStatus::Thinking,
                ..
            }
        );
        if debounced {
            self.pending_item_updates
                .insert(item.id().to_string(), item);
            return vec![ThreadEffect::ScheduleItemUpdate];
        }
        self.apply_item_update(item, cx)
    }

    pub(crate) fn dispatch(&mut self, action: ThreadAction, cx: &mut Context<Self>) {
        let effects = self.update_thread_state(action, cx);
        self.apply_thread_effects(effects, cx);
    }

    pub(crate) fn update_thread_state(
        &mut self,
        action: ThreadAction,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        match action {
            ThreadAction::SetConversation {
                conversation_id,
                items,
            } => self.set_conversation_inner(conversation_id, items, cx),
            ThreadAction::Sync {
                conversation_id,
                items,
                immediate,
                run_active,
            } => self.sync_inner(conversation_id, items, cx, immediate, run_active),
            ThreadAction::PushItem(item) => self.push_item_inner(item),
            ThreadAction::UpdateItem(item) => self.update_item_inner(item, cx),
            ThreadAction::RefreshItem(item) => {
                self.pending_item_updates.remove(item.id());
                self.patch_item_rows(item, cx)
            }
            ThreadAction::SetApprovalActive(active) => {
                if self.approval_active == active {
                    Vec::new()
                } else {
                    self.approval_active = active;
                    vec![ThreadEffect::Notify]
                }
            }
        }
    }

    pub(crate) fn apply_thread_effects(
        &mut self,
        effects: Vec<ThreadEffect>,
        cx: &mut Context<Self>,
    ) {
        for effect in effects {
            self.apply_thread_effect_with_context(effect, cx);
        }
    }

    pub(crate) fn apply_thread_effect_with_context(
        &mut self,
        effect: ThreadEffect,
        cx: &mut Context<Self>,
    ) {
        match effect {
            ThreadEffect::Notify => cx.notify(),
            ThreadEffect::ScrollToBottom => self.apply_thread_effect(effect),
            ThreadEffect::ScheduleStreamSync { interval_ms } => {
                self.schedule_stream_sync(interval_ms, cx)
            }
            ThreadEffect::ScheduleItemUpdate => self.schedule_item_update(cx),
        }
    }

    pub(crate) fn apply_thread_effect(&mut self, effect: ThreadEffect) {
        if matches!(effect, ThreadEffect::ScrollToBottom) {
            self.stick_to_bottom = true;
            self.user_scrolled_up = false;
            self.pending_scroll_bottom = true;
        }
    }

    pub(crate) fn apply_item_update(
        &mut self,
        item: ThreadItem,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        let Some(&item_ix) = self.item_index.get(item.id()) else {
            return Vec::new();
        };
        if let Some(old) = self.items.get(item_ix) {
            if !collapsed_item_header_changed(&item, old) {
                return Vec::new();
            }
        }
        self.patch_item_rows(item, cx)
    }

    pub(crate) fn patch_effects(&self, height_changed: bool) -> Vec<ThreadEffect> {
        let mut effects = vec![ThreadEffect::Notify];
        if height_changed && self.stick_to_bottom && !self.user_scrolled_up {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        effects
    }

    /// Hot path for streaming assistant ticks: update item + render cache, grow shell
    /// height only when needed so `v_virtual_list` skips O(n) offset recomputation.
    pub(crate) fn patch_streaming_assistant_row(
        &mut self,
        item_ix: usize,
        row_ix: usize,
    ) -> Vec<ThreadEffect> {
        let _profile = crate::shared::render_profile::span("patch_streaming_assistant_row");
        let row = RowRef::AssistantMessage {
            item_ix: item_ix as u32,
        };
        let prev = self.manifest.get(row_ix.wrapping_sub(1)).copied();
        let new_height = match self.items.get(item_ix) {
            Some(ThreadItem::AssistantMessage { id, markdown, .. }) => {
                let id = id.clone();
                let sanitized = crate::agent::text::sanitize_assistant_text(markdown);
                let gap = row_top_gap(row, prev, &self.items);
                self.flush_sealed_blocks_sanitized(&id, &sanitized);
                self.streaming_assistant_height_estimate_cached(&id, &sanitized, gap)
            }
            _ => return vec![ThreadEffect::Notify],
        };
        let current_height = self
            .row_sizes
            .get(row_ix)
            .map(|size| f32::from(size.height))
            .unwrap_or(0.0);

        let height_changed = (new_height - current_height).abs() > STREAMING_HEIGHT_EPSILON;
        if height_changed {
            let new_size = gpui::size(gpui::px(Tokens::THREAD_MAX_WIDTH), gpui::px(new_height));
            self.mutate_row_sizes(|sizes| {
                if row_ix < sizes.len() {
                    sizes[row_ix] = new_size;
                }
            });
        }

        self.patch_effects(height_changed)
    }

    pub(crate) fn patch_item_rows(
        &mut self,
        item: ThreadItem,
        _cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        let item_id = item.id().to_string();
        let Some(&item_ix) = self.item_index.get(&item_id) else {
            return Vec::new();
        };
        let was_streaming = matches!(
            self.items.get(item_ix),
            Some(ThreadItem::AssistantMessage {
                streaming: true,
                ..
            })
        );
        self.items[item_ix] = item;
        if was_streaming {
            if let ThreadItem::AssistantMessage {
                id,
                streaming: false,
                ..
            } = &self.items[item_ix]
            {
                self.sealed_blocks.remove(id);
                self.pending_sealed_parses.remove(id);
                self.markdown_cache.remove(id);
            }
        }

        if matches!(
            self.items[item_ix],
            ThreadItem::AssistantMessage {
                streaming: true,
                ..
            }
        ) {
            if let Some(header_ix) = self.header_ix_for_item(item_ix as u32) {
                let (start, end) = manifest_span(&self.manifest, header_ix);
                if end - start == 1
                    && matches!(
                        self.manifest.get(start),
                        Some(RowRef::AssistantMessage {
                            item_ix: ix
                        }) if *ix == item_ix as u32
                    )
                {
                    return self.patch_streaming_assistant_row(item_ix, start);
                }
            }
        }

        let item_ref = &self.items[item_ix];
        let Some(header_ix) = self.header_ix_for_item(item_ix as u32) else {
            return vec![ThreadEffect::Notify];
        };

        let (start, end) = manifest_span(&self.manifest, header_ix);
        let new_refs = row_refs_for_item_with_mode(
            item_ix as u32,
            item_ref,
            self.transcript_mode,
            &self.items,
        );
        let prev_before = self.manifest.get(start.wrapping_sub(1)).copied();
        let new_sizes = self.row_sizes_for_patch(&new_refs, prev_before);

        let new_len = self.replace_manifest_span_for_item(item_ix, start, end, new_refs, new_sizes);
        self.refresh_row_size_at(start + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());

        self.patch_effects(true)
    }

    pub(crate) fn patch_current_item_rows(&mut self, item_ix: usize) -> Vec<ThreadEffect> {
        let _profile = crate::shared::render_profile::span("patch_current_item_rows");
        let Some(item_ref) = self.items.get(item_ix) else {
            return Vec::new();
        };
        let Some(header_ix) = self.header_ix_for_item(item_ix as u32) else {
            return vec![ThreadEffect::Notify];
        };

        let (start, end) = manifest_span(&self.manifest, header_ix);
        let new_refs = row_refs_for_item_with_mode(
            item_ix as u32,
            item_ref,
            self.transcript_mode,
            &self.items,
        );
        let prev_before = self.manifest.get(start.wrapping_sub(1)).copied();
        let new_sizes = self.row_sizes_for_patch(&new_refs, prev_before);

        let new_len = self.replace_manifest_span_for_item(item_ix, start, end, new_refs, new_sizes);
        self.refresh_row_size_at(start + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());

        self.patch_effects(true)
    }

    pub(crate) fn rebuild_all(&mut self, items: Vec<ThreadItem>) -> Vec<ThreadEffect> {
        self.items = items;
        self.tool_output_cache.clear();
        self.pending_sealed_parses.clear();
        self.item_index = self
            .items
            .iter()
            .enumerate()
            .map(|(ix, item)| (item.id().to_string(), ix))
            .collect();
        self.manifest = build_manifest_with_transcript(&self.items, self.transcript_mode);
        self.row_sizes = Rc::new(row_sizes_for_manifest(&self.manifest, &self.items));
        self.rebuild_header_manifest_index();
        self.refresh_end_spacer_size();
        vec![ThreadEffect::Notify]
    }

    pub(crate) fn try_incremental_sync(&mut self, items: &[ThreadItem]) -> bool {
        if items.len() < self.items.len() {
            return false;
        }
        for i in 0..self.items.len() {
            if self.items[i].id() != items[i].id() {
                return false;
            }
        }
        for i in 0..self.items.len() {
            if self.items[i] != items[i] {
                let item = items[i].clone();
                if let Some(&item_ix) = self.item_index.get(item.id()) {
                    self.items[item_ix] = item.clone();
                    if let Some(header_ix) = self.header_ix_for_item(item_ix as u32) {
                        let (start, end) = manifest_span(&self.manifest, header_ix);
                        let new_refs = row_refs_for_item_with_mode(
                            item_ix as u32,
                            &item,
                            self.transcript_mode,
                            &self.items,
                        );
                        let prev_before = self.manifest.get(start.wrapping_sub(1)).copied();
                        let new_sizes = self.row_sizes_for_patch(&new_refs, prev_before);
                        let new_len = self.replace_manifest_span_for_item(
                            item_ix, start, end, new_refs, new_sizes,
                        );
                        self.refresh_row_size_at(start + new_len);
                    }
                }
            }
        }
        for item in &items[self.items.len()..] {
            self.push_item_inner(item.clone());
        }
        true
    }

    pub(crate) fn sync_inner(
        &mut self,
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        _cx: &mut Context<Self>,
        immediate: bool,
        run_active: bool,
    ) -> Vec<ThreadEffect> {
        self.conversation_id = Some(conversation_id);

        if immediate {
            self.pending_items = None;
            self.pending_item_updates.clear();
            self.pending_streaming_row_patches.clear();
            self.pending_mutated_item_patches.clear();
            self.last_stream_sync = None;
            self.stream_debounce_scheduled = false;
            return self.rebuild_all(items);
        }

        let item_streaming = items.iter().any(|item| match item {
            ThreadItem::AssistantMessage {
                streaming: true, ..
            } => true,
            ThreadItem::ToolCall {
                status: crate::features::shell::state::AgentStatus::RunningTool,
                ..
            } => true,
            ThreadItem::ReasoningStep {
                status: crate::features::shell::state::AgentStatus::Thinking,
                ..
            } => true,
            _ => false,
        });
        let streaming = run_active || item_streaming;

        if streaming {
            let interval_ms = if run_active {
                STREAM_FRAME_SYNC_MS
            } else if items.iter().any(|item| {
                matches!(
                    item,
                    ThreadItem::ToolCall {
                        status: crate::features::shell::state::AgentStatus::RunningTool,
                        ..
                    }
                )
            }) {
                STREAM_FRAME_SYNC_MS
            } else {
                ASSISTANT_STREAM_SYNC_MS
            };
            let defer = self
                .last_stream_sync
                .is_some_and(|t| t.elapsed() < Duration::from_millis(interval_ms));

            self.pending_items = Some(items.clone());

            if defer {
                return vec![ThreadEffect::ScheduleStreamSync { interval_ms }];
            }

            self.last_stream_sync = Some(Instant::now());
            self.pending_items = None;
        } else {
            self.pending_items = None;
            self.last_stream_sync = None;
            self.stream_debounce_scheduled = false;
        }

        self.effects_for_pending_items(items)
    }

    fn effects_for_pending_items(&mut self, items: Vec<ThreadItem>) -> Vec<ThreadEffect> {
        let new_tail = tail_signature(&items);
        let content_grew = items.len() > self.items.len() || new_tail != self.last_tail_signature;
        self.last_tail_signature = new_tail;

        let mut effects = if self.try_incremental_sync(&items) {
            self.refresh_end_spacer_size();
            vec![ThreadEffect::Notify]
        } else {
            self.rebuild_all(items)
        };
        if self.stick_to_bottom && !self.user_scrolled_up && content_grew {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        effects
    }

    pub(crate) fn schedule_stream_sync(&mut self, interval_ms: u64, cx: &mut Context<Self>) {
        if self.stream_debounce_scheduled {
            return;
        }
        self.stream_debounce_scheduled = true;
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_weak, cx| {
            Timer::after(Duration::from_millis(interval_ms)).await;
            entity
                .update(cx, |view, cx| {
                    view.stream_debounce_scheduled = false;
                    if let Some(items) = view.pending_items.take() {
                        view.last_stream_sync = Some(Instant::now());
                        let effects = view.effects_for_pending_items(items);
                        view.apply_thread_effects(effects, cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    pub(crate) fn schedule_item_update(&mut self, cx: &mut Context<Self>) {
        if self.item_update_debounce_scheduled {
            return;
        }
        self.item_update_debounce_scheduled = true;
        let batch_ms = if self.motion_paused || self.user_scrolled_up {
            self.stream_batch_ms.max(STREAM_BATCH_SLOW_MS)
        } else {
            self.stream_batch_ms
        };
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_weak, cx| {
            Timer::after(Duration::from_millis(batch_ms)).await;
            entity
                .update(cx, |view, cx| {
                    let _profile =
                        crate::shared::render_profile::span("ThreadView::schedule_item_update");
                    view.item_update_debounce_scheduled = false;
                    let patch_start = Instant::now();
                    let mut effects = Vec::new();

                    let stream_ids: Vec<_> = view.pending_streaming_row_patches.drain().collect();
                    crate::shared::render_profile::record(
                        "ThreadView::stream_patch_batch",
                        Duration::ZERO,
                        stream_ids.len() as u64,
                    );
                    for id in &stream_ids {
                        if let Some(&item_ix) = view.item_index.get(id) {
                            if let Some(header_ix) = view.header_ix_for_item(item_ix as u32) {
                                let (start, end) = manifest_span(&view.manifest, header_ix);
                                if end - start == 1 {
                                    effects
                                        .extend(view.patch_streaming_assistant_row(item_ix, start));
                                }
                            }
                        }
                    }

                    let mutated_ids: Vec<_> = view.pending_mutated_item_patches.drain().collect();
                    for id in &mutated_ids {
                        if let Some(&item_ix) = view.item_index.get(id) {
                            effects.extend(view.patch_current_item_rows(item_ix));
                        }
                    }

                    let pending: Vec<_> = view.pending_item_updates.drain().collect();
                    for (_, item) in pending {
                        effects.extend(view.apply_item_update(item, cx));
                    }
                    view.record_stream_patch_cost(patch_start.elapsed());
                    view.apply_thread_effects(effects, cx);
                })
                .ok();
        })
        .detach();
    }
}

```

## Agent activity components

### `crates/app/src/features/agent_activity/components/mod.rs`

```rust
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

```

### `crates/app/src/features/agent_activity/components/activity_step.rs`

```rust
use gpui::{FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

use crate::features::agent_activity::state::ActivityPhase;
use crate::features::shell::state::{SessionStep, StepStatus};
use crate::shared::components::step_icon::step_icon;
use crate::shared::state::ToolCatalog;
use crate::tokens::{Tokens, element_key};

#[allow(dead_code)]
pub fn phase_label(phase: &ActivityPhase) -> &'static str {
    match phase {
        ActivityPhase::Explore => "EXPLORE",
        ActivityPhase::Edit => "EDIT",
        ActivityPhase::Run => "RUN",
        ActivityPhase::Review => "REVIEW",
    }
}

#[allow(dead_code)]
pub fn activity_step(catalog: &ToolCatalog, step: &SessionStep) -> impl IntoElement {
    let (icon_name, icon_color) = step_icon(catalog, step);
    div()
        .id(element_key(
            "activity-step",
            &format!("{}-{}", step.item_ix, step.depth),
        ))
        .h(px(Tokens::ROW_HEIGHT_SM))
        .w_full()
        .pl(if step.depth > 0 {
            Tokens::spacing_3()
        } else {
            Tokens::spacing_0p5()
        })
        .pr(Tokens::spacing_1())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            Icon::new(icon_name)
                .size(px(14.0))
                .text_color(icon_color)
                .flex_shrink_0(),
        )
        .child(
            div()
                .text_size(Tokens::text_sm())
                .font_weight(FontWeight::MEDIUM)
                .text_color(step_label_color(step))
                .truncate()
                .child(step.label.clone()),
        )
        .when_some(step.detail.clone(), |el, detail| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .font_family("monospace")
                    .text_color(Tokens::tool_path_text())
                    .truncate()
                    .child(detail),
            )
        })
}

#[allow(dead_code)]
fn step_label_color(step: &SessionStep) -> gpui::Hsla {
    match step.status {
        StepStatus::Failed => Tokens::danger(),
        StepStatus::Running => Tokens::accent(),
        StepStatus::Done => match step.phase {
            ActivityPhase::Edit => Tokens::warning(),
            ActivityPhase::Review => Tokens::success(),
            _ => Tokens::text_secondary(),
        },
    }
}

```

### `crates/app/src/features/agent_activity/components/reasoning.rs`

```rust
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

```

### `crates/app/src/features/agent_activity/components/tool_call.rs`

```rust
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
                .text_color(Tokens::activity_verb_text())
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
        .pl(Tokens::spacing_7())
        .pr(Tokens::spacing_2())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .w_full()
                .min_w(px(0.0))
                .h(px(Tokens::ROW_HEIGHT_SM - 4.0))
                .px(Tokens::spacing_2())
                .flex()
                .items_center()
                .rounded(Tokens::radius_xs())
                .bg(Tokens::surface_hover().opacity(0.28))
                .child(render_tool_detail_content(item_id, detail, on_open_file)),
        )
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
        .min_w(px(64.0))
        .text_size(Tokens::text_xs())
        .line_height(Tokens::text_sm_leading_compact())
        .font_family(Tokens::ui_font_family())
        .font_weight(FontWeight::MEDIUM)
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

```

### `crates/app/src/features/agent_activity/components/diff.rs`

```rust
//! Diff summary rows in the thread timeline.

use gpui::{App, FontWeight, IntoElement, div, prelude::*};

use crate::features::shell::state::ActivityGroupPos;
use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
use crate::tokens::{Tokens, element_key};

pub fn render_diff_header_row(
    item_id: &str,
    files_changed: usize,
    additions: usize,
    deletions: usize,
    group_pos: Option<ActivityGroupPos>,
    on_toggle: impl Fn(&mut App) + 'static,
    on_review: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let stats = format!("+{additions} −{deletions}");
    let summary = format!(
        "Prepared changes in {files_changed} file{} · {stats}",
        if files_changed == 1 { "" } else { "s" }
    );

    activity_group_wrap(
        timeline_row(
            element_key("diff-header", item_id),
            div()
                .text_size(Tokens::text_sm())
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::text_secondary())
                .hover(|s| s.text_color(Tokens::text_primary()))
                .child(summary)
                .into_any_element(),
            div()
                .id(element_key("review", item_id))
                .flex_shrink_0()
                .cursor_pointer()
                .px(Tokens::spacing_1())
                .py(Tokens::spacing_0p5())
                .rounded(Tokens::radius_xs())
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .hover(|s| s.text_color(Tokens::text_primary()))
                .on_click(move |_, _, app: &mut App| on_review(app))
                .child("Open diff")
                .into_any_element(),
            move |_, _, app: &mut App| on_toggle(app),
        ),
        group_pos,
    )
}

pub fn render_diff_file_line_row(
    item_id: &str,
    path: &str,
    additions: usize,
    deletions: usize,
) -> impl IntoElement {
    div()
        .id(element_key("diff-file-line", item_id))
        .w_full()
        .pl(Tokens::spacing_1())
        .child(file_stat_line(path, additions, deletions))
}

fn file_stat_line(path: &str, additions: usize, deletions: usize) -> impl IntoElement {
    let path = path.to_string();
    div()
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .text_size(Tokens::text_code())
                .font_family("monospace")
                .text_color(Tokens::text_faint())
                .child(path),
        )
        .when(additions > 0, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_secondary())
                    .child(format!("+{additions}")),
            )
        })
        .when(deletions > 0, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::danger())
                    .child(format!("−{deletions}")),
            )
        })
}

```

### `crates/app/src/features/agent_activity/components/context_trace.rs`

```rust
//! Context evidence rows in the thread timeline.

use gpui::{App, FontWeight, IntoElement, div, prelude::*, px};

use crate::features::agent_activity::components::{
    ActivityRowVisual, activity_header_row_with_visual,
};
use crate::features::shell::state::{ActivityGroupPos, ContextEntryKind, ContextTraceEntry};
use crate::tokens::{Tokens, element_key};

pub fn render_context_trace_header_row(
    item_id: &str,
    entries: &[ContextTraceEntry],
    expanded: bool,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    activity_header_row_with_visual(
        ActivityRowVisual {
            row_key: "context-trace-row",
            header_key: "context-trace-header",
            item_id,
            running: false,
            show_loading: false,
            animate,
            group_pos,
        },
        "Used context".to_string(),
        Some(context_trace_summary(entries)),
        div()
            .flex_shrink_0()
            .text_size(Tokens::text_sm())
            .line_height(Tokens::text_sm_leading_compact())
            .text_color(Tokens::text_tertiary())
            .opacity(0.78)
            .child(if expanded { "⌄" } else { "›" })
            .into_any_element(),
        on_toggle,
    )
}

pub fn render_context_trace_line_row(item_id: &str, entry: &ContextTraceEntry) -> impl IntoElement {
    div()
        .id(element_key("context-trace-line", item_id))
        .w_full()
        .h(px(Tokens::ROW_HEIGHT_MD))
        .pl(Tokens::spacing_7())
        .pr(Tokens::spacing_2())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .w_full()
                .min_w(px(0.0))
                .h(px(Tokens::ROW_HEIGHT_MD - 4.0))
                .pr(Tokens::spacing_2())
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .rounded(Tokens::radius_xs())
                .child(context_kind_badge(entry.kind))
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .overflow_hidden()
                        .truncate()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading_compact())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_secondary())
                        .child(entry.label.clone()),
                )
                .when_some(entry.detail.clone(), |el, detail| {
                    el.child(
                        div()
                            .max_w(px(220.0))
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .truncate()
                            .text_size(Tokens::text_xs())
                            .text_color(Tokens::text_tertiary())
                            .child(detail),
                    )
                })
                .child(
                    div()
                        .max_w(px(260.0))
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .truncate()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::activity_meta_text())
                        .child(entry.reason.clone()),
                ),
        )
}

pub fn context_trace_summary(entries: &[ContextTraceEntry]) -> String {
    let mut counts = [0usize; 6];
    for entry in entries {
        counts[kind_index(entry.kind)] += 1;
    }

    let parts: Vec<String> = [
        ContextEntryKind::RepoMap,
        ContextEntryKind::FileSlice,
        ContextEntryKind::Symbol,
        ContextEntryKind::Search,
        ContextEntryKind::Command,
        ContextEntryKind::Rule,
    ]
    .into_iter()
    .filter_map(|kind| {
        let count = counts[kind_index(kind)];
        (count > 0).then(|| format!("{count} {}", kind_noun(kind, count)))
    })
    .collect();

    if parts.is_empty() {
        "No evidence recorded".to_string()
    } else {
        parts.join(" · ")
    }
}

fn context_kind_badge(kind: ContextEntryKind) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .min_w(px(48.0))
        .text_size(Tokens::text_xs())
        .font_weight(FontWeight::MEDIUM)
        .text_color(Tokens::text_tertiary())
        .child(kind_badge_label(kind))
}

fn kind_index(kind: ContextEntryKind) -> usize {
    match kind {
        ContextEntryKind::RepoMap => 0,
        ContextEntryKind::FileSlice => 1,
        ContextEntryKind::Symbol => 2,
        ContextEntryKind::Search => 3,
        ContextEntryKind::Command => 4,
        ContextEntryKind::Rule => 5,
    }
}

fn kind_noun(kind: ContextEntryKind, count: usize) -> &'static str {
    match (kind, count == 1) {
        (ContextEntryKind::RepoMap, true) => "repo map",
        (ContextEntryKind::RepoMap, false) => "repo maps",
        (ContextEntryKind::FileSlice, true) => "file slice",
        (ContextEntryKind::FileSlice, false) => "file slices",
        (ContextEntryKind::Symbol, true) => "symbol",
        (ContextEntryKind::Symbol, false) => "symbols",
        (ContextEntryKind::Search, true) => "search",
        (ContextEntryKind::Search, false) => "searches",
        (ContextEntryKind::Command, true) => "command",
        (ContextEntryKind::Command, false) => "commands",
        (ContextEntryKind::Rule, true) => "rule",
        (ContextEntryKind::Rule, false) => "rules",
    }
}

fn kind_badge_label(kind: ContextEntryKind) -> &'static str {
    match kind {
        ContextEntryKind::RepoMap => "Map",
        ContextEntryKind::FileSlice => "File",
        ContextEntryKind::Symbol => "Symbol",
        ContextEntryKind::Search => "Search",
        ContextEntryKind::Command => "Command",
        ContextEntryKind::Rule => "Rule",
    }
}

```

### `crates/app/src/features/agent_activity/components/approval.rs`

```rust
//! Approval UI — diff panel bar and thread hint row.

use crate::features::shell::state::ApprovalRisk;
use crate::shared::components::buttons::{btn_approve, btn_deny};
use crate::tokens::{Tokens, activity_action_line_with_loading};
use gpui::{App, FontWeight, IntoElement, div, prelude::*, px};

/// Sticky elevated approval card — primary blocking action surface near composer.
pub struct ApprovalCardProps {
    pub title: String,
    pub risk: ApprovalRisk,
    pub can_act: bool,
    pub allow_always_label: Option<String>,
    pub on_deny: Box<dyn Fn(&mut App) + 'static>,
    pub on_approve: Box<dyn Fn(&mut App) + 'static>,
    pub on_approve_always: Option<Box<dyn Fn(&mut App) + 'static>>,
}

pub fn approval_card(props: ApprovalCardProps) -> impl IntoElement {
    let title = props.title;
    let risk_label = props.risk.label().to_string();
    let on_deny = props.on_deny;
    let on_approve = props.on_approve;
    let on_approve_always = props.on_approve_always;
    let allow_always = props.allow_always_label;

    div()
        .id("approval-card")
        .w_full()
        .max_w(px(Tokens::COMPOSER_MAX_WIDTH))
        .pb(Tokens::spacing_2())
        .child(
            div()
                .w_full()
                .px(Tokens::spacing_4())
                .py(Tokens::spacing_3())
                .rounded(Tokens::radius_lg())
                .bg(Tokens::surface())
                .border_1()
                .border_color(Tokens::approval_border())
                .shadow_sm()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_3())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(Tokens::spacing_0p5())
                        .child(
                            div()
                                .text_size(Tokens::text_sm())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_primary())
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .text_color(Tokens::text_tertiary())
                                .child(risk_label),
                        ),
                )
                .when(props.can_act, |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(Tokens::spacing_2())
                            .child(
                                btn_approve("approve-once", "Allow once")
                                    .on_click(move |_, _, app: &mut App| on_approve(app)),
                            )
                            .child(
                                btn_deny("deny-approval-card", "Deny")
                                    .on_click(move |_, _, app: &mut App| on_deny(app)),
                            )
                            .when_some(on_approve_always, |el, cb| {
                                el.when_some(allow_always, |el, label| {
                                    el.child(
                                        btn_approve("approve-always", label)
                                            .on_click(move |_, _, app: &mut App| cb(app)),
                                    )
                                })
                            }),
                    )
                }),
        )
}

/// Inline thread row when a tool awaits approval (actions live in the composer bar).
pub fn thread_approval_row(
    title: &str,
    risk: &ApprovalRisk,
    resolved: bool,
    approval_active: bool,
) -> impl IntoElement {
    let detail = if resolved {
        None
    } else if approval_active {
        Some(format!(
            "Review above the composer to continue · {}",
            risk.label()
        ))
    } else {
        None
    };
    div()
        .id("thread-approval-row")
        .w_full()
        .child(activity_action_line_with_loading(
            title,
            detail.as_deref(),
            !resolved && approval_active,
            false,
            approval_active,
            "approval-legacy",
        ))
}

/// Bottom bar inside the diff panel — title, risk, Reject / Approve.

/// Bottom bar inside the diff panel — title, risk, Reject / Approve.
pub fn diff_approval_bar(
    title: &str,
    risk: &ApprovalRisk,
    can_act: bool,
    on_reject: impl Fn(&mut App) + 'static,
    on_approve: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let title = title.to_string();
    let risk_label = risk.label().to_string();

    div()
        .id("diff-approval-bar")
        .flex_shrink_0()
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .border_t_1()
        .border_color(Tokens::border_subtle())
        .bg(Tokens::panel_bg())
        .child(
            div()
                .w_full()
                .px(Tokens::spacing_3())
                .py(Tokens::spacing_1p5())
                .rounded(Tokens::radius_full())
                .bg(Tokens::surface())
                .border_1()
                .border_color(Tokens::border_subtle())
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_3())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(Tokens::spacing_0p5())
                        .child(
                            div()
                                .text_size(Tokens::text_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Tokens::text_primary())
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_tertiary())
                                .child(risk_label),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap(Tokens::spacing_1())
                        .when(can_act, |el| {
                            el.child(
                                btn_deny("deny-approval", "Not now")
                                    .on_click(move |_, _, app: &mut App| on_reject(app)),
                            )
                            .child(
                                btn_approve("approve-approval", "Continue")
                                    .on_click(move |_, _, app: &mut App| on_approve(app)),
                            )
                        }),
                ),
        )
}

```

### `crates/app/src/features/agent_activity/components/pending_action_bar.rs`

```rust
//! Blocking action controls shown directly above the composer.

use gpui::{FontWeight, IntoElement, SharedString, div, prelude::*, px};

use crate::shared::components::buttons::{btn_approve, btn_deny, btn_ghost_label};
use crate::tokens::Tokens;
use crate::tokens::icons;
use gpui_component::Icon;

pub struct PendingActionBarProps {
    pub approval: Option<ApprovalActionProps>,
    pub patch: Option<PatchActionProps>,
}

#[allow(dead_code)]
pub struct ApprovalActionProps {
    pub title: String,
    pub risk: crate::features::shell::state::ApprovalRisk,
    pub risk_label: String,
    pub allow_always_label: Option<String>,
    pub can_act: bool,
    pub on_reject: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_approve: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_approve_always: Box<dyn Fn(&mut gpui::App) + 'static>,
}

pub struct PatchActionProps {
    pub summary: String,
    pub on_open: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_cancel: Box<dyn Fn(&mut gpui::App) + 'static>,
}

pub fn pending_action_bar(props: PendingActionBarProps) -> impl IntoElement {
    div()
        .id("pending-action-bar")
        .w_full()
        .max_w(px(Tokens::COMPOSER_MAX_WIDTH))
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .pb(Tokens::spacing_2())
        .when_some(props.patch, |el, patch| el.child(patch_action_row(patch)))
        .when_some(props.approval, |el, approval| {
            el.child(approval_action_row(approval))
        })
}

fn approval_action_row(props: ApprovalActionProps) -> impl IntoElement {
    let on_reject = props.on_reject;
    let on_approve = props.on_approve;
    let on_approve_always = props.on_approve_always;
    let allow_label = props.allow_always_label.map(SharedString::from);

    div()
        .id("approval-action-row")
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_LG))
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .rounded(Tokens::radius_sm())
        .bg(Tokens::surface_active())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .gap(Tokens::spacing_3())
        .child(
            Icon::new(icons::TERMINAL)
                .size(px(14.0))
                .text_color(Tokens::warning()),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .overflow_hidden()
                        .child(props.title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::text_tertiary())
                        .child(format!(
                            "Blocked until you review this request · {}",
                            props.risk_label
                        )),
                ),
        )
        .when(props.can_act, |el| {
            el.child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .gap(Tokens::spacing_1())
                    .child(btn_deny("thread-reject-approval", "Not now").on_click(
                        move |_, _, app: &mut gpui::App| {
                            on_reject(app);
                        },
                    ))
                    .when_some(allow_label, |el, label| {
                        el.child(btn_ghost_label("thread-approve-always", label).on_click(
                            move |_, _, app: &mut gpui::App| {
                                on_approve_always(app);
                            },
                        ))
                    })
                    .child(btn_approve("thread-approve-approval", "Continue").on_click(
                        move |_, _, app: &mut gpui::App| {
                            on_approve(app);
                        },
                    )),
            )
        })
}

fn patch_action_row(props: PatchActionProps) -> impl IntoElement {
    let on_open = props.on_open;
    let on_cancel = props.on_cancel;

    div()
        .id("patch-action-row")
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_MD))
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_1p5())
        .rounded(Tokens::radius_xs())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            Icon::new(icons::GIT_COMPARE)
                .size(px(14.0))
                .text_color(Tokens::text_tertiary()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(Tokens::text_sm())
                .text_color(Tokens::text_secondary())
                .child(format!("Changes are ready to review · {}", props.summary)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .gap(Tokens::spacing_1())
                .child(btn_ghost_label("thread-open-diff", "Open diff").on_click(
                    move |_, _, app: &mut gpui::App| {
                        on_open(app);
                    },
                ))
                .child(btn_ghost_label("thread-cancel-patch", "Dismiss").on_click(
                    move |_, _, app: &mut gpui::App| {
                        on_cancel(app);
                    },
                )),
        )
}

```

## Adjacent thread UI components

### `crates/app/src/features/composer/components/mode_chips.rs`

```rust
//! Mode chip row — compact action mode selector under the composer pill.
//!
//! Renders a row of lightweight chips: Plan, Ask, Edit, Apply, Agent.
//! The selected chip is text-first: subtle background, clear text weight,
//! and red-accented Apply/Agent states without decorative icons.
//! Hover tooltips explain what each mode does.

use std::rc::Rc;

use agent_protocol::AgentMode;
use gpui::{FontWeight, Hsla, IntoElement, SharedString, div, prelude::*, px};
use gpui_component::tooltip::Tooltip;

use crate::tokens::Tokens;

/// A single mode option rendered as a chip.
struct ModeChip {
    mode: AgentMode,
    label: &'static str,
    tooltip_title: &'static str,
    tooltip_body: &'static str,
    /// Color used for the selected chip's bg, border, and text.
    selected_color: fn() -> Hsla,
}

/// Available mode chips in display order.
/// Each chip gets a distinct accent color when selected.
const MODE_CHIPS: &[ModeChip] = &[
    ModeChip {
        mode: AgentMode::PlanOnly,
        label: "Plan",
        tooltip_title: "Plan",
        tooltip_body: "Read-only planning — research the codebase and draft an implementation plan.",
        selected_color: Tokens::text_primary,
    },
    ModeChip {
        mode: AgentMode::ChatOnly,
        label: "Ask",
        tooltip_title: "Ask",
        tooltip_body: "Chat only — answers questions without tools, edits, or commands.",
        selected_color: Tokens::text_primary,
    },
    ModeChip {
        mode: AgentMode::SuggestPatch,
        label: "Edit",
        tooltip_title: "Edit",
        tooltip_body: "Suggest edits — propose diffs and previews without applying changes.",
        selected_color: Tokens::warning,
    },
    ModeChip {
        mode: AgentMode::ApplyWithApproval,
        label: "Apply",
        tooltip_title: "Apply with approval",
        tooltip_body: "Default mode — runs tools and applies patches after you approve them.",
        selected_color: Tokens::danger,
    },
    ModeChip {
        mode: AgentMode::FullAccessDangerous,
        label: "Agent",
        tooltip_title: "Agent",
        tooltip_body: "Full access — auto-runs tools and applies changes with minimal gates.",
        selected_color: Tokens::danger,
    },
];

pub struct ModeChipsProps {
    pub selected_mode: AgentMode,
    pub on_select: Rc<dyn Fn(AgentMode, &mut gpui::App) + 'static>,
}

/// Lightweight chip row for selecting the agent action mode.
/// Placed under the composer pill with 8px spacing.
/// Chips are 26px tall, pill-shaped (radius_full), with 6px between them.
pub fn mode_chips(props: ModeChipsProps) -> impl IntoElement {
    div().id("composer-mode-chips").child(
        div()
            .id("mode-chips-row")
            .flex()
            .items_center()
            .gap(Tokens::spacing_1p5())
            .children(MODE_CHIPS.iter().map(|chip| {
                let is_selected = props.selected_mode == chip.mode;
                let mode = chip.mode.clone();
                let on_select = props.on_select.clone();
                render_chip(chip, is_selected, mode, on_select)
            })),
    )
}

fn render_chip(
    chip: &ModeChip,
    is_selected: bool,
    mode: AgentMode,
    on_select: Rc<dyn Fn(AgentMode, &mut gpui::App) + 'static>,
) -> impl IntoElement {
    let accent = (chip.selected_color)();
    let bg = if is_selected {
        accent.opacity(if chip.label == "Apply" || chip.label == "Agent" {
            0.12
        } else {
            0.08
        })
    } else {
        Hsla::default()
    };
    let border = if is_selected {
        accent.opacity(if chip.label == "Apply" || chip.label == "Agent" {
            0.22
        } else {
            0.14
        })
    } else {
        Tokens::border_subtle()
    };
    let text_color = if is_selected {
        if chip.label == "Apply" || chip.label == "Agent" {
            Tokens::text_primary()
        } else {
            accent
        }
    } else {
        Tokens::text_tertiary()
    };

    div()
        .id(SharedString::from(format!(
            "mode-chip-{}",
            chip.label.to_lowercase()
        )))
        .h(px(Tokens::COMPOSER_MODE_CHIP_HEIGHT))
        .px(Tokens::spacing_2())
        .rounded(Tokens::radius_full())
        .bg(bg)
        .border_1()
        .border_color(border)
        .flex()
        .items_center()
        .cursor_pointer()
        .tooltip({
            let title = chip.tooltip_title.to_string();
            let body = chip.tooltip_body.to_string();
            move |window, cx| mode_chip_tooltip(&title, &body, window, cx)
        })
        .hover(|s| {
            if is_selected {
                s
            } else {
                s.bg(Tokens::surface_hover()).border_color(Tokens::border())
            }
        })
        .on_click(move |_, _, app: &mut gpui::App| {
            on_select(mode.clone(), app);
        })
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(if is_selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(text_color)
                .child(chip.label),
        )
}

const MODE_CHIP_TOOLTIP_WIDTH: f32 = 220.0;

fn mode_chip_tooltip(
    title: &str,
    body: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> gpui::AnyView {
    let title = title.to_string();
    let body = body.to_string();
    let width = px(MODE_CHIP_TOOLTIP_WIDTH);

    Tooltip::element(move |_, _| {
        div()
            .id("mode-chip-tooltip")
            .w(width)
            .flex()
            .flex_col()
            .gap(Tokens::spacing_0p5())
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .whitespace_normal()
                    .text_size(Tokens::text_sm())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Tokens::text_primary())
                    .child(title.clone()),
            )
            .child(
                div()
                    .w_full()
                    .whitespace_normal()
                    .text_size(Tokens::text_xs())
                    .line_height(Tokens::text_sm_leading())
                    .text_color(Tokens::text_secondary())
                    .child(body.clone()),
            )
    })
    .w(width)
    .build(window, cx)
}

```

### `crates/app/src/features/todos/components/mod.rs`

```rust
pub mod todo_list;

```

### `crates/app/src/features/todos/components/todo_list.rs`

```rust
use crate::features::shell::state::{
    PlanArtifact, PlanProgressCounts, TODO_ROW_H, TodoEntry, TodoState,
};
use crate::shared::components::flat_list_row::flat_list_row;
use crate::shared::components::markdown_preview::markdown_preview;
use crate::tokens::{Tokens, element_key, icons};
use gpui::{AnyElement, FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

const TODO_CIRCLE_PX: f32 = 16.0;
const TODO_CHECK_ICON_PX: f32 = 10.0;

/// Compact sticky todo progress header (evolved todo strip).
pub fn todo_progress_header(
    items: &[TodoEntry],
    expanded: bool,
    on_toggle_strip: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    todo_strip(items, expanded, on_toggle_strip)
}

/// Compact sticky todo strip for the top of the chat column.
pub fn todo_strip(
    items: &[TodoEntry],
    expanded: bool,
    on_toggle_strip: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    let total = items.len();
    let (current_ix, current_label) = current_todo_focus(items);
    let toggle = on_toggle_strip;

    div()
        .id("todo-strip")
        .w_full()
        .flex_shrink_0()
        .px(Tokens::thread_padding_x())
        .pt(Tokens::spacing_2())
        .pb(Tokens::spacing_1())
        .child(
            div()
                .id("todo-strip-panel")
                .w_full()
                .max_w(px(Tokens::THREAD_MAX_WIDTH))
                .px(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .rounded(Tokens::radius_sm())
                .border_1()
                .border_color(Tokens::border_subtle())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(
                    div()
                        .id("todo-strip-summary")
                        .h(px(Tokens::ROW_HEIGHT_SM))
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .child(
                            Icon::new(icons::CHECKLIST)
                                .size(Tokens::text_sm())
                                .text_color(Tokens::text_tertiary()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(Tokens::text_sm())
                                .line_height(Tokens::text_sm_leading_compact())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_bright())
                                .child(format!("{current_ix}/{total} todos")),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(Tokens::text_sm())
                                .line_height(Tokens::text_sm_leading_compact())
                                .text_color(Tokens::text_faint())
                                .child("·"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(Tokens::text_sm())
                                .line_height(Tokens::text_sm_leading_compact())
                                .text_color(Tokens::text_secondary())
                                .child(current_label),
                        )
                        .child(
                            div()
                                .id("todo-strip-toggle")
                                .flex_shrink_0()
                                .size(px(Tokens::ROW_HEIGHT_SM))
                                .rounded(Tokens::radius_xs())
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(Tokens::surface_hover()))
                                .on_click(move |_, _, app: &mut gpui::App| toggle(app))
                                .child(
                                    Icon::new(if expanded {
                                        icons::CHEVRON_DOWN
                                    } else {
                                        icons::CHEVRON_RIGHT
                                    })
                                    .size(Tokens::text_sm())
                                    .text_color(Tokens::text_tertiary()),
                                ),
                        ),
                )
                .when(expanded, |el| {
                    el.children(
                        items
                            .iter()
                            .enumerate()
                            .map(|(index, item)| todo_row(item, index))
                            .collect::<Vec<_>>(),
                    )
                }),
        )
}

fn current_todo_focus(items: &[TodoEntry]) -> (usize, String) {
    let total = items.len();
    if total == 0 {
        return (0, "No tasks".into());
    }
    if let Some((ix, item)) = items
        .iter()
        .enumerate()
        .find(|(_, t)| matches!(t.state, TodoState::InProgress))
    {
        return (ix + 1, item.content.clone());
    }
    if let Some((ix, item)) = items
        .iter()
        .enumerate()
        .find(|(_, t)| matches!(t.state, TodoState::Pending))
    {
        return (ix + 1, item.content.clone());
    }
    let done = items
        .iter()
        .filter(|t| matches!(t.state, TodoState::Completed))
        .count();
    (
        done.min(total),
        items
            .last()
            .map(|t| t.content.clone())
            .unwrap_or_else(|| "All done".into()),
    )
}

/// Full live execution checklist.
#[allow(dead_code)]
pub fn todo_checklist(id: &str, items: &[TodoEntry]) -> impl IntoElement {
    let done = items
        .iter()
        .filter(|t| matches!(t.state, TodoState::Completed))
        .count();
    div()
        .id(element_key("todo-list", id))
        .w_full()
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_1())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_0p5())
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(Icon::new(icons::CHECKLIST).size(Tokens::text_sm()))
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::text_tertiary())
                        .child(format!("TODO · {done}/{} done", items.len())),
                ),
        )
        .children(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| todo_row(item, index))
                .collect::<Vec<_>>(),
        )
}

/// Backward-compatible render entry for thread todo rows.
#[allow(dead_code)]
pub fn todo_list(id: &str, items: &[TodoEntry]) -> impl IntoElement {
    todo_checklist(id, items)
}

pub fn plan_artifact(artifact: PlanArtifact) -> impl IntoElement {
    let counts = plan_counts_from_markdown(&artifact.markdown);
    div()
        .id("plan-artifact")
        .w_full()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_3())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .child(Icon::new(icons::CHECKLIST).size(Tokens::text_sm()))
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_tertiary())
                                .child(format!("PLAN · {}", artifact.execution_state.label())),
                        ),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_faint())
                        .child(artifact.created_at.clone()),
                ),
        )
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child(counts.summary()),
        )
        .when_some(artifact.source_conversation_id.as_ref(), |el, source| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .child(format!("Source conversation: {}", source.0)),
            )
        })
        .when_some(artifact.started_at.as_ref(), |el, started_at| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .child(match artifact.completed_at.as_ref() {
                        Some(completed_at) => {
                            format!("Started {started_at} · Completed {completed_at}")
                        }
                        None => format!("Started {started_at}"),
                    }),
            )
        })
        .child(markdown_preview(&artifact.markdown, true))
}

fn plan_counts_from_markdown(markdown: &str) -> PlanProgressCounts {
    let total = markdown
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("- [ ] ") || trimmed.starts_with("- ")
        })
        .count();
    PlanProgressCounts {
        pending: total,
        ..PlanProgressCounts::default()
    }
}

fn todo_row(item: &TodoEntry, _index: usize) -> AnyElement {
    let strike = matches!(item.state, TodoState::Completed | TodoState::Cancelled);
    let text_color = if strike {
        Tokens::text_faint()
    } else if matches!(item.state, TodoState::InProgress) {
        Tokens::text_primary()
    } else {
        Tokens::text_secondary()
    };
    flat_list_row(
        element_key("todo-row", &item.id),
        TODO_ROW_H,
        Tokens::spacing_1(),
        Tokens::spacing_1(),
        false,
        false,
        None::<fn(&mut gpui::App)>,
        div()
            .flex_1()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(Tokens::spacing_2())
            .child(todo_status_circle(item))
            .child(
                div()
                    .text_size(Tokens::text_sm())
                    .text_color(text_color)
                    .when(matches!(item.state, TodoState::InProgress), |el| {
                        el.font_weight(FontWeight::MEDIUM)
                    })
                    .when(strike, |el| el.line_through())
                    .child(item.content.clone()),
            ),
    )
    .into_any_element()
}

fn todo_status_circle(item: &TodoEntry) -> AnyElement {
    let size = px(TODO_CIRCLE_PX);
    match item.state {
        TodoState::Completed => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .bg(Tokens::success())
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(icons::CHECK)
                    .size(px(TODO_CHECK_ICON_PX))
                    .text_color(Tokens::main_bg()),
            )
            .into_any_element(),
        TodoState::InProgress => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .border_1()
            .border_color(Tokens::accent())
            .into_any_element(),
        TodoState::Cancelled => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .border_1()
            .border_color(Tokens::border_subtle())
            .opacity(0.55)
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(icons::X_MARK)
                    .size(px(TODO_CHECK_ICON_PX))
                    .text_color(Tokens::text_faint()),
            )
            .into_any_element(),
        TodoState::Pending => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .border_1()
            .border_color(Tokens::border_subtle())
            .into_any_element(),
    }
}

```

### `crates/app/src/features/todos/mod.rs`

```rust
pub mod components;

```

## Thread schemas and state

### `crates/app/src/features/shell/state.rs`

```rust
//! Pure data models for the Vortex agentic UI.
//!
//! Every struct / enum in this module is a plain data holder — no rendering
//! logic, no GPUI Entity, no window handles.  This makes them trivial to
//! snapshot, serialise, and unit-test independently of the view layer.

#![allow(dead_code)]

use std::collections::HashSet;
use std::path::PathBuf;

// ════════════════════════════════════════════════════════════
//  ID types (newtype wrappers for type-safety)
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConversationId(pub String);

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentId(pub usize);

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for AgentId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

// ── Backward-compat alias ──────────────────────────────────
pub type SessionId = ConversationId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppNavItem {
    Chat,
    Search,
    Extensions,
    Automations,
    Settings,
}

/// Drop indicator position while dragging a sidebar session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarDropTarget {
    BeforeSession(ConversationId),
    AppendToProject(ProjectId),
}

// ════════════════════════════════════════════════════════════
//  Project
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root_path: String,
    pub git_branch: String,
    pub trusted: bool,
    pub index_status: ProjectIndexStatus,
    pub conversations: Vec<ConversationId>,
}

impl Project {
    pub fn new(
        id: impl Into<String>,
        name: &str,
        root_path: &str,
        git_branch: &str,
        trusted: bool,
    ) -> Self {
        Self {
            id: ProjectId(id.into()),
            name: name.to_string(),
            root_path: root_path.to_string(),
            git_branch: git_branch.to_string(),
            trusted,
            index_status: ProjectIndexStatus::default(),
            conversations: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IndexPhase {
    #[default]
    Unindexed,
    Queued,
    Scanning,
    Parsing,
    Summarizing,
    Ready,
    Stale,
    Failed,
}

impl IndexPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unindexed => "Unindexed",
            Self::Queued => "Queued",
            Self::Scanning => "Scanning",
            Self::Parsing => "Parsing",
            Self::Summarizing => "Summarizing",
            Self::Ready => "Indexed",
            Self::Stale => "Stale",
            Self::Failed => "Failed",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Scanning | Self::Parsing | Self::Summarizing
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProjectIndexStats {
    pub files_indexed: usize,
    pub skipped_ignore: usize,
    pub skipped_hidden: usize,
    pub skipped_binary: usize,
    pub skipped_large: usize,
    pub skipped_policy: usize,
    pub symbols_indexed: usize,
    pub summaries_cached: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProjectIndexStatus {
    pub phase: IndexPhase,
    pub last_indexed_at: Option<String>,
    pub last_error: Option<String>,
    pub stale: bool,
    pub active_ignore_sources: Vec<String>,
    pub stats: ProjectIndexStats,
}

impl ProjectIndexStatus {
    pub fn badge_label(&self) -> &'static str {
        if self.phase.is_active() {
            "Indexing"
        } else {
            self.phase.label()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ContextTraceSummary {
    pub kind: ContextEntryKind,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ReadCacheRecap {
    pub entries: usize,
    pub bytes: usize,
    pub hits: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PageCacheRecap {
    pub configured: bool,
    pub cached_pages: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ContextInspectorRecap {
    pub project_status: Option<ProjectIndexStatus>,
    pub context_trace: Vec<ContextTraceSummary>,
    pub context_entries: Vec<ContextTraceEntry>,
    pub read_cache: ReadCacheRecap,
    pub page_cache: PageCacheRecap,
}

// ════════════════════════════════════════════════════════════
//  Conversation (formerly Session)
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
pub struct Conversation {
    pub id: ConversationId,
    pub project_id: ProjectId,
    pub title: String,
    pub updated_at: String,
    pub agents: Vec<AgentId>,
    pub active_agent_id: Option<AgentId>,
    pub thread_items: Vec<ThreadItem>,
    pub context_chips: Vec<ContextChip>,
    /// Live execution checklist mirrored from `todo_write` / `TodoUpdated`.
    pub active_todos: Vec<TodoEntry>,
    /// Reviewed Plan Mode artifact, separate from the live todo checklist.
    pub plan_artifact: Option<PlanArtifact>,
}

/// Show at most this many todos before "See more" when the list is collapsible.
pub const TODO_STRIP_PREVIEW_COUNT: usize = 5;
/// Collapse/expand when there are at least this many todos.
pub const TODO_STRIP_COLLAPSE_THRESHOLD: usize = 6;

/// Visible slice for the sticky todo strip — focuses on in-progress work, not the first rows.
pub fn todo_strip_visible_range(
    items: &[TodoEntry],
    expanded: bool,
    preview_count: usize,
) -> std::ops::Range<usize> {
    let total = items.len();
    if expanded || total <= preview_count {
        return 0..total;
    }
    let focus = items
        .iter()
        .position(|t| matches!(t.state, TodoState::InProgress))
        .or_else(|| {
            items
                .iter()
                .position(|t| matches!(t.state, TodoState::Pending))
        })
        .unwrap_or(total.saturating_sub(1));
    let mut start = focus.saturating_sub(1);
    let end = (start + preview_count).min(total);
    if end - start < preview_count {
        start = end.saturating_sub(preview_count);
    }
    start..end
}

impl Conversation {
    pub fn new(
        id: impl Into<String>,
        project_id: ProjectId,
        title: &str,
        updated_at: &str,
    ) -> Self {
        Self {
            id: ConversationId(id.into()),
            project_id,
            title: title.to_string(),
            updated_at: updated_at.to_string(),
            agents: Vec::new(),
            active_agent_id: None,
            thread_items: Vec::new(),
            context_chips: Vec::new(),
            active_todos: Vec::new(),
            plan_artifact: None,
        }
    }
}

// Backward-compat
pub type Session = Conversation;

/// Lightweight session row for the sidebar — excludes heavy `thread_items`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarSession {
    pub id: ConversationId,
    pub project_id: ProjectId,
    pub title: String,
    pub updated_at: String,
}

impl SidebarSession {
    pub fn from_conversation(conv: &Conversation) -> Self {
        Self {
            id: conv.id.clone(),
            project_id: conv.project_id.clone(),
            title: conv.title.clone(),
            updated_at: conv.updated_at.clone(),
        }
    }
}

// ════════════════════════════════════════════════════════════
//  Agent
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Thinking,
    RunningTool,
    WaitingApproval,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentMode {
    Local,
    Cloud,
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub id: AgentId,
    pub conversation_id: ConversationId,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub mode: AgentMode,
    pub status: AgentStatus,
    pub current_action: Option<String>,
    pub progress_label: Option<String>,
}

impl Agent {
    pub fn new(
        id: usize,
        conversation_id: ConversationId,
        name: &str,
        provider: &str,
        model: &str,
    ) -> Self {
        Self {
            id: AgentId(id),
            conversation_id,
            name: name.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            mode: AgentMode::Cloud,
            status: AgentStatus::Idle,
            current_action: None,
            progress_label: None,
        }
    }
}

// ════════════════════════════════════════════════════════════
//  Context chips
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
pub struct ContextChip {
    pub label: String,
    pub kind: ChipKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChipKind {
    Repo,
    File,
    Branch,
    Tool,
}

// ════════════════════════════════════════════════════════════
//  Thread items
// ════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalRisk {
    Low,
    Medium,
    High,
    Critical,
}

impl ApprovalRisk {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low risk",
            Self::Medium => "Medium risk",
            Self::High => "High risk",
            Self::Critical => "Critical risk",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffFileSummary {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub recommended: bool,
}

/// Category of a context-trace entry — drives collapsed counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ContextEntryKind {
    #[default]
    RepoMap,
    FileSlice,
    Symbol,
    Search,
    Command,
    Rule,
}

impl ContextEntryKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::RepoMap => "Repo maps",
            Self::FileSlice => "File slices",
            Self::Symbol => "Symbols",
            Self::Search => "Searches",
            Self::Command => "Commands",
            Self::Rule => "Rules",
        }
    }
}

/// One item the agent pulled into context, rendered in the "Context used" row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextTraceEntry {
    pub kind: ContextEntryKind,
    pub label: String,
    pub detail: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceMeta {
    pub summary: Option<String>,
    pub recommended_option_id: Option<String>,
    pub allow_custom: bool,
    pub blocking_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanArtifact {
    pub markdown: String,
    pub source_run_id: String,
    pub created_at: String,
    pub execution_state: PlanExecutionState,
    pub source_conversation_id: Option<ConversationId>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlanExecutionState {
    #[default]
    NotStarted,
    Starting,
    Implementing,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
    Stale,
}

impl PlanExecutionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "Not started",
            Self::Starting => "Starting",
            Self::Implementing => "Implementing",
            Self::WaitingApproval => "Waiting approval",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Stale => "Stale",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Implementing | Self::WaitingApproval
        )
    }

    pub fn can_start(self) -> bool {
        matches!(
            self,
            Self::NotStarted | Self::Stale | Self::Failed | Self::Cancelled
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PlanProgressCounts {
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub cancelled: usize,
}

impl PlanProgressCounts {
    pub fn total(self) -> usize {
        self.pending + self.in_progress + self.completed + self.cancelled
    }

    pub fn is_done(self) -> bool {
        self.total() > 0 && self.pending == 0 && self.in_progress == 0
    }

    pub fn summary(self) -> String {
        format!(
            "{} pending · {} active · {} done · {} cancelled",
            self.pending, self.in_progress, self.completed, self.cancelled
        )
    }
}

#[cfg(test)]
mod tests {
    use super::PlanExecutionState;

    #[test]
    fn plan_execution_state_helpers_classify_lifecycle() {
        assert!(PlanExecutionState::Starting.is_active());
        assert!(PlanExecutionState::Implementing.is_active());
        assert!(PlanExecutionState::WaitingApproval.is_active());
        assert!(!PlanExecutionState::Failed.is_active());

        assert!(PlanExecutionState::NotStarted.can_start());
        assert!(PlanExecutionState::Stale.can_start());
        assert!(PlanExecutionState::Failed.can_start());
        assert!(PlanExecutionState::Cancelled.can_start());
        assert!(!PlanExecutionState::Starting.can_start());
        assert!(!PlanExecutionState::Completed.can_start());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageAttachmentPreview {
    File(PathBuf),
    Bytes { mime_type: String, bytes: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageAttachment {
    pub label: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub preview: MessageAttachmentPreview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoState {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoEntry {
    pub id: String,
    pub content: String,
    pub state: TodoState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreadItem {
    UserMessage {
        id: String,
        text: String,
        attachments: Vec<MessageAttachment>,
        expanded: bool,
    },
    AssistantMessage {
        id: String,
        markdown: String,
        streaming: bool,
        depth: u8,
        parent_call_id: Option<String>,
    },
    SubagentRun {
        id: String,
        task: String,
        model: String,
        summary: String,
        expanded: bool,
        status: AgentStatus,
        child_run_id: String,
        parent_call_id: String,
    },
    ReasoningStep {
        id: String,
        title: String,
        summary: String,
        expanded: bool,
        status: AgentStatus,
        depth: u8,
        parent_call_id: Option<String>,
    },
    ToolCall {
        id: String,
        tool_name: String,
        command: Option<String>,
        output: Option<String>,
        expanded: bool,
        status: AgentStatus,
        depth: u8,
        parent_call_id: Option<String>,
    },
    DiffSummary {
        id: String,
        files_changed: usize,
        additions: usize,
        deletions: usize,
        files: Vec<DiffFileSummary>,
        expanded: bool,
        depth: u8,
        parent_call_id: Option<String>,
    },
    RunError {
        id: String,
        message: String,
        session_ref: Option<String>,
        retryable: bool,
    },
    ChoiceRequest {
        id: String,
        prompt: String,
        options: Vec<ChoiceOption>,
        meta: ChoiceMeta,
        selected: Option<String>,
        resolved: bool,
    },
    ApprovalRequest {
        id: String,
        title: String,
        risk: ApprovalRisk,
        resolved: bool,
    },
    TodoList {
        id: String,
        items: Vec<TodoEntry>,
    },
    PlanStatus {
        id: String,
        state: PlanExecutionState,
        summary: String,
        counts: PlanProgressCounts,
        source_conversation_id: Option<ConversationId>,
    },
    ContextTrace {
        id: String,
        entries: Vec<ContextTraceEntry>,
        expanded: bool,
    },
}

impl ThreadItem {
    pub fn id(&self) -> &str {
        match self {
            ThreadItem::UserMessage { id, .. }
            | ThreadItem::AssistantMessage { id, .. }
            | ThreadItem::SubagentRun { id, .. }
            | ThreadItem::ReasoningStep { id, .. }
            | ThreadItem::ToolCall { id, .. }
            | ThreadItem::DiffSummary { id, .. }
            | ThreadItem::RunError { id, .. }
            | ThreadItem::ChoiceRequest { id, .. }
            | ThreadItem::ApprovalRequest { id, .. }
            | ThreadItem::TodoList { id, .. }
            | ThreadItem::PlanStatus { id, .. }
            | ThreadItem::ContextTrace { id, .. } => id,
        }
    }

    pub fn can_expand(&self) -> bool {
        match self {
            ThreadItem::UserMessage { text, .. } => user_message_truncatable(text),
            ThreadItem::SubagentRun { .. }
            | ThreadItem::ReasoningStep { .. }
            | ThreadItem::ToolCall { .. }
            | ThreadItem::DiffSummary { .. } => true,
            ThreadItem::ContextTrace { entries, .. } => !entries.is_empty(),
            _ => false,
        }
    }

    pub fn is_expanded(&self) -> bool {
        match self {
            ThreadItem::UserMessage { expanded, .. }
            | ThreadItem::SubagentRun { expanded, .. }
            | ThreadItem::ReasoningStep { expanded, .. }
            | ThreadItem::ToolCall { expanded, .. }
            | ThreadItem::DiffSummary { expanded, .. }
            | ThreadItem::ContextTrace { expanded, .. } => *expanded,
            _ => true,
        }
    }

    /// Reasoning, tools, diff summaries, and context traces — the compact agent activity band.
    pub fn is_agent_activity(&self) -> bool {
        matches!(
            self,
            ThreadItem::ReasoningStep { .. }
                | ThreadItem::SubagentRun { .. }
                | ThreadItem::ToolCall { .. }
                | ThreadItem::DiffSummary { .. }
                | ThreadItem::PlanStatus { .. }
                | ThreadItem::ContextTrace { .. }
        )
    }
}

// ════════════════════════════════════════════════════════════
//  Diff panel (split view)
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffRowKind {
    HunkHeader,
    Collapsed,
    Context,
    Add,
    Remove,
}

/// Backward-compat alias.
pub type DiffLineKind = DiffRowKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffRow {
    Context {
        old_line: usize,
        new_line: usize,
        text: String,
    },
    Added {
        new_line: usize,
        text: String,
    },
    Removed {
        old_line: usize,
        text: String,
    },
    HunkHeader {
        label: String,
    },
    Collapsed {
        count: usize,
    },
}

/// Backward-compat flat line type used by legacy call sites.
#[derive(Clone, Debug)]
pub struct DiffLine {
    pub kind: DiffRowKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub content: String,
}

impl DiffRow {
    pub fn kind(&self) -> DiffRowKind {
        match self {
            DiffRow::Context { .. } => DiffRowKind::Context,
            DiffRow::Added { .. } => DiffRowKind::Add,
            DiffRow::Removed { .. } => DiffRowKind::Remove,
            DiffRow::HunkHeader { .. } => DiffRowKind::HunkHeader,
            DiffRow::Collapsed { .. } => DiffRowKind::Collapsed,
        }
    }

    pub fn to_legacy_line(&self) -> DiffLine {
        match self {
            DiffRow::Context {
                old_line,
                new_line,
                text,
            } => DiffLine {
                kind: DiffRowKind::Context,
                old_line: Some(*old_line),
                new_line: Some(*new_line),
                content: text.clone(),
            },
            DiffRow::Added { new_line, text } => DiffLine {
                kind: DiffRowKind::Add,
                old_line: None,
                new_line: Some(*new_line),
                content: text.clone(),
            },
            DiffRow::Removed { old_line, text } => DiffLine {
                kind: DiffRowKind::Remove,
                old_line: Some(*old_line),
                new_line: None,
                content: text.clone(),
            },
            DiffRow::HunkHeader { label } => DiffLine {
                kind: DiffRowKind::HunkHeader,
                old_line: None,
                new_line: None,
                content: label.clone(),
            },
            DiffRow::Collapsed { count } => DiffLine {
                kind: DiffRowKind::Collapsed,
                old_line: None,
                new_line: None,
                content: format!("{count} unchanged lines"),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: usize,
    pub new_start: usize,
    pub rows: Vec<DiffRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    pub added: usize,
    pub removed: usize,
    pub hunks: Vec<DiffHunk>,
    pub flat_rows: Vec<DiffRow>,
}

impl DiffFile {
    /// Flatten hunks into render rows (fixed-height uniform_list friendly).
    pub fn flat_rows(&self) -> &[DiffRow] {
        &self.flat_rows
    }

    /// Legacy accessor for flat line list.
    pub fn lines(&self) -> Vec<DiffLine> {
        self.flat_rows()
            .iter()
            .map(|r| r.to_legacy_line())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ReviewPanelTab {
    #[default]
    Changes,
    Plan,
}

#[derive(Clone, Debug)]
pub struct DiffPanelState {
    pub open: bool,
    /// User closed the panel; suppress auto-open on streaming preview updates.
    pub suppress_auto_open: bool,
    pub active_tab: ReviewPanelTab,
    pub selected_file: usize,
    pub files: Vec<DiffFile>,
    pub pending_approval: Option<PendingDiffApproval>,
    /// Patch waiting for explicit Apply in the diff panel.
    pub pending_patch_id: Option<String>,
    pub applied: bool,
}

#[derive(Clone, Debug)]
pub struct PendingDiffApproval {
    pub title: String,
    pub risk: ApprovalRisk,
}

#[derive(Clone, Debug)]
pub struct PendingThreadApproval {
    pub title: String,
    pub risk: ApprovalRisk,
    pub allow_always_label: Option<String>,
}

impl Default for DiffPanelState {
    fn default() -> Self {
        Self {
            open: false,
            suppress_auto_open: false,
            active_tab: ReviewPanelTab::Changes,
            selected_file: 0,
            files: Vec::new(),
            pending_approval: None,
            pending_patch_id: None,
            applied: false,
        }
    }
}

/// Max visible lines for the first user prompt before "See more".
pub const USER_MESSAGE_PREVIEW_LINES: usize = 5;

const USER_MESSAGE_CHARS_PER_LINE: usize = 55;

pub fn user_message_truncatable(text: &str) -> bool {
    text.lines().count() > USER_MESSAGE_PREVIEW_LINES
        || text.len() > USER_MESSAGE_CHARS_PER_LINE * USER_MESSAGE_PREVIEW_LINES
}

pub fn first_user_message_ix(items: &[ThreadItem]) -> Option<usize> {
    items
        .iter()
        .position(|item| matches!(item, ThreadItem::UserMessage { .. }))
}

// ════════════════════════════════════════════════════════════
//  Drawer state
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawerMode {
    Hidden,
    Changes,
    FileView,
    Browser,
    Terminal,
}

impl DrawerMode {
    pub fn label(&self) -> &'static str {
        match self {
            DrawerMode::Hidden => "",
            DrawerMode::Changes => "Changes",
            DrawerMode::FileView => "File",
            DrawerMode::Browser => "Browser",
            DrawerMode::Terminal => "Terminal",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DrawerState {
    pub mode: DrawerMode,
    pub width: f32,
}

impl Default for DrawerState {
    fn default() -> Self {
        Self {
            mode: DrawerMode::Hidden,
            width: 440.0,
        }
    }
}

// Backward-compat — prefer `crate::ui::state::InspectorMode` from `inspector.rs`.
pub type InspectorState = DrawerState;

// ════════════════════════════════════════════════════════════
//  Events (used for agentic streaming)
// ════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
pub enum AgentUiEvent {
    AgentStarted(Agent),
    TextDelta {
        item_id: String,
        delta: String,
    },
    ToolStarted {
        id: String,
        name: String,
        command: Option<String>,
    },
    ToolOutputDelta {
        id: String,
        delta: String,
    },
    ToolFinished {
        id: String,
        ok: bool,
    },
    ApprovalRequested {
        id: String,
        title: String,
        risk: ApprovalRisk,
    },
    AgentFinished {
        id: AgentId,
    },
}

// ════════════════════════════════════════════════════════════
//  Delta buffer — lightweight, stateless helper
// ════════════════════════════════════════════════════════════

/// Secondary coalescer window for streaming deltas that still go through the full
/// `update_thread_item` clone path (reasoning). The hot assistant path bypasses the
/// time gate entirely and is frame-coalesced inside `ThreadView`. Kept small so it
/// only absorbs sub-frame token floods rather than introducing perceptible latency.
pub const DELTA_BUFFER_FLUSH_MS: u64 = 16;

pub struct DeltaBuffer {
    pending_text: String,
    last_flush: std::time::Instant,
}

impl Default for DeltaBuffer {
    fn default() -> Self {
        Self {
            pending_text: String::new(),
            last_flush: std::time::Instant::now(),
        }
    }
}

impl DeltaBuffer {
    pub fn push(&mut self, delta: &str) {
        self.pending_text.push_str(delta);
    }

    pub fn should_flush(&self) -> bool {
        self.last_flush.elapsed() >= std::time::Duration::from_millis(DELTA_BUFFER_FLUSH_MS)
    }

    pub fn take(&mut self) -> String {
        self.last_flush = std::time::Instant::now();
        std::mem::take(&mut self.pending_text)
    }

    pub fn pending_text(&self) -> &str {
        &self.pending_text
    }
}

// ════════════════════════════════════════════════════════════
//  Expanded items set
// ════════════════════════════════════════════════════════════

pub type ExpandedItems = HashSet<String>;

// ════════════════════════════════════════════════════════════
//  Re-exports from sibling feature modules
//  (types moved during crate restructuring — re-exported here
//   so existing `use crate::features::shell::state::*`
//   imports continue to work)
// ════════════════════════════════════════════════════════════

pub use crate::features::agent_activity::state::{SessionStep, StepKind, StepStatus};
pub use crate::features::chat::manifest::{
    ActivityGroupPos, REASONING_OUTPUT_PREVIEW_LINES, TODO_ROW_H, TOOL_OUTPUT_PREVIEW_BYTES,
    TOOL_OUTPUT_PREVIEW_LINES,
};
pub use crate::features::chat::state::{
    TaskViewModel, build_task_view, project_timeline, should_emit_thread_item,
};
pub use crate::features::composer::state::{CommandRun, excerpt_output};
pub use crate::features::diff_panel::state::{ProviderErrorVm, SessionRunState};
pub use crate::features::inspector::artifact::{
    Artifact, ArtifactId, ArtifactKind, ArtifactSelection, ArtifactStore,
};
pub use crate::features::inspector::state::{
    DockPlacement, InspectorMode, InspectorTabId, InspectorTabKind, InspectorTabs, InspectorView,
};

```

### `crates/app/src/shared/state/transcript.rs`

```rust
//! Transcript display mode — controls timeline density.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TranscriptMode {
    /// Minimal journal: user turns, final summaries, errors, approvals.
    Summary,
    /// Default work journal: compact tool rows with expandable thinking summaries.
    #[default]
    Normal,
    /// Debug-friendly: raw tool I/O and full reasoning steps.
    Verbose,
}

impl TranscriptMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Normal => "Normal",
            Self::Verbose => "Verbose",
        }
    }

    pub fn shows_reasoning_rows(self) -> bool {
        matches!(self, Self::Normal | Self::Verbose)
    }

    pub fn shows_tool_output_rows(self) -> bool {
        matches!(self, Self::Verbose)
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Summary => Self::Normal,
            Self::Normal => Self::Verbose,
            Self::Verbose => Self::Summary,
        }
    }
}

```

### `crates/agent_protocol/src/event.rs`

```rust
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    AgentErrorView, AgentMode, AndroidActionTrace, AndroidActionVisualization, AndroidJourney,
    AndroidObservation, AndroidSessionState, ApprovalId, ModelId, OutputStreamKind, PatchId,
    RiskLevel, RunId, RunStatus, SessionId, ToolCallId, ToolName, ToolStatus,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    #[serde(default)]
    pub recommended: bool,
}

/// Category of a single context-trace entry, used for collapsed counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextEntryKind {
    /// A compact repo map / structural tree.
    RepoMap,
    /// A sliced file range opened into context.
    FileSlice,
    /// A symbol (function / struct / class) inspected.
    Symbol,
    /// A search / grep performed to gather context.
    Search,
    /// A command whose output was folded into context.
    Command,
    /// A rule file (e.g. `AGENTS.md`) applied to the run.
    Rule,
}

/// One item the agent pulled into context for a run, surfaced in the
/// "Context used" thread row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextTraceEntry {
    pub kind: ContextEntryKind,
    /// Primary label — e.g. a path or "Repo map".
    pub label: String,
    /// Optional detail — e.g. a line range like `52-118` or a symbol kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// One short line explaining why it was included.
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSectionEstimate {
    pub name: String,
    pub tokens: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentEvent {
    RunStarted {
        run_id: RunId,
        session_id: SessionId,
        model: ModelId,
        mode: AgentMode,
        #[serde(default)]
        depth: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_run_id: Option<RunId>,
    },
    ContextBuilt {
        run_id: RunId,
        token_estimate: usize,
        files: Vec<PathBuf>,
        summaries: Vec<String>,
        #[serde(default)]
        section_estimates: Vec<ContextSectionEstimate>,
    },
    AssistantTextDelta {
        run_id: RunId,
        text: String,
    },
    ReasoningDelta {
        run_id: RunId,
        text: String,
    },
    ToolCallStarted {
        run_id: RunId,
        call_id: ToolCallId,
        name: ToolName,
        args_preview: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dedupe_key: Option<String>,
        risk: RiskLevel,
    },
    ToolCallUpdated {
        run_id: RunId,
        call_id: ToolCallId,
        args_preview: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dedupe_key: Option<String>,
    },
    ApprovalRequested {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<RunId>,
        approval_id: ApprovalId,
        call_id: ToolCallId,
        risk: RiskLevel,
        reason: String,
        command_preview: Option<String>,
        affected_paths: Vec<PathBuf>,
    },
    ChoiceRequested {
        run_id: RunId,
        choice_id: String,
        prompt: String,
        options: Vec<crate::ChoiceOption>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recommended_option_id: Option<String>,
        #[serde(default)]
        allow_custom: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blocking_reason: Option<String>,
    },
    TodoUpdated {
        run_id: RunId,
        todos: Vec<crate::TodoItem>,
    },
    ContextTrace {
        run_id: RunId,
        entries: Vec<ContextTraceEntry>,
    },
    PlanUpdated {
        run_id: RunId,
        markdown: String,
        created_at: String,
    },
    AndroidSessionUpdated {
        run_id: RunId,
        session: AndroidSessionState,
    },
    AndroidObservationUpdated {
        run_id: RunId,
        observation: AndroidObservation,
    },
    AndroidActionPreviewed {
        run_id: RunId,
        action: AndroidActionVisualization,
    },
    AndroidActionCompleted {
        run_id: RunId,
        action: AndroidActionTrace,
    },
    AndroidJourneyUpdated {
        run_id: RunId,
        journey: AndroidJourney,
    },
    SubagentStarted {
        parent_run_id: RunId,
        child_run_id: RunId,
        call_id: ToolCallId,
        #[serde(default = "default_subagent_model")]
        model: ModelId,
        task: String,
    },
    SubagentFinished {
        parent_run_id: RunId,
        child_run_id: RunId,
        call_id: ToolCallId,
        status: RunStatus,
        summary: String,
    },
    ToolOutputDelta {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<RunId>,
        call_id: ToolCallId,
        stream: OutputStreamKind,
        chunk: String,
    },
    ToolCallFinished {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<RunId>,
        call_id: ToolCallId,
        status: ToolStatus,
        summary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<String>,
    },
    PatchPreviewUpdated {
        call_id: ToolCallId,
        unified_diff: String,
    },
    PatchProposed {
        patch_id: PatchId,
        files: Vec<PathBuf>,
        unified_diff: String,
        risk: RiskLevel,
    },
    PatchApplied {
        patch_id: PatchId,
        files: Vec<PathBuf>,
    },
    UsageUpdated {
        run_id: RunId,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: Option<u64>,
        cache_write_tokens: Option<u64>,
        estimated_cost_usd: Option<f64>,
    },
    RunFinished {
        run_id: RunId,
        status: RunStatus,
    },
    RunFailed {
        run_id: RunId,
        error: AgentErrorView,
    },
    /// User-initiated command failed (e.g. stale approval after reload).
    CommandFailed {
        message: String,
    },
}

fn default_subagent_model() -> ModelId {
    ModelId::new("parent model")
}

impl AgentEvent {
    pub fn run_id(&self) -> Option<&RunId> {
        match self {
            Self::RunStarted { run_id, .. }
            | Self::ContextBuilt { run_id, .. }
            | Self::AssistantTextDelta { run_id, .. }
            | Self::ReasoningDelta { run_id, .. }
            | Self::ToolCallStarted { run_id, .. }
            | Self::ToolCallUpdated { run_id, .. }
            | Self::ChoiceRequested { run_id, .. }
            | Self::TodoUpdated { run_id, .. }
            | Self::ContextTrace { run_id, .. }
            | Self::PlanUpdated { run_id, .. }
            | Self::AndroidSessionUpdated { run_id, .. }
            | Self::AndroidObservationUpdated { run_id, .. }
            | Self::AndroidActionPreviewed { run_id, .. }
            | Self::AndroidActionCompleted { run_id, .. }
            | Self::AndroidJourneyUpdated { run_id, .. }
            | Self::UsageUpdated { run_id, .. }
            | Self::RunFinished { run_id, .. }
            | Self::RunFailed { run_id, .. } => Some(run_id),
            Self::SubagentStarted { parent_run_id, .. }
            | Self::SubagentFinished { parent_run_id, .. } => Some(parent_run_id),
            Self::ApprovalRequested {
                run_id: Some(run_id),
                ..
            }
            | Self::ToolCallFinished {
                run_id: Some(run_id),
                ..
            }
            | Self::ToolOutputDelta {
                run_id: Some(run_id),
                ..
            } => Some(run_id),
            _ => None,
        }
    }
}

```

### `crates/agent_protocol/src/mode.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    ChatOnly,
    ReadOnlyInspect,
    PlanOnly,
    SuggestPatch,
    ApplyWithApproval,
    AutoSafe,
    FullAccessDangerous,
}

impl Default for AgentMode {
    fn default() -> Self {
        Self::ApplyWithApproval
    }
}

impl AgentMode {
    pub fn can_read_files(&self) -> bool {
        !matches!(self, Self::ChatOnly)
    }

    pub fn can_run_virtual_bash(&self) -> bool {
        matches!(
            self,
            Self::ReadOnlyInspect
                | Self::PlanOnly
                | Self::SuggestPatch
                | Self::ApplyWithApproval
                | Self::AutoSafe
                | Self::FullAccessDangerous
        )
    }

    pub fn can_propose_patches(&self) -> bool {
        matches!(
            self,
            Self::SuggestPatch
                | Self::ApplyWithApproval
                | Self::AutoSafe
                | Self::FullAccessDangerous
        )
    }

    pub fn can_apply_patches(&self) -> bool {
        matches!(
            self,
            Self::ApplyWithApproval | Self::AutoSafe | Self::FullAccessDangerous
        )
    }

    pub fn can_run_real_commands(&self) -> bool {
        matches!(
            self,
            Self::ApplyWithApproval | Self::AutoSafe | Self::FullAccessDangerous
        )
    }

    /// Agent mode applies proposed patches without pausing for explicit user approval.
    pub fn auto_applies_patches(&self) -> bool {
        matches!(self, Self::FullAccessDangerous)
    }
}

```

### `crates/agent_protocol/src/command.rs`

```rust
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    AgentMode, ApprovalId, CheckpointId, EventId, ModelId, PatchId, ProjectId, RunId, SessionId,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentCommand {
    StartRun {
        project_id: ProjectId,
        session_id: SessionId,
        prompt: String,
        model: ModelId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent_model: Option<ModelId>,
        mode: AgentMode,
        attachments: Vec<ContextAttachment>,
    },
    CancelRun {
        run_id: RunId,
    },
    ApproveTool {
        approval_id: ApprovalId,
    },
    ApproveToolAlways {
        approval_id: ApprovalId,
    },
    SubmitChoice {
        choice_id: String,
        option_id: String,
    },
    RejectTool {
        approval_id: ApprovalId,
        reason: Option<String>,
    },
    ApprovePatch {
        patch_id: PatchId,
        scope: crate::PatchApprovalScope,
    },
    RejectPatch {
        patch_id: PatchId,
        reason: Option<String>,
    },
    CompactSession {
        session_id: SessionId,
    },
    RetryFromEvent {
        session_id: SessionId,
        event_id: EventId,
    },
    SwitchModel {
        session_id: SessionId,
        model: ModelId,
    },
    RollbackCheckpoint {
        checkpoint_id: CheckpointId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextAttachment {
    pub source: AttachmentSource,
    pub kind: AttachmentKind,
    pub mime_type: Option<String>,
    pub display_name: Option<String>,
    pub size_bytes: Option<u64>,
}

impl ContextAttachment {
    pub fn file(path: PathBuf) -> Self {
        Self {
            source: AttachmentSource::Path(path),
            kind: AttachmentKind::File,
            mime_type: None,
            display_name: None,
            size_bytes: None,
        }
    }

    pub fn image_file(path: PathBuf, mime_type: impl Into<String>, size_bytes: u64) -> Self {
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string());
        Self {
            source: AttachmentSource::Path(path),
            kind: AttachmentKind::Image,
            mime_type: Some(mime_type.into()),
            display_name,
            size_bytes: Some(size_bytes),
        }
    }

    pub fn image_bytes(
        bytes: Vec<u8>,
        mime_type: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let size_bytes = bytes.len() as u64;
        Self {
            source: AttachmentSource::Bytes(bytes),
            kind: AttachmentKind::Image,
            mime_type: Some(mime_type.into()),
            display_name: Some(display_name.into()),
            size_bytes: Some(size_bytes),
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match &self.source {
            AttachmentSource::Path(path) => Some(path),
            AttachmentSource::Bytes(_) => None,
        }
    }

    pub fn display_label(&self) -> String {
        self.display_name
            .clone()
            .or_else(|| {
                self.path()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
            })
            .unwrap_or_else(|| "attachment".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AttachmentSource {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttachmentKind {
    File,
    Image,
    Selection,
    Diagnostic,
}

```

## Thread integration and reducer glue

### `crates/app/src/window/render.rs`

```rust
//! Render impl for AgentWindow.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnimationExt, Context, Entity, IntoElement, Render, SharedString, Window, div, prelude::*, px,
};
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};

use super::AgentWindow;
use super::AppScreen;
use super::conversation_workspace::ConversationWorkspaceVm;
use crate::features::chat::thread_view::{self as thread};
use crate::features::composer::layout::{self as composer, ComposerProps};

use crate::features::inspector::layout::{self as artifact_inspector, DockedInspectorProps};
use crate::features::settings::layout::{self as settings_ui, SettingsProps};
use crate::features::settings::state::SettingsSection;
use crate::features::shell::sidebar_helpers::{self as sidebar};
use crate::features::shell::state::{DockPlacement, InspectorTabId};
use crate::features::terminal::layout::TerminalPanelProps;
use crate::features::todos::components::todo_list::todo_progress_header;
use crate::features::top_bar::layout::{self as top_bar_ui, TopBarProps};
use crate::shared::components::context_usage_ring::ContextUsageProps;
use crate::shared::components::tab_bar::{DraggedTab, dock_tab_drag_placement};
use crate::shared::components::workspace_readiness::{
    WorkspaceReadinessCardProps, workspace_readiness_status,
};
use crate::shared::state::TranscriptMode;
use crate::tokens::Tokens;
use crate::tokens::motion::Motion;
use crate::tokens::theme::{current_mode, current_theme_name, themes_for_mode};
use agent_protocol::AgentMode;

// ── ViewState (cheap, cloneable) ──────────────────────────────────────────

struct ViewState {
    entity: Entity<AgentWindow>,
    screen: AppScreen,
    sidebar_collapsed: bool,
    dark_mode: bool,
    active_theme: String,
    theme_list: Vec<SharedString>,
    selected_provider: String,
    selected_model: String,
    selected_subagent_model: Option<String>,
    selected_settings_section: SettingsSection,
    model_items: Arc<[String]>,
    model_search_keys: Arc<[Arc<str>]>,
    sidebar_view: Entity<crate::features::shell::layout::SidebarView>,
    safety_mode: AgentMode,
    transcript_mode: TranscriptMode,
    conversation_workspace: ConversationWorkspaceVm,
}

// ── Cbs (cloneable callbacks) ─────────────────────────────────────────────

#[derive(Clone)]
struct Cbs {
    toggle_sidebar: Rc<dyn Fn(&mut gpui::App)>,
    toggle_inspector: Rc<dyn Fn(&mut gpui::App)>,
    toggle_terminal: Rc<dyn Fn(&mut gpui::App)>,
    send: Rc<dyn Fn(&mut Window, &mut gpui::App)>,
    cancel: Rc<dyn Fn(&mut gpui::App)>,
    select_inspector_tab: Rc<dyn Fn(InspectorTabId, &mut gpui::App)>,
    close_inspector_tab: Rc<dyn Fn(InspectorTabId, &mut gpui::App)>,
    reorder_inspector_tab: Rc<dyn Fn(InspectorTabId, InspectorTabId, &mut gpui::App)>,
    new_inspector_tab: Rc<dyn Fn(&mut gpui::App)>,
    transcript_mode: Rc<dyn Fn(TranscriptMode, &mut gpui::App)>,
    open_project: Rc<dyn Fn(&mut Window, &mut gpui::App)>,
    new_chat: Rc<dyn Fn(&mut gpui::App)>,
    open_chat: Rc<dyn Fn(&mut gpui::App)>,
    open_search: Rc<dyn Fn(&mut gpui::App)>,
    open_extensions: Rc<dyn Fn(&mut gpui::App)>,
    open_automations: Rc<dyn Fn(&mut gpui::App)>,
    open_settings: Rc<dyn Fn(&mut gpui::App)>,
    open_context: Rc<dyn Fn(&mut gpui::App)>,
    trust_project: Rc<dyn Fn(&mut gpui::App)>,
    toggle_todo_strip: Rc<dyn Fn(&mut gpui::App)>,
    new_terminal_tab: Rc<dyn Fn(&mut gpui::App)>,
    close_terminal_tab: Rc<dyn Fn(u64, &mut gpui::App)>,
    select_terminal_tab: Rc<dyn Fn(u64, &mut gpui::App)>,
    reorder_terminal_tab: Rc<dyn Fn(u64, u64, &mut gpui::App)>,
    move_inspector_tab_to_dock: Rc<dyn Fn(InspectorTabId, DockPlacement, &mut gpui::App)>,
    on_bottom_resize: Rc<dyn Fn(f32, &mut gpui::App)>,
    on_right_dock_resize: Rc<dyn Fn(f32, &mut gpui::App)>,
}

// ═══════════════════════════════════════════════════════════════════════════
//  RENDER
// ═══════════════════════════════════════════════════════════════════════════

impl Render for AgentWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::shared::render_profile::span("AgentWindow::render");
        self.prepare_render(window, cx);
        let cbs = self.build_cbs(cx);
        let vs = self.build_view_state(cx);
        root(self, vs, cbs)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  PREPARATION
// ═══════════════════════════════════════════════════════════════════════════

impl AgentWindow {
    fn prepare_render(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_thread_view(cx);
        self.ensure_sidebar_view(cx);
        self.sync_inspector_open();
        self.prepare_active_subagent_transcript(cx);
    }

    fn build_view_state(&mut self, cx: &mut Context<Self>) -> ViewState {
        let _profile = crate::shared::render_profile::span("build_view_state");
        let dark_mode = current_mode(cx).is_dark();
        let active_theme = current_theme_name(cx).to_string();
        let theme_list = themes_for_mode(cx, dark_mode);
        let (model_items, model_search_keys) = self.model_picker_items_for_selected_provider();

        ViewState {
            entity: cx.entity().clone(),
            screen: self.screen,
            sidebar_collapsed: self.sidebar_collapsed,
            dark_mode,
            active_theme,
            theme_list,
            selected_provider: self.selected_provider.clone(),
            selected_model: self.selected_model.clone(),
            selected_subagent_model: self.selected_subagent_model.clone(),
            selected_settings_section: self.selected_settings_section,
            model_items,
            model_search_keys,
            sidebar_view: self
                .sidebar_view
                .clone()
                .expect("sidebar view initialized in prepare_render"),
            safety_mode: self.safety_mode.clone(),
            transcript_mode: self.transcript_mode,
            conversation_workspace: self.build_conversation_workspace(cx),
        }
    }

    fn build_cbs(&self, cx: &mut Context<Self>) -> Cbs {
        let e = cx.entity().clone();
        Cbs {
            toggle_sidebar: rc1(e.clone(), |v, cx| v.toggle_sidebar(cx)),
            toggle_inspector: rc1(e.clone(), |v, cx| v.toggle_diff_panel(cx)),
            toggle_terminal: rc1(e.clone(), |v, cx| v.toggle_terminal_panel(cx)),
            send: Rc::new({
                let e = e.clone();
                move |window: &mut Window, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.send_message(window, cx))
                }
            }),
            cancel: rc1(e.clone(), |v, cx| v.cancel_active_run(cx)),
            select_inspector_tab: Rc::new({
                let e = e.clone();
                move |tab_id: InspectorTabId, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.select_inspector_tab(tab_id, cx))
                }
            }),
            close_inspector_tab: Rc::new({
                let e = e.clone();
                move |tab_id: InspectorTabId, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.close_inspector_tab(tab_id, cx))
                }
            }),
            reorder_inspector_tab: Rc::new({
                let e = e.clone();
                move |dragged_id: InspectorTabId, target_id: InspectorTabId, app: &mut gpui::App| {
                    e.update(app, |v, cx| {
                        v.reorder_inspector_tab(dragged_id, target_id, cx)
                    })
                }
            }),
            new_inspector_tab: rc1(e.clone(), |v, cx| v.new_inspector_tab(cx)),
            transcript_mode: Rc::new({
                let e = e.clone();
                move |mode: TranscriptMode, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.set_transcript_mode(mode, cx))
                }
            }),
            open_project: Rc::new({
                let e = e.clone();
                move |window: &mut Window, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.open_project_folder(window, cx))
                }
            }),
            new_chat: rc1(e.clone(), |v, cx| v.new_conversation_from_nav(cx)),
            open_chat: rc1(e.clone(), |v, cx| v.open_chat(cx)),
            open_search: rc1(e.clone(), |v, cx| v.open_search(cx)),
            open_extensions: rc1(e.clone(), |v, cx| v.open_extensions(cx)),
            open_automations: rc1(e.clone(), |v, cx| v.open_automations(cx)),
            open_settings: rc1(e.clone(), |v, cx| v.open_settings(cx)),
            open_context: rc1(e.clone(), |v, cx| v.open_context_workspace_panel(cx)),
            trust_project: rc1(e.clone(), |v, cx| v.trust_selected_project(cx)),
            toggle_todo_strip: rc1(e.clone(), |v, cx| v.toggle_todo_strip(cx)),
            new_terminal_tab: rc1(e.clone(), |v, cx| v.new_terminal_tab(cx)),
            close_terminal_tab: Rc::new({
                let e = e.clone();
                move |id: u64, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.close_terminal_tab(id, cx))
                }
            }),
            reorder_terminal_tab: Rc::new({
                let e = e.clone();
                move |dragged_id: u64, target_id: u64, app: &mut gpui::App| {
                    e.update(app, |v, cx| {
                        v.reorder_terminal_tab(dragged_id, target_id, cx)
                    })
                }
            }),
            move_inspector_tab_to_dock: Rc::new({
                let e = e.clone();
                move |tab_id: InspectorTabId, dock: DockPlacement, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.move_inspector_tab_to_dock(tab_id, dock, cx))
                }
            }),
            on_bottom_resize: Rc::new({
                let e = e.clone();
                move |h: f32, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.set_bottom_panel_height(h, cx))
                }
            }),
            on_right_dock_resize: Rc::new({
                let e = e.clone();
                move |w: f32, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.set_right_dock_width(w, cx))
                }
            }),
            select_terminal_tab: Rc::new({
                let e = e;
                move |id: u64, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.select_terminal_tab(id, cx))
                }
            }),
        }
    }
}

fn rc1<F>(e: Entity<AgentWindow>, f: F) -> Rc<dyn Fn(&mut gpui::App)>
where
    F: Fn(&mut AgentWindow, &mut gpui::Context<AgentWindow>) + 'static,
{
    Rc::new(move |app| e.update(app, |v, cx| f(v, cx)))
}

fn to_boxed(f: Rc<dyn Fn(&mut gpui::App)>) -> Box<dyn Fn(&mut gpui::App) + 'static> {
    Box::new(move |app| f(app))
}

// ═══════════════════════════════════════════════════════════════════════════
//  LAYOUT
// ═══════════════════════════════════════════════════════════════════════════

// Root layout
fn root(agent: &AgentWindow, vs: ViewState, cbs: Cbs) -> impl IntoElement {
    if vs.screen == AppScreen::Settings {
        return div()
            .id("agent-window")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(Tokens::app_bg())
            .text_color(Tokens::text_primary())
            .child(settings_column(&vs, &cbs))
            .into_any_element();
    }

    div()
        .id("agent-window")
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(Tokens::app_bg())
        .text_color(Tokens::text_primary())
        .child(top_bar(&vs, &cbs))
        .child(
            div()
                .id("agent-body")
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(body(agent, &vs, &cbs)),
        )
        .into_any_element()
}

// ── Top bar ───────────────────────────────────────────────────────────────

fn top_bar(vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let title = top_bar_title(vs);
    let title_copy = title.clone();
    top_bar_ui::render_top_bar(TopBarProps {
        conversation_title: title,
        sidebar_collapsed: vs.sidebar_collapsed,
        inspector_open: vs.conversation_workspace.inspector.open,
        terminal_panel_open: vs.conversation_workspace.terminal.open,
        show_panel_controls: vs.screen == AppScreen::Chat,
        workspace_readiness_label: (!vs.conversation_workspace.readiness.is_ready())
            .then(|| vs.conversation_workspace.readiness.summary_label()),
        on_toggle_sidebar: Some(to_boxed(cbs.toggle_sidebar.clone())),
        on_toggle_inspector: Some(to_boxed(cbs.toggle_inspector.clone())),
        on_toggle_terminal: Some(to_boxed(cbs.toggle_terminal.clone())),
        on_new_chat: Some(to_boxed(cbs.new_chat.clone())),
        on_title_copy: Some(Box::new({
            let title = title_copy;
            move |app: &mut gpui::App| {
                if !title.trim().is_empty() {
                    app.write_to_clipboard(title.clone().into());
                }
            }
        })),
        on_new_terminal_tab: Some(to_boxed(cbs.new_terminal_tab.clone())),
        on_workspace_status: Some(to_boxed(cbs.open_settings.clone())),
        on_open_settings: Some(cbs.open_settings.clone()),
    })
}

// ── Body (sidebar + main workspace + terminal) ────────────────────────────

fn body(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    h_resizable("agent-body-resizable")
        // ── Sidebar panel (left) ──
        .child(
            resizable_panel()
                .visible(!vs.sidebar_collapsed)
                .size(px(if vs.sidebar_collapsed {
                    0.0
                } else {
                    Tokens::SIDEBAR_WIDTH
                }))
                .size_range(if vs.sidebar_collapsed {
                    px(0.0)..px(0.0)
                } else {
                    px(Tokens::SIDEBAR_MIN_WIDTH)..px(Tokens::SIDEBAR_MAX_WIDTH)
                })
                .child(
                    div()
                        .size_full()
                        .min_w(px(0.0))
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .child(sidebar::render_sidebar(vs.sidebar_view.clone()))
                        .with_animation("sidebar-fade", Motion::fade_in(), |el, d| el.opacity(d)),
                ),
        )
        // ── Main workspace ──
        .child(
            resizable_panel()
                .size_range(px(Tokens::CENTER_MIN_WIDTH)..px(6000.0))
                .child(main_workspace(agent, vs, cbs)),
        )
}

// ── Main workspace (center panel + bottom dock) ───────────────────────────

fn main_workspace(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let on_bottom_resize = {
        let cb = cbs.on_bottom_resize.clone();
        move |state: &gpui::Entity<gpui_component::resizable::ResizableState>,
              _: &mut Window,
              cx: &mut gpui::App| {
            let sizes = state.read(cx).sizes();
            if sizes.len() >= 2 {
                cb(f32::from(sizes[1]), cx);
            }
        }
    };

    div()
        .id("main-workspace")
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .relative()
        .child(
            v_resizable("main-workspace-resizable")
                .on_resize(on_bottom_resize)
                // ── Center panel (top) ──
                .child(
                    resizable_panel()
                        .size_range(px(Tokens::CENTER_MIN_WIDTH)..px(6000.0))
                        .child(center_panel(agent, vs, cbs)),
                )
                // ── Terminal panel (bottom) ──
                .child(
                    resizable_panel()
                        .visible(
                            vs.screen == AppScreen::Chat && vs.conversation_workspace.terminal.open,
                        )
                        .size(px(vs.conversation_workspace.terminal.bottom_panel_height))
                        .size_range(
                            px(Tokens::BOTTOM_PANEL_MIN_HEIGHT)
                                ..px(Tokens::BOTTOM_PANEL_MAX_HEIGHT),
                        )
                        .child(
                            div()
                                .size_full()
                                .min_w(px(0.0))
                                .min_h(px(0.0))
                                .overflow_hidden()
                                .child(bottom_dock_content(agent, vs, cbs))
                                .with_animation("terminal-fade", Motion::fade_in(), |el, d| {
                                    el.opacity(d)
                                }),
                        ),
                ),
        )
        .when(
            vs.screen == AppScreen::Chat && !vs.conversation_workspace.terminal.open,
            |el| {
                let move_tab = cbs.move_inspector_tab_to_dock.clone();
                el.child(
                    div()
                        .id("bottom-dock-drop-zone")
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(72.0))
                        .drag_over::<DraggedTab>(move |style, drag, _, _| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Bottom)
                            {
                                style.bg(Tokens::accent().alpha(0.12))
                            } else {
                                style
                            }
                        })
                        .on_drop(move |drag: &DraggedTab, _, app: &mut gpui::App| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Bottom)
                            {
                                move_tab(drag.tab_id, DockPlacement::Bottom, app);
                            }
                        }),
                )
            },
        )
}

// ── Center panel (chat column + inspector) ────────────────────────────────

fn center_panel(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let on_right_resize = {
        let cb = cbs.on_right_dock_resize.clone();
        move |state: &gpui::Entity<gpui_component::resizable::ResizableState>,
              _: &mut Window,
              cx: &mut gpui::App| {
            let sizes = state.read(cx).sizes();
            if sizes.len() >= 2 {
                cb(f32::from(sizes[1]), cx);
            }
        }
    };

    div()
        .id("workspace-panels-row")
        .size_full()
        .min_h(px(0.0))
        .overflow_hidden()
        .relative()
        .child(
            h_resizable("workspace-row-resizable")
                .on_resize(on_right_resize)
                .child(
                    resizable_panel()
                        .size_range(px(Tokens::CENTER_MIN_WIDTH)..px(6000.0))
                        .child(column(agent, vs, cbs)),
                )
                .child(
                    resizable_panel()
                        .visible(
                            vs.screen == AppScreen::Chat
                                && vs.conversation_workspace.inspector.open,
                        )
                        .size(px(vs.conversation_workspace.inspector.right_dock_width))
                        .size_range(
                            px(Tokens::DIFF_PANEL_MIN_WIDTH)..px(Tokens::DIFF_PANEL_MAX_WIDTH),
                        )
                        .child(inspector_content(agent, vs, cbs)),
                ),
        )
        .when(
            vs.screen == AppScreen::Chat && !vs.conversation_workspace.inspector.open,
            |el| {
                let move_tab = cbs.move_inspector_tab_to_dock.clone();
                el.child(
                    div()
                        .id("right-dock-drop-zone")
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .w(px(72.0))
                        .drag_over::<DraggedTab>(move |style, drag, _, _| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Right)
                            {
                                style.bg(Tokens::accent().alpha(0.12))
                            } else {
                                style
                            }
                        })
                        .on_drop(move |drag: &DraggedTab, _, app: &mut gpui::App| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Right)
                            {
                                move_tab(drag.tab_id, DockPlacement::Right, app);
                            }
                        }),
                )
            },
        )
}

// ── Column ────────────────────────────────────────────────────────────────

fn column(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    match vs.screen {
        AppScreen::Chat => chat_column(agent, vs, cbs).into_any_element(),
        AppScreen::Search => placeholder_screen(
            "Search",
            "Project-wide search and search workflows will live here.",
            "Use the sidebar search box to filter threads, or open the Context/Search inspector for tool evidence today.",
            Some(("Back to threads", cbs.open_chat.clone())),
        )
        .into_any_element(),
        AppScreen::Extensions => placeholder_screen(
            "Extensions",
            "Manage connected tools and extension modules from one place.",
            "This pass adds the product surface and routing; backend management flows can land behind this screen next.",
            Some(("Back to threads", cbs.open_chat.clone())),
        )
        .into_any_element(),
        AppScreen::Automations => placeholder_screen(
            "Automations",
            "Scheduled tasks, monitors, and follow-ups will be managed here.",
            "The screen is now routable from the sidebar chrome and ready for automation-specific state.",
            Some(("Back to threads", cbs.open_chat.clone())),
        )
        .into_any_element(),
        AppScreen::Settings => settings_column(vs, cbs).into_any_element(),
    }
}

fn settings_column(vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    div()
        .id("settings-column")
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .bg(Tokens::main_bg())
        .child(settings_ui::render_settings(SettingsProps {
            dark_mode: vs.dark_mode,
            active_theme: vs.active_theme.clone(),
            themes: vs.theme_list.clone(),
            safety_mode: vs.safety_mode.clone(),
            transcript_mode: vs.transcript_mode,
            selected_section: vs.selected_settings_section,
            selected_provider: vs.selected_provider.clone(),
            selected_model: vs.selected_model.clone(),
            selected_subagent_model: vs.selected_subagent_model.clone(),
            model_items: vs.model_items.clone(),
            model_search_keys: vs.model_search_keys.clone(),
            workspace_readiness: vs.conversation_workspace.readiness.clone(),
            entity: vs.entity.clone(),
            on_transcript_mode: Some(Box::new({
                let cb = cbs.transcript_mode.clone();
                move |mode, app| cb(mode, app)
            })),
            on_open_project: Some(Box::new({
                let cb = cbs.open_project.clone();
                move |window, app| cb(window, app)
            })),
            on_trust_project: Some(to_boxed(cbs.trust_project.clone())),
            on_open_context: Some(to_boxed(cbs.open_context.clone())),
        }))
}

// ── Chat column ───────────────────────────────────────────────────────────

fn chat_column(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    div()
        .id("chat-column")
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .min_w(px(Tokens::CENTER_MIN_WIDTH))
        .min_h(px(0.0))
        .overflow_hidden()
        .bg(Tokens::main_bg())
        .when(!vs.conversation_workspace.readiness.is_ready(), |col| {
            col.child(render_workspace_readiness_banner(vs, cbs))
        })
        .when(!vs.conversation_workspace.active_todos.is_empty(), |col| {
            col.child(todo_progress_header(
                &vs.conversation_workspace.active_todos,
                vs.conversation_workspace.todo_strip_expanded,
                to_boxed(cbs.toggle_todo_strip.clone()),
            ))
        })
        .child(thread_layer(vs))
        .child(composer_layer(agent, vs, cbs))
}

// ── Thread layer ──────────────────────────────────────────────────────────

fn render_workspace_readiness_banner(vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let readiness = &vs.conversation_workspace.readiness;
    let show_trust = readiness.has_project && !readiness.project_trusted;
    let show_context = readiness.has_project;

    div()
        .px(Tokens::thread_padding_x())
        .pt(Tokens::spacing_3())
        .child(
            div()
                .max_w(px(Tokens::THREAD_MAX_WIDTH))
                .child(workspace_readiness_status(WorkspaceReadinessCardProps {
                    readiness: vs.conversation_workspace.readiness.clone(),
                    on_open_settings: Some(cbs.open_settings.clone()),
                    on_open_project: Some(cbs.open_project.clone()),
                    on_trust_project: if show_trust {
                        Some(cbs.trust_project.clone())
                    } else {
                        None
                    },
                    on_open_context: if show_context {
                        Some(cbs.open_context.clone())
                    } else {
                        None
                    },
                })),
        )
}

fn thread_layer(vs: &ViewState) -> impl IntoElement {
    div()
        .id("chat-thread-layer")
        .flex_1()
        .min_h(px(0.0))
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .child(thread::render_thread(
                    vs.conversation_workspace.thread_view.clone(),
                )),
        )
}

// ── Composer layer ────────────────────────────────────────────────────────

fn composer_layer(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let composer_vm = &vs.conversation_workspace.composer;
    let context_usage: ContextUsageProps = composer_vm.context_usage.to_props();
    let (pending_actions, sticky_approval) = agent.pending_composer_actions(vs.entity.clone());

    div()
        .id("chat-composer-layer")
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .child(composer::render_composer(ComposerProps {
            has_text: composer_vm.has_text,
            input_expanded: composer_vm.input_expanded,
            on_send: Some(Box::new({
                let cb = cbs.send.clone();
                move |window, app| cb(window, app)
            })),
            is_running: composer_vm.is_running,
            on_cancel: Some(Box::new({
                let cb = cbs.cancel.clone();
                move |app| cb(app)
            })),
            input_entity: composer_vm.input_entity.clone(),
            selected_mode: composer_vm.selected_mode.clone(),
            context_usage,
            pending_actions,
            sticky_approval,
            composer_dimmed: composer_vm.dimmed,
            composer_disabled: composer_vm.disabled,
            entity: vs.entity.clone(),
            selected_branch: composer_vm.selected_branch.clone(),
            branch_items: composer_vm.branch_items.clone(),
            selected_model: composer_vm.selected_model.clone(),
            model_items: composer_vm.model_items.clone(),
            model_search_keys: composer_vm.model_search_keys.clone(),
            pending_image_attachments: composer_vm.pending_image_attachments.clone(),
            composer_error: composer_vm.composer_error.clone(),
        }))
}

fn docked_inspector_props(
    vs: &ViewState,
    cbs: &Cbs,
    dock: DockPlacement,
    width: f32,
    show_left_border: bool,
    show_top_border: bool,
    show_close_button: bool,
) -> DockedInspectorProps {
    let inspector = &vs.conversation_workspace.inspector;
    let terminal = &vs.conversation_workspace.terminal;

    DockedInspectorProps {
        dock,
        width,
        tabs: inspector.tabs.clone(),
        store: inspector.store.clone(),
        selection: inspector.selection.clone(),
        selected_subagent: match dock {
            DockPlacement::Right => inspector.selected_subagent_right.clone(),
            DockPlacement::Bottom => inspector.selected_subagent_bottom.clone(),
        },
        context_recap: inspector.context_recap.clone(),
        on_select_tab: Some(cbs.select_inspector_tab.clone()),
        on_close_tab: Some(cbs.close_inspector_tab.clone()),
        on_new_tab: None,
        on_reorder_tab: Some(cbs.reorder_inspector_tab.clone()),
        on_move_tab_to_dock: Some(cbs.move_inspector_tab_to_dock.clone()),
        terminal_panel: Some(TerminalPanelProps {
            tabs: terminal.tabs.clone(),
            terminal_view: terminal.terminal_view.clone(),
            on_new_tab: Some(cbs.new_terminal_tab.clone()),
            on_close_tab: Some(cbs.close_terminal_tab.clone()),
            on_select_tab: Some(cbs.select_terminal_tab.clone()),
            on_reorder_tabs: Some(cbs.reorder_terminal_tab.clone()),
        }),
        android_session: inspector.android_session.clone(),
        diff_panel: {
            let mut props = inspector.diff_panel_props(vs.entity.clone(), show_close_button);
            props.patch_apply_enabled &= !vs.safety_mode.auto_applies_patches();
            props
        },
        show_left_border,
        show_top_border,
    }
}

// ── Inspector content ─────────────────────────────────────────────────────

fn inspector_content(_agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let _profile = crate::shared::render_profile::span("inspector_content");
    div()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .child(artifact_inspector::render_docked_inspector(
            docked_inspector_props(
                vs,
                cbs,
                DockPlacement::Right,
                vs.conversation_workspace.inspector.right_dock_width,
                true,
                false,
                true,
            ),
        ))
        .with_animation("inspector-fade", Motion::fade_in(), |el, d| el.opacity(d))
}

fn bottom_dock_content(_agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    artifact_inspector::render_docked_inspector(docked_inspector_props(
        vs,
        cbs,
        DockPlacement::Bottom,
        0.0,
        false,
        true,
        false,
    ))
}

fn top_bar_title(vs: &ViewState) -> String {
    match vs.screen {
        AppScreen::Chat => {
            if vs.conversation_workspace.title.is_empty() {
                "New thread".into()
            } else {
                vs.conversation_workspace.title.clone()
            }
        }
        AppScreen::Search => "Search".into(),
        AppScreen::Extensions => "Extensions".into(),
        AppScreen::Automations => "Automations".into(),
        AppScreen::Settings => "Settings".into(),
    }
}

fn placeholder_screen(
    title: &'static str,
    body: &'static str,
    detail: &'static str,
    action: Option<(&'static str, Rc<dyn Fn(&mut gpui::App)>)>,
) -> impl IntoElement {
    use crate::shared::components::buttons::btn_outline;

    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(Tokens::main_bg())
        .p(Tokens::spacing_6())
        .child(
            div()
                .max_w(px(560.0))
                .w_full()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_3())
                .child(
                    div()
                        .text_size(Tokens::text_xl())
                        .line_height(Tokens::text_xl_leading())
                        .text_color(Tokens::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_md())
                        .line_height(Tokens::text_md_leading())
                        .text_color(Tokens::text_secondary())
                        .child(body),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading())
                        .text_color(Tokens::text_tertiary())
                        .child(detail),
                )
                .when_some(action, |el, (label, cb)| {
                    el.child(
                        btn_outline("placeholder-action", label)
                            .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                    )
                }),
        )
}

```

### `crates/app/src/window/types.rs`

```rust
use std::sync::Arc;

use crate::features::chat::thread_view::ThreadView;
use crate::features::shell::state::{AgentStatus, SessionRunState, ThreadItem};
use crate::features::terminal::components::terminal_view::TerminalView;

#[derive(Clone)]
pub(crate) struct ModelPickerCache {
    pub(crate) provider: String,
    pub(crate) openrouter_revision: u64,
    pub(crate) items: Arc<[String]>,
    pub(crate) search_keys: Arc<[Arc<str>]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ConversationRunState {
    pub(crate) running: bool,
    pub(crate) waiting_approval: bool,
    pub(crate) pending_patch: bool,
    pub(crate) pending_choice: bool,
    pub(crate) blocked: bool,
    pub(crate) failed: bool,
}

impl ConversationRunState {
    pub(crate) fn from_thread_items(
        thread_items: &[ThreadItem],
        running: bool,
        pending_patch: bool,
        blocked: bool,
        failed: bool,
    ) -> Self {
        let mut state = Self {
            running,
            pending_patch,
            blocked,
            failed,
            ..Self::default()
        };
        for item in thread_items {
            match item {
                ThreadItem::ApprovalRequest {
                    resolved: false, ..
                }
                | ThreadItem::ToolCall {
                    status: AgentStatus::WaitingApproval,
                    ..
                } => state.waiting_approval = true,
                ThreadItem::ChoiceRequest {
                    resolved: false, ..
                } => state.pending_choice = true,
                _ => {}
            }
        }
        state
    }

    pub(crate) fn has_active_work(self) -> bool {
        self.running || self.waiting_approval || self.pending_patch || self.pending_choice
    }

    pub(crate) fn session_run_state(self, agent_status: Option<&AgentStatus>) -> SessionRunState {
        if self.blocked {
            return SessionRunState::BlockedError;
        }
        if self.waiting_approval || self.pending_patch {
            return SessionRunState::WaitingApproval;
        }
        if self.running {
            return match agent_status {
                Some(AgentStatus::Thinking) => SessionRunState::Planning,
                _ => SessionRunState::Running,
            };
        }
        if self.failed {
            return SessionRunState::Failed;
        }
        SessionRunState::from_agent_status(agent_status.unwrap_or(&AgentStatus::Idle), false)
    }

    pub(crate) fn composer_dimmed(self) -> bool {
        self.waiting_approval || self.blocked
    }

    pub(crate) fn composer_disabled(self) -> bool {
        self.blocked
    }
}

pub(crate) struct TerminalTab {
    pub(crate) id: u64,
    pub(crate) label: String,
    pub(crate) session: Arc<terminal::TerminalSession>,
    pub(crate) view: gpui::Entity<TerminalView>,
}

pub(crate) struct TerminalTabGroup {
    pub(crate) tabs: Vec<TerminalTab>,
    pub(crate) active_tab_id: u64,
    pub(crate) next_tab_id: u64,
}

impl TerminalTabGroup {
    pub(crate) fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: 0,
            next_tab_id: 1,
        }
    }

    pub(crate) fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.iter().find(|tab| tab.id == self.active_tab_id)
    }
}

pub(crate) struct SubagentTranscript {
    pub(crate) item_id: String,
    pub(crate) parent_call_id: String,
    pub(crate) task: String,
    pub(crate) model: String,
    pub(crate) summary: String,
    pub(crate) status: AgentStatus,
    pub(crate) assistant_count: usize,
    pub(crate) reasoning_count: usize,
    pub(crate) tool_count: usize,
    pub(crate) diff_count: usize,
    pub(crate) last_event_label: Option<String>,
    pub(crate) item_ids: Vec<String>,
    pub(crate) view: Option<gpui::Entity<ThreadView>>,
}

impl SubagentTranscript {
    pub(crate) fn status_label(&self) -> &'static str {
        match self.status {
            AgentStatus::Thinking | AgentStatus::RunningTool => "Running",
            AgentStatus::WaitingApproval => "Waiting approval",
            AgentStatus::Completed => "Completed",
            AgentStatus::Failed => "Failed",
            AgentStatus::Idle => "Idle",
        }
    }

    pub(crate) fn activity_summary(&self) -> String {
        let mut parts = Vec::new();
        if self.tool_count > 0 {
            parts.push(format!(
                "{} tool{}",
                self.tool_count,
                if self.tool_count == 1 { "" } else { "s" }
            ));
        }
        if self.diff_count > 0 {
            parts.push(format!(
                "{} diff{}",
                self.diff_count,
                if self.diff_count == 1 { "" } else { "s" }
            ));
        }
        if self.reasoning_count > 0 {
            parts.push(format!(
                "{} reasoning step{}",
                self.reasoning_count,
                if self.reasoning_count == 1 { "" } else { "s" }
            ));
        }
        if self.assistant_count > 0 {
            parts.push(format!(
                "{} answer{}",
                self.assistant_count,
                if self.assistant_count == 1 { "" } else { "s" }
            ));
        }
        if parts.is_empty() {
            "No child activity yet".to_string()
        } else {
            parts.join(", ")
        }
    }
}

```

### `crates/app/src/window/bridge/thread_bridge.rs`

```rust
//! Thread bridge — sync `Conversation.thread_items` with `ThreadView`.

use gpui::Context;

use super::super::AgentWindow;
use crate::features::chat::thread_view::AssistantActionProjection;
use crate::features::shell::state::{ConversationId, ThreadItem};

impl AgentWindow {
    pub fn push_thread_item(
        &mut self,
        conversation_id: ConversationId,
        item: ThreadItem,
        cx: &mut Context<Self>,
    ) {
        let selected = self.selected_conversation_id.as_ref() == Some(&conversation_id);
        if let Some(conv) = self
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id)
        {
            let ix = conv.thread_items.len();
            let item_id = item.id().to_string();
            conv.thread_items.push(item.clone());
            self.register_thread_item_index(&conversation_id, &item_id, ix);
        }
        self.register_subagent_item(&item, cx);
        if selected {
            self.ensure_thread_view(cx);
            if let Some(thread) = self.thread_view.clone() {
                thread.update(cx, |view, cx| view.push_item(item, cx));
            }
        }
    }

    fn register_thread_item_index(
        &mut self,
        conversation_id: &ConversationId,
        item_id: &str,
        index: usize,
    ) {
        self.thread_item_indices
            .entry(conversation_id.clone())
            .or_default()
            .insert(item_id.to_string(), index);
    }

    pub(crate) fn rebuild_thread_item_index(&mut self, conversation_id: &ConversationId) {
        let Some(conv) = self.conversations.iter().find(|c| c.id == *conversation_id) else {
            self.thread_item_indices.remove(conversation_id);
            return;
        };
        let index = conv
            .thread_items
            .iter()
            .enumerate()
            .map(|(ix, item)| (item.id().to_string(), ix))
            .collect();
        self.thread_item_indices
            .insert(conversation_id.clone(), index);
    }

    fn thread_item_index_for(
        &mut self,
        conversation_id: &ConversationId,
        item_id: &str,
    ) -> Option<usize> {
        if let Some(ix) = self
            .thread_item_indices
            .get(conversation_id)
            .and_then(|m| m.get(item_id).copied())
        {
            return Some(ix);
        }
        self.rebuild_thread_item_index(conversation_id);
        self.thread_item_indices
            .get(conversation_id)
            .and_then(|m| m.get(item_id).copied())
    }

    pub fn append_assistant_delta(
        &mut self,
        conversation_id: ConversationId,
        item_id: &str,
        chunk: &str,
        cx: &mut Context<Self>,
    ) {
        if chunk.is_empty() {
            return;
        }
        let selected = self.selected_conversation_id.as_ref() == Some(&conversation_id);
        if let Some(item_ix) = self.thread_item_index_for(&conversation_id, item_id) {
            if let Some(conv) = self
                .conversations
                .iter_mut()
                .find(|c| c.id == conversation_id)
            {
                if let Some(item) = conv.thread_items.get_mut(item_ix) {
                    if let ThreadItem::AssistantMessage { markdown, .. } = item {
                        markdown.push_str(chunk);
                    }
                }
            }
        }
        if selected {
            self.ensure_thread_view(cx);
            if let Some(thread) = self.thread_view.clone() {
                thread.update(cx, |view, cx| {
                    view.append_assistant_delta(item_id, chunk, cx)
                });
            }
        }
        self.append_subagent_assistant_delta(item_id, chunk, cx);
    }

    pub fn append_reasoning_delta(
        &mut self,
        conversation_id: ConversationId,
        item_id: &str,
        chunk: &str,
        cx: &mut Context<Self>,
    ) {
        if chunk.is_empty() {
            return;
        }
        let selected = self.selected_conversation_id.as_ref() == Some(&conversation_id);
        if let Some(item_ix) = self.thread_item_index_for(&conversation_id, item_id) {
            if let Some(conv) = self
                .conversations
                .iter_mut()
                .find(|c| c.id == conversation_id)
            {
                if let Some(ThreadItem::ReasoningStep { summary, .. }) =
                    conv.thread_items.get_mut(item_ix)
                {
                    summary.push_str(chunk);
                }
            }
        }
        if selected {
            self.ensure_thread_view(cx);
            if let Some(thread) = self.thread_view.clone() {
                thread.update(cx, |view, cx| {
                    view.append_reasoning_delta(item_id, chunk, cx)
                });
            }
        }
        self.append_subagent_reasoning_delta(item_id, chunk, cx);
    }

    pub fn append_tool_output_delta(
        &mut self,
        conversation_id: ConversationId,
        item_id: &str,
        prefix: &str,
        chunk: &str,
        cx: &mut Context<Self>,
    ) {
        if prefix.is_empty() && chunk.is_empty() {
            return;
        }
        let selected = self.selected_conversation_id.as_ref() == Some(&conversation_id);
        if let Some(item_ix) = self.thread_item_index_for(&conversation_id, item_id) {
            if let Some(conv) = self
                .conversations
                .iter_mut()
                .find(|c| c.id == conversation_id)
            {
                if let Some(ThreadItem::ToolCall { output, .. }) =
                    conv.thread_items.get_mut(item_ix)
                {
                    let output = output.get_or_insert_with(String::new);
                    output.push_str(prefix);
                    output.push_str(chunk);
                }
            }
        }
        if selected {
            self.ensure_thread_view(cx);
            if let Some(thread) = self.thread_view.clone() {
                thread.update(cx, |view, cx| {
                    view.append_tool_output_delta(item_id, prefix, chunk, cx)
                });
            }
        }
        self.append_subagent_tool_output_delta(item_id, prefix, chunk, cx);
    }

    pub fn update_thread_item(
        &mut self,
        conversation_id: ConversationId,
        item_id: &str,
        mutator: impl FnOnce(&mut ThreadItem),
        cx: &mut Context<Self>,
    ) {
        let selected = self.selected_conversation_id.as_ref() == Some(&conversation_id);
        let Some(item_ix) = self.thread_item_index_for(&conversation_id, item_id) else {
            return;
        };
        let Some(conv) = self
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id)
        else {
            return;
        };

        let Some(item) = conv.thread_items.get_mut(item_ix) else {
            self.rebuild_thread_item_index(&conversation_id);
            return;
        };
        if item.id() != item_id {
            self.rebuild_thread_item_index(&conversation_id);
            return;
        }

        mutator(item);
        let updated_item = item.clone();
        self.update_subagent_item(&updated_item, cx);
        if selected {
            if let Some(thread) = self.thread_view.clone() {
                thread.update(cx, |view, cx| view.update_item(updated_item, cx));
            }
        }
    }

    pub(crate) fn remove_thread_item(
        &mut self,
        conversation_id: ConversationId,
        item_id: &str,
        cx: &mut Context<Self>,
    ) {
        let selected = self.selected_conversation_id.as_ref() == Some(&conversation_id);
        if let Some(conv) = self
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id)
        {
            conv.thread_items.retain(|item| item.id() != item_id);
            self.rebuild_thread_item_index(&conversation_id);
        }
        self.rebuild_subagent_projections(&conversation_id);
        if selected {
            let items = self.thread_items_for(&conversation_id);
            let run_active = self.thread_run_active(&conversation_id);
            if let Some(thread) = self.thread_view.clone() {
                thread.update(cx, |view, cx| {
                    view.sync(conversation_id, items, run_active, cx)
                });
            }
        }
    }

    pub(crate) fn sync_thread_view(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) {
        self.sync_thread_view_impl(conversation_id, cx, false);
    }

    fn sync_thread_view_impl(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
        immediate: bool,
    ) {
        let _profile = crate::shared::render_profile::span("AgentWindow::sync_thread_view_impl");
        self.refresh_task_projection(&conversation_id);
        self.ensure_thread_view(cx);
        let items = self.thread_items_for(&conversation_id);
        let run_active = self.thread_run_active(&conversation_id);
        let transcript_mode = self.transcript_mode;
        let assistant_actions = self.assistant_action_projection();
        let Some(thread) = self.thread_view.clone() else {
            return;
        };
        if immediate {
            thread.update(cx, |view, cx| {
                view.set_assistant_actions(assistant_actions, cx);
                view.set_transcript_mode(transcript_mode);
                view.sync_live(conversation_id, items, run_active, cx)
            });
        } else {
            thread.update(cx, |view, cx| {
                view.set_assistant_actions(assistant_actions, cx);
                view.set_transcript_mode(transcript_mode);
                view.sync(conversation_id, items, run_active, cx)
            });
        }
        if self.request_thread_scroll_to_bottom {
            thread.update(cx, |view, _| view.scroll_to_bottom());
            self.request_thread_scroll_to_bottom = false;
        }
    }

    pub(crate) fn sync_thread_approval_state(&mut self, cx: &mut Context<Self>) {
        let active = self.pending_approval_id.is_some();
        let assistant_actions = self.assistant_action_projection();
        if let Some(thread) = self.thread_view.clone() {
            thread.update(cx, |view, cx| {
                view.set_assistant_actions(assistant_actions, cx);
                view.set_approval_active(active, cx);
            });
        }
    }

    pub(crate) fn assistant_action_projection(&self) -> AssistantActionProjection {
        let can_approve = self.pending_approval_id.is_some();
        AssistantActionProjection {
            can_retry: self.can_retry_last_user_turn(),
            can_open_diff: can_approve
                || self.diff_panel.pending_patch_id.is_some()
                || !self.diff_panel.files.is_empty(),
            can_approve,
        }
    }

    pub fn toggle_thread_item(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let Some(conv_id) = self.selected_conversation_id.clone() else {
            return;
        };

        let mut refreshed_item = None;
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
            for item in &mut conv.thread_items {
                if item.id() != item_id || !item.can_expand() {
                    continue;
                }
                match item {
                    ThreadItem::UserMessage { expanded, .. }
                    | ThreadItem::SubagentRun { expanded, .. }
                    | ThreadItem::ReasoningStep { expanded, .. }
                    | ThreadItem::ToolCall { expanded, .. }
                    | ThreadItem::DiffSummary { expanded, .. }
                    | ThreadItem::ContextTrace { expanded, .. } => {
                        *expanded = !*expanded;
                        refreshed_item = Some(item.clone());
                    }
                    _ => {}
                }
                break;
            }
        }

        if let Some(item) = refreshed_item.clone() {
            self.update_subagent_item(&item, cx);
        }
        if let Some(thread) = self.thread_view.clone() {
            let item = refreshed_item;
            thread.update(cx, |view, cx| {
                if let Some(item) = item {
                    view.cancel_pending_item(item_id);
                    view.refresh_item(item, cx);
                }
            });
        }
    }
}

```

### `crates/app/src/window/bridge/subagent_bridge.rs`

```rust
//! Subagent transcript projection — keeps inspector tabs off the full thread render path.

use gpui::{AppContext, Context};

use super::super::{AgentWindow, SubagentTranscript};
use crate::features::chat::thread_view::ThreadView;
use crate::features::shell::state::{ConversationId, ThreadItem};

impl AgentWindow {
    pub(crate) fn rebuild_subagent_projections(&mut self, conversation_id: &ConversationId) {
        self.subagent_transcripts.clear();
        self.subagent_by_parent_call.clear();

        let Some(conv) = self.conversations.iter().find(|c| c.id == *conversation_id) else {
            return;
        };

        for item in &conv.thread_items {
            if let ThreadItem::SubagentRun {
                id,
                task,
                model,
                summary,
                status,
                parent_call_id,
                ..
            } = item
            {
                self.subagent_by_parent_call
                    .insert(parent_call_id.clone(), id.clone());
                self.subagent_transcripts.insert(
                    id.clone(),
                    SubagentTranscript {
                        item_id: id.clone(),
                        parent_call_id: parent_call_id.clone(),
                        task: task.clone(),
                        model: model.clone(),
                        summary: summary.clone(),
                        status: status.clone(),
                        assistant_count: 0,
                        reasoning_count: 0,
                        tool_count: 0,
                        diff_count: 0,
                        last_event_label: None,
                        item_ids: Vec::new(),
                        view: None,
                    },
                );
            } else if let Some(parent_call_id) = subagent_parent_call_id(item) {
                if let Some(subagent_id) = self.subagent_by_parent_call.get(parent_call_id) {
                    if let Some(projection) = self.subagent_transcripts.get_mut(subagent_id) {
                        projection.item_ids.push(item.id().to_string());
                    }
                }
            }
        }

        self.refresh_all_subagent_projection_metadata(conversation_id);
    }

    pub(crate) fn ensure_subagent_transcript_view(
        &mut self,
        item_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<ThreadView>> {
        if !self.subagent_transcripts.contains_key(item_id) {
            let conv_id = self.selected_conversation_id.clone()?;
            self.rebuild_subagent_projections(&conv_id);
        }

        if self
            .subagent_transcripts
            .get(item_id)
            .and_then(|projection| projection.view.clone())
            .is_none()
        {
            let items = self.subagent_items_for(item_id);
            let agent = cx.entity();
            let view = cx.new(|cx| ThreadView::new_embedded(agent, items, cx));
            if let Some(projection) = self.subagent_transcripts.get_mut(item_id) {
                projection.view = Some(view);
            }
        }

        self.subagent_transcripts
            .get(item_id)
            .and_then(|projection| projection.view.clone())
    }

    pub(crate) fn subagent_items_for(&self, item_id: &str) -> Vec<ThreadItem> {
        let Some(conv_id) = &self.selected_conversation_id else {
            return Vec::new();
        };
        let Some(conv) = self.conversations.iter().find(|c| c.id == *conv_id) else {
            return Vec::new();
        };
        let Some(projection) = self.subagent_transcripts.get(item_id) else {
            return Vec::new();
        };

        projection
            .item_ids
            .iter()
            .filter_map(|child_id| {
                conv.thread_items
                    .iter()
                    .find(|item| item.id() == child_id)
                    .cloned()
            })
            .collect()
    }

    pub(crate) fn register_subagent_item(&mut self, item: &ThreadItem, cx: &mut Context<Self>) {
        match item {
            ThreadItem::SubagentRun {
                id,
                task,
                model,
                summary,
                status,
                parent_call_id,
                ..
            } => {
                self.subagent_by_parent_call
                    .insert(parent_call_id.clone(), id.clone());
                self.subagent_transcripts
                    .entry(id.clone())
                    .and_modify(|projection| {
                        projection.task = task.clone();
                        projection.model = model.clone();
                        projection.summary = summary.clone();
                        projection.status = status.clone();
                        projection.parent_call_id = parent_call_id.clone();
                    })
                    .or_insert_with(|| SubagentTranscript {
                        item_id: id.clone(),
                        parent_call_id: parent_call_id.clone(),
                        task: task.clone(),
                        model: model.clone(),
                        summary: summary.clone(),
                        status: status.clone(),
                        assistant_count: 0,
                        reasoning_count: 0,
                        tool_count: 0,
                        diff_count: 0,
                        last_event_label: None,
                        item_ids: Vec::new(),
                        view: None,
                    });
            }
            _ => {
                let Some(parent_call_id) = subagent_parent_call_id(item) else {
                    return;
                };
                let Some(subagent_id) = self.subagent_by_parent_call.get(parent_call_id).cloned()
                else {
                    return;
                };
                let Some(projection) = self.subagent_transcripts.get_mut(&subagent_id) else {
                    return;
                };
                if !projection.item_ids.iter().any(|id| id == item.id()) {
                    projection.item_ids.push(item.id().to_string());
                }
                accumulate_subagent_metrics(projection, item);
                if let Some(view) = projection.view.clone() {
                    let item = item.clone();
                    view.update(cx, |view, cx| view.push_item(item, cx));
                }
            }
        }
    }

    pub(crate) fn update_subagent_item(&mut self, item: &ThreadItem, cx: &mut Context<Self>) {
        match item {
            ThreadItem::SubagentRun {
                id,
                task,
                model,
                summary,
                status,
                ..
            } => {
                if let Some(projection) = self.subagent_transcripts.get_mut(id) {
                    projection.task = task.clone();
                    projection.model = model.clone();
                    projection.summary = summary.clone();
                    projection.status = status.clone();
                }
                cx.notify();
            }
            _ => {
                let Some(parent_call_id) = subagent_parent_call_id(item) else {
                    return;
                };
                let Some(subagent_id) = self.subagent_by_parent_call.get(parent_call_id).cloned()
                else {
                    return;
                };
                if let Some(conv_id) = self.selected_conversation_id.clone() {
                    self.refresh_subagent_projection_for_item(&subagent_id, &conv_id);
                }
                let Some(view) = self
                    .subagent_transcripts
                    .get(&subagent_id)
                    .and_then(|projection| projection.view.clone())
                else {
                    return;
                };
                let item = item.clone();
                view.update(cx, |view, cx| view.update_item(item, cx));
            }
        }
    }

    pub(crate) fn append_subagent_assistant_delta(
        &mut self,
        item_id: &str,
        chunk: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(subagent_id) = self.subagent_id_for_child_item(item_id) else {
            return;
        };
        self.touch_subagent_last_event(&subagent_id, "Assistant reply updated");
        let Some(view) = self
            .subagent_transcripts
            .get(&subagent_id)
            .and_then(|projection| projection.view.clone())
        else {
            return;
        };
        view.update(cx, |view, cx| {
            view.append_assistant_delta(item_id, chunk, cx)
        });
    }

    pub(crate) fn append_subagent_reasoning_delta(
        &mut self,
        item_id: &str,
        chunk: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(subagent_id) = self.subagent_id_for_child_item(item_id) else {
            return;
        };
        self.touch_subagent_last_event(&subagent_id, "Reasoning updated");
        let Some(view) = self
            .subagent_transcripts
            .get(&subagent_id)
            .and_then(|projection| projection.view.clone())
        else {
            return;
        };
        view.update(cx, |view, cx| {
            view.append_reasoning_delta(item_id, chunk, cx)
        });
    }

    pub(crate) fn append_subagent_tool_output_delta(
        &mut self,
        item_id: &str,
        prefix: &str,
        chunk: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(subagent_id) = self.subagent_id_for_child_item(item_id) else {
            return;
        };
        self.touch_subagent_last_event(&subagent_id, "Tool output updated");
        let Some(view) = self
            .subagent_transcripts
            .get(&subagent_id)
            .and_then(|projection| projection.view.clone())
        else {
            return;
        };
        view.update(cx, |view, cx| {
            view.append_tool_output_delta(item_id, prefix, chunk, cx)
        });
    }

    fn subagent_id_for_child_item(&self, item_id: &str) -> Option<String> {
        self.subagent_transcripts
            .iter()
            .find(|(_, projection)| projection.item_ids.iter().any(|id| id == item_id))
            .map(|(subagent_id, _)| subagent_id.clone())
    }

    fn refresh_all_subagent_projection_metadata(&mut self, conversation_id: &ConversationId) {
        let ids: Vec<String> = self.subagent_transcripts.keys().cloned().collect();
        for id in ids {
            self.refresh_subagent_projection_for_item(&id, conversation_id);
        }
    }

    fn refresh_subagent_projection_for_item(
        &mut self,
        subagent_id: &str,
        conversation_id: &ConversationId,
    ) {
        let Some(conv) = self.conversations.iter().find(|c| c.id == *conversation_id) else {
            return;
        };
        let Some(projection) = self.subagent_transcripts.get_mut(subagent_id) else {
            return;
        };
        projection.assistant_count = 0;
        projection.reasoning_count = 0;
        projection.tool_count = 0;
        projection.diff_count = 0;
        projection.last_event_label = None;
        for child_id in projection.item_ids.clone() {
            if let Some(item) = conv.thread_items.iter().find(|item| item.id() == child_id) {
                accumulate_subagent_metrics(projection, item);
            }
        }
    }

    fn touch_subagent_last_event(&mut self, subagent_id: &str, label: impl Into<String>) {
        if let Some(projection) = self.subagent_transcripts.get_mut(subagent_id) {
            projection.last_event_label = Some(label.into());
        }
    }
}

fn accumulate_subagent_metrics(projection: &mut SubagentTranscript, item: &ThreadItem) {
    match item {
        ThreadItem::AssistantMessage { markdown, .. } => {
            projection.assistant_count += 1;
            let snippet = markdown
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("Assistant reply");
            projection.last_event_label = Some(trim_event_label(snippet));
        }
        ThreadItem::ReasoningStep { title, .. } => {
            projection.reasoning_count += 1;
            projection.last_event_label = Some(trim_event_label(title));
        }
        ThreadItem::ToolCall { tool_name, .. } => {
            projection.tool_count += 1;
            projection.last_event_label = Some(format!("Tool: {tool_name}"));
        }
        ThreadItem::DiffSummary { files_changed, .. } => {
            projection.diff_count += 1;
            projection.last_event_label = Some(format!("Diff: {files_changed} file(s) changed"));
        }
        _ => {}
    }
}

fn trim_event_label(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Updated".to_string();
    }
    let mut out = trimmed.chars().take(72).collect::<String>();
    if trimmed.chars().count() > 72 {
        out.push('…');
    }
    out
}

pub(crate) fn subagent_parent_call_id(item: &ThreadItem) -> Option<&str> {
    match item {
        ThreadItem::AssistantMessage { parent_call_id, .. }
        | ThreadItem::ReasoningStep { parent_call_id, .. }
        | ThreadItem::ToolCall { parent_call_id, .. }
        | ThreadItem::DiffSummary { parent_call_id, .. } => parent_call_id.as_deref(),
        _ => None,
    }
}

#[cfg(test)]
fn subagent_projection_child_ids(items: &[ThreadItem], subagent_item_id: &str) -> Vec<String> {
    let Some(parent_call_id) = items.iter().find_map(|item| {
        let ThreadItem::SubagentRun {
            id, parent_call_id, ..
        } = item
        else {
            return None;
        };
        (id == subagent_item_id).then_some(parent_call_id.as_str())
    }) else {
        return Vec::new();
    };

    items
        .iter()
        .filter(|item| subagent_parent_call_id(item) == Some(parent_call_id))
        .map(|item| item.id().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{subagent_parent_call_id, subagent_projection_child_ids};
    use crate::features::shell::state::{AgentStatus, DiffFileSummary, ThreadItem};

    #[test]
    fn maps_child_rows_to_parent_call_id() {
        let item = ThreadItem::AssistantMessage {
            id: "assistant-1".into(),
            markdown: "hello".into(),
            streaming: false,
            depth: 1,
            parent_call_id: Some("delegate-call".into()),
        };
        assert_eq!(subagent_parent_call_id(&item), Some("delegate-call"));
    }

    #[test]
    fn excludes_top_level_rows_from_subagent_projection() {
        let item = ThreadItem::ReasoningStep {
            id: "reason-1".into(),
            title: "Thinking".into(),
            summary: "top level".into(),
            expanded: false,
            status: AgentStatus::Thinking,
            depth: 0,
            parent_call_id: None,
        };
        assert_eq!(subagent_parent_call_id(&item), None);
    }

    #[test]
    fn subagent_projection_collects_only_related_child_rows() {
        let items = vec![
            ThreadItem::SubagentRun {
                id: "subagent-1".into(),
                task: "check".into(),
                model: "model".into(),
                summary: String::new(),
                expanded: true,
                status: AgentStatus::RunningTool,
                child_run_id: "child-run".into(),
                parent_call_id: "delegate-call".into(),
            },
            ThreadItem::AssistantMessage {
                id: "assistant-child".into(),
                markdown: "child".into(),
                streaming: false,
                depth: 1,
                parent_call_id: Some("delegate-call".into()),
            },
            ThreadItem::ReasoningStep {
                id: "reason-child".into(),
                title: "Thinking".into(),
                summary: "child".into(),
                expanded: false,
                status: AgentStatus::Thinking,
                depth: 1,
                parent_call_id: Some("delegate-call".into()),
            },
            ThreadItem::ToolCall {
                id: "tool-child".into(),
                tool_name: "read_file".into(),
                command: None,
                output: None,
                expanded: false,
                status: AgentStatus::Completed,
                depth: 1,
                parent_call_id: Some("delegate-call".into()),
            },
            ThreadItem::DiffSummary {
                id: "diff-child".into(),
                files_changed: 1,
                additions: 1,
                deletions: 0,
                files: vec![DiffFileSummary {
                    path: "src/lib.rs".into(),
                    added: 1,
                    removed: 0,
                }],
                expanded: false,
                depth: 1,
                parent_call_id: Some("delegate-call".into()),
            },
            ThreadItem::AssistantMessage {
                id: "assistant-top".into(),
                markdown: "top".into(),
                streaming: false,
                depth: 0,
                parent_call_id: None,
            },
        ];

        assert_eq!(
            subagent_projection_child_ids(&items, "subagent-1"),
            vec![
                "assistant-child".to_string(),
                "reason-child".to_string(),
                "tool-child".to_string(),
                "diff-child".to_string()
            ]
        );
    }
}

```

### `crates/app/src/window/orchestration/agent_run.rs`

```rust
//! Agent run orchestration — send_message, start_agent_run, cancel_active_run.

use gpui::{Context, Window};

use super::super::{AgentWindow, ConversationRunState};
use crate::agent::ReducerState;
use crate::features::composer::state::PendingImageSource;
use crate::features::shell::state::{
    AgentStatus, ConversationId, MessageAttachment, MessageAttachmentPreview, ThreadItem,
};

impl AgentWindow {
    pub fn try_start_simulation(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.simulations_running.contains(&conversation_id) {
            return false;
        }
        self.simulations_running.insert(conversation_id);
        self.sync_agent_status(AgentStatus::Thinking);
        cx.notify();
        true
    }

    pub fn finish_simulation(&mut self, conversation_id: ConversationId, cx: &mut Context<Self>) {
        self.simulations_running.remove(&conversation_id);
        cx.notify();
    }

    pub fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input_state.read(cx).value().trim().to_string();
        let image_attachments = self.pending_image_attachments.clone();
        if text.is_empty() && image_attachments.is_empty() {
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            return;
        }

        if !image_attachments.is_empty() && !self.selected_model_supports_image_input() {
            self.composer_error = Some(format!(
                "{} does not support image input. Choose a vision-capable model.",
                self.selected_model
            ));
            cx.notify();
            return;
        }

        let conv_id = match self.ensure_send_target(cx) {
            Some(cid) => cid,
            None => {
                tracing::warn!("send blocked: no project/session available");
                return;
            }
        };

        self.reconcile_stale_run_state();
        if self.conversation_run_state(&conv_id).has_active_work() {
            return;
        }

        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
            let id = format!("user-msg-{}", conv.thread_items.len());
            let message_text = if text.is_empty() {
                "Image attachment".to_string()
            } else {
                text.clone()
            };
            conv.thread_items.push(ThreadItem::UserMessage {
                id,
                text: message_text,
                attachments: message_attachments_from_pending(&image_attachments),
                expanded: false,
            });
        }
        if self.safety_mode != agent_protocol::AgentMode::PlanOnly {
            self.mark_plan_stale(&conv_id);
        }

        if !text.is_empty() {
            self.maybe_rename_conversation_from_prompt(&conv_id, &text, cx);
        }

        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.pending_image_attachments.clear();
        self.composer_error = None;

        self.sync_plan_status_for_conversation(&conv_id, cx);
        self.sync_thread_view(conv_id.clone(), cx);

        let run_prompt = if text.is_empty() {
            "Please inspect the attached image(s).".to_string()
        } else {
            text.clone()
        };
        let attachments = image_attachments
            .iter()
            .map(|attachment| attachment.to_context_attachment())
            .collect();
        if let Err(err) = self.start_agent_run(&run_prompt, attachments, cx) {
            tracing::error!("failed to start agent run: {err}");
            if let Some(cid) = &self.selected_conversation_id {
                self.running_conversations.remove(cid);
            }
            self.reset_agent_status_to_idle();
            let error_id = format!("error-{}", uuid::Uuid::new_v4());
            self.push_thread_item(
                conv_id.clone(),
                ThreadItem::RunError {
                    id: error_id,
                    message: format!("Could not start run: {err}"),
                    session_ref: None,
                    retryable: true,
                },
                cx,
            );
            self.sync_thread_view(conv_id, cx);
        }

        cx.notify();
    }

    pub fn start_agent_run(
        &mut self,
        prompt: &str,
        attachments: Vec<agent_protocol::ContextAttachment>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (project_id, session_id) =
            match (&self.selected_project_id, &self.selected_conversation_id) {
                (Some(pid), Some(cid)) => (
                    crate::agent::proto_project_id(pid),
                    crate::agent::proto_session_id(cid),
                ),
                _ => return Err("no project or session selected".into()),
            };

        self.reconcile_stale_run_state();
        if self.selected_conversation_has_active_work() {
            return Err("a run is already in progress for this conversation".into());
        }

        if let Some(cid) = self.selected_conversation_id.clone() {
            self.running_conversations.insert(cid.clone());
            self.finalize_inflight_thread_items(&cid, AgentStatus::Failed, AgentStatus::Failed);
            self.sync_thread_view(cid.clone(), cx);
        }

        self.reducer_state = ReducerState::default();
        self.sync_agent_status(AgentStatus::Thinking);

        if let Some(cid) = self.selected_conversation_id.clone() {
            self.push_thinking_indicator(cid, cx);
        }

        let model_name = if self.agent_bridge.uses_mock {
            self.selected_model.clone()
        } else {
            crate::shared::state::openrouter_model_slug(
                &self.selected_provider,
                &self.selected_model,
            )
        };
        let model = agent_protocol::ModelId::new(model_name);
        let subagent_model = self.selected_subagent_model.as_ref().map(|model| {
            let model_name = if self.agent_bridge.uses_mock {
                model.clone()
            } else {
                crate::shared::state::openrouter_model_slug(&self.selected_provider, model)
            };
            agent_protocol::ModelId::new(model_name)
        });
        let prompt = if let Some(conv) = self
            .conversations
            .iter()
            .find(|c| Some(&c.id) == self.selected_conversation_id.as_ref())
        {
            let context = crate::agent::text::conversation_context_from_thread(&conv.thread_items);
            crate::agent::text::prompt_with_conversation_context(&context, prompt)
        } else {
            prompt.to_string()
        };
        self.agent_bridge
            .send(agent_protocol::AgentCommand::StartRun {
                project_id,
                session_id,
                prompt,
                model,
                subagent_model,
                mode: self.safety_mode.clone(),
                attachments,
            })?;
        cx.notify();
        Ok(())
    }

    pub fn cancel_active_run(&mut self, cx: &mut Context<Self>) {
        if let Some(run_id) = &self.active_run_id {
            self.agent_bridge
                .send(agent_protocol::AgentCommand::CancelRun {
                    run_id: run_id.clone(),
                })
                .ok();
        }
        self.pending_approval_id = None;
        self.pending_thread_approval = None;
        self.diff_panel.pending_approval = None;
        self.diff_panel.pending_patch_id = None;
        if let Some(cid) = self.selected_conversation_id.clone() {
            self.running_conversations.remove(&cid);
            self.mark_plan_execution_cancelled(&cid);
            self.cancel_in_progress_todos(&cid);
            self.resolve_pending_choices(&cid);
            self.sync_plan_status_for_conversation(&cid, cx);
            self.sync_thread_view(cid, cx);
        }
        self.reset_agent_status_to_idle();
        self.sync_thread_approval_state(cx);
        cx.notify();
    }

    pub fn can_retry_last_user_turn(&self) -> bool {
        self.selected_conversation_id
            .as_ref()
            .and_then(|cid| self.conversations.iter().find(|conv| &conv.id == cid))
            .and_then(|conv| {
                conv.thread_items.iter().rev().find_map(|item| match item {
                    ThreadItem::UserMessage {
                        text, attachments, ..
                    } => Some((text, attachments)),
                    _ => None,
                })
            })
            .is_some_and(|(text, attachments)| !text.trim().is_empty() || !attachments.is_empty())
    }

    pub fn retry_last_user_turn(&mut self, cx: &mut Context<Self>) {
        let Some(conv_id) = self.selected_conversation_id.clone() else {
            return;
        };
        let Some((prompt, attachments)) = self
            .conversations
            .iter()
            .find(|conv| conv.id == conv_id)
            .and_then(|conv| {
                conv.thread_items.iter().rev().find_map(|item| match item {
                    ThreadItem::UserMessage {
                        text, attachments, ..
                    } => Some((text.clone(), attachments.clone())),
                    _ => None,
                })
            })
        else {
            return;
        };

        if self.selected_conversation_has_active_work() {
            return;
        }

        self.select_conversation(conv_id.clone(), cx);
        self.composer_error = None;

        let prompt = if prompt.trim().is_empty() {
            "Please inspect the attached image(s).".to_string()
        } else {
            prompt
        };
        let attachments = attachments
            .iter()
            .filter_map(message_attachment_to_context_attachment)
            .collect::<Vec<_>>();

        if let Err(err) = self.start_agent_run(&prompt, attachments, cx) {
            tracing::error!("failed to retry run: {err}");
            self.running_conversations.remove(&conv_id);
            self.reset_agent_status_to_idle();
            self.push_thread_item(
                conv_id.clone(),
                ThreadItem::RunError {
                    id: format!("retry-error-{}", uuid::Uuid::new_v4()),
                    message: format!("Could not retry run: {err}"),
                    session_ref: None,
                    retryable: false,
                },
                cx,
            );
            self.sync_thread_view(conv_id, cx);
        }
        cx.notify();
    }

    pub(crate) fn conversation_run_state(
        &self,
        conversation_id: &ConversationId,
    ) -> ConversationRunState {
        let running = self.running_conversations.contains(conversation_id)
            || self.simulations_running.contains(conversation_id);
        let pending_patch = self.selected_conversation_id.as_ref() == Some(conversation_id)
            && self.diff_panel.pending_patch_id.is_some();
        let thread_items = self
            .conversations
            .iter()
            .find(|conv| conv.id == *conversation_id)
            .map(|conv| conv.thread_items.as_slice())
            .unwrap_or(&[]);
        let selected = self.selected_conversation_id.as_ref() == Some(conversation_id);
        ConversationRunState::from_thread_items(
            thread_items,
            running,
            pending_patch,
            self.provider_blocked.is_some(),
            selected && matches!(self.status.agent_status, Some(AgentStatus::Failed)),
        )
    }
}

fn message_attachments_from_pending(
    attachments: &[crate::features::composer::state::PendingImageAttachment],
) -> Vec<MessageAttachment> {
    attachments
        .iter()
        .map(|attachment| MessageAttachment {
            label: attachment.display_name.clone(),
            mime_type: attachment.mime_type.clone(),
            size_bytes: attachment.size_bytes,
            preview: match &attachment.source {
                PendingImageSource::File(path) => MessageAttachmentPreview::File(path.clone()),
                PendingImageSource::Clipboard(bytes) => MessageAttachmentPreview::Bytes {
                    mime_type: attachment.mime_type.clone(),
                    bytes: bytes.clone(),
                },
            },
        })
        .collect()
}

fn message_attachment_to_context_attachment(
    attachment: &MessageAttachment,
) -> Option<agent_protocol::ContextAttachment> {
    match &attachment.preview {
        MessageAttachmentPreview::File(path) => {
            Some(agent_protocol::ContextAttachment::image_file(
                path.clone(),
                attachment.mime_type.clone(),
                attachment.size_bytes,
            ))
        }
        MessageAttachmentPreview::Bytes { mime_type, bytes } => {
            Some(agent_protocol::ContextAttachment::image_bytes(
                bytes.clone(),
                mime_type.clone(),
                attachment.label.clone(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::shell::state::{ApprovalRisk, ChoiceMeta, ChoiceOption};

    #[test]
    fn conversation_run_state_blocks_unresolved_thread_work() {
        let items = vec![
            ThreadItem::ApprovalRequest {
                id: "approval".into(),
                title: "Run command".into(),
                risk: ApprovalRisk::Medium,
                resolved: false,
            },
            ThreadItem::ChoiceRequest {
                id: "choice".into(),
                prompt: "Pick".into(),
                options: vec![ChoiceOption {
                    id: "a".into(),
                    label: "A".into(),
                    description: None,
                    recommended: true,
                }],
                meta: ChoiceMeta {
                    summary: None,
                    recommended_option_id: None,
                    allow_custom: false,
                    blocking_reason: None,
                },
                selected: None,
                resolved: false,
            },
        ];

        let state = ConversationRunState::from_thread_items(&items, false, false, false, false);

        assert!(state.waiting_approval);
        assert!(state.pending_choice);
        assert!(state.has_active_work());
    }

    #[test]
    fn conversation_run_state_ignores_resolved_thread_work() {
        let items = vec![ThreadItem::ApprovalRequest {
            id: "approval".into(),
            title: "Run command".into(),
            risk: ApprovalRisk::Low,
            resolved: true,
        }];

        let state = ConversationRunState::from_thread_items(&items, false, false, false, false);

        assert!(!state.has_active_work());
    }
}

```

### `crates/app/src/agent/reducer.rs`

```rust
use agent_protocol::{AgentEvent, OutputStreamKind, RunId, RunStatus, ToolStatus};
use gpui::Context;

use crate::agent::text::{
    is_default_conversation_title, sanitize_assistant_delta, title_from_prompt,
};
use crate::features::diff_panel::layout as diff_panel;
use crate::features::shell::state::{
    AgentStatus, ApprovalRisk, Artifact, ArtifactId, CommandRun, ConversationId, DeltaBuffer,
    DiffPanelState, InspectorMode, InspectorView, PendingDiffApproval, PendingThreadApproval,
    ReviewPanelTab, ThreadItem, excerpt_output,
};
use crate::shared::state::{
    TimelineRowKind, context_for_model, default_expansion, estimate_cost_usd, format_context_label,
    format_cost_usd, pricing_for_model,
};
use crate::ui::agent_window::AgentWindow;

#[derive(Default)]
pub struct ReducerState {
    pub stream_runs: std::collections::HashMap<RunId, RunStreamState>,
    pub tool_items: std::collections::HashMap<String, String>,
    /// Maps a stable tool-owned dedupe key → thread item id.
    pub tool_dedupe_keys: std::collections::HashMap<String, String>,
    pub run_conversations: std::collections::HashMap<RunId, ConversationId>,
    pub run_depths: std::collections::HashMap<RunId, u8>,
    pub child_parent_calls: std::collections::HashMap<RunId, String>,
    pub subagent_items: std::collections::HashMap<RunId, String>,
    pub last_event_conversation: Option<ConversationId>,
}

#[derive(Default)]
pub struct RunStreamState {
    pub assistant_buffer: DeltaBuffer,
    pub reasoning_buffer: DeltaBuffer,
    pub assistant_item_id: Option<String>,
    pub reasoning_item_id: Option<String>,
    pub conversation_id: Option<ConversationId>,
    pub depth: u8,
    pub parent_call_id: Option<String>,
}

impl ReducerState {
    fn stream_state_mut(
        &mut self,
        run_id: &RunId,
        conversation_id: Option<ConversationId>,
    ) -> &mut RunStreamState {
        let depth = self.run_depths.get(run_id).copied().unwrap_or(0);
        let parent_call_id = self.child_parent_calls.get(run_id).cloned();
        let state = self.stream_runs.entry(run_id.clone()).or_default();
        if state.conversation_id.is_none() {
            state.conversation_id = conversation_id;
        }
        state.depth = depth;
        state.parent_call_id = parent_call_id;
        state
    }
}

impl AgentWindow {
    #[allow(dead_code)]
    pub fn apply_agent_event(&mut self, event: AgentEvent, cx: &mut Context<Self>) {
        self.apply_agent_events(vec![event], cx);
    }

    pub fn apply_agent_events(&mut self, events: Vec<AgentEvent>, cx: &mut Context<Self>) {
        let mut sync_live = false;
        let mut sync_conv = None;
        let mut notify_window = false;

        for event in &events {
            if event_needs_window_notify(event) {
                notify_window = true;
            }
            let live = matches!(
                event,
                AgentEvent::AssistantTextDelta { .. }
                    | AgentEvent::ReasoningDelta { .. }
                    | AgentEvent::ToolOutputDelta { .. }
                    | AgentEvent::ToolCallUpdated { .. }
                    | AgentEvent::ToolCallStarted { .. }
                    | AgentEvent::ToolCallFinished { .. }
                    | AgentEvent::TodoUpdated { .. }
                    | AgentEvent::PatchPreviewUpdated { .. }
                    | AgentEvent::PatchProposed { .. }
                    | AgentEvent::AndroidSessionUpdated { .. }
                    | AgentEvent::AndroidObservationUpdated { .. }
                    | AgentEvent::AndroidActionPreviewed { .. }
                    | AgentEvent::AndroidActionCompleted { .. }
                    | AgentEvent::AndroidJourneyUpdated { .. }
            );
            if live {
                sync_live = true;
            }
        }

        for event in events {
            self.apply_agent_event_inner(event, cx);
            sync_conv = self
                .reducer_state
                .last_event_conversation
                .clone()
                .or(sync_conv);
        }

        if let Some(conv_id) = sync_conv.or_else(|| self.selected_conversation_id.clone()) {
            if !sync_live {
                self.sync_thread_view(conv_id, cx);
            }
        }
        if notify_window {
            cx.notify();
        }
    }

    fn conv_id_for_event(&self, event: &AgentEvent) -> Option<ConversationId> {
        if let Some(run_id) = event.run_id() {
            if let Some(conv_id) = self.reducer_state.run_conversations.get(run_id) {
                return Some(conv_id.clone());
            }
        }
        self.selected_conversation_id.clone()
    }

    fn set_event_conversation(&mut self, conv_id: Option<ConversationId>) {
        self.reducer_state.last_event_conversation = conv_id;
    }

    pub(crate) fn finalize_inflight_thread_items(
        &mut self,
        conv_id: &ConversationId,
        tool_status: AgentStatus,
        reasoning_status: AgentStatus,
    ) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            for item in &mut conv.thread_items {
                match item {
                    ThreadItem::ToolCall { status, .. }
                        if matches!(status, AgentStatus::RunningTool) =>
                    {
                        *status = tool_status.clone();
                    }
                    ThreadItem::ReasoningStep { status, .. }
                        if matches!(status, AgentStatus::Thinking) =>
                    {
                        *status = reasoning_status.clone();
                    }
                    ThreadItem::AssistantMessage { streaming, .. } if *streaming => {
                        *streaming = false;
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn resolve_approval_item(&mut self, conv_id: &ConversationId) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            for item in &mut conv.thread_items {
                if let ThreadItem::ApprovalRequest { resolved, .. } = item {
                    *resolved = true;
                }
            }
        }
    }

    fn pause_assistant_stream(
        &mut self,
        run_id: &RunId,
        conv_id: &ConversationId,
        cx: &mut Context<Self>,
    ) {
        self.flush_text_buffers(run_id, conv_id, cx);
        let Some(item_id) = self
            .reducer_state
            .stream_runs
            .get(run_id)
            .and_then(|state| state.assistant_item_id.clone())
        else {
            return;
        };
        let pending_empty = self
            .reducer_state
            .stream_runs
            .get(run_id)
            .is_none_or(|state| state.assistant_buffer.pending_text().is_empty());
        let sanitized = self
            .conversations
            .iter()
            .find(|c| c.id == *conv_id)
            .and_then(|c| {
                c.thread_items.iter().find_map(|item| {
                    if let ThreadItem::AssistantMessage { id, markdown, .. } = item {
                        if id == &item_id {
                            return Some(crate::agent::text::sanitize_assistant_text(markdown));
                        }
                    }
                    None
                })
            })
            .unwrap_or_default();
        if sanitized.is_empty() && pending_empty {
            if let Some(state) = self.reducer_state.stream_runs.get_mut(run_id) {
                state.assistant_item_id = None;
            }
            self.remove_thread_item(conv_id.clone(), &item_id, cx);
            return;
        }
        let still_streaming = self
            .conversations
            .iter()
            .find(|c| c.id == *conv_id)
            .and_then(|c| {
                c.thread_items.iter().find_map(|item| {
                    if let ThreadItem::AssistantMessage { id, streaming, .. } = item {
                        if id == &item_id && *streaming {
                            return Some(true);
                        }
                    }
                    None
                })
            })
            .unwrap_or(false);
        if !still_streaming && pending_empty {
            return;
        }
        self.update_thread_item(
            conv_id.clone(),
            &item_id,
            |item| {
                if let ThreadItem::AssistantMessage {
                    markdown,
                    streaming,
                    ..
                } = item
                {
                    *markdown = sanitized;
                    *streaming = false;
                }
            },
            cx,
        );
    }

    pub(crate) fn maybe_rename_conversation_from_prompt(
        &mut self,
        conv_id: &ConversationId,
        prompt: &str,
        cx: &mut Context<Self>,
    ) {
        let new_title = title_from_prompt(prompt);
        let should_rename = self
            .conversations
            .iter()
            .find(|c| c.id == *conv_id)
            .is_some_and(|c| is_default_conversation_title(&c.title));
        if !should_rename {
            return;
        }
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            conv.title = new_title.clone();
        }
        let session_id = agent_protocol::SessionId::new(conv_id.0.clone());
        if self
            .agent_bridge
            .update_session_title(&session_id, &new_title)
            .is_err()
        {
            tracing::warn!("failed to persist session title");
        }
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    fn thread_row_expanded(
        &self,
        kind: TimelineRowKind,
        status: AgentStatus,
        failed: bool,
    ) -> bool {
        default_expansion(self.transcript_mode, kind, status, failed, false)
    }

    fn complete_reasoning(
        &mut self,
        run_id: &RunId,
        conv_id: &ConversationId,
        cx: &mut Context<Self>,
    ) {
        if let Some(item_id) = self
            .reducer_state
            .stream_runs
            .get(run_id)
            .and_then(|state| state.reasoning_item_id.clone())
        {
            self.update_thread_item(
                conv_id.clone(),
                &item_id,
                |item| {
                    if let ThreadItem::ReasoningStep { status, .. } = item {
                        *status = AgentStatus::Completed;
                    }
                },
                cx,
            );
        }
    }

    fn flush_text_buffers(
        &mut self,
        run_id: &RunId,
        conv_id: &ConversationId,
        cx: &mut Context<Self>,
    ) {
        let (assistant_item_id, assistant_chunk, reasoning_item_id, reasoning_chunk) = {
            let Some(state) = self.reducer_state.stream_runs.get_mut(run_id) else {
                return;
            };
            let assistant_chunk = if state.assistant_buffer.should_flush()
                || !state.assistant_buffer.pending_text().is_empty()
            {
                Some(state.assistant_buffer.take())
            } else {
                None
            };
            let reasoning_chunk = if !state.reasoning_buffer.pending_text().is_empty() {
                Some(state.reasoning_buffer.take())
            } else {
                None
            };
            (
                state.assistant_item_id.clone(),
                assistant_chunk,
                state.reasoning_item_id.clone(),
                reasoning_chunk,
            )
        };
        if let Some(chunk) = assistant_chunk {
            if !chunk.is_empty() {
                if let Some(item_id) = assistant_item_id {
                    self.update_thread_item(
                        conv_id.clone(),
                        &item_id,
                        |item| {
                            if let ThreadItem::AssistantMessage { markdown, .. } = item {
                                markdown.push_str(&chunk);
                            }
                        },
                        cx,
                    );
                }
            }
        }
        if let Some(chunk) = reasoning_chunk {
            if !chunk.is_empty() {
                if let Some(item_id) = reasoning_item_id {
                    self.update_thread_item(
                        conv_id.clone(),
                        &item_id,
                        |item| {
                            if let ThreadItem::ReasoningStep { summary, .. } = item {
                                summary.push_str(&chunk);
                            }
                        },
                        cx,
                    );
                }
            }
        }
    }

    fn upsert_tool_row(
        &mut self,
        conv_id: ConversationId,
        run_id: &RunId,
        depth: u8,
        parent_call_id: Option<String>,
        call_id: &str,
        name: String,
        args_preview: String,
        dedupe_key: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(item_id) = self.reducer_state.tool_items.get(call_id) {
            let item_id = item_id.clone();
            self.update_thread_item(
                conv_id,
                &item_id,
                |item| {
                    if let ThreadItem::ToolCall { command, .. } = item {
                        if !args_preview.is_empty() {
                            *command = Some(args_preview);
                        }
                    }
                },
                cx,
            );
        } else if let Some(dedupe_key) = dedupe_key {
            if let Some(existing_id) = self.reducer_state.tool_dedupe_keys.get(&dedupe_key) {
                self.reducer_state
                    .tool_items
                    .insert(call_id.to_string(), existing_id.clone());
                return;
            }
            self.complete_reasoning(run_id, &conv_id, cx);
            let id = format!("tool-{call_id}");
            self.reducer_state
                .tool_items
                .insert(call_id.to_string(), id.clone());
            self.reducer_state
                .tool_dedupe_keys
                .insert(dedupe_key, id.clone());
            self.sync_agent_status(AgentStatus::RunningTool);
            let tool_expanded = self.thread_row_expanded(
                TimelineRowKind::ToolCall,
                AgentStatus::RunningTool,
                false,
            );
            self.push_thread_item(
                conv_id,
                ThreadItem::ToolCall {
                    id,
                    tool_name: name,
                    command: if args_preview.is_empty() {
                        None
                    } else {
                        Some(args_preview)
                    },
                    output: None,
                    expanded: tool_expanded,
                    status: AgentStatus::RunningTool,
                    depth,
                    parent_call_id: parent_call_id.clone(),
                },
                cx,
            );
        } else {
            self.complete_reasoning(run_id, &conv_id, cx);
            let id = format!("tool-{call_id}");
            self.reducer_state
                .tool_items
                .insert(call_id.to_string(), id.clone());
            self.sync_agent_status(AgentStatus::RunningTool);
            let tool_expanded = self.thread_row_expanded(
                TimelineRowKind::ToolCall,
                AgentStatus::RunningTool,
                false,
            );
            self.push_thread_item(
                conv_id,
                ThreadItem::ToolCall {
                    id,
                    tool_name: name,
                    command: if args_preview.is_empty() {
                        None
                    } else {
                        Some(args_preview)
                    },
                    output: None,
                    expanded: tool_expanded,
                    status: AgentStatus::RunningTool,
                    depth,
                    parent_call_id,
                },
                cx,
            );
        }
    }

    fn apply_agent_event_inner(&mut self, event: AgentEvent, cx: &mut Context<Self>) {
        let conv_id = self.conv_id_for_event(&event);
        self.set_event_conversation(conv_id.clone());
        match event {
            AgentEvent::RunStarted {
                run_id,
                session_id,
                depth,
                parent_run_id: _,
                ..
            } => {
                self.reducer_state
                    .run_conversations
                    .insert(run_id.clone(), ConversationId(session_id.0.clone()));
                self.reducer_state.run_depths.insert(run_id.clone(), depth);
                let conv_id = ConversationId(session_id.0.clone());
                self.reducer_state
                    .stream_state_mut(&run_id, Some(conv_id.clone()));
                if depth == 0 {
                    self.active_run_id = Some(run_id.clone());
                }
                self.run_cost_usd = 0.0;
                self.status.estimated_cost = None;
                self.status.input_tokens = 0;
                self.status.output_tokens = 0;
                self.status.cache_read_tokens = 0;
                self.status.cache_write_tokens = 0;
                self.status.token_usage = format!(
                    "0 / {}",
                    format_context_label(context_for_model(
                        &self.selected_provider,
                        &self.selected_model,
                        &self.openrouter_models,
                    ))
                );
                self.reducer_state.tool_dedupe_keys.clear();
                self.sync_agent_status(AgentStatus::Thinking);
                if depth == 0 {
                    self.running_conversations.insert(conv_id.clone());
                    let existing_reason_id = self
                        .conversations
                        .iter()
                        .find(|c| c.id == conv_id)
                        .and_then(|conv| {
                            conv.thread_items.iter().find_map(|item| {
                                if let ThreadItem::ReasoningStep {
                                    id,
                                    status: AgentStatus::Thinking,
                                    depth: 0,
                                    ..
                                } = item
                                {
                                    Some(id.clone())
                                } else {
                                    None
                                }
                            })
                        });
                    if let Some(id) = existing_reason_id {
                        if let Some(state) = self.reducer_state.stream_runs.get_mut(&run_id) {
                            state.reasoning_item_id = Some(id);
                        }
                    }
                    let reasoning_exists = self
                        .reducer_state
                        .stream_runs
                        .get(&run_id)
                        .and_then(|state| state.reasoning_item_id.as_ref())
                        .is_some();
                    if !reasoning_exists {
                        let id = format!("reason-{}", uuid_simple());
                        if let Some(state) = self.reducer_state.stream_runs.get_mut(&run_id) {
                            state.reasoning_item_id = Some(id.clone());
                        }
                        let reasoning_expanded = self.thread_row_expanded(
                            TimelineRowKind::ReasoningNote,
                            AgentStatus::Thinking,
                            false,
                        );
                        self.push_thread_item(
                            conv_id,
                            ThreadItem::ReasoningStep {
                                id,
                                title: "Thinking".into(),
                                summary: String::new(),
                                expanded: reasoning_expanded,
                                status: AgentStatus::Thinking,
                                depth: 0,
                                parent_call_id: None,
                            },
                            cx,
                        );
                    }
                }
            }
            AgentEvent::ContextBuilt { .. } => {
                if self.active_run_id.is_some() {
                    self.sync_agent_status(AgentStatus::Thinking);
                }
            }
            AgentEvent::AssistantTextDelta { run_id, text } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let text = sanitize_assistant_delta(&text);
                if text.is_empty() {
                    return;
                }
                // Pass assistant text straight through — `ThreadView` coalesces to one
                // commit per display frame, so a reducer-side time gate would only add
                // latency. The buffer is still used to normalize the chunk + reset timing.
                let (chunk, existing_item_id, depth, parent_call_id) = {
                    let state = self
                        .reducer_state
                        .stream_state_mut(&run_id, Some(conv_id.clone()));
                    state.assistant_buffer.push(&text);
                    (
                        state.assistant_buffer.take(),
                        state.assistant_item_id.clone(),
                        state.depth,
                        state.parent_call_id.clone(),
                    )
                };
                if existing_item_id.is_none() {
                    let id = format!("assistant-{}", uuid_simple());
                    if let Some(state) = self.reducer_state.stream_runs.get_mut(&run_id) {
                        state.assistant_item_id = Some(id.clone());
                    }
                    self.push_thread_item(
                        conv_id.clone(),
                        ThreadItem::AssistantMessage {
                            id,
                            markdown: chunk,
                            streaming: true,
                            depth,
                            parent_call_id,
                        },
                        cx,
                    );
                } else if let Some(item_id) = existing_item_id {
                    self.append_assistant_delta(conv_id.clone(), &item_id, &chunk, cx);
                }
            }
            AgentEvent::ReasoningDelta { run_id, text } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let (existing_item_id, depth, parent_call_id) = {
                    let state = self
                        .reducer_state
                        .stream_state_mut(&run_id, Some(conv_id.clone()));
                    (
                        state.reasoning_item_id.clone(),
                        state.depth,
                        state.parent_call_id.clone(),
                    )
                };
                if existing_item_id.is_none() {
                    let id = format!("reason-{}", uuid_simple());
                    if let Some(state) = self.reducer_state.stream_runs.get_mut(&run_id) {
                        state.reasoning_item_id = Some(id.clone());
                    }
                    let reasoning_expanded = self.thread_row_expanded(
                        TimelineRowKind::ReasoningNote,
                        AgentStatus::Thinking,
                        false,
                    );
                    self.push_thread_item(
                        conv_id.clone(),
                        ThreadItem::ReasoningStep {
                            id,
                            title: "Thinking".into(),
                            summary: text,
                            expanded: reasoning_expanded,
                            status: AgentStatus::Thinking,
                            depth,
                            parent_call_id,
                        },
                        cx,
                    );
                } else {
                    let (chunk, should_flush, item_id) = {
                        let state = self
                            .reducer_state
                            .stream_state_mut(&run_id, Some(conv_id.clone()));
                        state.reasoning_buffer.push(&text);
                        let should_flush = state.reasoning_buffer.should_flush();
                        let chunk = if should_flush {
                            Some(state.reasoning_buffer.take())
                        } else {
                            None
                        };
                        (chunk, should_flush, state.reasoning_item_id.clone())
                    };
                    if !should_flush {
                        return;
                    }
                    if let (Some(item_id), Some(chunk)) = (item_id, chunk) {
                        self.append_reasoning_delta(conv_id.clone(), &item_id, &chunk, cx);
                    }
                }
            }
            AgentEvent::ToolCallStarted {
                call_id,
                name,
                args_preview,
                dedupe_key,
                run_id,
                ..
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                self.pause_assistant_stream(&run_id, &conv_id, cx);
                let depth = self
                    .reducer_state
                    .run_depths
                    .get(&run_id)
                    .copied()
                    .unwrap_or(0);
                let parent_call_id = self.reducer_state.child_parent_calls.get(&run_id).cloned();
                self.upsert_tool_row(
                    conv_id,
                    &run_id,
                    depth,
                    parent_call_id,
                    &call_id.0,
                    name.clone(),
                    args_preview.clone(),
                    dedupe_key,
                    cx,
                );
                if self.tool_catalog.is_shell_tool(&name) {
                    let cwd = self
                        .selected_project_id
                        .as_ref()
                        .and_then(|pid| self.projects.iter().find(|p| p.id == *pid))
                        .map(|p| p.root_path.clone())
                        .unwrap_or_default();
                    let started_at_ms = chrono::Utc::now().timestamp_millis();
                    self.command_run_ledger.insert(CommandRun {
                        id: call_id.0.clone(),
                        session_id: None,
                        command: args_preview,
                        cwd,
                        started_at_ms,
                        ended_at_ms: None,
                        exit_code: None,
                        output_excerpt: None,
                    });
                }
            }
            AgentEvent::ToolCallUpdated {
                call_id,
                args_preview,
                dedupe_key,
                ..
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                if let Some(current_id) = self.reducer_state.tool_items.get(&call_id.0).cloned() {
                    let item_id = if let Some(dedupe_key) = dedupe_key {
                        if let Some(existing_id) = self
                            .reducer_state
                            .tool_dedupe_keys
                            .get(&dedupe_key)
                            .cloned()
                        {
                            if existing_id != current_id {
                                self.reducer_state
                                    .tool_items
                                    .insert(call_id.0.clone(), existing_id.clone());
                                self.remove_thread_item(conv_id.clone(), &current_id, cx);
                                existing_id
                            } else {
                                current_id.clone()
                            }
                        } else {
                            self.reducer_state
                                .tool_dedupe_keys
                                .insert(dedupe_key, current_id.clone());
                            current_id.clone()
                        }
                    } else {
                        current_id.clone()
                    };
                    self.update_thread_item(
                        conv_id,
                        &item_id,
                        |item| {
                            if let ThreadItem::ToolCall { command, .. } = item {
                                if !args_preview.is_empty() {
                                    *command = Some(args_preview);
                                }
                            }
                        },
                        cx,
                    );
                }
            }
            AgentEvent::ToolOutputDelta {
                call_id,
                stream,
                chunk,
                ..
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let prefix = match stream {
                    OutputStreamKind::Stdout => "",
                    OutputStreamKind::Stderr => "[stderr] ",
                };
                if let Some(item_id) = self.reducer_state.tool_items.get(&call_id.0) {
                    let item_id = item_id.clone();
                    let skip_output = self
                        .conversations
                        .iter()
                        .find(|c| c.id == conv_id)
                        .and_then(|c| c.thread_items.iter().find(|item| item.id() == item_id))
                        .is_some_and(|item| {
                            matches!(
                                item,
                                ThreadItem::ToolCall { tool_name, .. }
                                    if self.tool_catalog.suppresses_live_output(tool_name)
                            )
                        });
                    if !skip_output {
                        self.append_tool_output_delta(conv_id, &item_id, prefix, &chunk, cx);
                    }
                }
            }
            AgentEvent::ToolCallFinished {
                call_id,
                status,
                summary,
                body,
                ..
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let summary_text = summary.clone();
                let body_text = body.clone();
                let agent_status = match status {
                    ToolStatus::Completed => AgentStatus::Completed,
                    ToolStatus::Failed | ToolStatus::Denied => AgentStatus::Failed,
                    ToolStatus::Cancelled => AgentStatus::Failed,
                    ToolStatus::Running => AgentStatus::RunningTool,
                };
                if let Some(item_id) = self.reducer_state.tool_items.get(&call_id.0) {
                    let item_id = item_id.clone();
                    let failed = matches!(agent_status, AgentStatus::Failed);
                    let tool_expanded = self.thread_row_expanded(
                        TimelineRowKind::ToolCall,
                        agent_status.clone(),
                        failed,
                    );
                    self.update_thread_item(
                        conv_id.clone(),
                        &item_id,
                        |item| {
                            if let ThreadItem::ToolCall {
                                status: s,
                                output,
                                expanded,
                                ..
                            } = item
                            {
                                *s = agent_status.clone();
                                if let Some(full) = body {
                                    *output = Some(full);
                                } else if output.is_none() && !summary.is_empty() {
                                    *output = Some(summary);
                                }
                                *expanded = tool_expanded;
                            }
                        },
                        cx,
                    );
                }
                if self.pending_approval_id.is_some() {
                    self.clear_pending_approval_ui(&conv_id, cx);
                }
                if let Some(item_id) = self.reducer_state.tool_items.get(&call_id.0) {
                    let artifact_id = ArtifactId::new(format!("tool-{item_id}"));
                    if let Some(conv) = self.conversations.iter().find(|c| c.id == conv_id) {
                        if let Some(ThreadItem::ToolCall {
                            tool_name,
                            command,
                            output,
                            ..
                        }) = conv.thread_items.iter().find(|i| i.id() == item_id)
                        {
                            let title = self.tool_row_label(tool_name, command.as_deref(), false);
                            if self.tool_catalog.is_shell_tool(tool_name) {
                                let full_output = output.clone().unwrap_or_else(|| {
                                    body_text.clone().unwrap_or(summary_text.clone())
                                });
                                let excerpt = excerpt_output(&full_output);
                                let exit_code = match status {
                                    ToolStatus::Completed => Some(0),
                                    ToolStatus::Failed | ToolStatus::Denied => Some(1),
                                    ToolStatus::Cancelled => Some(130),
                                    ToolStatus::Running => None,
                                };
                                self.command_run_ledger.finish(
                                    &call_id.0,
                                    exit_code,
                                    Some(excerpt.clone()),
                                    chrono::Utc::now().timestamp_millis(),
                                );
                                self.artifact_store.upsert(Artifact::terminal(
                                    artifact_id.0.clone(),
                                    title,
                                    excerpt,
                                    Some(item_id.clone()),
                                ));
                            }
                        }
                    }
                }
            }
            AgentEvent::PatchPreviewUpdated { unified_diff, .. } => {
                self.schedule_diff_preview_parse(unified_diff, cx);
            }
            AgentEvent::PatchProposed {
                patch_id,
                files: _,
                unified_diff,
                ..
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let parsed = diff_panel::parse_unified_diff(&unified_diff);
                let (additions, deletions) = parsed
                    .iter()
                    .fold((0usize, 0usize), |(a, d), f| (a + f.added, d + f.removed));
                let file_summaries: Vec<_> = parsed
                    .iter()
                    .map(|f| crate::features::shell::state::DiffFileSummary {
                        path: f.path.clone(),
                        added: f.added,
                        removed: f.removed,
                    })
                    .collect();
                self.push_thread_item(
                    conv_id.clone(),
                    ThreadItem::DiffSummary {
                        id: format!("diff-{}", uuid_simple()),
                        files_changed: file_summaries.len(),
                        additions,
                        deletions,
                        files: file_summaries,
                        expanded: self.thread_row_expanded(
                            TimelineRowKind::DiffSummary,
                            AgentStatus::Completed,
                            false,
                        ),
                        depth: 0,
                        parent_call_id: None,
                    },
                    cx,
                );
                let auto_apply = self.safety_mode.auto_applies_patches();
                if auto_apply {
                    self.diff_panel.pending_patch_id = None;
                    self.diff_panel.applied = true;
                } else {
                    self.diff_panel.pending_patch_id = Some(patch_id.0.clone());
                    self.diff_panel.applied = false;
                }
                self.artifact_store
                    .update_diff_files("patch-preview", parsed.clone());
                self.artifact_store
                    .set_primary_patch(Some(patch_id.0.clone()));
                self.apply_diff_panel_now(&unified_diff, cx);
                if auto_apply {
                    if self.active_run_id.is_some() {
                        self.sync_agent_status(AgentStatus::RunningTool);
                    }
                } else {
                    self.mark_plan_execution_waiting_approval(&conv_id);
                    self.sync_plan_status_for_conversation(&conv_id, cx);
                    self.sync_agent_status(AgentStatus::WaitingApproval);
                }
                self.request_thread_scroll_to_bottom = true;
                self.sync_thread_view(conv_id, cx);
            }
            AgentEvent::ChoiceRequested {
                choice_id,
                prompt,
                options,
                summary,
                recommended_option_id,
                allow_custom,
                blocking_reason,
                ..
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let recommended = recommended_option_id.clone();
                self.push_thread_item(
                    conv_id,
                    ThreadItem::ChoiceRequest {
                        id: choice_id,
                        prompt,
                        options: options
                            .into_iter()
                            .map(|opt| {
                                let is_recommended = opt.recommended
                                    || recommended.as_ref().is_some_and(|id| id == &opt.id);
                                crate::features::shell::state::ChoiceOption {
                                    id: opt.id,
                                    label: opt.label,
                                    description: opt.description,
                                    recommended: is_recommended,
                                }
                            })
                            .collect(),
                        meta: crate::features::shell::state::ChoiceMeta {
                            summary,
                            recommended_option_id,
                            allow_custom,
                            blocking_reason,
                        },
                        selected: None,
                        resolved: false,
                    },
                    cx,
                );
            }
            AgentEvent::TodoUpdated { todos, .. } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let items: Vec<crate::features::shell::state::TodoEntry> = todos
                    .into_iter()
                    .map(|t| crate::features::shell::state::TodoEntry {
                        id: t.id,
                        content: t.content,
                        state: match t.status {
                            agent_protocol::TodoStatus::Pending => {
                                crate::features::shell::state::TodoState::Pending
                            }
                            agent_protocol::TodoStatus::InProgress => {
                                crate::features::shell::state::TodoState::InProgress
                            }
                            agent_protocol::TodoStatus::Completed => {
                                crate::features::shell::state::TodoState::Completed
                            }
                            agent_protocol::TodoStatus::Cancelled => {
                                crate::features::shell::state::TodoState::Cancelled
                            }
                        },
                    })
                    .collect();
                if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
                    conv.active_todos = items;
                }
                self.sync_plan_status_for_conversation(&conv_id, cx);
                // Skip full-window notify — todo strip updates are cosmetic during
                // streaming and will catch up on the next window-notifiable event.
            }
            AgentEvent::ContextTrace { entries, .. } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                if entries.is_empty() {
                    return;
                }
                let entries: Vec<crate::features::shell::state::ContextTraceEntry> = entries
                    .into_iter()
                    .map(|e| crate::features::shell::state::ContextTraceEntry {
                        kind: map_context_entry_kind(e.kind),
                        label: e.label,
                        detail: e.detail,
                        reason: e.reason,
                    })
                    .collect();
                self.push_thread_item(
                    conv_id,
                    ThreadItem::ContextTrace {
                        id: format!("context-{}", uuid_simple()),
                        entries,
                        expanded: false,
                    },
                    cx,
                );
            }
            AgentEvent::PlanUpdated {
                run_id,
                markdown,
                created_at,
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                let previous_plan = self
                    .conversations
                    .iter()
                    .find(|c| c.id == conv_id)
                    .and_then(|conv| conv.plan_artifact.clone());
                if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
                    conv.plan_artifact = Some(crate::features::shell::state::PlanArtifact {
                        markdown: markdown.clone(),
                        source_run_id: run_id.0.clone(),
                        created_at: created_at.clone(),
                        execution_state: if previous_plan.is_some_and(|plan| {
                            !matches!(
                                plan.execution_state,
                                crate::features::shell::state::PlanExecutionState::Completed
                            )
                        }) {
                            crate::features::shell::state::PlanExecutionState::Stale
                        } else {
                            crate::features::shell::state::PlanExecutionState::NotStarted
                        },
                        source_conversation_id: None,
                        started_at: None,
                        completed_at: None,
                    });
                    self.artifact_store.upsert(Artifact::plan(
                        format!("plan-{}", run_id.0),
                        conv.plan_artifact.clone().expect("just set"),
                    ));
                }
                self.sync_plan_status_for_conversation(&conv_id, cx);
                self.diff_panel.active_tab = ReviewPanelTab::Plan;
                self.inspector_tabs.select_builtin(InspectorView::Plan);
                self.apply_active_inspector_tab();
                self.set_inspector_mode(InspectorMode::Review, cx);
            }
            AgentEvent::AndroidSessionUpdated { session, .. } => {
                self.android_session = session;
            }
            AgentEvent::AndroidObservationUpdated { observation, .. } => {
                self.android_session.status = "Ready".into();
                self.android_session.current_app = observation.package.clone();
                self.android_session.current_activity = observation.activity.clone();
                self.android_session.device = observation.device.clone();
                self.android_session.current_action = None;
                self.android_session.latest_observation = Some(observation);
            }
            AgentEvent::AndroidActionPreviewed { action, .. } => {
                self.android_session.current_action = Some(action);
            }
            AgentEvent::AndroidActionCompleted { action, .. } => {
                self.android_session.status = "Ready".into();
                self.android_session.current_action = None;
                self.android_session.recent_actions.push(action);
                if self.android_session.recent_actions.len() > 50 {
                    let excess = self.android_session.recent_actions.len() - 50;
                    self.android_session.recent_actions.drain(0..excess);
                }
            }
            AgentEvent::AndroidJourneyUpdated { journey, .. } => {
                self.android_session.active_journey = Some(journey);
            }
            AgentEvent::SubagentStarted {
                parent_run_id: _,
                child_run_id,
                call_id,
                model,
                task,
            } => {
                self.reducer_state
                    .child_parent_calls
                    .insert(child_run_id.clone(), call_id.0.clone());
                if let Some(state) = self.reducer_state.stream_runs.get_mut(&child_run_id) {
                    state.parent_call_id = Some(call_id.0.clone());
                    state.depth = 1;
                }
                let Some(conv_id) = conv_id else {
                    return;
                };
                let id = format!("subagent-{}", child_run_id.0);
                self.reducer_state
                    .subagent_items
                    .insert(child_run_id.clone(), id.clone());
                self.push_thread_item(
                    conv_id,
                    ThreadItem::SubagentRun {
                        id,
                        task,
                        model: model.0,
                        summary: String::new(),
                        expanded: true,
                        status: AgentStatus::RunningTool,
                        child_run_id: child_run_id.0,
                        parent_call_id: call_id.0,
                    },
                    cx,
                );
            }
            AgentEvent::SubagentFinished {
                child_run_id,
                status,
                summary,
                ..
            } => {
                if let Some(conv_id) = conv_id {
                    if let Some(item_id) = self
                        .reducer_state
                        .subagent_items
                        .get(&child_run_id)
                        .cloned()
                    {
                        self.update_thread_item(
                            conv_id,
                            &item_id,
                            |item| {
                                if let ThreadItem::SubagentRun {
                                    summary: item_summary,
                                    status: item_status,
                                    ..
                                } = item
                                {
                                    *item_summary = summary.clone();
                                    *item_status = match status {
                                        RunStatus::Completed => AgentStatus::Completed,
                                        RunStatus::PausedForApproval | RunStatus::Running => {
                                            AgentStatus::RunningTool
                                        }
                                        RunStatus::Cancelled | RunStatus::Failed => {
                                            AgentStatus::Failed
                                        }
                                    };
                                }
                            },
                            cx,
                        );
                    }
                }
            }
            AgentEvent::PatchApplied { .. } => {
                self.diff_panel.pending_approval = None;
                self.diff_panel.pending_patch_id = None;
                self.diff_panel.applied = true;
                self.pending_thread_approval = None;
                if self.active_run_id.is_some() {
                    self.sync_agent_status(AgentStatus::Thinking);
                }
                if let Some(conv_id) = conv_id.clone() {
                    self.mark_plan_execution_implementing(&conv_id);
                    self.sync_plan_status_for_conversation(&conv_id, cx);
                    self.sync_thread_view(conv_id, cx);
                }
            }
            AgentEvent::ApprovalRequested {
                approval_id,
                call_id,
                reason,
                risk,
                command_preview,
                ..
            } => {
                let Some(conv_id) = conv_id else {
                    return;
                };
                self.pending_approval_id = Some(approval_id.0.clone());
                self.sync_thread_approval_state(cx);
                self.mark_plan_execution_waiting_approval(&conv_id);
                self.sync_plan_status_for_conversation(&conv_id, cx);
                self.sync_agent_status(AgentStatus::WaitingApproval);
                let title = command_preview.clone().unwrap_or(reason.clone());
                let mapped_risk = map_risk(risk);
                let tool_kind = self
                    .reducer_state
                    .tool_items
                    .get(&call_id.0)
                    .and_then(|item_id| {
                        self.conversations
                            .iter()
                            .find(|c| c.id == conv_id)
                            .and_then(|c| c.thread_items.iter().find(|i| i.id() == item_id))
                    })
                    .map(|item| match item {
                        ThreadItem::ToolCall { tool_name, .. } => tool_name.clone(),
                        _ => String::new(),
                    });
                let is_patch_tool = tool_kind
                    .as_deref()
                    .is_some_and(|n| self.tool_catalog.is_patch_tool(n));
                let is_shell_tool = tool_kind
                    .as_deref()
                    .is_some_and(|n| self.tool_catalog.is_shell_tool(n));
                if !is_patch_tool && !is_shell_tool {
                    self.push_thread_item(
                        conv_id.clone(),
                        ThreadItem::ApprovalRequest {
                            id: approval_id.0.clone(),
                            title: title.clone(),
                            risk: mapped_risk.clone(),
                            resolved: false,
                        },
                        cx,
                    );
                }
                if let Some(item_id) = self.reducer_state.tool_items.get(&call_id.0).cloned() {
                    let preview = command_preview.clone();
                    let tool_expanded = self.thread_row_expanded(
                        TimelineRowKind::ToolCall,
                        AgentStatus::WaitingApproval,
                        false,
                    );
                    self.update_thread_item(
                        conv_id.clone(),
                        &item_id,
                        |item| {
                            if let ThreadItem::ToolCall {
                                status,
                                command,
                                expanded,
                                ..
                            } = item
                            {
                                *status = AgentStatus::WaitingApproval;
                                if let Some(cmd) = preview.filter(|c| !c.is_empty()) {
                                    *command = Some(cmd);
                                }
                                *expanded = tool_expanded;
                            }
                        },
                        cx,
                    );
                }
                self.diff_panel.pending_approval = Some(PendingDiffApproval {
                    title: title.clone(),
                    risk: mapped_risk.clone(),
                });
                self.pending_thread_approval = Some(PendingThreadApproval {
                    title,
                    risk: mapped_risk,
                    allow_always_label: if is_shell_tool {
                        command_preview
                            .as_deref()
                            .and_then(allow_always_label)
                            .or_else(|| Some("Always allow".to_string()))
                    } else {
                        None
                    },
                });
                if !self.diff_panel.files.is_empty()
                    && !self.diff_panel.suppress_auto_open
                    && is_patch_tool
                {
                    self.inspector_tabs.select_builtin(InspectorView::Changes);
                    self.apply_active_inspector_tab();
                    self.set_inspector_mode(InspectorMode::Review, cx);
                }
                self.request_thread_scroll_to_bottom = true;
            }
            AgentEvent::CommandFailed { message } => {
                self.pending_approval_id = None;
                self.diff_panel.pending_approval = None;
                self.pending_thread_approval = None;
                self.sync_thread_approval_state(cx);
                if let Some(conv_id) = conv_id.clone() {
                    self.finalize_inflight_thread_items(
                        &conv_id,
                        AgentStatus::Failed,
                        AgentStatus::Failed,
                    );
                    self.resolve_approval_item(&conv_id);
                    self.push_thread_item(
                        conv_id.clone(),
                        ThreadItem::RunError {
                            id: format!("cmd-err-{}", uuid_simple()),
                            message,
                            session_ref: self.active_run_id.as_ref().map(|r| r.0.clone()),
                            retryable: true,
                        },
                        cx,
                    );
                    self.running_conversations.remove(&conv_id);
                    self.mark_plan_execution_failed(&conv_id);
                    self.sync_plan_status_for_conversation(&conv_id, cx);
                }
                self.reducer_state.stream_runs.clear();
                self.reducer_state.tool_items.clear();
                self.reset_agent_status_to_idle();
                self.active_run_id = None;
            }
            AgentEvent::UsageUpdated {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_write_tokens,
                estimated_cost_usd,
                ..
            } => {
                self.status.input_tokens += input_tokens;
                self.status.output_tokens += output_tokens;
                self.status.cache_read_tokens += cache_read_tokens.unwrap_or(0);
                self.status.cache_write_tokens += cache_write_tokens.unwrap_or(0);

                let max = context_for_model(
                    &self.selected_provider,
                    &self.selected_model,
                    &self.openrouter_models,
                );
                let max_label = format_context_label(max);

                if let Some(cost) = estimated_cost_usd {
                    self.run_cost_usd += cost;
                    self.status.estimated_cost = Some(format_cost_usd(self.run_cost_usd));
                } else if let Some(pricing) = pricing_for_model(
                    &self.selected_provider,
                    &self.selected_model,
                    &self.openrouter_models,
                    &self.model_pricing,
                ) {
                    self.run_cost_usd += estimate_cost_usd(
                        input_tokens,
                        output_tokens,
                        cache_read_tokens.unwrap_or(0),
                        cache_write_tokens.unwrap_or(0),
                        &pricing,
                    );
                    self.status.estimated_cost = Some(format_cost_usd(self.run_cost_usd));
                }

                self.status.token_usage = format!(
                    "{} / {}",
                    format_tokens(self.status.input_tokens),
                    max_label
                );
            }
            AgentEvent::RunFinished { run_id, status, .. } => {
                let run_depth = self
                    .reducer_state
                    .run_depths
                    .get(&run_id)
                    .copied()
                    .unwrap_or(0);
                let sweep_conv = self
                    .reducer_state
                    .run_conversations
                    .get(&run_id)
                    .cloned()
                    .or_else(|| self.selected_conversation_id.clone());
                if let Some(conv_id) = sweep_conv.clone() {
                    self.flush_text_buffers(&run_id, &conv_id, cx);
                    if let Some(item_id) = self
                        .reducer_state
                        .stream_runs
                        .get(&run_id)
                        .and_then(|state| state.assistant_item_id.clone())
                    {
                        self.update_thread_item(
                            conv_id.clone(),
                            &item_id,
                            |item| {
                                if let ThreadItem::AssistantMessage { streaming, .. } = item {
                                    *streaming = false;
                                }
                            },
                            cx,
                        );
                    }
                    let (tool_status, reasoning_status) = match status {
                        RunStatus::Cancelled => (AgentStatus::Failed, AgentStatus::Completed),
                        RunStatus::Failed => (AgentStatus::Failed, AgentStatus::Failed),
                        _ => (AgentStatus::Completed, AgentStatus::Completed),
                    };
                    self.finalize_inflight_thread_items(&conv_id, tool_status, reasoning_status);
                    self.complete_reasoning(&run_id, &conv_id, cx);
                    self.running_conversations.remove(&conv_id);
                    if run_depth == 0 {
                        self.finish_plan_execution_for_run_status(&conv_id, &status);
                        self.sync_plan_status_for_conversation(&conv_id, cx);
                    }
                }
                self.reducer_state.run_conversations.remove(&run_id);
                self.reducer_state.run_depths.remove(&run_id);
                self.reducer_state.child_parent_calls.remove(&run_id);
                self.reducer_state.stream_runs.remove(&run_id);
                self.reducer_state.subagent_items.remove(&run_id);
                if run_depth == 0 {
                    self.reducer_state.tool_items.clear();
                    self.reducer_state.tool_dedupe_keys.clear();
                }
                if !matches!(status, RunStatus::PausedForApproval) {
                    self.pending_approval_id = None;
                    self.diff_panel.pending_approval = None;
                    self.pending_thread_approval = None;
                    self.sync_thread_approval_state(cx);
                }
                if run_depth == 0 {
                    self.active_run_id = None;
                }
                self.sync_agent_status(match status {
                    RunStatus::Completed => AgentStatus::Completed,
                    RunStatus::Cancelled => AgentStatus::Idle,
                    RunStatus::Failed => AgentStatus::Failed,
                    RunStatus::PausedForApproval => AgentStatus::WaitingApproval,
                    RunStatus::Running => AgentStatus::Thinking,
                });
            }
            AgentEvent::RunFailed { run_id, error, .. } => {
                let run_depth = self
                    .reducer_state
                    .run_depths
                    .get(&run_id)
                    .copied()
                    .unwrap_or(0);
                let sweep_conv = self
                    .reducer_state
                    .run_conversations
                    .get(&run_id)
                    .cloned()
                    .or_else(|| conv_id);
                if let Some(conv_id) = sweep_conv.clone() {
                    self.finalize_inflight_thread_items(
                        &conv_id,
                        AgentStatus::Failed,
                        AgentStatus::Failed,
                    );
                    self.complete_reasoning(&run_id, &conv_id, cx);
                    self.push_thread_item(
                        conv_id.clone(),
                        ThreadItem::RunError {
                            id: format!("error-{}", uuid_simple()),
                            message: error.message,
                            session_ref: Some(run_id.0.clone()),
                            retryable: error.recoverable,
                        },
                        cx,
                    );
                    self.running_conversations.remove(&conv_id);
                    if run_depth == 0 {
                        self.mark_plan_execution_failed(&conv_id);
                        self.sync_plan_status_for_conversation(&conv_id, cx);
                    }
                }
                self.reducer_state.run_conversations.remove(&run_id);
                self.reducer_state.run_depths.remove(&run_id);
                self.reducer_state.child_parent_calls.remove(&run_id);
                self.reducer_state.stream_runs.remove(&run_id);
                self.reducer_state.subagent_items.remove(&run_id);
                if run_depth == 0 {
                    self.reducer_state.tool_items.clear();
                    self.reducer_state.tool_dedupe_keys.clear();
                }
                self.pending_approval_id = None;
                self.diff_panel.pending_approval = None;
                self.pending_thread_approval = None;
                self.sync_thread_approval_state(cx);
                self.sync_agent_status(AgentStatus::Failed);
                if run_depth == 0 {
                    self.active_run_id = None;
                }
            }
        }
    }

    pub fn replay_session_events(&mut self, session_key: &str, cx: &mut Context<Self>) {
        let session = agent_protocol::SessionId::new(session_key);
        let conv_id = ConversationId(session_key.to_string());
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
            conv.thread_items.clear();
            conv.active_todos.clear();
            conv.plan_artifact = None;
        }
        self.thread_item_indices.remove(&conv_id);
        self.subagent_transcripts.clear();
        self.subagent_by_parent_call.clear();
        let events = self
            .agent_bridge
            .runtime
            .store
            .load_session_events(&session)
            .unwrap_or_default();
        self.reducer_state = ReducerState::default();
        for stored in events {
            self.apply_agent_event_inner(stored.event, cx);
        }
        self.rebuild_thread_item_index(&conv_id);
        self.rebuild_subagent_projections(&conv_id);
        self.sync_thread_view(conv_id, cx);
        cx.notify();
    }
}

/// Streaming thread deltas update `ThreadView` directly — skip full-window repaint.
/// Also skip tool-call lifecycle events: they push/patch items through the thread
/// bridge and any needed window-level updates (status, diff panel) are handled in
/// the event handler itself.
fn event_needs_window_notify(event: &AgentEvent) -> bool {
    !matches!(
        event,
        AgentEvent::AssistantTextDelta { .. }
            | AgentEvent::ReasoningDelta { .. }
            | AgentEvent::ToolOutputDelta { .. }
            | AgentEvent::ToolCallUpdated { .. }
            | AgentEvent::ToolCallStarted { .. }
            | AgentEvent::ToolCallFinished { .. }
            | AgentEvent::TodoUpdated { .. }
            | AgentEvent::PatchPreviewUpdated { .. }
            | AgentEvent::AndroidSessionUpdated { .. }
            | AgentEvent::AndroidObservationUpdated { .. }
            | AgentEvent::AndroidActionPreviewed { .. }
            | AgentEvent::AndroidActionCompleted { .. }
            | AgentEvent::AndroidJourneyUpdated { .. }
            | AgentEvent::UsageUpdated { .. }
    )
}

fn map_context_entry_kind(
    kind: agent_protocol::ContextEntryKind,
) -> crate::features::shell::state::ContextEntryKind {
    use crate::features::shell::state::ContextEntryKind as S;
    match kind {
        agent_protocol::ContextEntryKind::RepoMap => S::RepoMap,
        agent_protocol::ContextEntryKind::FileSlice => S::FileSlice,
        agent_protocol::ContextEntryKind::Symbol => S::Symbol,
        agent_protocol::ContextEntryKind::Search => S::Search,
        agent_protocol::ContextEntryKind::Command => S::Command,
        agent_protocol::ContextEntryKind::Rule => S::Rule,
    }
}

fn map_risk(risk: agent_protocol::RiskLevel) -> ApprovalRisk {
    match risk {
        agent_protocol::RiskLevel::SafeRead | agent_protocol::RiskLevel::Low => ApprovalRisk::Low,
        agent_protocol::RiskLevel::Medium => ApprovalRisk::Medium,
        agent_protocol::RiskLevel::High => ApprovalRisk::High,
        agent_protocol::RiskLevel::Critical => ApprovalRisk::Critical,
    }
}

fn allow_always_label(command: &str) -> Option<String> {
    let mut parts = command.split_whitespace();
    let program = parts.next()?;
    let first_arg = parts.find(|part| !part.starts_with('-'));
    let label = match first_arg {
        Some(arg) => format!("Always allow {program} {arg}"),
        None => format!("Always allow {program}"),
    };
    Some(label)
}

fn format_tokens(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

fn uuid_simple() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_state_is_scoped_per_run() {
        let parent = RunId::new("parent");
        let child = RunId::new("child");
        let mut state = ReducerState::default();
        state.run_depths.insert(parent.clone(), 0);
        state.run_depths.insert(child.clone(), 1);
        state
            .child_parent_calls
            .insert(child.clone(), "delegate-call".into());

        state
            .stream_state_mut(&parent, Some(ConversationId("session".into())))
            .assistant_item_id = Some("assistant-parent".into());
        state
            .stream_state_mut(&child, Some(ConversationId("session".into())))
            .assistant_item_id = Some("assistant-child".into());

        let parent_state = state.stream_runs.get(&parent).expect("parent stream");
        let child_state = state.stream_runs.get(&child).expect("child stream");
        assert_eq!(
            parent_state.assistant_item_id.as_deref(),
            Some("assistant-parent")
        );
        assert_eq!(
            child_state.assistant_item_id.as_deref(),
            Some("assistant-child")
        );
        assert_eq!(child_state.depth, 1);
        assert_eq!(child_state.parent_call_id.as_deref(), Some("delegate-call"));
    }
}

#[allow(dead_code)]
fn parse_diff_panel(unified_diff: &str) -> DiffPanelState {
    let mut panel = DiffPanelState::default();
    panel.files = diff_panel::parse_unified_diff(unified_diff);
    panel
}

```
