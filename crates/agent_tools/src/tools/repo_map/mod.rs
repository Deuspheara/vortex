use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult, ToolSummaryArgPath, ToolSummaryPolicy,
};
use async_trait::async_trait;
use project_index::MapBudget;
use serde_json::{Value, json};

use crate::shared::open_repo_index;
use crate::tool::AgentTool;

/// Emits a compact, navigable `<repo_index>` map of the workspace (directories + files) so the
/// model can reason over structure before opening specific files. Backed by `project_index`'s
/// content-hash-keyed SQLite cache.
pub struct RepoMapTool;

#[async_trait]
impl AgentTool for RepoMapTool {
    fn name(&self) -> &'static str {
        "repo_map"
    }

    fn description(&self) -> &'static str {
        "Return a compact tree map of the project (directories and files with language tags). \
         Use it to orient before reading files. Optional `depth` limits nesting; optional `focus` \
         restricts the map to a subdirectory."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "depth": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Maximum directory nesting depth to render (default compact)."
                },
                "focus": {
                    "type": "string",
                    "description": "Relative path prefix to restrict the map to a subtree."
                }
            }
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
                arg_paths: vec![ToolSummaryArgPath {
                    field: "focus".into(),
                    ..ToolSummaryArgPath::default()
                }],
                ..ToolSummaryPolicy::default()
            },
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Folder
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Mapping repository".into()
        } else {
            "Repo map".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("focus")
            .and_then(|v| v.as_str())
            .map(|f| f.to_string())
            .unwrap_or_default()
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only repository structure map".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let depth = args
            .get("depth")
            .and_then(|v| v.as_u64())
            .map(|d| d as usize);
        let focus = args
            .get("focus")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let index = open_repo_index(&ctx)?;

        let mut budget = MapBudget::compact().with_focus(focus);
        if let Some(depth) = depth {
            budget = budget.with_depth(depth);
        }
        let output = index.compact_map(budget);

        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
        })
    }
}
