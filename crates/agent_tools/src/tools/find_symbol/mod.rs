use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::open_repo_index;
use crate::tool::{AgentTool, default_finish_summary};

pub struct FindSymbolTool;

#[async_trait]
impl AgentTool for FindSymbolTool {
    fn name(&self) -> &'static str {
        "find_symbol"
    }

    fn description(&self) -> &'static str {
        "Find code symbols (functions, structs, classes, etc.) by name. Returns compact \
         <symbol_result> blocks with path and line ranges. Optional `kind` filters by symbol type."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Symbol name to search for (substring match supported via ranking)."
                },
                "kind": {
                    "type": "string",
                    "description": "Optional symbol kind filter: function, struct, class, enum, trait, etc."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 10,
                    "description": "Maximum number of matches to return."
                }
            },
            "required": ["name"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::Only(vec![
                ToolPack::CodeEdit,
                ToolPack::UiBrowser,
                ToolPack::GitCi,
                ToolPack::Planning,
                ToolPack::General,
            ]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Search
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Finding symbol".into()
        } else {
            "Find symbol".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    fn finish_summary(&self, args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("symbol");
        format!("Found {name}")
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only symbol lookup".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing required argument: name".to_string())?;
        let kind = args.get("kind").and_then(|v| v.as_str());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 50) as usize;

        let index = open_repo_index(&ctx)?;
        let output = index.find_symbol(name, kind, limit);

        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
        })
    }
}
