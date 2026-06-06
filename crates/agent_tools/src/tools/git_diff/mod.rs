use agent_protocol::{
    IconToken, ToolAssessment, ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy,
    ToolRepoRequirement, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::{NOT_A_GIT_REPO, is_git_repo, tool_error};
use crate::tool::{AgentTool, default_finish_summary};
use crate::tools::git_status::safe_git;

pub struct GitDiffTool;

#[async_trait]
impl AgentTool for GitDiffTool {
    fn name(&self) -> &'static str {
        "git_diff"
    }

    fn description(&self) -> &'static str {
        "Show git diff for the project, optionally staged only or scoped to one path (git repositories only)"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "staged": { "type": "boolean", "default": false },
                "path": { "type": "string", "description": "Optional relative path to diff" },
                "context_lines": { "type": "integer", "default": 3, "description": "Unified diff context lines" }
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
            repo_requirement: ToolRepoRequirement::GitRepository,
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::GitCompare
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Reading diff".into()
        } else {
            "Git diff".into()
        }
    }

    fn finish_summary(&self, args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("project");
        format!("Diff {path}")
    }

    fn args_preview(&self, args: &Value) -> String {
        let mut parts = Vec::new();
        if args
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            parts.push("staged".into());
        }
        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            parts.push(path.to_string());
        }
        if let Some(lines) = args.get("context_lines").and_then(|v| v.as_u64()) {
            if lines != 3 {
                parts.push(format!("-U{lines}"));
            }
        }
        parts.join(" · ")
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(safe_git(ctx))
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        if !is_git_repo(&ctx.project_root) {
            return Ok(tool_error(self.name(), NOT_A_GIT_REPO));
        }
        let staged = args
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let context_lines = args
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(3)
            .min(100);
        let mut diff_args = vec!["diff"];
        if staged {
            diff_args.push("--cached");
        }
        let unified_arg = format!("-U{context_lines}");
        diff_args.push(unified_arg.as_str());
        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            diff_args.push("--");
            diff_args.push(path);
        }
        let output = match std::process::Command::new("git")
            .args(diff_args)
            .current_dir(&ctx.project_root)
            .output()
        {
            Ok(output) => output,
            Err(err) => return Ok(tool_error(self.name(), err.to_string())),
        };
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            return Ok(tool_error(
                self.name(),
                if stderr.is_empty() {
                    "git diff failed".into()
                } else {
                    stderr
                },
            ));
        }
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output: stdout,
            is_error: false,
        })
    }
}
