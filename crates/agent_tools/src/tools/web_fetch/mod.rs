use std::time::Duration;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::web::{
    fetch_http, public_http_url_from_tool_args, structured_output, tool_mode_allows_network,
};
use crate::tool::{AgentTool, default_finish_summary};

pub struct WebFetchTool;

#[async_trait]
impl AgentTool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn description(&self) -> &'static str {
        "Fetch one URL with text-first extraction. Returns untrusted structured content, links, warnings, and provenance."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "url": { "type": "string" },
                "mode": { "type": "string", "enum": ["auto", "http", "reader", "provider", "browser"] },
                "extract": { "type": "string", "enum": ["markdown", "html", "text", "links", "metadata"] },
                "max_bytes": { "type": "integer", "minimum": 1000, "maximum": 500000 }
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
            "Fetching web page".into()
        } else {
            "Fetched web page".into()
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
        "Fetched web page".into()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !tool_mode_allows_network(ctx) {
            return Ok(denied("web fetch is not allowed in the current agent mode"));
        }
        let url = match public_http_url_from_tool_args(args, "url") {
            Ok(url) => url,
            Err(reason) => return Ok(denied(&reason)),
        };
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| e.to_string())?;
        let data = fetch_http(&client, &args).await?;
        let status = data.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
        let output = structured_output("Fetched web page", data)?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: !(200..400).contains(&status),
        })
    }
}

fn denied(reason: &str) -> ToolAssessment {
    ToolAssessment {
        risk: RiskLevel::High,
        requires_approval: false,
        reason: reason.into(),
        affected_paths: vec![],
        network_access: NetworkAccess::AllowGet,
        writes_to_disk: false,
        runs_real_process: false,
        denied: true,
    }
}
