use agent_protocol::{
    IconToken, NetworkAccess, NetworkPolicy, RealCommandRequest, RiskLevel, ToolAssessment,
    ToolCapabilities, ToolCategory, ToolContext, ToolModeGate, ToolPack, ToolPackPolicy,
    ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use real_process::RealProcessExecutor;
use serde_json::{Value, json};

use crate::shared::{classify_real_command, decision_to_assessment};
use crate::tool::{AgentTool, default_finish_summary};

pub struct RunRealCommandTool;

#[async_trait]
impl AgentTool for RunRealCommandTool {
    fn name(&self) -> &'static str {
        "run_real_command"
    }

    fn description(&self) -> &'static str {
        "Run a real host command with streamed output and explicit exit status (requires approval)"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "program": { "type": "string" },
                "args": { "type": "array", "items": { "type": "string" } },
                "timeout_secs": { "type": "integer", "default": 120 }
            },
            "required": ["program"]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::RealCommand,
            ..ToolCapabilities::default()
        }
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::RunRealCommands,
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
        let exit = output
            .lines()
            .find(|l| l.starts_with("exit_code:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(0);
        if exit == 0 {
            "Command completed".into()
        } else {
            format!("Command exited with code {exit}")
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        let program = args.get("program").and_then(|v| v.as_str()).unwrap_or("");
        let cmd_args: Vec<&str> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        let joined = if cmd_args.is_empty() {
            program.to_string()
        } else {
            format!("{program} {}", cmd_args.join(" "))
        };
        joined.chars().take(120).collect()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !ctx.mode.can_run_real_commands() {
            return Ok(ToolAssessment {
                risk: RiskLevel::Critical,
                requires_approval: false,
                reason: "run_real_command is not allowed in the current agent mode".into(),
                affected_paths: vec![],
                network_access: NetworkAccess::Disabled,
                writes_to_disk: false,
                runs_real_process: false,
                denied: true,
            });
        }
        let decision = classify_real_command(args);
        let (denied, requires_approval, risk, reason) = decision_to_assessment(&decision);
        let program = args
            .get("program")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        Ok(ToolAssessment {
            risk,
            requires_approval,
            reason: if reason.is_empty() {
                format!("executes `{program}` on host")
            } else {
                reason
            },
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: true,
            denied,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let program = args
            .get("program")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing program".to_string())?;
        let cmd_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let timeout_secs = args
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(120);

        RealProcessExecutor::validate_cwd(&ctx.project_root, &ctx.project_root)
            .map_err(|e| e.to_string())?;

        let args_display = cmd_args.join(" ");
        let request = RealCommandRequest {
            program: program.to_string(),
            args: cmd_args,
            cwd: ctx.project_root.clone(),
            timeout_secs,
            stdin: None,
            network_policy: NetworkPolicy::Disabled,
            approval_id: None,
        };

        let cancel = agent_protocol::CancellationToken::new();
        let output_sink = ctx.output_sink.clone();
        let result = RealProcessExecutor::run(request, cancel, move |stream, chunk| {
            if let Some(sink) = &output_sink {
                (sink.emit)(stream, chunk);
            }
        })
        .await
        .map_err(|e| e.to_string())?;

        let output = format!(
            "$ {} {}\n\nstdout:\n{}\n\nstderr:\n{}\n\nexit_code: {}",
            program, args_display, result.stdout, result.stderr, result.exit_code
        );
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: result.exit_code != 0,
        })
    }
}
