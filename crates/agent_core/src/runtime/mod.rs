use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use agent_context::{
    ContextBuilder, ContextPacket, ModelContextState, classify_task, tool_pack_for_task,
};
use agent_models::{MockProvider, ModelProvider};
use agent_protocol::{
    AgentCommand, AgentError, AgentErrorView, AgentEvent, AgentMode, AgentRunLimits,
    ApprovalDecision, AssistantToolCall, CancellationToken, ContextAttachment, ModelDelta, ModelId,
    ModelMessage, ModelMessageRole, OutputStreamKind, PatchId, ProjectId, RiskLevel, RunId,
    RunStatus, SessionId, ToolCallId, ToolContext, ToolOutputSink, ToolResult, ToolStatus,
};
use agent_store::{EventStore, StoredApprovalRule, StoredRun, StoredSession};
use agent_tools::{
    BrowserMcpConfigState, ToolOrchestrator, ToolRegistry, args_preview, args_preview_raw,
    is_git_repo, task_visible_tool_specs, tool_finish_summary,
};
use chrono::Utc;
use flume::{Receiver, Sender};
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::ChannelEventSink;

#[derive(PartialEq)]
pub(crate) enum ToolLoopAction {
    Continue,
    Paused,
}

/// Tools whose `ToolResult` is a serialized [`agent_protocol::PatchProposal`] and therefore flow
/// through the propose → preview → apply + checkpoint pipeline (set `pending_patch`, emit
/// `PatchProposed`, pause for approval). Keeping this in one place avoids scattered match arms.
pub(crate) fn is_patch_proposing_tool(registry: &ToolRegistry, name: &str) -> bool {
    registry.has_category(name, agent_protocol::ToolCategory::PatchProposal)
}

/// Pure read-only tools with no approval/pause semantics, so a batch from one model turn can be
/// executed concurrently without ordering hazards.
pub(crate) fn is_parallel_safe_tool(registry: &ToolRegistry, name: &str) -> bool {
    registry.is_parallel_safe(name)
}

pub struct AgentRuntimeConfig {
    pub checkpoint_dir: PathBuf,
    pub browser_mcp_config: BrowserMcpConfigState,
    pub limits: AgentRunLimits,
}

pub struct AgentRuntime {
    pub store: Arc<dyn EventStore>,
    pub provider: Arc<dyn ModelProvider>,
    pub tools: Arc<ToolOrchestrator>,
    pub context_builder: ContextBuilder,
    pub limits: AgentRunLimits,
    pub event_tx: Sender<AgentEvent>,
    pub command_tx: Sender<AgentCommand>,
    active_runs: Arc<Mutex<HashMap<RunId, ActiveRun>>>,
    /// Approvals awaiting user action, keyed by approval id (survives brief run state churn).
    pending_approvals: Arc<Mutex<HashMap<String, (RunId, PendingApprovalState)>>>,
}

struct ActiveRun {
    session_id: SessionId,
    project_id: ProjectId,
    _parent_run_id: Option<RunId>,
    depth: u8,
    project_root: PathBuf,
    mode: AgentMode,
    model: ModelId,
    subagent_model: Option<ModelId>,
    cancel: CancellationToken,
    prompt: String,
    attachments: Vec<ContextAttachment>,
    message_history: Vec<ModelMessage>,
    model_context_state: ModelContextState,
    task_class: agent_protocol::TaskClass,
    tool_pack: agent_protocol::ToolPack,
    tool_results: Vec<ToolResult>,
    tool_call_count: usize,
    started_at: Instant,
    pending_approval: Option<PendingApprovalState>,
    announced_tools: HashSet<ToolCallId>,
    in_flight_tools: HashSet<ToolCallId>,
    /// Per-run cache of read_file results keyed by request path.
    read_cache: HashMap<String, String>,
    read_cache_hits: usize,
    /// Last proposed patch awaiting explicit Apply from the user.
    pending_patch: Option<agent_protocol::PatchProposal>,
    /// Persistent todo/plan list maintained by the `todo_write` tool.
    todos: Vec<agent_protocol::TodoItem>,
    /// Outstanding `ask_user` choice awaiting a `SubmitChoice` command.
    pending_choice: Option<PendingChoiceState>,
    android_lane: AndroidExecutionLane,
}

