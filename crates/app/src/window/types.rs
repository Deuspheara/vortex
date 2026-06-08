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
