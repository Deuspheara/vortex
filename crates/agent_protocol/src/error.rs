use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentErrorView {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
}

#[derive(Clone, Debug)]
pub enum AgentError {
    LoopLimitExceeded,
    ToolCallLimitExceeded,
    RuntimeLimitExceeded,
    Cancelled,
    UnknownTool(String),
    ApprovalDenied(String),
    Provider(String),
    Store(String),
    Other(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoopLimitExceeded => write!(f, "loop limit exceeded"),
            Self::ToolCallLimitExceeded => write!(f, "tool call limit exceeded"),
            Self::RuntimeLimitExceeded => write!(f, "runtime limit exceeded"),
            Self::Cancelled => write!(f, "run cancelled"),
            Self::UnknownTool(name) => write!(f, "unknown tool: {name}"),
            Self::ApprovalDenied(reason) => write!(f, "approval denied: {reason}"),
            Self::Provider(msg) => write!(f, "provider error: {msg}"),
            Self::Store(msg) => write!(f, "store error: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AgentError {}
