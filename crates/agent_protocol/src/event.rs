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
