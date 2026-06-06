use std::time::Duration;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::web::{search_with_providers, structured_output, tool_mode_allows_network};
use crate::tool::{AgentTool, default_finish_summary};

pub struct WebSearchTool;

#[async_trait]
impl AgentTool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Search the web through configured providers. Returns untrusted source results with provider metadata."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": { "type": "string" },
                "max_results": { "type": "integer", "minimum": 1, "maximum": 20 },
                "freshness": { "type": "string", "enum": ["any", "day", "week", "month", "year"] },
                "domains": { "type": "array", "items": { "type": "string" } },
                "providers": {
                    "oneOf": [
                        { "type": "string", "enum": ["auto", "exa", "tavily", "jina", "firecrawl", "openai", "anthropic"] },
                        { "type": "array", "items": { "type": "string", "enum": ["auto", "exa", "tavily", "jina", "firecrawl", "openai", "anthropic"] } }
                    ]
                }
            },
            "required": ["query"]
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
            "Searching web".into()
        } else {
            "Searched web".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("query")
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
        "Searched web".into()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !tool_mode_allows_network(ctx) {
            return Ok(denied(
                "web search is not allowed in the current agent mode",
            ));
        }
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if query.trim().is_empty() {
            return Ok(denied("query is required"));
        }
        Ok(ToolAssessment {
            risk: RiskLevel::High,
            requires_approval: true,
            reason: format!("searches the web for {query}"),
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
            .build()
            .map_err(|e| e.to_string())?;
        let (results, warnings) = search_with_providers(&client, &args).await;
        let summary = format!("Web search returned {} result(s)", results.len());
        let output = structured_output(
            summary,
            json!({
                "results": results,
                "warnings": warnings,
                "untrusted": true
            }),
        )?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: results.is_empty(),
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
