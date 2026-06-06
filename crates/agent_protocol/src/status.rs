use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Running,
    PausedForApproval,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Denied,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputStreamKind {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchApprovalScope {
    All,
    File(String),
    Hunk { file: String, hunk_index: usize },
}
