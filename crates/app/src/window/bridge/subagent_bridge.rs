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