#[derive(Clone, Debug, Default)]
struct AndroidExecutionLane {
    last_observation_id: Option<String>,
    action_since_observation: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ReadCacheStats {
    pub entries: usize,
    pub bytes: usize,
    pub hits: usize,
}

#[derive(Clone)]
struct PendingChoiceState {
    choice_id: String,
    call_id: ToolCallId,
    tool_name: String,
    options: Vec<agent_protocol::ChoiceOption>,
}

#[derive(Clone)]
struct PendingApprovalState {
    approval_id: agent_protocol::ApprovalId,
    call_id: ToolCallId,
    tool_name: String,
    arguments: serde_json::Value,
    risk: RiskLevel,
    command_pattern: Option<String>,
}

mod context;
mod delegate;
mod run_loop;
mod tool_exec;
mod tool_finish;

impl AgentRuntime {
    pub fn new(
        store: Arc<dyn EventStore>,
        provider: Arc<dyn ModelProvider>,
        config: AgentRuntimeConfig,
    ) -> (Self, Receiver<AgentEvent>, Receiver<AgentCommand>) {
        let (event_tx, event_rx) = flume::unbounded();
        let (command_tx, command_rx) = flume::unbounded();
        let registry = Arc::new(ToolRegistry::new(
            config.checkpoint_dir,
            config.browser_mcp_config,
        ));
        let tools = Arc::new(ToolOrchestrator {
            registry,
            store: store.clone(),
        });
        let runtime = Self {
            store,
            provider,
            tools,
            context_builder: ContextBuilder::default(),
            limits: config.limits,
            event_tx,
            command_tx,
            active_runs: Arc::new(Mutex::new(HashMap::new())),
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        };
        (runtime, event_rx, command_rx)
    }

    pub fn tool_catalog(&self) -> Vec<agent_protocol::ToolDescriptor> {
        self.tools.registry.catalog()
    }

    pub async fn read_cache_stats_for_session(&self, session_id: &SessionId) -> ReadCacheStats {
        let runs = self.active_runs.lock().await;
        runs.values()
            .find(|run| &run.session_id == session_id)
            .map(|run| ReadCacheStats {
                entries: run.read_cache.len(),
                bytes: run.read_cache.values().map(|value| value.len()).sum(),
                hits: run.read_cache_hits,
            })
            .unwrap_or_default()
    }

    pub fn tool_row_label(&self, name: &str, command: Option<&str>, running: bool) -> String {
        self.tools.registry.row_label(name, command, running)
    }

    pub fn tool_icon(&self, name: &str) -> Option<agent_protocol::IconToken> {
        self.tools.registry.descriptor(name).map(|d| d.icon)
    }

    pub fn with_mock(
        store: Arc<dyn EventStore>,
        config: AgentRuntimeConfig,
    ) -> (Self, Receiver<AgentEvent>, Receiver<AgentCommand>) {
        Self::new(store, Arc::new(MockProvider::default()), config)
    }

    pub fn spawn(
        self: Arc<Self>,
        handle: tokio::runtime::Handle,
        command_rx: Receiver<AgentCommand>,
    ) {
        let worker = handle.clone();
        handle.spawn(async move {
            while let Ok(command) = command_rx.recv_async().await {
                let runtime = self.clone();
                worker.spawn(async move {
                    if let Err(err) = runtime.handle_command(command.clone()).await {
                        if let AgentCommand::StartRun { session_id, .. } = &command {
                            runtime.emit_pre_run_failure(session_id.clone(), err);
                        } else {
                            tracing::error!("agent command failed: {err}");
                            runtime.emit_command_failed(&command, err);
                        }
                    }
                });
            }
        });
    }

    fn emit_pre_run_failure(&self, session_id: SessionId, err: AgentError) {
        let run_id = RunId::new(format!("failed-{}", uuid::Uuid::new_v4()));
        let _ = self.event_tx.send(AgentEvent::RunFailed {
            run_id,
            error: AgentErrorView {
                code: "command_failed".into(),
                message: format!("{err} (session {session_id})"),
                recoverable: true,
            },
        });
    }

