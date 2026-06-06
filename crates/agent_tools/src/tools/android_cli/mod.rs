use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use android_device::AndroidCliDriver;
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tool::{AgentTool, default_finish_summary};

pub struct AndroidCliDoctorTool;
pub struct AndroidCliInfoTool;
pub struct AndroidCliRunTool;
pub struct AndroidCliTestJourneyTool;
pub struct AndroidCliDocsSearchTool;
pub struct AndroidCliDocsFetchTool;

macro_rules! cli_tool {
    ($ty:ident, $name:literal, $desc:literal, $label:literal, [$($arg:expr),*], $approval:expr) => {
        #[async_trait]
        impl AgentTool for $ty {
            fn name(&self) -> &'static str { $name }
            fn description(&self) -> &'static str { $desc }
            fn schema(&self) -> Value { cli_schema($name) }
            fn policy(&self) -> ToolPolicy { cli_policy($name) }
            fn icon(&self) -> IconToken { IconToken::Terminal }
            fn label(&self, running: bool) -> String {
                if running { format!("{}…", $label) } else { $label.into() }
            }
            fn args_preview(&self, args: &Value) -> String {
                args.get("query")
                    .or_else(|| args.get("target"))
                    .or_else(|| args.get("journey"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            }
            fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
                if is_error {
                    default_finish_summary($label, output, true)
                } else {
                    serde_json::from_str::<Value>(output)
                        .ok()
                        .and_then(|value| value.get("summary").and_then(|v| v.as_str()).map(ToOwned::to_owned))
                        .unwrap_or_else(|| $label.into())
                }
            }
            async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
                cli_assessment(args, ctx, $approval, $label)
            }
            async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
                let mut cli_args = vec![$($arg.to_string()),*];
                extend_args($name, &args, &mut cli_args)?;
                execute_cli($name, &ctx, cli_args).await
            }
        }
    };
}

cli_tool!(
    AndroidCliDoctorTool,
    "android_cli.doctor",
    "Run Android CLI environment diagnostics.",
    "Android CLI doctor",
    ["info"],
    false
);
cli_tool!(
    AndroidCliInfoTool,
    "android_cli.info",
    "Inspect Android CLI project/device metadata.",
    "Android CLI info",
    ["info"],
    false
);
cli_tool!(
    AndroidCliRunTool,
    "android_cli.run",
    "Run the current Android project through the official Android CLI.",
    "Android CLI run",
    ["run"],
    false
);
cli_tool!(
    AndroidCliTestJourneyTool,
    "android_cli.test_journey",
    "Run a Journey through Android CLI when the local Android CLI supports it.",
    "Android CLI journey",
    ["journey", "run"],
    false
);
cli_tool!(
    AndroidCliDocsSearchTool,
    "android_cli.docs_search",
    "Search Android CLI Android Knowledge Base documentation.",
    "Android docs search",
    ["docs", "search"],
    false
);
cli_tool!(
    AndroidCliDocsFetchTool,
    "android_cli.docs_fetch",
    "Fetch Android CLI Android Knowledge Base documentation.",
    "Android docs fetch",
    ["docs", "fetch"],
    false
);

fn cli_policy(name: &str) -> ToolPolicy {
    let pack_policy = match name {
        "android_cli.docs_search" | "android_cli.docs_fetch" => {
            ToolPackPolicy::Only(vec![ToolPack::Research, ToolPack::General])
        }
        "android_cli.test_journey" => {
            ToolPackPolicy::Only(vec![ToolPack::UiBrowser, ToolPack::General])
        }
        "android_cli.doctor" | "android_cli.info" => ToolPackPolicy::Only(vec![
            ToolPack::Dependency,
            ToolPack::CodeEdit,
            ToolPack::UiBrowser,
            ToolPack::GitCi,
            ToolPack::General,
        ]),
        _ => ToolPackPolicy::Only(vec![
            ToolPack::CodeEdit,
            ToolPack::UiBrowser,
            ToolPack::GitCi,
            ToolPack::General,
        ]),
    };
    ToolPolicy {
        mode_gate: ToolModeGate::RunRealCommands,
        pack_policy,
        ..ToolPolicy::default()
    }
}

fn cli_schema(name: &str) -> Value {
    match name {
        "android_cli.docs_search" => json!({
            "type":"object","additionalProperties":false,
            "properties":{"query":{"type":"string"}},
            "required":["query"]
        }),
        "android_cli.docs_fetch" => json!({
            "type":"object","additionalProperties":false,
            "properties":{"target":{"type":"string"}},
            "required":["target"]
        }),
        "android_cli.test_journey" => json!({
            "type":"object","additionalProperties":false,
            "properties":{"journey":{"type":"string"},"project_dir":{"type":"string"}},
            "required":["journey"]
        }),
        _ => json!({
            "type":"object","additionalProperties":false,
            "properties":{"project_dir":{"type":"string"}}
        }),
    }
}

fn extend_args(name: &str, args: &Value, cli_args: &mut Vec<String>) -> Result<(), String> {
    match name {
        "android_cli.docs_search" => cli_args.push(required(args, "query")?.to_string()),
        "android_cli.docs_fetch" => cli_args.push(required(args, "target")?.to_string()),
        "android_cli.test_journey" => cli_args.push(required(args, "journey")?.to_string()),
        _ => {}
    }
    if let Some(project_dir) = args.get("project_dir").and_then(|v| v.as_str()) {
        cli_args.push(format!("--project_dir={project_dir}"));
    }
    Ok(())
}

async fn execute_cli(
    name: &str,
    ctx: &ToolContext,
    cli_args: Vec<String>,
) -> Result<ToolResult, String> {
    let refs: Vec<&str> = cli_args.iter().map(String::as_str).collect();
    let output = AndroidCliDriver::new(&ctx.project_root).run(&refs).await?;
    Ok(ToolResult {
        call_id: agent_protocol::ToolCallId::new(""),
        name: name.into(),
        output: serde_json::to_string_pretty(&json!({
            "summary": format!("{} completed", name.replace('_', " ")),
            "output": output,
        }))
        .unwrap(),
        is_error: false,
    })
}

fn cli_assessment(
    _args: &Value,
    ctx: &ToolContext,
    requires_approval: bool,
    reason: &str,
) -> Result<ToolAssessment, String> {
    let denied = !ctx.mode.can_run_real_commands();
    Ok(ToolAssessment {
        risk: if requires_approval {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        },
        requires_approval,
        reason: if denied {
            "Android CLI tools require real command capability".into()
        } else {
            reason.into()
        },
        affected_paths: vec![ctx.project_root.clone()],
        network_access: NetworkAccess::Disabled,
        writes_to_disk: false,
        runs_real_process: true,
        denied,
    })
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing {key}"))
}
