use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::provenance::{Confidence, SourceProvenance};
use crate::shared::web::{structured_output, truncate_chars};
use crate::tool::AgentTool;

pub struct WebExtractTool;

#[async_trait]
impl AgentTool for WebExtractTool {
    fn name(&self) -> &'static str {
        "web_extract"
    }

    fn description(&self) -> &'static str {
        "Extract structured data from already-fetched untrusted text or a provider result. V1 is deterministic and does not call a model."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "source": {},
                "schema": {},
                "instruction": { "type": "string" },
                "provider": { "type": "string", "enum": ["auto", "tavily", "firecrawl", "model"] }
            },
            "required": ["source", "schema"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::Only(vec![ToolPack::Research, ToolPack::General]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Globe
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Extracting web data".into()
        } else {
            "Extracted web data".into()
        }
    }

    async fn assess(&self, _args: &Value, _ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "deterministic extraction from provided untrusted source text".into(),
            affected_paths: vec![],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        let source = args.get("source").cloned().unwrap_or(Value::Null);
        let schema = args.get("schema").cloned().unwrap_or(Value::Null);
        let provider = args
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        let source_text = source_to_text(&source);
        let (sample, truncated) = truncate_chars(&source_text, 12_000);
        let warnings = provider_warnings(provider, truncated);
        let output = structured_output(
            "Prepared web extraction input",
            json!({
                "extraction": null,
                "schema": schema,
                "source_sample": sample,
                "confidence": Confidence::Low,
                "warnings": warnings,
                "provenance": SourceProvenance::new(None, None, "local", "schema_prepare"),
                "untrusted": true
            }),
        )?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
        })
    }
}

fn source_to_text(source: &Value) -> String {
    if let Some(s) = source.as_str() {
        return s.to_string();
    }
    serde_json::to_string_pretty(source).unwrap_or_else(|_| source.to_string())
}

fn provider_warnings(provider: &str, truncated: bool) -> Vec<String> {
    let mut warnings = Vec::new();
    if matches!(provider, "tavily" | "firecrawl" | "model") {
        warnings.push(format!(
            "{provider} extraction adapter is reserved; v1 prepared deterministic extraction input only"
        ));
    }
    if truncated {
        warnings.push("source sample truncated to 12000 characters".into());
    }
    warnings
}
