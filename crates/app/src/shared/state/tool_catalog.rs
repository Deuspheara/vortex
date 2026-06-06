use std::collections::HashMap;

use agent_protocol::{IconToken, ToolCapabilities, ToolCategory, ToolDescriptor};

/// UI-side lookup for tool presentation metadata (populated from the agent registry).
#[derive(Clone, Debug, Default)]
pub struct ToolCatalog {
    by_name: HashMap<String, ToolDescriptor>,
}

impl ToolCatalog {
    pub fn from_descriptors(descriptors: Vec<ToolDescriptor>) -> Self {
        let by_name = descriptors
            .into_iter()
            .map(|d| (d.name.clone(), d))
            .collect();
        Self { by_name }
    }

    pub fn icon(&self, name: &str) -> IconToken {
        self.by_name
            .get(normalized_tool_name(name))
            .map(|d| d.icon)
            .unwrap_or(IconToken::Terminal)
    }

    pub fn capabilities(&self, name: &str) -> ToolCapabilities {
        self.by_name
            .get(normalized_tool_name(name))
            .map(|d| d.capabilities)
            .unwrap_or_default()
    }

    pub fn has_category(&self, name: &str, category: ToolCategory) -> bool {
        self.capabilities(name).category == category
    }

    pub fn is_shell_tool(&self, name: &str) -> bool {
        matches!(
            self.capabilities(name).category,
            ToolCategory::VirtualCommand | ToolCategory::RealCommand
        )
    }

    pub fn is_patch_tool(&self, name: &str) -> bool {
        matches!(
            self.capabilities(name).category,
            ToolCategory::PatchProposal | ToolCategory::PatchApply
        )
    }

    pub fn suppresses_live_output(&self, name: &str) -> bool {
        self.capabilities(name).suppress_live_output
    }

    #[allow(dead_code)]
    pub fn phase_for_tool(
        &self,
        name: &str,
    ) -> crate::features::agent_activity::state::ActivityPhase {
        use crate::features::agent_activity::state::ActivityPhase;
        match normalized_tool_name(name) {
            "git_diff" | "git_status" | "diff" => ActivityPhase::Review,
            other => match self.capabilities(other).category {
                ToolCategory::Read | ToolCategory::Search => ActivityPhase::Explore,
                ToolCategory::PatchProposal | ToolCategory::PatchApply => ActivityPhase::Edit,
                ToolCategory::VirtualCommand
                | ToolCategory::RealCommand
                | ToolCategory::AskUser
                | ToolCategory::Delegate
                | ToolCategory::Other => ActivityPhase::Run,
            },
        }
    }
}

fn normalized_tool_name(name: &str) -> &str {
    match name {
        "RunCommand" | "shell" => "run_real_command",
        _ => name
            .split("<|")
            .next()
            .unwrap_or(name)
            .split_whitespace()
            .next()
            .unwrap_or(name),
    }
}