    fn emit_command_failed(&self, command: &AgentCommand, err: AgentError) {
        let _ = command;
        let _ = self.event_tx.send(AgentEvent::CommandFailed {
            message: err.to_string(),
        });
    }

    async fn clear_pending_approvals_for_run(&self, run_id: &RunId) {
        let mut map = self.pending_approvals.lock().await;
        map.retain(|_, (rid, _)| rid != run_id);
    }

    pub async fn handle_command(&self, command: AgentCommand) -> Result<(), AgentError> {
        match &command {
            AgentCommand::StartRun {
                session_id,
                model,
                mode,
                prompt,
                ..
            } => tracing::info!(
                command = "StartRun",
                session_id = %session_id.0,
                model = %model.0,
                mode = ?mode,
                prompt_len = prompt.len(),
                "agent command received"
            ),
            AgentCommand::CancelRun { run_id } => {
                tracing::info!(command = "CancelRun", run_id = %run_id.0, "agent command received")
            }
            AgentCommand::ApproveTool { approval_id } => {
                tracing::info!(
                    command = "ApproveTool",
                    approval_id = %approval_id.0,
                    "agent command received"
                )
            }
            AgentCommand::ApproveToolAlways { approval_id } => {
                tracing::info!(
                    command = "ApproveToolAlways",
                    approval_id = %approval_id.0,
                    "agent command received"
                )
            }
            AgentCommand::RejectTool {
                approval_id,
                reason,
            } => tracing::info!(
                command = "RejectTool",
                approval_id = %approval_id.0,
                reason = ?reason,
                "agent command received"
            ),
            AgentCommand::SubmitChoice {
                choice_id,
                option_id,
            } => tracing::info!(
                command = "SubmitChoice",
                choice_id = %choice_id,
                option_id = %option_id,
                "agent command received"
            ),
            AgentCommand::RollbackCheckpoint { checkpoint_id } => {
                tracing::info!(
                    command = "RollbackCheckpoint",
                    checkpoint_id = %checkpoint_id.0,
                    "agent command received"
                )
            }
            _ => tracing::info!(command = "other", "agent command received"),
        }

        match command {
            AgentCommand::StartRun {
                project_id,
                session_id,
                prompt,
                model,
                subagent_model,
                mode,
                attachments,
            } => {
                self.start_run(
                    project_id,
                    session_id,
                    prompt,
                    model,
                    subagent_model,
                    mode,
                    attachments,
                )
                .await
            }
            AgentCommand::CancelRun { run_id } => self.cancel_run(&run_id).await,
            AgentCommand::ApproveTool { approval_id } => {
                self.resolve_approval(&approval_id, true, false, None).await
            }
            AgentCommand::ApproveToolAlways { approval_id } => {
                self.resolve_approval(&approval_id, true, true, None).await
            }
            AgentCommand::RejectTool {
                approval_id,
                reason,
            } => {
                self.resolve_approval(&approval_id, false, false, reason)
                    .await
            }
            AgentCommand::SubmitChoice {
                choice_id,
                option_id,
            } => self.submit_choice(&choice_id, &option_id).await,
            AgentCommand::ApprovePatch { patch_id, scope: _ } => {
                self.apply_approved_patch(&patch_id).await
            }
            AgentCommand::RejectPatch { patch_id, reason } => {
                self.reject_patch(&patch_id, reason).await
            }
            AgentCommand::CompactSession { session_id } => self.compact_session(&session_id).await,
            AgentCommand::RollbackCheckpoint { checkpoint_id } => {
                self.rollback_checkpoint(&checkpoint_id).await
            }
            _ => Ok(()),
        }
    }

