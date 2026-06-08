use std::path::PathBuf;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolCapabilities, ToolCategory,
    ToolContext, ToolModeGate, ToolPack, ToolPackPolicy, ToolPolicy, ToolResult,
    ToolSummaryArgPath, ToolSummaryArgRange, ToolSummaryPolicy,
};
use agent_sandbox::PathPolicy;
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::tool::{AgentTool, default_finish_summary};

pub struct ReadFileTool;

#[async_trait]
impl AgentTool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read a UTF-8 text file, optionally a 1-based line range. Large output is truncated with a continuation hint."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": { "type": "string", "description": "Relative path within the project" },
                "start_line": { "type": "integer", "description": "1-based first line" },
                "end_line": { "type": "integer", "description": "1-based last line" }
            },
            "required": ["path"]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::Read,
            parallel_safe: true,
            cache_output: true,
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
            summary: ToolSummaryPolicy {
                prefer_line_bounded_follow_up: true,
                arg_paths: vec![ToolSummaryArgPath {
                    field: "path".into(),
                    ..ToolSummaryArgPath::default()
                }],
                arg_range: Some(ToolSummaryArgRange {
                    path_field: "path".into(),
                    start_line_field: "start_line".into(),
                    end_line_field: "end_line".into(),
                }),
                ..ToolSummaryPolicy::default()
            },
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::File
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Reading file".into()
        } else {
            "Read file".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        let path = command.filter(|c| !c.is_empty() && *c != "{}").map(|p| {
            std::path::Path::new(p)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(p)
        });
        match path {
            Some(p) if running => format!("Reading {p}"),
            Some(p) => format!("Read {p}"),
            None if running => "Reading file".into(),
            None => "Read file".into(),
        }
    }

    fn finish_summary(&self, args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("file");
        format!("Read {path}")
    }

    fn args_preview(&self, args: &Value) -> String {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        match (
            args.get("start_line").and_then(|v| v.as_u64()),
            args.get("end_line").and_then(|v| v.as_u64()),
        ) {
            (Some(start), Some(end)) => format!("{path}:{start}-{end}"),
            (Some(start), None) => format!("{path}:{start}-"),
            (None, Some(end)) => format!("{path}:-{end}"),
            (None, None) => path,
        }
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        let path = args_path(args)?;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_read(&path)?;
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only file access".into(),
            affected_paths: vec![resolved],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let path = args_path(&args)?;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_read(&path)?;
        let content = std::fs::read_to_string(&resolved).map_err(|e| e.to_string())?;
        let start_line = args
            .get("start_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .max(1) as usize;
        let explicit_end = args
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        if let Some(end) = explicit_end {
            if end < start_line {
                return Err("end_line must be >= start_line".into());
            }
        }

        let all_lines: Vec<&str> = content.lines().collect();
        let total_lines = all_lines.len();
        let first_ix = start_line.saturating_sub(1);
        if first_ix >= total_lines && total_lines > 0 {
            return Err(format!(
                "start_line {start_line} is past end of file ({total_lines} lines)"
            ));
        }

        let max_lines = if explicit_end.is_some() {
            MAX_EXPLICIT_READ_LINES
        } else {
            DEFAULT_READ_LINES
        };
        let max_bytes = if explicit_end.is_some() {
            MAX_EXPLICIT_READ_BYTES
        } else {
            DEFAULT_READ_BYTES
        };

        // Hard cap so a single read can never blow the context budget.
        let requested_last = explicit_end.unwrap_or(total_lines);
        let cap_last = (first_ix + max_lines).min(requested_last);
        let last_ix = cap_last.min(total_lines);

        let mut out_lines: Vec<&str> = Vec::new();
        let mut bytes = 0usize;
        let mut byte_truncated_at: Option<usize> = None;
        for (offset, line) in all_lines[first_ix..last_ix].iter().enumerate() {
            bytes += line.len() + 1;
            if bytes > max_bytes && !out_lines.is_empty() {
                byte_truncated_at = Some(first_ix + offset);
                break;
            }
            out_lines.push(line);
        }

        let shown_last = byte_truncated_at.unwrap_or(last_ix); // exclusive index of last shown + 1
        let mut output = out_lines.join("\n");
        let truncated = shown_last < total_lines && shown_last < requested_last;
        if truncated {
            let next_start = shown_last + 1;
            output.push_str(&format!(
                "\n\n[truncated: showing lines {}-{} of {}; call read_file with start_line={} to continue]",
                start_line, shown_last, total_lines, next_start
            ));
        }
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error: false,
        })
    }
}

/// Maximum lines returned by a single `read_file` call before truncation.
const DEFAULT_READ_LINES: usize = 250;
const MAX_EXPLICIT_READ_LINES: usize = 400;
/// Maximum bytes returned by a single `read_file` call before truncation.
const DEFAULT_READ_BYTES: usize = 16 * 1024;
const MAX_EXPLICIT_READ_BYTES: usize = 24 * 1024;

fn args_path(args: &Value) -> Result<PathBuf, String> {
    args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "missing path".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::runtime::Runtime;

    #[test]
    fn read_file_truncates_large_default_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        let body = (0..300)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, body).unwrap();

        let tool = ReadFileTool;
        let runtime = Runtime::new().unwrap();
        let result = runtime
            .block_on(tool.execute(
                json!({ "path": "big.txt" }),
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
