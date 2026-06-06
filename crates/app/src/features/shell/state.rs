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
    Implementing,
    Completed,
    Stale,
}

impl PlanExecutionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "Not started",
            Self::Implementing => "Implementing",
            Self::Completed => "Completed",
            Self::Stale => "Stale",
        }
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct DiffHunk {
    pub old_start: usize,
    pub new_start: usize,
    pub rows: Vec<DiffRow>,
}

#[derive(Clone, Debug)]
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