    /// Reconstruct a bounded message history from the session event log so a new run inherits the
    /// prior conversation (cross-run memory). Uses plain assistant/system messages (no dangling
    /// tool-call pairing) and compacts to half the history budget to leave room for fresh activity.
    fn hydrate_history(&self, session_id: &SessionId) -> Vec<ModelMessage> {
        let events = self
            .store
            .load_session_events(session_id)
            .unwrap_or_default();
        let mut messages: Vec<ModelMessage> = Vec::new();
        let mut assistant_buf = String::new();
        let flush = |buf: &mut String, out: &mut Vec<ModelMessage>| {
            if !buf.trim().is_empty() {
                out.push(ModelMessage {
                    role: ModelMessageRole::Assistant,
                    content: std::mem::take(buf).into(),
                    tool_call_id: None,
                    name: None,
                    tool_calls: None,
                });
            } else {
                buf.clear();
            }
        };
        for stored in events {
            match stored.event {
                AgentEvent::AssistantTextDelta { text, .. } => assistant_buf.push_str(&text),
                AgentEvent::ToolCallFinished { summary, .. } => {
                    flush(&mut assistant_buf, &mut messages);
                    if !summary.trim().is_empty() {
                        messages.push(ModelMessage {
                            role: ModelMessageRole::System,
                            content: format!("[earlier tool result] {summary}").into(),
                            tool_call_id: None,
                            name: None,
                            tool_calls: None,
                        });
                    }
                }
                AgentEvent::RunFinished { .. } | AgentEvent::RunFailed { .. } => {
                    flush(&mut assistant_buf, &mut messages);
                }
                _ => {}
            }
        }
        flush(&mut assistant_buf, &mut messages);
        let budget = (self.context_builder.budget.max_history_tokens / 2).max(1);
        agent_context::compact_history(&messages, budget)
    }

    /// Compact the active run's in-memory history for a session, freeing context budget.
    async fn compact_session(&self, session_id: &SessionId) -> Result<(), AgentError> {
        let run_id = {
            let runs = self.active_runs.lock().await;
            runs.iter()
                .find(|(_, run)| &run.session_id == session_id)
                .map(|(rid, _)| rid.clone())
        };
        let Some(run_id) = run_id else {
            return Ok(());
        };
        let token_estimate = {
            let mut runs = self.active_runs.lock().await;
            let Some(run) = runs.get_mut(&run_id) else {
                return Ok(());
            };
            let budget = (self.context_builder.budget.max_history_tokens / 2).max(1);
            run.message_history = agent_context::compact_history(&run.message_history, budget);
            run.message_history
                .iter()
                .map(agent_context::message_tokens)
                .sum()
        };
        let sink = self.sink();
        sink.emit(
            &run_id,
            AgentEvent::ContextBuilt {
                run_id: run_id.clone(),
                token_estimate,
                files: Vec::new(),
                summaries: vec!["session compacted".to_string()],
                section_estimates: vec![agent_protocol::ContextSectionEstimate {
                    name: "recent_turns".into(),
                    tokens: token_estimate,
                }],
            },
        )
        .await
        .map_err(|e| AgentError::Store(e))?;
        Ok(())
    }

