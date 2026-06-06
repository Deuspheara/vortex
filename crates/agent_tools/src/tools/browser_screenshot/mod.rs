use std::path::PathBuf;
use std::sync::Arc;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::BrowserMcpClient;
use crate::shared::web::{
    public_http_url_from_tool_args, structured_output, tool_mode_allows_network,
};
use crate::tool::{AgentTool, default_finish_summary};

pub struct BrowserScreenshotTool {
    pub client: Arc<BrowserMcpClient>,
    pub artifact_dir: PathBuf,
}

#[async_trait]
impl AgentTool for BrowserScreenshotTool {
    fn name(&self) -> &'static str {
        "browser_screenshot"
    }

    fn description(&self) -> &'static str {
        "Open a URL through the configured browser MCP server and save a full-page or selector screenshot to the controlled artifact cache."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "url": { "type": "string" },
                "selector": { "type": "string" },
                "full_page": { "type": "boolean" },
                "wait_for": { "type": "string", "enum": ["load", "networkidle", "selector"] }
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
            "Taking browser screenshot".into()
        } else {
            "Took browser screenshot".into()
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
        "Took browser screenshot".into()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if let Some(reason) = self.client.unavailable_reason() {
            return Ok(denied(&reason));
        }
        if !tool_mode_allows_network(ctx) {
            return Ok(denied(
                "browser screenshot is not allowed in the current agent mode",
            ));
        }
        let url = match public_http_url_from_tool_args(args, "url") {
            Ok(url) => url,
            Err(reason) => return Ok(denied(&reason)),
        };
        Ok(ToolAssessment {
            risk: RiskLevel::High,
            requires_approval: true,
            reason: format!("opens {url} and writes a screenshot artifact"),
            affected_paths: vec![self.artifact_dir.clone()],
            network_access: NetworkAccess::AllowGet,
            writes_to_disk: true,
            runs_real_process: true,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        std::fs::create_dir_all(&self.artifact_dir).map_err(|e| e.to_string())?;
        let path = self
            .artifact_dir
            .join(format!("browser-{}.png", uuid::Uuid::new_v4()));
        let mut params = args.as_object().cloned().unwrap_or_default();
        params.insert(
            "output_path".into(),
            Value::String(path.to_string_lossy().to_string()),
        );
        let mut data = self.client.screenshot(Value::Object(params)).await?;
        if let Value::Object(ref mut object) = data {
            let path = Value::String(path.to_string_lossy().to_string());
            object.entry("output_path").or_insert_with(|| path.clone());
            object.entry("image_path").or_insert(path);
        }
        let output = structured_output("Took browser screenshot", data)?;
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
