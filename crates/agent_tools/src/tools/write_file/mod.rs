use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use agent_sandbox::PathPolicy;
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::{
    generate_unified_diff, make_patch_proposal, partial_json_string_field,
    patch_diff_from_streaming_json,
};
use crate::tool::{AgentTool, default_finish_summary};

/// Create or overwrite a file. Produces a `PatchProposal` (preview only) so the write flows
/// through the standard propose → preview → apply + checkpoint pipeline — never direct to disk.
pub struct WriteFileTool;

#[async_trait]
impl AgentTool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Create or overwrite a file with the given content. Preview only; apply needs approval."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "Relative path of the file to write" },
                "content": { "type": "string", "description": "Full new file content" }
            },
            "required": ["path", "content"]
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
            "Writing file".into()
        } else {
            "Wrote file".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        match command.filter(|c| !c.is_empty() && *c != "{}") {
            Some(p) if running => format!("Writing {p}"),
            Some(p) => format!("Wrote {p}"),
            None if running => "Writing file".into(),
            None => "Wrote file".into(),
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
        format!("Wrote {path}")
    }

    fn streaming_patch_preview(
        &self,
        tool_args: &str,
        project_root: &std::path::Path,
    ) -> Option<String> {
        let extracted = patch_diff_from_streaming_json(tool_args)?;
        if looks_like_unified_diff(&extracted) {
            return Some(extracted);
        }
        let path = partial_json_string_field(tool_args, "path")?;
        let original = std::fs::read_to_string(project_root.join(&path)).unwrap_or_default();
        Some(generate_unified_diff(&path, &original, &extracted))
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !ctx.mode.can_propose_patches() {
            return Ok(denied(
                "writing files is not allowed in the current agent mode",
            ));
        }
        let path = arg_str(args, "path")?;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_write(std::path::Path::new(&path))?;
        Ok(ToolAssessment {
            risk: RiskLevel::Low,
            requires_approval: false,
            reason: "file write proposal (preview only)".into(),
            affected_paths: vec![resolved],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let path = arg_str(&args, "path")?;
        let content = arg_str(&args, "content")?;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_write(std::path::Path::new(&path))?;
        let original = std::fs::read_to_string(&resolved).unwrap_or_default();

        let diff = generate_unified_diff(&path, &original, &content);
        if diff.is_empty() {
            return Ok(ToolResult {
                call_id: agent_protocol::ToolCallId::new(""),
                name: self.name().to_string(),
                output: format!("No changes: `{path}` already has this content"),
                is_error: false,
            });
        }
        let proposal = make_patch_proposal(&diff, &format!("Write {path}"), &ctx)?;
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

fn looks_like_unified_diff(text: &str) -> bool {
    let t = text.trim();
    t.contains("@@")
        || t.starts_with("--- ")
        || t.starts_with("+++ ")
        || t.starts_with("diff --git")
}

fn denied(reason: &str) -> ToolAssessment {
    ToolAssessment {
        risk: RiskLevel::Low,
        requires_approval: false,
        reason: reason.into(),
        affected_paths: vec![],
        network_access: NetworkAccess::Disabled,
        writes_to_disk: false,
        runs_real_process: false,
        denied: true,
    }
}
