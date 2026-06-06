use std::path::Path;

use agent_protocol::{IconToken, ToolCapabilities, ToolDescriptor, ToolPolicy};
use async_trait::async_trait;
use serde_json::Value;

pub use agent_protocol::ToolAssessment;

pub fn default_finish_summary(label: &str, output: &str, is_error: bool) -> String {
    if is_error {
        let detail = output.lines().next().unwrap_or("failed");
        return format!(
            "{label} failed: {}",
            detail.chars().take(80).collect::<String>()
        );
    }
    label.to_string()
}

#[async_trait]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> Value;

    async fn assess(
        &self,
        args: &Value,
        ctx: &agent_protocol::ToolContext,
    ) -> Result<ToolAssessment, String>;

    async fn execute(
        &self,
        args: Value,
        ctx: agent_protocol::ToolContext,
    ) -> Result<agent_protocol::ToolResult, String>;

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities::default()
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy::default()
    }

    fn icon(&self) -> IconToken {
        IconToken::Terminal
    }

    fn label(&self, running: bool) -> String {
        let name = self.name();
        if running {
            format!("Running {name}")
        } else {
            name.to_string()
        }
    }

    /// One-line label with optional args preview folded in (used by thread UI).
    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        let cleaned = command.and_then(crate::shared::sanitize_display_arg);
        let preview = cleaned.as_deref();
        match preview {
            Some(cmd) if running => format!("{} {cmd}", self.label(true)),
            Some(cmd) => format!("{} {cmd}", self.label(false)),
            None => self.label(running),
        }
    }

    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        default_finish_summary(&self.label(false), output, is_error)
    }

    fn args_preview(&self, args: &Value) -> String {
        if args.as_object().is_some_and(|o| o.is_empty()) {
            return String::new();
        }
        for key in [
            "path", "query", "script", "command", "url", "symbol", "name",
        ] {
            if let Some(s) = args.get(key).and_then(|v| v.as_str()) {
                let line = s.lines().next().unwrap_or(s);
                return line.chars().take(120).collect();
            }
        }
        String::new()
    }

    fn streaming_patch_preview(&self, _tool_args: &str, _project_root: &Path) -> Option<String> {
        None
    }

    fn streaming_args_preview(&self, raw_json: &str) -> Option<String> {
        let partial = crate::shared::partial_args_from_schema(&self.schema(), raw_json)?;
        crate::shared::sanitize_display_arg(&self.args_preview(&partial))
    }

    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            name: self.name().to_string(),
            description: self.description().to_string(),
            icon: self.icon(),
            capabilities: self.capabilities(),
            policy: self.policy(),
        }
    }
}
