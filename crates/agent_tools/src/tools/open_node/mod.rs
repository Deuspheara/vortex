use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult, ToolSummaryArgPath, ToolSummaryPolicy,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::open_repo_index;
use crate::tool::{AgentTool, default_finish_summary};

pub struct OpenNodeTool;

#[async_trait]
impl AgentTool for OpenNodeTool {
    fn name(&self) -> &'static str {
        "open_node"
    }

    fn description(&self) -> &'static str {
        "Open an indexed node by id (symbol id from find_symbol, or a relative file path) and \
         return a compact <file_slice> with the relevant source lines."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "Symbol id (path#kind#name#line) or relative file path."
                }
            },
            "required": ["node_id"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::Only(vec![
                ToolPack::Dependency,
                ToolPack::CodeEdit,
                ToolPack::UiBrowser,
                ToolPack::GitCi,
                ToolPack::Planning,
                ToolPack::General,
            ]),
            summary: ToolSummaryPolicy {
                prefer_line_bounded_follow_up: true,
                arg_paths: vec![ToolSummaryArgPath {
                    field: "node_id".into(),
                    ..ToolSummaryArgPath::default()
                }],
                ..ToolSummaryPolicy::default()
            },
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::File
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Opening node".into()
        } else {
            "Open node".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("node_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        let label = self.label(running);
        command
            .and_then(crate::shared::sanitize_display_arg)
            .map(|node| format!("{label} {node}"))
            .unwrap_or(label)
    }

    fn finish_summary(&self, args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        let id = args
            .get("node_id")
            .and_then(|v| v.as_str())
            .unwrap_or("node");
        format!("Opened {id}")
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only indexed file slice".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let node_id = args
            .get("node_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing required argument: node_id".to_string())?;

        let index = open_repo_index(&ctx)?;
        match index.open_node(node_id) {
            Ok(output) => Ok(ToolResult {
                call_id: agent_protocol::ToolCallId::new(""),
                name: self.name().to_string(),
                output,
                is_error: false,
            }),
            Err(message) => Ok(ToolResult {
                call_id: agent_protocol::ToolCallId::new(""),
                name: self.name().to_string(),
                output: message,
                is_error: true,
            }),
        }
    }
}
