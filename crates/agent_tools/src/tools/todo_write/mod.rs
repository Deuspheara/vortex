use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPackPolicy,
    ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tool::AgentTool;

/// Persistent todo checklist. Execution is intercepted by the runtime (it owns the run/session
/// todo state and emits `AgentEvent::TodoUpdated`), analogous to `delegate`.
pub struct TodoWriteTool;

#[async_trait]
impl AgentTool for TodoWriteTool {
    fn name(&self) -> &'static str {
        "todo_write"
    }

    fn description(&self) -> &'static str {
        "Maintain a todo list for multi-step tasks. Mark items in_progress/completed as you go."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The todo items",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "id": { "type": "string" },
                            "content": { "type": "string" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "cancelled"]
                            }
                        },
                        "required": ["id", "content", "status"]
                    }
                },
                "merge": { "type": "boolean", "default": true, "description": "Merge by id instead of replacing the whole list" }
            },
            "required": ["todos"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::All,
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Checklist
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Updating todos".into()
        } else {
            "Updated todos".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        let count = args
            .get("todos")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        format!("{count} items")
    }

    fn streaming_args_preview(&self, raw_json: &str) -> Option<String> {
        if raw_json.contains("\"todos\"") {
            Some("Updating checklist".into())
        } else {
            None
        }
    }

    fn finish_summary(&self, _args: &Value, output: &str, _is_error: bool) -> String {
        if output.is_empty() {
            "Updated todos".into()
        } else {
            output.lines().next().unwrap_or("Updated todos").to_string()
        }
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "updates the in-memory todo checklist only".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, _args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        // The runtime intercepts todo_write to update run/session state and emit TodoUpdated.
        Err("todo_write must be handled by the agent runtime".into())
    }
}
