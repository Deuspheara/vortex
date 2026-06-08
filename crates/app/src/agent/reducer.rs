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
