use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    SafeRead,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Allow,
    AskUser { reason: String, risk: RiskLevel },
    Deny { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkAccess {
    Disabled,
    AllowGet,
    AllowAll,
}
