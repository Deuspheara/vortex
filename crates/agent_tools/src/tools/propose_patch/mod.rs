use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::{
    make_patch_proposal, parse_patch_files, patch_diff_from_args, patch_diff_from_streaming_json,
    patch_diff_preview,
};
use crate::tool::{AgentTool, default_finish_summary};

pub struct ProposePatchTool;

#[async_trait]
impl AgentTool for ProposePatchTool {
    fn name(&self) -> &'static str {
        "propose_patch"
    }

    fn description(&self) -> &'static str {
        "Propose a unified diff patch without writing to disk. Pass the full diff in unified_diff (---/+++/@@ hunks)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "unified_diff": {
                    "type": "string",
                    "description": "Complete unified diff with ---/+++ headers and @@ hunks"
                },
                "summary": { "type": "string" }
            },
            "required": ["unified_diff"]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::PatchProposal,
            ..ToolCapabilities::default()
        }
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ProposePatches,
            pack_policy: ToolPackPolicy::Only(vec![
                ToolPack::Dependency,
                ToolPack::CodeEdit,
                ToolPack::UiBrowser,
                ToolPack::GitCi,
                ToolPack::General,
            ]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Pencil
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Proposing patch".into()
        } else {
            "Proposed patch".into()
        }
    }

    fn finish_summary(&self, _args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        "Proposed patch".into()
    }

    fn args_preview(&self, args: &Value) -> String {
        if let Ok(diff) = patch_diff_from_args(args) {
            return patch_diff_preview(&diff);
        }
        String::new()
    }

    fn streaming_patch_preview(
        &self,
        tool_args: &str,
        _project_root: &std::path::Path,
    ) -> Option<String> {
        patch_diff_from_streaming_json(tool_args)
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !ctx.mode.can_propose_patches() {
            return Ok(ToolAssessment {
                risk: RiskLevel::Low,
                requires_approval: false,
                reason: "patch proposal is not allowed in the current agent mode".into(),
                affected_paths: vec![],
                network_access: NetworkAccess::Disabled,
                writes_to_disk: false,
                runs_real_process: false,
                denied: true,
            });
        }
        let diff = patch_diff_from_args(args).unwrap_or_default();
        let files = parse_patch_files(&diff, &ctx.project_root)?;
        Ok(ToolAssessment {
            risk: RiskLevel::Low,
            requires_approval: false,
            reason: "patch proposal only".into(),
            affected_paths: files.iter().map(|f| f.path.clone()).collect(),
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let diff = patch_diff_from_args(&args)?;
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("Proposed patch");
        let proposal = make_patch_proposal(&diff, summary, &ctx)?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output: serde_json::to_string(&proposal).map_err(|e| e.to_string())?,
            is_error: false,
        })
    }
}
