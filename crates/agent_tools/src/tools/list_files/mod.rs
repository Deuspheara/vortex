use std::path::PathBuf;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use project_index::{FileSort, ListFilesOptions, ProjectIndex};
use serde_json::{Value, json};

use crate::tool::{AgentTool, default_finish_summary};

pub struct ListFilesTool;

#[async_trait]
impl AgentTool for ListFilesTool {
    fn name(&self) -> &'static str {
        "list_files"
    }

    fn description(&self) -> &'static str {
        "List project files by glob/path with sort order. Respects .gitignore. Prefer over find/ls."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "Relative directory to scope to" },
                "pattern": { "type": "string", "description": "Glob, e.g. src/**/*.rs" },
                "exclude": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Globs to exclude"
                },
                "respect_git_ignore": { "type": "boolean", "default": true },
                "sort_by": {
                    "type": "string",
                    "enum": ["path", "modified_desc"],
                    "default": "path"
                },
                "max_files": { "type": "integer", "default": 200 }
            }
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::Read,
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
        IconToken::Folder
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Listing files".into()
        } else {
            "Listed files".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        let path = command.filter(|c| !c.is_empty() && *c != "{}");
        match path {
            Some(p) if p.starts_with("max ") => {
                if running {
                    format!("Listing {p}")
                } else {
                    format!("Listed {p}")
                }
            }
            Some(p) if running => format!("Listing {p}"),
            Some(p) => format!("Listed {p}"),
            None if running => "Listing files".into(),
            None => "Listed files".into(),
        }
    }

    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), output, true);
        }
        let count = output.lines().filter(|l| !l.is_empty()).count();
        format!("Listed {count} files")
    }

    fn args_preview(&self, args: &Value) -> String {
        let mut parts = Vec::new();
        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            parts.push(path.to_string());
        }
        if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
            parts.push(format!("glob {pattern}"));
        }
        if let Some(sort_by) = args.get("sort_by").and_then(|v| v.as_str()) {
            if sort_by == "modified_desc" {
                parts.push("newest first".into());
            }
        }
        if let Some(max) = args.get("max_files").and_then(|v| v.as_u64()) {
            parts.push(format!("max {max}"));
        }
        parts.join(" · ")
    }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only directory listing".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let index = ProjectIndex::new(&ctx.project_root);
        let options = ListFilesOptions {
            base: args.get("path").and_then(|v| v.as_str()).map(PathBuf::from),
            pattern: args
                .get("pattern")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            exclude: args
                .get("exclude")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            max_files: args
                .get("max_files")
                .and_then(|v| v.as_u64())
                .unwrap_or(200) as usize,
            respect_git_ignore: args
                .get("respect_git_ignore")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            sort_by: match args.get("sort_by").and_then(|v| v.as_str()) {
                Some("modified_desc") => FileSort::ModifiedDesc,
                _ => FileSort::PathAsc,
            },
        };
        let files = index
            .list_files_with_options(&options)
            .map_err(|e| e.to_string())?;
        let output = files
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
        })
    }
}
