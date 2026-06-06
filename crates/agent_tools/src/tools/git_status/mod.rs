use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolRepoRequirement, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::{NOT_A_GIT_REPO, is_git_repo, tool_error};
use crate::tool::{AgentTool, default_finish_summary};

pub struct GitStatusTool;

#[async_trait]
impl AgentTool for GitStatusTool {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn description(&self) -> &'static str {
        "Show concise git status for the project, including branch and short file state (git repositories only)"
    }

    fn schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
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
            "Checking git status".into()
        } else {
            "Git status".into()
        }
    }

    fn finish_summary(&self, _args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        "Git status".into()
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(safe_git(ctx))
    }

    async fn execute(&self, _args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        if !is_git_repo(&ctx.project_root) {
            return Ok(tool_error(self.name(), NOT_A_GIT_REPO));
        }
        let output = match std::process::Command::new("git")
            .args(["status", "--short", "--branch"])
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
                    "git status failed".into()
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

pub fn safe_git(ctx: &ToolContext) -> ToolAssessment {
    ToolAssessment {
        risk: RiskLevel::SafeRead,
        requires_approval: false,
        reason: "read-only git metadata".into(),
        affected_paths: vec![ctx.project_root.clone()],
        network_access: NetworkAccess::Disabled,
        writes_to_disk: false,
        // git_status / git_diff shell out to the real `git` binary; report it accurately
        // for auditing and the UI even though the operation is read-only and needs no approval.
        runs_real_process: true,
        denied: false,
    }
}
