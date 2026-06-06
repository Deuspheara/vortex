use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use agent_sandbox::PathPolicy;
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::{generate_deletion_diff, make_patch_proposal, partial_json_string_field};
use crate::tool::{AgentTool, default_finish_summary};

/// Delete a file. Produces a `PatchProposal` (preview only) so the deletion flows through the
/// standard propose → preview → apply + checkpoint pipeline (reversible via checkpoint).
pub struct DeleteFileTool;

#[async_trait]
impl AgentTool for DeleteFileTool {
    fn name(&self) -> &'static str {
        "delete_file"
    }

    fn description(&self) -> &'static str {
        "Delete a file. Preview only; apply needs approval and is checkpoint-backed."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "Relative path of the file to delete" }
            },
            "required": ["path"]
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
            "Deleting file".into()
        } else {
            "Deleted file".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        match command.filter(|c| !c.is_empty() && *c != "{}") {
            Some(p) if running => format!("Deleting {p}"),
            Some(p) => format!("Deleted {p}"),
            None if running => "Deleting file".into(),
            None => "Deleted file".into(),
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    fn finish_summary(&self, args: &Value, output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), output, true);
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("file");
        format!("Deleted {path}")
    }

    fn streaming_patch_preview(
        &self,
        tool_args: &str,
        project_root: &std::path::Path,
    ) -> Option<String> {
        let path = partial_json_string_field(tool_args, "path")?;
        let original = std::fs::read_to_string(project_root.join(&path)).ok()?;
        Some(generate_deletion_diff(&path, &original))
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !ctx.mode.can_propose_patches() {
            return Ok(denied(
                "deleting files is not allowed in the current agent mode",
            ));
        }
        let path = arg_str(args, "path")?;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_write(std::path::Path::new(&path))?;
        Ok(ToolAssessment {
            risk: RiskLevel::Medium,
            requires_approval: false,
            reason: "file deletion proposal (preview only)".into(),
            affected_paths: vec![resolved],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let path = arg_str(&args, "path")?;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_write(std::path::Path::new(&path))?;
        let original = std::fs::read_to_string(&resolved)
            .map_err(|e| format!("cannot delete `{path}`: {e}"))?;

        let diff = generate_deletion_diff(&path, &original);
        let proposal = make_patch_proposal(&diff, &format!("Delete {path}"), &ctx)?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output: serde_json::to_string(&proposal).map_err(|e| e.to_string())?,
            is_error: false,
        })
    }
}

fn arg_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("missing `{key}`"))
}

fn denied(reason: &str) -> ToolAssessment {
    ToolAssessment {
        risk: RiskLevel::Medium,
        requires_approval: false,
        reason: reason.into(),
        affected_paths: vec![],
        network_access: NetworkAccess::Disabled,
        writes_to_disk: false,
        runs_real_process: false,
        denied: true,
    }
}
