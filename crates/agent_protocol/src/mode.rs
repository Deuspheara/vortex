use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    ChatOnly,
    ReadOnlyInspect,
    PlanOnly,
    SuggestPatch,
    ApplyWithApproval,
    AutoSafe,
    FullAccessDangerous,
}

impl Default for AgentMode {
    fn default() -> Self {
        Self::ApplyWithApproval
    }
}

impl AgentMode {
    pub fn can_read_files(&self) -> bool {
        !matches!(self, Self::ChatOnly)
    }

    pub fn can_run_virtual_bash(&self) -> bool {
        matches!(
            self,
            Self::ReadOnlyInspect
                | Self::PlanOnly
                | Self::SuggestPatch
                | Self::ApplyWithApproval
                | Self::AutoSafe
                | Self::FullAccessDangerous
        )
    }

    pub fn can_propose_patches(&self) -> bool {
        matches!(
            self,
            Self::SuggestPatch
                | Self::ApplyWithApproval
                | Self::AutoSafe
                | Self::FullAccessDangerous
        )
    }

    pub fn can_apply_patches(&self) -> bool {
        matches!(
            self,
            Self::ApplyWithApproval | Self::AutoSafe | Self::FullAccessDangerous
        )
    }

    pub fn can_run_real_commands(&self) -> bool {
        matches!(
            self,
            Self::ApplyWithApproval | Self::AutoSafe | Self::FullAccessDangerous
        )
    }

    /// Agent mode applies proposed patches without pausing for explicit user approval.
    pub fn auto_applies_patches(&self) -> bool {
        matches!(self, Self::FullAccessDangerous)
    }
}
