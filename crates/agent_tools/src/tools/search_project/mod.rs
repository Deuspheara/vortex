use std::path::PathBuf;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use project_index::{ProjectIndex, SearchOptions};
use serde_json::{Value, json};

use crate::tool::{AgentTool, default_finish_summary};

pub struct SearchProjectTool;

#[async_trait]
impl AgentTool for SearchProjectTool {
    fn name(&self) -> &'static str {
        "search_project"
    }

    fn description(&self) -> &'static str {
        "Search file contents (literal, or regex when regex=true) with glob/path filters and context lines. Prefer over grep."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": { "type": "string", "description": "Literal text, or regex when regex=true" },
                "path": { "type": "string", "description": "Relative directory to scope to" },
                "include": { "type": "string", "description": "Glob to limit files, e.g. src/**/*.rs" },
                "exclude": { "type": "string", "description": "Glob to exclude files" },
                "regex": { "type": "boolean", "default": false },
                "case_sensitive": { "type": "boolean", "default": false },
                "context_before": { "type": "integer", "default": 0 },
                "context_after": { "type": "integer", "default": 0 },
                "names_only": { "type": "boolean", "default": false },
                "max_matches_per_file": { "type": "integer", "default": 10 },
                "max_hits": { "type": "integer", "default": 50 },
                "respect_git_ignore": { "type": "boolean", "default": true }
            },
            "required": ["query"]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::Search,
            parallel_safe: true,
            cache_output: false,
            persist_result_body: true,
            suppress_live_output: true,
        }
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
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Search
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Searching project".into()
        } else {
            "Searched project".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        match command.filter(|c| !c.is_empty() && *c != "{}") {
            Some(q) if running => format!("Searching for “{q}”"),
            Some(q) => format!("Searched for “{q}”"),
            None if running => "Searching project".into(),
            None => "Searched project".into(),
        }
    }

    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), output, true);
        }
        let count = output.lines().filter(|l| !l.is_empty()).count();
        format!("Found {count} matches")
    }

    fn args_preview(&self, args: &Value) -> String {
        let mut parts = Vec::new();
        if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
            parts.push(query.chars().take(80).collect::<String>());
        }
        if let Some(include) = args.get("include").and_then(|v| v.as_str()) {
            parts.push(format!("in {include}"));
        }
        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            parts.push(format!("@ {path}"));
        }
        if args.get("regex").and_then(|v| v.as_bool()).unwrap_or(false) {
            parts.push("regex".into());
        }
        parts.join(" · ")
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only search".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing query".to_string())?;
        let index = ProjectIndex::new(&ctx.project_root);
        let options = SearchOptions {
            query: query.to_string(),
            base: args.get("path").and_then(|v| v.as_str()).map(PathBuf::from),
            include: args
                .get("include")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            exclude: args
                .get("exclude")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            regex: args.get("regex").and_then(|v| v.as_bool()).unwrap_or(false),
            case_sensitive: args
                .get("case_sensitive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            context_before: args
                .get("context_before")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize,
            context_after: args
                .get("context_after")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize,
            max_hits: args.get("max_hits").and_then(|v| v.as_u64()).unwrap_or(12) as usize,
            max_matches_per_file: args
                .get("max_matches_per_file")
                .and_then(|v| v.as_u64())
                .unwrap_or(3) as usize,
            names_only: args
                .get("names_only")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            respect_git_ignore: args
                .get("respect_git_ignore")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        };
        let hits = index
            .search_with_options(&options)
            .map_err(|e| e.to_string())?;
        let mut output = hits
            .iter()
            .map(|h| {
                if options.names_only {
                    return h.path.display().to_string();
                }

                let mut lines = Vec::new();
                for before in &h.before {
                    lines.push(format!(
                        "{}-{}- {}",
                        h.path.display(),
                        before.line_number,
                        before.line
                    ));
                }
                lines.push(format!(
                    "{}:{}: {}",
                    h.path.display(),
                    h.line_number,
                    h.line
                ));
                for after in &h.after {
                    lines.push(format!(
                        "{}+{}+ {}",
                        h.path.display(),
                        after.line_number,
                        after.line
                    ));
                }
                lines.join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if output.len() > MAX_OUTPUT_BYTES {
            let mut truncated = output.chars().take(MAX_OUTPUT_BYTES).collect::<String>();
            truncated.push_str(
                "\n\n[truncated: narrow query, include path filter, or reduce context lines]",
            );
            output = truncated;
        }
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
        })
    }
}

const MAX_OUTPUT_BYTES: usize = 12 * 1024;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::runtime::Runtime;

    #[test]
    fn search_project_truncates_large_output() {
        let dir = tempfile::tempdir().unwrap();
        let body = (0..1000)
            .map(|i| format!("needle line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir.path().join("big.txt"), body).unwrap();
        let tool = SearchProjectTool;
        let runtime = Runtime::new().unwrap();
        let result = runtime
            .block_on(tool.execute(
                json!({ "query": "needle", "max_hits": 200, "max_matches_per_file": 200 }),
                ToolContext {
                    project_root: dir.path().to_path_buf(),
                    project_id: agent_protocol::ProjectId::new("p"),
                    session_id: agent_protocol::SessionId::new("s"),
                    run_id: agent_protocol::RunId::new("r"),
                    mode: agent_protocol::AgentMode::ReadOnlyInspect,
                    output_sink: None,
                },
            ))
            .unwrap();
        assert!(result.output.contains("[truncated:"));
    }
}
