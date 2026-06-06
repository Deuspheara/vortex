use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tool::AgentTool;

/// Ask the user to pick between options. Execution is intercepted by the runtime, which emits
/// `AgentEvent::ChoiceRequested`, pauses the run, and resumes on `SubmitChoice`.
pub struct AskUserTool;

#[async_trait]
impl AgentTool for AskUserTool {
    fn name(&self) -> &'static str {
        "ask_user"
    }

    fn description(&self) -> &'static str {
        "Ask the user to choose between options when a decision is needed to proceed."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "prompt": { "type": "string", "description": "The question to ask" },
                "summary": { "type": "string", "description": "Short label for the decision" },
                "blocking_reason": { "type": "string", "description": "Why the run is paused for this decision" },
                "recommended_option_id": { "type": "string", "description": "The option id recommended by the agent" },
                "allow_custom": { "type": "boolean", "default": false, "description": "Whether the user may provide a custom answer" },
                "options": {
                    "type": "array",
                    "description": "Selectable options",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "id": { "type": "string" },
                            "label": { "type": "string" },
                            "description": { "type": "string" },
                            "recommended": { "type": "boolean", "default": false }
                        },
                        "required": ["id", "label"]
                    }
                }
            },
            "required": ["prompt", "options"]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::AskUser,
            ..ToolCapabilities::default()
        }
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::All,
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Question
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Asking user".into()
        } else {
            "Asked user".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("prompt")
            .and_then(|v| v.as_str())
            .map(|p| p.chars().take(120).collect())
            .unwrap_or_default()
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "requests a user decision".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, _args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        Err("ask_user must be handled by the agent runtime".into())
    }
}
