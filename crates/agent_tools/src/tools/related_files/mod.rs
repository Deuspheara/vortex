use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::open_repo_index;
use crate::tool::{AgentTool, default_finish_summary};

pub struct RelatedFilesTool;

#[async_trait]
impl AgentTool for RelatedFilesTool {
    fn name(&self) -> &'static str {
        "related_files"
    }

    fn description(&self) -> &'static str {
        "List files related to a path via import/use relationships. Returns a compact \
         <related_files> block."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative file path within the project."
                }
            },
            "required": ["path"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::Only(vec![
                ToolPack::CodeEdit,
                ToolPack::UiBrowser,
                ToolPack::Planning,
                ToolPack::General,
            ]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Folder
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Finding related files".into()
        } else {
            "Related files".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    fn finish_summary(&self, args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("file");
        format!("Related to {path}")
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only import relationship lookup".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing required argument: path".to_string())?;

        let index = open_repo_index(&ctx)?;
        let output = index.related_files(path);

        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
        })
    }
}
