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
use crate::features::agent_activity::components::diff::{
    render_diff_file_line_row, render_diff_header_row,
};
use crate::features::agent_activity::components::reasoning::{
    render_reasoning_header_row, render_reasoning_preview_line_row,
};
use crate::features::agent_activity::components::tool_call::{
    render_tool_header_row, render_tool_output_line_row, render_tool_output_truncated_row,
};
use crate::features::agent_activity::components::{activity_header_row, activity_output_line_row};
use crate::features::chat::components::choice_card::choice_card;
use crate::features::chat::components::error_card::{ErrorCardProps, error_card};
use crate::features::chat::components::message::user_message;
use crate::features::chat::components::timeline_section::timeline_section_header;
use crate::features::chat::manifest::{
    RowRef, activity_group_pos, assistant_content_height, assistant_provenance_for_item,
    assistant_provenance_height, build_manifest_with_transcript, build_tool_output_preview,
    collapsed_item_header_changed, context_trace_counts_summary, context_trace_entry_line,
    manifest_span, phase_from_u8, reasoning_preview_line_text, row_height_with_collapsed,
    row_refs_for_item_with_mode, row_size, row_sizes_for_manifest, row_sizes_for_rows, row_top_gap,
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

    pub fn open_context_trace(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let agent = self.agent.clone();
        let target_id = item_id.to_string();
        agent.update(cx, move |window, cx| {
            window.open_context_trace(&target_id, cx);
        });
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
