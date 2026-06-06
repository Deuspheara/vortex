use serde::{Deserialize, Serialize};

use crate::{ToolCallId, ToolName};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkPolicy {
    Disabled,
    AllowGetToApprovedHosts(Vec<String>),
    AllowProjectConfigured,
    FullInternetWithApproval,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RealCommandRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub timeout_secs: u64,
    pub stdin: Option<String>,
    pub network_policy: NetworkPolicy,
    pub approval_id: Option<crate::ApprovalId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingToolCall {
    pub id: ToolCallId,
    pub name: ToolName,
    pub arguments: serde_json::Value,
    pub args_preview: String,
}
