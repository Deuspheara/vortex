use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolNestingPolicy, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tool::AgentTool;

/// Delegate work to a child run. Execution is handled by the runtime.
pub struct DelegateTool;

#[async_trait]
impl AgentTool for DelegateTool {
    fn name(&self) -> &'static str {
        "delegate"
    }

    fn description(&self) -> &'static str {
        "Delegate work to a child run (depth 0 only)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": { "type": "string" }
            },
            "required": ["task"]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::Delegate,
            ..ToolCapabilities::default()
        }
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::Only(vec![ToolPack::Planning, ToolPack::General]),
            nesting: ToolNestingPolicy::RootRunOnly,
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Bot
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Delegating task".into()
        } else {
            "Delegated task".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("task")
            .and_then(|v| v.as_str())
            .map(|t| t.chars().take(120).collect())
            .unwrap_or_default()
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::Medium,
            requires_approval: true,
            reason: "This delegates work to a nested subagent run".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, _args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        Err("delegate must be executed by the agent runtime".into())
    }
}
