use std::time::Duration;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::web::{fetch_http, public_http_url_from_tool_args, structured_output};
use crate::tool::{AgentTool, default_finish_summary};

/// GET-only network fetch. Disabled by default; requires approval and a command-capable mode,
/// honouring the "network disabled by default" runtime rule. Content is treated as untrusted.
pub struct FetchUrlTool;

#[async_trait]
impl AgentTool for FetchUrlTool {
    fn name(&self) -> &'static str {
        "fetch_url"
    }

    fn description(&self) -> &'static str {
        "Compatibility alias for web_fetch. Fetches a URL with HTTP GET and returns structured untrusted content."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "url": { "type": "string", "description": "Absolute http(s) URL to GET" }
            },
            "required": ["url"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::RunRealCommands,
            pack_policy: ToolPackPolicy::Only(vec![ToolPack::Research, ToolPack::General]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Globe
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Fetching URL".into()
        } else {
            "Fetched URL".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        match command.filter(|c| !c.is_empty() && *c != "{}") {
            Some(u) if running => format!("Fetching {u}"),
            Some(u) => format!("Fetched {u}"),
            None if running => "Fetching URL".into(),
            None => "Fetched URL".into(),
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .chars()
            .take(120)
            .collect()
    }

    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), output, true);
        }
        "Fetched URL".into()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !ctx.mode.can_run_real_commands() {
            return Ok(ToolAssessment {
                risk: RiskLevel::High,
                requires_approval: false,
                reason: "network fetch is not allowed in the current agent mode".into(),
                affected_paths: vec![],
                network_access: NetworkAccess::AllowGet,
                writes_to_disk: false,
                runs_real_process: false,
                denied: true,
            });
        }
        let url = args.get("url").and_then(|v| v.as_str()).unwrap_or_default();
        if public_http_url_from_tool_args(args, "url").is_err() {
            return Ok(ToolAssessment {
                risk: RiskLevel::High,
                requires_approval: false,
                reason: "only public absolute http(s) URLs are allowed".into(),
                affected_paths: vec![],
                network_access: NetworkAccess::AllowGet,
                writes_to_disk: false,
                runs_real_process: false,
                denied: true,
            });
        }
        Ok(ToolAssessment {
            risk: RiskLevel::High,
            requires_approval: true,
            reason: format!("fetches {url} over the network"),
            affected_paths: vec![],
            network_access: NetworkAccess::AllowGet,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing url".to_string())?
            .to_string();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let data = fetch_http(&client, &json!({ "url": url, "extract": "text" })).await?;
        let status = data.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
        let output = structured_output("Fetched URL", data)?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: !(200..400).contains(&status),
        })
    }
}
