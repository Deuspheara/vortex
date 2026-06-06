use std::sync::Arc;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::BrowserSidecarClient;
use crate::shared::web::{
    public_http_url_from_tool_args, structured_output, tool_mode_allows_network,
};
use crate::tool::{AgentTool, default_finish_summary};

pub struct BrowserSnapshotTool {
    pub client: Arc<BrowserSidecarClient>,
}

#[async_trait]
impl AgentTool for BrowserSnapshotTool {
    fn name(&self) -> &'static str {
        "browser_snapshot"
    }

    fn description(&self) -> &'static str {
        "Open a URL in a sandboxed Playwright browser and return visible text, accessibility tree, DOM summary, and interactive elements."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "url": { "type": "string" },
                "wait_for": { "type": "string", "enum": ["load", "networkidle", "selector"] },
                "selector": { "type": "string" },
                "include_dom": { "type": "boolean" },
                "include_accessibility_tree": { "type": "boolean" }
            },
            "required": ["url"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::RunRealCommands,
            pack_policy: ToolPackPolicy::Only(vec![ToolPack::UiBrowser, ToolPack::General]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Globe
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Capturing browser snapshot".into()
        } else {
            "Captured browser snapshot".into()
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
        "Captured browser snapshot".into()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !tool_mode_allows_network(ctx) {
            return Ok(denied(
                "browser snapshot is not allowed in the current agent mode",
            ));
        }
        let url = match public_http_url_from_tool_args(args, "url") {
            Ok(url) => url,
            Err(reason) => return Ok(denied(&reason)),
        };
        Ok(ToolAssessment {
            risk: RiskLevel::High,
            requires_approval: true,
            reason: format!("opens {url} in a sandboxed browser"),
            affected_paths: vec![],
            network_access: NetworkAccess::AllowGet,
            writes_to_disk: false,
            runs_real_process: true,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        let data = self.client.snapshot(args).await?;
        let output = structured_output("Captured browser snapshot", data)?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
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
        runs_real_process: true,
        denied: true,
    }
}
