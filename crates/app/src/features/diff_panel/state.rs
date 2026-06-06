//! Session run state — task header, tool row status, provider blocks.

use crate::features::shell::state::AgentStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SessionRunState {
    #[default]
    Idle,
    Planning,
    Running,
    WaitingApproval,
    BlockedError,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ToolState {
    Pending,
    Running,
    WaitingApproval,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderErrorVm {
    pub code: u16,
    pub message: String,
    pub retryable: bool,
}

impl SessionRunState {
    pub fn from_agent_status(status: &AgentStatus, blocked: bool) -> Self {
        if blocked {
            return Self::BlockedError;
        }
        match status {
            AgentStatus::Idle => Self::Idle,
            AgentStatus::Thinking => Self::Planning,
            AgentStatus::RunningTool => Self::Running,
            AgentStatus::WaitingApproval => Self::WaitingApproval,
            AgentStatus::Completed => Self::Done,
            AgentStatus::Failed => Self::Failed,
        }
    }

    #[allow(dead_code)]
    pub fn composer_placeholder(self, blocked: Option<&ProviderErrorVm>) -> &'static str {
        if let Some(err) = blocked {
            if err.code == 403 {
                return "Provider access blocked — open Settings to fix API key or model access";
            }
            return "Run blocked — open Settings or switch model";
        }
        match self {
            Self::WaitingApproval => "Waiting for your approval…",
            Self::BlockedError => "Run blocked — check provider settings",
            Self::Running | Self::Planning => "Agent is working…",
            _ => "Ask anything, @ mention, / actions",
        }
    }

    pub fn composer_dimmed(self) -> bool {
        matches!(self, Self::WaitingApproval | Self::BlockedError)
    }

    pub fn composer_disabled(self) -> bool {
        matches!(self, Self::BlockedError)
    }
}