    async fn rollback_checkpoint(
        &self,
        checkpoint_id: &agent_protocol::CheckpointId,
    ) -> Result<(), AgentError> {
        let checkpoint = self
            .store
            .get_checkpoint(checkpoint_id)
            .map_err(|e| AgentError::Store(e))?
            .ok_or_else(|| AgentError::Other("checkpoint not found".into()))?;

        for snapshot in &checkpoint.file_snapshots {
            if snapshot.old_content_path.exists() {
                if let Some(parent) = snapshot.path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| AgentError::Other(e.to_string()))?;
                }
                std::fs::copy(&snapshot.old_content_path, &snapshot.path)
                    .map_err(|e| AgentError::Other(e.to_string()))?;
            }
        }
        Ok(())
    }

    async fn start_run(
        &self,
        project_id: ProjectId,
        session_id: SessionId,
        prompt: String,
        model: ModelId,
        subagent_model: Option<ModelId>,
        mode: AgentMode,
        attachments: Vec<ContextAttachment>,
    ) -> Result<(), AgentError> {
        let project = self
            .store
            .get_project(&project_id)
            .map_err(|e| AgentError::Store(e))?
            .ok_or_else(|| AgentError::Other("project not found".into()))?;

        let run_id = RunId::new(uuid::Uuid::new_v4().to_string());
        let cancel = CancellationToken::new();
        let hydrated_history = self.hydrate_history(&session_id);
        let task_class = classify_task(&prompt);
        let tool_pack = tool_pack_for_task(task_class);
        tracing::info!(
            run_id = %run_id.0,
            session_id = %session_id.0,
            model = %model.0,
            mode = ?mode,
            prompt_len = prompt.len(),
            "starting agent run"
        );
        self.store
            .create_run(&StoredRun {
                id: run_id.clone(),
                session_id: session_id.clone(),
                parent_run_id: None,
                depth: 0,
                model: model.clone(),
                mode: mode.clone(),
                status: RunStatus::Running,
                started_at: Utc::now(),
                finished_at: None,
            })
            .map_err(|e| AgentError::Store(e))?;

        self.active_runs.lock().await.insert(
            run_id.clone(),
            ActiveRun {
                session_id: session_id.clone(),
                project_id: project_id.clone(),
                _parent_run_id: None,
                depth: 0,
                project_root: PathBuf::from(&project.root_path),
                mode,
                model: model.clone(),
                subagent_model,
                cancel: cancel.clone(),
                prompt: prompt.clone(),
                attachments: attachments.clone(),
                message_history: hydrated_history,
                model_context_state: ModelContextState::new(&prompt),
                task_class,
                tool_pack,
                tool_results: Vec::new(),
                tool_call_count: 0,
                started_at: Instant::now(),
                pending_approval: None,
                announced_tools: HashSet::new(),
                in_flight_tools: HashSet::new(),
                read_cache: HashMap::new(),
                read_cache_hits: 0,
                pending_patch: None,
                todos: Vec::new(),
                pending_choice: None,
                android_lane: AndroidExecutionLane::default(),
            },
        );

        let sink = self.sink();
        sink.emit(
            &run_id,
            AgentEvent::RunStarted {
                run_id: run_id.clone(),
                session_id,
                model,
                mode: self
                    .active_runs
                    .lock()
                    .await
                    .get(&run_id)
                    .map(|r| r.mode.clone())
                    .unwrap_or_default(),
                depth: 0,
                parent_run_id: None,
            },
        )
        .await
        .map_err(|e| AgentError::Store(e))?;

        if let Err(e) = self.run_loop(&run_id, prompt, attachments, cancel).await {
            self.fail_run(&run_id, e).await?;
        }
        Ok(())
    }

    async fn append_assistant_text(&self, run_id: &RunId, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
            run.message_history.push(ModelMessage {
                role: ModelMessageRole::Assistant,
                content: text.to_string().into(),
                tool_call_id: None,
                name: None,
                tool_calls: None,
            });
        }
    }

    async fn append_assistant_tool_call(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: serde_json::Value,
    ) {
        if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
            run.message_history.push(ModelMessage {
                role: ModelMessageRole::Assistant,
                content: String::new().into(),
                tool_call_id: None,
                name: None,
                tool_calls: Some(vec![AssistantToolCall {
                    id: call_id.clone(),
                    name: name.to_string(),
                    arguments,
                }]),
            });
        }
    }

    async fn append_tool_message(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        name: &str,
        content: &str,
        _is_error: bool,
    ) {
        if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
            run.message_history.push(ModelMessage {
                role: ModelMessageRole::Tool,
                content: content.to_string().into(),
                tool_call_id: Some(call_id.clone()),
                name: Some(name.to_string()),
                tool_calls: None,
            });
        }
    }

    async fn resume_run_loop(&self, run_id: &RunId) -> Result<(), AgentError> {
        let (prompt, attachments, cancel) = {
            let runs = self.active_runs.lock().await;
            let run = runs
                .get(run_id)
                .ok_or(AgentError::Other("run not found".into()))?;
            (
                run.prompt.clone(),
                run.attachments.clone(),
                run.cancel.clone(),
            )
        };
        self.run_loop(run_id, prompt, attachments, cancel).await
    }

    async fn take_pending_approval(
        &self,
        approval_id: &agent_protocol::ApprovalId,
    ) -> Option<(RunId, PendingApprovalState)> {
        let mut map = self.pending_approvals.lock().await;
        if let Some(entry) = map.remove(&approval_id.0) {
            let mut runs = self.active_runs.lock().await;
            if let Some(run) = runs.get_mut(&entry.0) {
                if run
                    .pending_approval
                    .as_ref()
                    .is_some_and(|p| p.approval_id == *approval_id)
                {
                    run.pending_approval = None;
                }
            }
            return Some(entry);
        }
        drop(map);
        let mut runs = self.active_runs.lock().await;
        for (rid, run) in runs.iter_mut() {
            if run
                .pending_approval
                .as_ref()
                .is_some_and(|p| p.approval_id == *approval_id)
            {
                return Some((rid.clone(), run.pending_approval.take().unwrap()));
            }
        }
        None
    }

    async fn resolve_approval(
        &self,
        approval_id: &agent_protocol::ApprovalId,
        approved: bool,
        remember: bool,
        reason: Option<String>,
    ) -> Result<(), AgentError> {
        let Some((run_id, pending)) = self.take_pending_approval(approval_id).await else {
            tracing::debug!(
                approval_id = %approval_id.0,
                "approval already resolved (duplicate ApproveTool)"
            );
            return Ok(());
        };

        if !approved {
            let denial_reason = reason.unwrap_or_else(|| "denied by user".into());
            let sink = self.sink();
            sink.emit(
                &run_id,
                AgentEvent::ToolCallFinished {
                    run_id: Some(run_id.clone()),
                    call_id: pending.call_id.clone(),
                    status: ToolStatus::Denied,
                    summary: denial_reason.clone(),
                    body: None,
                },
            )
            .await
            .map_err(|e| AgentError::Store(e))?;
            if let Some(run) = self.active_runs.lock().await.get_mut(&run_id) {
                run.in_flight_tools.remove(&pending.call_id);
            }
            self.append_tool_message(
                &run_id,
                &pending.call_id,
                &pending.tool_name,
                &denial_reason,
                true,
            )
            .await;
            self.store
                .update_run_status(&run_id, RunStatus::Running, None)
                .map_err(|e| AgentError::Store(e))?;
            return self.resume_run_loop(&run_id).await;
        }

        let (project_root, project_id, session_id, mode) = {
            let runs = self.active_runs.lock().await;
            let run = runs
                .get(&run_id)
                .ok_or(AgentError::Other("run missing".into()))?;
            (
                run.project_root.clone(),
                run.project_id.clone(),
                run.session_id.clone(),
                run.mode.clone(),
            )
        };

        if remember {
            self.store
                .save_approval_rule(&StoredApprovalRule {
                    project_id: project_id.clone(),
                    tool_name: pending.tool_name.clone(),
                    command_pattern: pending.command_pattern.clone(),
                    path_prefix: None,
                    max_risk: pending.risk,
                    expires_at: None,
                })
                .map_err(AgentError::Store)?;
        }

        self.execute_tool_call(
            &run_id,
            &pending.call_id,
            &pending.tool_name,
            pending.arguments,
            &project_root,
            &project_id,
            &session_id,
            &mode,
        )
        .await?;
        self.store
            .update_run_status(&run_id, RunStatus::Running, None)
            .map_err(|e| AgentError::Store(e))?;
        self.resume_run_loop(&run_id).await
    }

    async fn apply_approved_patch(&self, patch_id: &PatchId) -> Result<(), AgentError> {
        let (run_id, proposal) = {
            let mut runs = self.active_runs.lock().await;
            let mut found = None;
            for (rid, run) in runs.iter_mut() {
                if run
                    .pending_patch
                    .as_ref()
                    .is_some_and(|p| &p.id == patch_id)
                {
                    found = Some((rid.clone(), run.pending_patch.take().unwrap()));
                    break;
                }
            }
            found.ok_or_else(|| AgentError::Other("patch not found or already applied".into()))?
        };

        let (project_root, project_id, session_id, mode) = {
            let runs = self.active_runs.lock().await;
            let run = runs
                .get(&run_id)
                .ok_or(AgentError::Other("run not found".into()))?;
            (
                run.project_root.clone(),
                run.project_id.clone(),
                run.session_id.clone(),
                run.mode.clone(),
            )
        };

        let call_id = agent_protocol::ToolCallId::new(uuid::Uuid::new_v4().to_string());
        let args = serde_json::json!({ "unified_diff": proposal.unified_diff });
        tracing::info!(
            run_id = %run_id.0,
            patch_id = %patch_id.0,
            "applying approved patch"
        );
        self.execute_tool_call(
            &run_id,
            &call_id,
            "apply_patch",
            args,
            &project_root,
            &project_id,
            &session_id,
            &mode,
        )
        .await?;
        self.store
            .update_run_status(&run_id, RunStatus::Running, None)
            .map_err(|e| AgentError::Store(e))?;
        self.resume_run_loop(&run_id).await
    }

    /// Resolve an outstanding `ask_user` choice: finish the tool call with the picked option as
    /// its result, append it to history, and resume the paused run.
    async fn submit_choice(&self, choice_id: &str, option_id: &str) -> Result<(), AgentError> {
        let found = {
            let mut runs = self.active_runs.lock().await;
            let mut hit = None;
            for (rid, run) in runs.iter_mut() {
                if run
                    .pending_choice
                    .as_ref()
                    .is_some_and(|c| c.choice_id == choice_id)
                {
                    hit = Some((rid.clone(), run.pending_choice.take().unwrap()));
                    break;
                }
            }
            hit
        };
        let Some((run_id, pending)) = found else {
            tracing::debug!(choice_id, "choice already resolved or unknown");
            return Ok(());
        };

        let label = pending
            .options
            .iter()
            .find(|o| o.id == option_id)
            .map(|o| o.label.clone())
            .unwrap_or_else(|| option_id.to_string());
        let output = format!("User selected: {label} (id: {option_id})");
        let result = ToolResult {
            call_id: pending.call_id.clone(),
            name: pending.tool_name.clone(),
            output,
            is_error: false,
        };
        self.finish_tool(&run_id, &pending.call_id, &serde_json::Value::Null, &result)
            .await?;
        self.store
            .update_run_status(&run_id, RunStatus::Running, None)
            .map_err(|e| AgentError::Store(e))?;
        self.resume_run_loop(&run_id).await
    }

    /// Discard a proposed-but-unapplied patch: clear `pending_patch`, tell the model it was
    /// rejected (so it can adapt), and resume the paused run. Nothing is written to disk.
    async fn reject_patch(
        &self,
        patch_id: &PatchId,
        reason: Option<String>,
    ) -> Result<(), AgentError> {
        let run_id = {
            let mut runs = self.active_runs.lock().await;
            let mut hit = None;
            for (rid, run) in runs.iter_mut() {
                if run
                    .pending_patch
                    .as_ref()
                    .is_some_and(|p| &p.id == patch_id)
                {
                    run.pending_patch = None;
                    hit = Some(rid.clone());
                    break;
                }
            }
            hit
        };
        let Some(run_id) = run_id else {
            tracing::debug!(patch_id = %patch_id.0, "reject for unknown/already-resolved patch");
            return Ok(());
        };

        let note = match reason {
            Some(r) if !r.is_empty() => {
                format!("The user rejected the proposed patch. Reason: {r}")
            }
            _ => {
                "The user rejected the proposed patch. Do not re-apply it; reconsider.".to_string()
            }
        };
        if let Some(run) = self.active_runs.lock().await.get_mut(&run_id) {
            run.message_history.push(ModelMessage {
                role: ModelMessageRole::User,
                content: note.into(),
                tool_call_id: None,
                name: None,
                tool_calls: None,
            });
        }
        self.store
            .update_run_status(&run_id, RunStatus::Running, None)
            .map_err(|e| AgentError::Store(e))?;
        self.resume_run_loop(&run_id).await
    }

    async fn cancel_run(&self, run_id: &RunId) -> Result<(), AgentError> {
        if let Some(run) = self.active_runs.lock().await.get(run_id) {
            run.cancel.cancel();
        }
        self.finalize_inflight_tools(run_id, ToolStatus::Cancelled, "Run cancelled")
            .await;
        let sink = self.sink();
        sink.emit(
            run_id,
            AgentEvent::RunFinished {
                run_id: run_id.clone(),
                status: RunStatus::Cancelled,
            },
        )
        .await
        .map_err(|e| AgentError::Store(e))?;
        self.store
            .update_run_status(run_id, RunStatus::Cancelled, Some(Utc::now()))
            .map_err(|e| AgentError::Store(e))?;
        self.clear_pending_approvals_for_run(run_id).await;
        self.active_runs.lock().await.remove(run_id);
        Ok(())
    }

    async fn fail_run(&self, run_id: &RunId, error: AgentError) -> Result<(), AgentError> {
        self.finalize_inflight_tools(run_id, ToolStatus::Failed, &error.to_string())
            .await;
        let sink = self.sink();
        sink.emit(
            run_id,
            AgentEvent::RunFailed {
                run_id: run_id.clone(),
                error: AgentErrorView {
                    code: "runtime".into(),
                    message: error.to_string(),
                    recoverable: false,
                },
            },
        )
        .await
        .map_err(|e| AgentError::Store(e))?;
        self.store
            .update_run_status(run_id, RunStatus::Failed, Some(Utc::now()))
            .map_err(|e| AgentError::Store(e))?;
        self.clear_pending_approvals_for_run(run_id).await;
        self.active_runs.lock().await.remove(run_id);
        Ok(())
    }

    async fn check_runtime_limit(&self, run_id: &RunId) -> Result<(), AgentError> {
        let runs = self.active_runs.lock().await;
        let run = runs
            .get(run_id)
            .ok_or(AgentError::Other("run not found".into()))?;
        if run.started_at.elapsed() > Duration::from_secs(self.limits.max_runtime_seconds) {
            drop(runs);
            return self
                .fail_run(run_id, AgentError::RuntimeLimitExceeded)
                .await;
        }
        if run.tool_call_count > self.limits.max_tool_calls {
            drop(runs);
            return self
                .fail_run(run_id, AgentError::ToolCallLimitExceeded)
                .await;
        }
        Ok(())
    }

    fn sink(&self) -> ChannelEventSink {
        ChannelEventSink::new(self.store.clone(), self.event_tx.clone())
    }

    async fn finalize_inflight_tools(&self, run_id: &RunId, status: ToolStatus, summary: &str) {
        let call_ids: Vec<ToolCallId> = {
            let mut runs = self.active_runs.lock().await;
            let Some(run) = runs.get_mut(run_id) else {
                return;
            };
            run.in_flight_tools.drain().collect()
        };
        if call_ids.is_empty() {
            return;
        }
        let sink = self.sink();
        for call_id in call_ids {
            let _ = sink
                .emit(
                    run_id,
                    AgentEvent::ToolCallFinished {
                        run_id: Some(run_id.clone()),
                        call_id,
                        status: status.clone(),
                        summary: summary.to_string(),
                        body: None,
                    },
                )
                .await;
        }
    }

    pub fn send_command(&self, command: AgentCommand) -> Result<(), AgentError> {
        self.command_tx
            .send(command)
            .map_err(|e| AgentError::Other(e.to_string()))
    }

    pub fn ensure_seed_data(&self, workspace_root: &PathBuf) -> Result<(), String> {
        let now = Utc::now();
        let project_id = ProjectId::new("proj-vortex");
        self.store.upsert_project(&agent_store::StoredProject {
            id: project_id.clone(),
            root_path: workspace_root.display().to_string(),
            name: "vortex".into(),
            trusted: false,
            created_at: now,
            updated_at: now,
        })?;
        let session_id = SessionId::new("sess-default");
        if self.store.get_session(&session_id)?.is_none() {
            self.store.create_session(&StoredSession {
                id: session_id,
                project_id,
                title: "Default session".into(),
                created_at: now,
                updated_at: now,
            })?;
        }
        Ok(())
    }
}

const TOOL_OUTPUT_CHUNK: usize = 512;

async fn emit_tool_output_live(
    tx: &Sender<AgentEvent>,
    run_id: &RunId,
    call_id: &ToolCallId,
    stream: OutputStreamKind,
    output: &str,
) {
    if output.is_empty() {
        return;
    }
    let mut offset = 0;
    while offset < output.len() {
        let mut end = (offset + TOOL_OUTPUT_CHUNK).min(output.len());
        while end > offset && !output.is_char_boundary(end) {
            end -= 1;
        }
        let _ = tx.send(AgentEvent::ToolOutputDelta {
            run_id: Some(run_id.clone()),
            call_id: call_id.clone(),
            stream: stream.clone(),
            chunk: output[offset..end].to_string(),
        });
        offset = end;
        tokio::task::yield_now().await;
    }
}
