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
        cx.notify();
    }
}
