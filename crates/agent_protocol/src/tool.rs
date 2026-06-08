use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{AndroidToolEvidence, ModelId, OutputStreamKind, ToolCallId, ToolName, ToolPolicy};

#[derive(Clone)]
pub struct ToolOutputSink {
    pub emit: Arc<dyn Fn(OutputStreamKind, String) + Send + Sync>,
}

impl std::fmt::Debug for ToolOutputSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ToolOutputSink")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolAssessment {
    pub risk: crate::RiskLevel,
    pub requires_approval: bool,
    pub reason: String,
    pub affected_paths: Vec<std::path::PathBuf>,
    pub network_access: crate::NetworkAccess,
    pub writes_to_disk: bool,
    pub runs_real_process: bool,
    #[serde(default)]
    pub denied: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: ToolCallId,
    pub name: ToolName,
    pub output: String,
    pub is_error: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResultRange {
    pub path: PathBuf,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResultSummary {
    pub call_id: ToolCallId,
    pub tool: ToolName,
    pub summary: String,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub affected_paths: Vec<PathBuf>,
    #[serde(default)]
    pub ranges: Vec<ToolResultRange>,
    pub raw_handle: String,
    pub token_cost: usize,
    pub truncated: bool,
    #[serde(default)]
    pub next_actions: Vec<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub android_evidence: Option<AndroidToolEvidence>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    DependencyUpdate,
    CodeGeneration,
    BugFix,
    UiChange,
    TestFailure,
    Refactor,
    ArchitectureQuestion,
    Unknown,
}

impl Default for TaskClass {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextBudgetProfile {
    SmallTask,
    Normal,
    Deep,
}

impl Default for ContextBudgetProfile {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPack {
    Dependency,
    CodeEdit,
    UiBrowser,
    Research,
    GitCi,
    Planning,
    General,
}

impl Default for ToolPack {
    fn default() -> Self {
        Self::General
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPhase {
    Explore,
    Edit,
    Validate,
}

impl Default for ToolPhase {
    fn default() -> Self {
        Self::Explore
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: ToolName,
    pub description: String,
    pub parameters: Value,
    #[serde(default)]
    pub policy: ToolPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptToolSpec {
    pub name: ToolName,
    pub description: String,
    pub parameters: Value,
}

pub fn prompt_tool_payload(tools: &[PromptToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ModelProviderCapabilities {
    #[serde(default)]
    pub supports_prompt_cache_key: bool,
    #[serde(default)]
    pub supports_stateful_turns: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolContext {
    pub project_root: std::path::PathBuf,
    pub project_id: crate::ProjectId,
    pub session_id: crate::SessionId,
    pub run_id: crate::RunId,
    pub mode: crate::AgentMode,
    #[serde(skip)]
    pub output_sink: Option<ToolOutputSink>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ModelMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistantToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: ModelMessageRole,
    pub content: ModelMessageContent,
    pub tool_call_id: Option<ToolCallId>,
    pub name: Option<ToolName>,
    pub tool_calls: Option<Vec<AssistantToolCall>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ModelMessageContent {
    Text(String),
    Parts(Vec<ModelContentPart>),
}

impl ModelMessageContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.as_str()),
            Self::Parts(_) => None,
        }
    }

    pub fn to_text_lossy(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Parts(parts) => parts
                .iter()
                .filter_map(|part| match part {
                    ModelContentPart::Text { text } => Some(text.as_str()),
                    ModelContentPart::ImageUrl { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(text) => text.is_empty(),
            Self::Parts(parts) => parts.is_empty(),
        }
    }

    pub fn estimated_chars(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Parts(parts) => parts.iter().map(ModelContentPart::estimated_chars).sum(),
        }
    }

    pub fn cap_text(self, max_chars: usize) -> Self {
        match self {
            Self::Text(text) => Self::Text(cap_text_head_tail(&text, max_chars)),
            Self::Parts(parts) => Self::Parts(
                parts
                    .into_iter()
                    .map(|part| match part {
                        ModelContentPart::Text { text } => ModelContentPart::Text {
                            text: cap_text_head_tail(&text, max_chars),
                        },
                        image @ ModelContentPart::ImageUrl { .. } => image,
                    })
                    .collect(),
            ),
        }
    }
}

impl From<String> for ModelMessageContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ModelMessageContent {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModelContentPart {
    Text { text: String },
    ImageUrl { url: String, mime_type: String },
}

impl ModelContentPart {
    fn estimated_chars(&self) -> usize {
        match self {
            Self::Text { text } => text.len(),
            // Base64 chars are part of the provider request and should be reflected in estimates.
            Self::ImageUrl { url, mime_type } => url.len() + mime_type.len(),
        }
    }
}

fn cap_text_head_tail(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars || max_chars == 0 {
        return text.to_string();
    }
    let head = max_chars * 3 / 4;
    let tail = max_chars / 4;
    let head_str: String = text.chars().take(head).collect();
    let tail_str: String = text
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head_str}\n…[truncated to fit context budget]…\n{tail_str}")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: ModelId,
    pub messages: Vec<ModelMessage>,
    pub tools: Vec<PromptToolSpec>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ModelDelta {
    Text(String),
    Reasoning(String),
    ToolCallStarted {
        id: ToolCallId,
        name: String,
    },
    ToolCallArgumentsDelta {
        id: ToolCallId,
        json_delta: String,
    },
    ToolCallCompleted {
        id: ToolCallId,
        name: String,
        arguments: Value,
    },
    Usage(ModelUsage),
    Done,
}
