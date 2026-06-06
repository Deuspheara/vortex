use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use agent_sandbox::PathPolicy;
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::{generate_unified_diff, make_patch_proposal, partial_json_string_field};
use crate::tool::{AgentTool, default_finish_summary};

/// Structured search/replace edit. Produces a `PatchProposal` (preview only) that flows through
/// the standard propose → preview → apply + checkpoint pipeline — it never writes to disk.
pub struct EditFileTool;

#[async_trait]
impl AgentTool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn description(&self) -> &'static str {
        "Edit a file by replacing old_string with new_string. Preview only; apply needs approval."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "Relative path of the file to edit" },
                "old_string": { "type": "string", "description": "Exact text to replace (include enough context to be unique)" },
                "new_string": { "type": "string", "description": "Replacement text" },
                "replace_all": { "type": "boolean", "default": false, "description": "Replace every occurrence instead of requiring a unique match" }
            },
            "required": ["path", "old_string", "new_string"]
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
            "Editing file".into()
        } else {
            "Edited file".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        match command.filter(|c| !c.is_empty() && *c != "{}") {
            Some(p) if running => format!("Editing {p}"),
            Some(p) => format!("Edited {p}"),
            None if running => "Editing file".into(),
            None => "Edited file".into(),
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
        format!("Edited {path}")
    }

    fn streaming_patch_preview(
        &self,
        tool_args: &str,
        project_root: &std::path::Path,
    ) -> Option<String> {
        let path = partial_json_string_field(tool_args, "path")?;
        let old_string = partial_json_string_field(tool_args, "old_string")?;
        let new_string = partial_json_string_field(tool_args, "new_string")?;
        let original = std::fs::read_to_string(project_root.join(&path)).ok()?;
        let updated = if partial_json_bool_field(tool_args, "replace_all").unwrap_or(false) {
            original.replace(&old_string, &new_string)
        } else {
            original.replacen(&old_string, &new_string, 1)
        };
        if updated == original {
            return None;
        }
        Some(generate_unified_diff(&path, &original, &updated))
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !ctx.mode.can_propose_patches() {
            return Ok(denied(
                "structured edits are not allowed in the current agent mode",
            ));
        }
        let path = arg_str(args, "path")?;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_write(std::path::Path::new(&path))?;
        Ok(ToolAssessment {
            risk: RiskLevel::Low,
            requires_approval: false,
            reason: "structured edit proposal (preview only)".into(),
            affected_paths: vec![resolved],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let path = arg_str(&args, "path")?;
        let old_string = arg_str(&args, "old_string")?;
        let new_string = arg_str(&args, "new_string")?;
        let replace_all = args
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_write(std::path::Path::new(&path))?;
        let original =
            std::fs::read_to_string(&resolved).map_err(|e| format!("cannot read `{path}`: {e}"))?;

        let occurrences = original.matches(&old_string).count();
        if occurrences == 0 {
            return Err(format!("old_string not found in `{path}`"));
        }
        if occurrences > 1 && !replace_all {
            return Err(format!(
                "old_string occurs {occurrences} times in `{path}`; add more context or pass replace_all=true"
            ));
        }
        let updated = if replace_all {
            original.replace(&old_string, &new_string)
        } else {
            original.replacen(&old_string, &new_string, 1)
        };

        let diff = generate_unified_diff(&path, &original, &updated);
        if diff.is_empty() {
            return Ok(ToolResult {
                call_id: agent_protocol::ToolCallId::new(""),
                name: self.name().to_string(),
                output: format!("No changes: `{path}` already matches new_string"),
                is_error: false,
            });
        }
        let proposal = make_patch_proposal(&diff, &format!("Edit {path}"), &ctx)?;
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

fn partial_json_bool_field(input: &str, key: &str) -> Option<bool> {
    let needle = format!(r#"\"{key}\""#);
    let start = input.find(&needle)?;
    let mut after = input[start + needle.len()..].trim_start();
    if let Some(rest) = after.strip_prefix(':') {
        after = rest.trim_start();
    } else {
        return None;
    }
    if let Some(rest) = after.strip_prefix("true") {
        if rest
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
        {
            return Some(true);
        }
    }
    if let Some(rest) = after.strip_prefix("false") {
        if rest
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
        {
            return Some(false);
        }
    }
    None
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
