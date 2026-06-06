use agent_protocol::{
    IconToken, NetworkAccess, OutputStreamKind, RiskLevel, ToolAssessment, ToolCapabilities,
    ToolCategory, ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use agent_shell::{
    CommandRegistry, ExecRequest, ExecutionLimits, OverlayFs, Shell, ShellPolicy, VirtualPath,
};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::tool::{AgentTool, default_finish_summary};

pub struct BashVirtualTool;

const DEFAULT_FAKE_SHELL_TIMEOUT_MS: u64 = 30_000;
const MIN_FAKE_SHELL_TIMEOUT_MS: u64 = 1_000;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[async_trait]
impl AgentTool for BashVirtualTool {
    fn name(&self) -> &'static str {
        "bash_virtual"
    }

    fn description(&self) -> &'static str {
        "Run a restricted Rust fake shell over the project virtual filesystem. It never runs real bash or host binaries. Prefer read_file and search_project for precision."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Fake-shell command string" },
                "script": { "type": "string", "description": "Deprecated alias for command" },
                "cwd": { "type": "string", "description": "Virtual cwd, defaults to /workspace" },
                "timeout_ms": { "type": "integer", "default": DEFAULT_FAKE_SHELL_TIMEOUT_MS },
                "max_output_bytes": { "type": "integer", "default": DEFAULT_MAX_OUTPUT_BYTES }
            },
            "anyOf": [
                { "required": ["command"] },
                { "required": ["script"] }
            ]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::VirtualCommand,
            ..ToolCapabilities::default()
        }
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::RunVirtualBash,
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
        IconToken::Terminal
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Running command".into()
        } else {
            "Ran command".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        match command.filter(|c| !c.is_empty() && *c != "{}") {
            Some(cmd) if running => format!("Running `{cmd}`"),
            Some(cmd) => format!("Ran `{cmd}`"),
            None if running => "Running command".into(),
            None => "Ran command".into(),
        }
    }

    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), output, true);
        }
        let exit = serde_json::from_str::<Value>(output)
            .ok()
            .and_then(|v| {
                v.get("exit_code")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32)
            })
            .unwrap_or(0);
        if exit == 0 {
            "Ran command".into()
        } else {
            format!("Command exited with code {exit}")
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("command")
            .or_else(|| args.get("script"))
            .and_then(|v| v.as_str())
            .map(|script| {
                let line = script.lines().next().unwrap_or(script);
                line.chars().take(120).collect()
            })
            .unwrap_or_default()
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "restricted fake shell; no real process or network access".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let command = args
            .get("command")
            .or_else(|| args.get("script"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing command".to_string())?;
        let timeout_ms = args
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_FAKE_SHELL_TIMEOUT_MS)
            .clamp(MIN_FAKE_SHELL_TIMEOUT_MS, 300_000);
        let max_output_bytes = args
            .get("max_output_bytes")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_OUTPUT_BYTES)
            .min(DEFAULT_MAX_OUTPUT_BYTES);

        let mut policy = ShellPolicy::default();
        policy.allow_writes = ctx.mode.can_apply_patches();
        policy.max_output_bytes = max_output_bytes;

        let limits = ExecutionLimits {
            max_output_bytes,
            timeout_ms,
            ..ExecutionLimits::default()
        };
        let fs = Arc::new(OverlayFs::new(
            &ctx.project_root,
            policy.max_file_read_bytes,
        ));
        let mut shell = Shell::new(
            fs,
            CommandRegistry::with_defaults(),
            limits,
            policy,
            VirtualPath::workspace(),
        );
        let result = shell.exec(ExecRequest {
            command: command.to_string(),
            cwd: args.get("cwd").and_then(|v| v.as_str()).map(str::to_string),
            env: vec![],
            max_output_bytes,
            timeout_ms,
        });

        if let Some(sink) = &ctx.output_sink {
            if !result.stdout.is_empty() {
                (sink.emit)(OutputStreamKind::Stdout, result.stdout.clone());
            }
            if !result.stderr.is_empty() {
                (sink.emit)(OutputStreamKind::Stderr, result.stderr.clone());
            }
        }

        let output = serde_json::to_string(&json!({
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exit_code": result.exit_code,
            "duration_ms": result.duration_ms,
            "truncated": result.truncated,
        }))
        .map_err(|e| e.to_string())?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: result.exit_code != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use agent_protocol::{AgentMode, ProjectId, RunId, SessionId, ToolContext};
    use tempfile::tempdir;

    use super::*;

    fn ctx(root: std::path::PathBuf, mode: AgentMode) -> ToolContext {
        ToolContext {
            project_root: root,
            project_id: ProjectId::new("project"),
            session_id: SessionId::new("session"),
            run_id: RunId::new("run"),
            mode,
            output_sink: None,
        }
    }

    fn output_value(result: ToolResult) -> Value {
        serde_json::from_str(&result.output).unwrap()
    }

    #[tokio::test]
    async fn bash_virtual_reads_workspace_file() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        let result = BashVirtualTool
            .execute(
                json!({ "command": "cat /workspace/src/main.rs" }),
                ctx(dir.path().to_path_buf(), AgentMode::ApplyWithApproval),
            )
            .await
            .unwrap();
        let output = output_value(result);
        assert_eq!(output["exit_code"], 0);
        assert!(output["stdout"].as_str().unwrap().contains("fn main"));
    }

    #[tokio::test]
    async fn bash_virtual_never_runs_real_commands() {
        let dir = tempdir().unwrap();
        let result = BashVirtualTool
            .execute(
                json!({ "command": "cargo test" }),
                ctx(dir.path().to_path_buf(), AgentMode::ApplyWithApproval),
            )
            .await
            .unwrap();
        let output = output_value(result);
        assert_eq!(output["exit_code"], 127);
        assert!(
            output["stderr"]
                .as_str()
                .unwrap()
                .contains("command not found")
        );
    }

    #[tokio::test]
    async fn bash_virtual_blocks_host_paths() {
        let dir = tempdir().unwrap();
        for command in ["cat /etc/passwd", "cat ../../.ssh/id_rsa"] {
            let result = BashVirtualTool
                .execute(
                    json!({ "command": command }),
                    ctx(dir.path().to_path_buf(), AgentMode::ApplyWithApproval),
                )
                .await
                .unwrap();
            let output = output_value(result);
            assert_ne!(output["exit_code"], 0);
        }
    }
}
