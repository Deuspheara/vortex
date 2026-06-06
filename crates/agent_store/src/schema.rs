use agent_protocol::{
    AgentEvent, EventId, ModelId, PatchId, ProjectId, RunId, RunStatus, SessionId, ToolCallId,
};
use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub struct StoredProject {
    pub id: ProjectId,
    pub root_path: String,
    pub name: String,
    pub trusted: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct StoredSession {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct StoredRun {
    pub id: RunId,
    pub session_id: SessionId,
    pub parent_run_id: Option<RunId>,
    pub depth: u8,
    pub model: ModelId,
    pub mode: agent_protocol::AgentMode,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct StoredEvent {
    pub id: EventId,
    pub run_id: RunId,
    pub sequence: i64,
    pub event: AgentEvent,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct StoredToolCall {
    pub id: ToolCallId,
    pub run_id: RunId,
    pub name: String,
    pub args_json: String,
    pub risk: agent_protocol::RiskLevel,
    pub status: agent_protocol::ToolStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct StoredPatchProposal {
    pub id: PatchId,
    pub run_id: RunId,
    pub base_git_sha: Option<String>,
    pub diff: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct StoredApprovalRule {
    pub project_id: ProjectId,
    pub tool_name: String,
    pub command_pattern: Option<String>,
    pub path_prefix: Option<String>,
    pub max_risk: agent_protocol::RiskLevel,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Stable SQLite primary key for a tool call within a run (provider ids may repeat across runs).
pub fn storage_tool_call_id(run_id: &RunId, call_id: &ToolCallId) -> ToolCallId {
    ToolCallId::new(format!("{}/{}", run_id.0, call_id.0))
}

pub trait EventStore: Send + Sync {
    fn init(&self) -> Result<(), String>;

    fn upsert_project(&self, project: &StoredProject) -> Result<(), String>;
    fn list_projects(&self) -> Result<Vec<StoredProject>, String>;
    fn get_project(&self, id: &ProjectId) -> Result<Option<StoredProject>, String>;

    fn create_session(&self, session: &StoredSession) -> Result<(), String>;
    fn update_session_title(&self, id: &SessionId, title: &str) -> Result<(), String>;
    fn delete_session(&self, session_id: &SessionId) -> Result<(), String>;
    fn list_sessions(&self, project_id: &ProjectId) -> Result<Vec<StoredSession>, String>;
    fn get_session(&self, id: &SessionId) -> Result<Option<StoredSession>, String>;

    fn delete_project(&self, project_id: &ProjectId) -> Result<(), String>;

    fn create_run(&self, run: &StoredRun) -> Result<(), String>;
    fn update_run_status(
        &self,
        id: &RunId,
        status: RunStatus,
        finished_at: Option<DateTime<Utc>>,
    ) -> Result<(), String>;
    fn get_run(&self, id: &RunId) -> Result<Option<StoredRun>, String>;
    fn active_run_for_session(&self, session_id: &SessionId) -> Result<Option<StoredRun>, String>;

    fn append_event(&self, event: &StoredEvent) -> Result<(), String>;
    /// Assign the next sequence and insert in one transaction (avoids concurrent duplicate keys).
    fn record_event(&self, run_id: &RunId, event: AgentEvent) -> Result<StoredEvent, String>;
    fn load_run_events(&self, run_id: &RunId) -> Result<Vec<StoredEvent>, String>;
    fn load_session_events(&self, session_id: &SessionId) -> Result<Vec<StoredEvent>, String>;
    fn next_event_sequence(&self, run_id: &RunId) -> Result<i64, String>;

    fn record_tool_call(&self, call: &StoredToolCall) -> Result<(), String>;
    fn update_tool_call(
        &self,
        id: &ToolCallId,
        status: agent_protocol::ToolStatus,
        finished_at: Option<DateTime<Utc>>,
    ) -> Result<(), String>;

    fn record_patch_proposal(&self, proposal: &StoredPatchProposal) -> Result<(), String>;
    fn update_patch_status(&self, id: &PatchId, status: &str) -> Result<(), String>;

    fn save_approval_rule(&self, rule: &StoredApprovalRule) -> Result<(), String>;
    fn list_approval_rules(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<StoredApprovalRule>, String>;

    fn save_checkpoint(
        &self,
        checkpoint: &agent_protocol::WorkspaceCheckpoint,
    ) -> Result<(), String>;
    fn get_checkpoint(
        &self,
        id: &agent_protocol::CheckpointId,
    ) -> Result<Option<agent_protocol::WorkspaceCheckpoint>, String>;
}
