use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use agent_protocol::{
    AgentError, CancellationToken, ModelContentPart, ModelDelta, ModelMessage, ModelMessageContent,
    ModelMessageRole, ModelRequest, ModelUsage, ToolCallId,
};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::inline_tool_calls::{InlineToolCall, InlineToolCallParser};
use crate::{ModelProvider, ModelStream};

pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Clone)]
pub struct OpenRouterModelInfo {
    pub id: String,
    pub name: String,
    pub context_length: Option<u64>,
    pub prompt_per_token: f64,
    pub completion_per_token: f64,
    pub supports_image_input: bool,
}

impl OpenRouterProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            api_key: api_key.into(),
            base_url: "https://openrouter.ai/api/v1".into(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Fetch tool-capable text models from OpenRouter.
    pub async fn list_models(&self) -> Result<Vec<OpenRouterModelInfo>, AgentError> {
        let response = self
            .client
            .get(format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AgentError::Provider(openrouter_http_error(status, &text)));
        }

        let parsed: ModelsListResponse = response
            .json()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        let mut models: Vec<OpenRouterModelInfo> = parsed
            .data
            .into_iter()
            .filter(|m| model_supports_tools(m) && model_outputs_text(m))
            .filter_map(|m| {
                let prompt = parse_price(&m.pricing.prompt)?;
                let completion = parse_price(&m.pricing.completion)?;
                let supports_image_input = model_supports_image_input(&m);
                Some(OpenRouterModelInfo {
                    id: m.id,
                    name: m.name,
                    context_length: m.context_length,
                    prompt_per_token: prompt,
                    completion_per_token: completion,
                    supports_image_input,
                })
            })
            .collect();

        models.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(models)
    }
}

fn serialize_message(message: &ModelMessage) -> serde_json::Value {
    let role = match message.role {
        ModelMessageRole::System => "system",
        ModelMessageRole::User => "user",
        ModelMessageRole::Assistant => "assistant",
        ModelMessageRole::Tool => "tool",
    };

    match message.role {
        ModelMessageRole::Tool => {
            let mut obj = serde_json::json!({
                "role": role,
                "content": message.content.to_text_lossy(),
            });
            if let Some(tool_call_id) = &message.tool_call_id {
                obj["tool_call_id"] = serde_json::Value::String(tool_call_id.0.clone());
            }
            if let Some(name) = &message.name {
                obj["name"] = serde_json::Value::String(name.clone());
            }
            obj
        }
        ModelMessageRole::Assistant if message.tool_calls.is_some() => {
            let tool_calls: Vec<serde_json::Value> = message
                .tool_calls
                .as_ref()
                .map(|calls| {
                    calls
                        .iter()
                        .map(|call| {
                            serde_json::json!({
                                "id": call.id.0,
                                "type": "function",
                                "function": {
                                    "name": call.name,
                                    "arguments": call.arguments.to_string(),
                                }
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let content = if message.content.is_empty() {
                serde_json::Value::Null
            } else {
                serialize_content(&message.content)
            };

            serde_json::json!({
                "role": role,
                "content": content,
                "tool_calls": tool_calls,
            })
        }
        _ => serde_json::json!({
            "role": role,
            "content": serialize_content(&message.content),
        }),
    }
}

fn serialize_content(content: &ModelMessageContent) -> serde_json::Value {
    match content {
        ModelMessageContent::Text(text) => serde_json::Value::String(text.clone()),
        ModelMessageContent::Parts(parts) => serde_json::Value::Array(
            parts
                .iter()
                .map(|part| match part {
                    ModelContentPart::Text { text } => serde_json::json!({
                        "type": "text",
                        "text": text,
                    }),
                    ModelContentPart::ImageUrl { url, .. } => serde_json::json!({
                        "type": "image_url",
                        "image_url": {
                            "url": url,
                        },
                    }),
                })
                .collect(),
        ),
    }
}

fn serialize_tools(tools: &[agent_protocol::ToolSpec]) -> Vec<serde_json::Value> {
    let last = tools.len().saturating_sub(1);
    tools
        .iter()
        .enumerate()
        .map(|(ix, tool)| {
            let mut value = serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            });
            // A single cache breakpoint after the last tool covers the whole (stable) tool block.
            if ix == last {
                value["cache_control"] = serde_json::json!({ "type": "ephemeral" });
            }
            value
        })
        .collect()
}

/// Rewrite a message's plain string content into a single cacheable text block so providers that
/// support prompt caching (e.g. Anthropic via OpenRouter) can reuse the stable system prefix.
fn with_cache_control(mut message: serde_json::Value) -> serde_json::Value {
    if let Some(text) = message.get("content").and_then(|c| c.as_str()) {
        message["content"] = serde_json::json!([
            {
                "type": "text",
                "text": text,
                "cache_control": { "type": "ephemeral" }
            }
        ]);
    }
    message
}

#[derive(Debug, Deserialize)]
struct ModelsListResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    name: String,
    context_length: Option<u64>,
    pricing: ModelPricing,
    architecture: ModelArchitecture,
    supported_parameters: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModelPricing {
    prompt: String,
    completion: String,
}

#[derive(Debug, Deserialize)]
struct ModelArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    output_modalities: Vec<String>,
}

fn parse_price(raw: &str) -> Option<f64> {
    raw.parse().ok()
}

fn model_supports_tools(model: &ModelEntry) -> bool {
    model.supported_parameters.iter().any(|p| p == "tools")
}

fn model_outputs_text(model: &ModelEntry) -> bool {
    model
        .architecture
        .output_modalities
        .iter()
        .any(|m| m == "text")
}

fn model_supports_image_input(model: &ModelEntry) -> bool {
    model
        .architecture
        .input_modalities
        .iter()
        .any(|m| m == "image")
}

#[derive(Debug, Deserialize)]
struct ChatResponseChunk {
    choices: Vec<ChatChoice>,
    usage: Option<UsageChunk>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    delta: DeltaChunk,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DeltaChunk {
    content: Option<String>,
    reasoning: Option<String>,
    tool_calls: Option<Vec<DeltaToolCall>>,
}

#[derive(Debug, Deserialize, Default)]
struct DeltaToolCall {
    index: Option<usize>,
    id: Option<String>,
    function: Option<DeltaToolFunction>,
}

#[derive(Debug, Deserialize, Default)]
struct DeltaToolFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageChunk {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    cost: Option<f64>,
    prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    cached_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

struct PartialToolCall {
    id: ToolCallId,
    name: String,
    arguments: String,
    started: bool,
}

struct SseParser {
    pending_tools: HashMap<usize, PartialToolCall>,
    inline_parser: InlineToolCallParser,
    done: bool,
}

impl SseParser {
    fn new<I, S>(tool_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            pending_tools: HashMap::new(),
            inline_parser: InlineToolCallParser::with_tool_names(tool_names),
            done: false,
        }
    }

    fn push_inline_tool_deltas(
        &mut self,
        calls: Vec<InlineToolCall>,
        deltas: &mut Vec<Result<ModelDelta, AgentError>>,
    ) {
        for call in calls {
            let id = ToolCallId::new(self.inline_parser.next_tool_id());
            let args_str =
                serde_json::to_string(&call.arguments).unwrap_or_else(|_| "{}".to_string());
            deltas.push(Ok(ModelDelta::ToolCallStarted {
                id: id.clone(),
                name: call.name.clone(),
            }));
            deltas.push(Ok(ModelDelta::ToolCallArgumentsDelta {
                id: id.clone(),
                json_delta: args_str,
            }));
            deltas.push(Ok(ModelDelta::ToolCallCompleted {
                id,
                name: call.name,
                arguments: call.arguments,
            }));
        }
    }

    fn parse_line(&mut self, line: &str) -> Vec<Result<ModelDelta, AgentError>> {
        let line = line.trim();
        if !line.starts_with("data:") {
            return Vec::new();
        }
        let payload = line.trim_start_matches("data:").trim();
        if payload == "[DONE]" {
            self.done = true;
            self.inline_parser.reset();
            let mut deltas = self.finalize_tool_calls();
            deltas.push(Ok(ModelDelta::Done));
            return deltas;
        }

        let parsed: ChatResponseChunk = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(err) => {
                warn!("openrouter: skipped malformed SSE JSON: {err}");
                return Vec::new();
            }
        };

        let mut deltas = Vec::new();
        if let Some(choice) = parsed.choices.first() {
            if let Some(content) = &choice.delta.content {
                if !content.is_empty() {
                    let parsed = self.inline_parser.push(content);
                    if !parsed.clean_text.is_empty() {
                        deltas.push(Ok(ModelDelta::Text(parsed.clean_text)));
                    }
                    self.push_inline_tool_deltas(parsed.calls, &mut deltas);
                }
            }
            if let Some(reasoning) = &choice.delta.reasoning {
                if !reasoning.is_empty() {
                    deltas.push(Ok(ModelDelta::Reasoning(reasoning.clone())));
                }
            }
            if let Some(tool_calls) = &choice.delta.tool_calls {
                for tool_call in tool_calls {
                    let index = tool_call.index.unwrap_or(0);
                    let entry =
                        self.pending_tools
                            .entry(index)
                            .or_insert_with(|| PartialToolCall {
                                id: ToolCallId::new(String::new()),
                                name: String::new(),
                                arguments: String::new(),
                                started: false,
                            });

                    if let Some(id) = &tool_call.id {
                        if !id.is_empty() {
                            entry.id = ToolCallId::new(id.clone());
                        }
                    }
                    if let Some(function) = &tool_call.function {
                        if let Some(name) = &function.name {
                            if !name.is_empty() {
                                entry.name = name.clone();
                            }
                        }
                        if let Some(args) = &function.arguments {
                            if !args.is_empty() {
                                entry.arguments.push_str(args);
                                if !entry.id.0.is_empty() {
                                    deltas.push(Ok(ModelDelta::ToolCallArgumentsDelta {
                                        id: entry.id.clone(),
                                        json_delta: args.clone(),
                                    }));
                                }
                            }
                        }
                    }

                    if !entry.started && !entry.id.0.is_empty() && !entry.name.is_empty() {
                        entry.started = true;
                        deltas.push(Ok(ModelDelta::ToolCallStarted {
                            id: entry.id.clone(),
                            name: entry.name.clone(),
                        }));
                    }
                }
            }

            if choice.finish_reason.as_deref() == Some("tool_calls") {
                deltas.extend(self.finalize_tool_calls());
            }
        }

        if let Some(usage) = parsed.usage {
            deltas.push(Ok(ModelDelta::Usage(ModelUsage {
                input_tokens: usage.prompt_tokens.unwrap_or(0),
                output_tokens: usage.completion_tokens.unwrap_or(0),
                cache_read_tokens: usage
                    .prompt_tokens_details
                    .as_ref()
                    .and_then(|d| d.cached_tokens),
                cache_write_tokens: usage
                    .prompt_tokens_details
                    .as_ref()
                    .and_then(|d| d.cache_write_tokens),
                cost_usd: usage.cost,
            })));
        }

        deltas
    }

    fn finalize_tool_calls(&mut self) -> Vec<Result<ModelDelta, AgentError>> {
        let mut indices: Vec<_> = self.pending_tools.keys().copied().collect();
        indices.sort_unstable();
        let mut deltas = Vec::new();
        for index in indices {
            let Some(partial) = self.pending_tools.remove(&index) else {
                continue;
            };
            if partial.id.0.is_empty() || partial.name.is_empty() {
                continue;
            }
            let arguments = if partial.arguments.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&partial.arguments)
                    .unwrap_or_else(|_| serde_json::Value::String(partial.arguments.clone()))
            };
            deltas.push(Ok(ModelDelta::ToolCallCompleted {
                id: partial.id,
                name: partial.name,
                arguments,
            }));
        }
        deltas
    }

    fn finish(&mut self) -> Vec<Result<ModelDelta, AgentError>> {
        let mut deltas = self.finalize_tool_calls();
        if !self.done {
            deltas.push(Ok(ModelDelta::Done));
            self.done = true;
        }
        deltas
    }
}

#[async_trait]
impl ModelProvider for OpenRouterProvider {
    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, AgentError> {
        let mut messages: Vec<serde_json::Value> =
            request.messages.iter().map(serialize_message).collect();
        // Cache the (stable) system prefix so repeated turns reuse it.
        if let Some(first) = messages.first().cloned() {
            if first.get("role").and_then(|r| r.as_str()) == Some("system") {
                messages[0] = with_cache_control(first);
            }
        }

        let mut body = serde_json::json!({
            "model": request.model.0,
            "messages": messages,
            "stream": true,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
        });

        if !request.tools.is_empty() {
            body["tools"] = serde_json::Value::Array(serialize_tools(&request.tools));
            body["tool_choice"] = serde_json::Value::String("auto".into());
        }

        info!(
            model = %request.model.0,
            messages = request.messages.len(),
            tools = request.tools.len(),
            "openrouter request"
        );

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(AgentError::Provider(openrouter_http_error(status, &text)));
        }

        let byte_stream = response.bytes_stream();
        let (tx, rx) = mpsc::unbounded_channel();

        let inline_tool_names: Vec<String> =
            request.tools.iter().map(|tool| tool.name.clone()).collect();

        tokio::spawn(async move {
            let mut parser = SseParser::new(inline_tool_names);
            let mut buffer = String::new();
            let mut byte_stream = byte_stream;

            while let Some(chunk_result) = byte_stream.next().await {
                if cancel.is_cancelled() {
                    let _ = tx.send(Err(AgentError::Cancelled));
                    return;
                }

                let chunk = match chunk_result {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        let _ = tx.send(Err(AgentError::Provider(err.to_string())));
                        return;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(newline_idx) = buffer.find('\n') {
                    let line = buffer[..newline_idx].to_string();
                    buffer.drain(..=newline_idx);
                    for delta in parser.parse_line(&line) {
                        if tx.send(delta).is_err() {
                            return;
                        }
                    }
                }
            }

            if !buffer.trim().is_empty() {
                for delta in parser.parse_line(&buffer) {
                    if tx.send(delta).is_err() {
                        return;
                    }
                }
            }

            for delta in parser.finish() {
                if tx.send(delta).is_err() {
                    return;
                }
            }
        });

        let stream = UnboundedReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }
}

fn openrouter_http_error(status: reqwest::StatusCode, body: &str) -> String {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return format!(
            "OpenRouter authentication failed (HTTP {status}). Check the API key and account access."
        );
    }
    let body = body.trim();
    if body.is_empty() {
        format!("OpenRouter request failed (HTTP {status})")
    } else {
        format!("OpenRouter request failed (HTTP {status}): {body}")
    }
}

struct UnboundedReceiverStream<T> {
    rx: mpsc::UnboundedReceiver<T>,
}

impl<T> UnboundedReceiverStream<T> {
    fn new(rx: mpsc::UnboundedReceiver<T>) -> Self {
        Self { rx }
    }
}

impl<T> Stream for UnboundedReceiverStream<T> {
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::{ModelContentPart, ModelDelta, ModelMessageContent, ModelMessageRole};

    #[test]
    fn sse_parser_emits_all_parallel_tool_completions() {
        let mut parser = SseParser::new(["read_file"]);
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[
            {"index":0,"id":"call_a","function":{"name":"read_file","arguments":"{\"path\":\"a.md\"}"}},
            {"index":1,"id":"call_b","function":{"name":"read_file","arguments":"{\"path\":\"b.md\"}"}},
            {"index":2,"id":"call_c","function":{"name":"read_file","arguments":"{\"path\":\"c.md\"}"}}
        ]},"finish_reason":"tool_calls"}]}"#;

        let deltas: Vec<ModelDelta> = parser
            .parse_line(line)
            .into_iter()
            .map(|r| r.expect("delta"))
            .collect();

        let started: Vec<_> = deltas
            .iter()
            .filter_map(|d| match d {
                ModelDelta::ToolCallStarted { id, name } => Some((id.0.as_str(), name.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(started.len(), 3);

        let completed: Vec<_> = deltas
            .iter()
            .filter_map(|d| match d {
                ModelDelta::ToolCallCompleted { id, name, .. } => {
                    Some((id.0.as_str(), name.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(completed.len(), 3);
        assert!(completed.iter().any(|(id, _)| *id == "call_a"));
        assert!(completed.iter().any(|(id, _)| *id == "call_b"));
        assert!(completed.iter().any(|(id, _)| *id == "call_c"));
    }

    #[test]
    fn sse_parser_skips_arg_deltas_without_tool_id() {
        let mut parser = SseParser::new(["read_file"]);
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[
            {"index":0,"function":{"name":"read_file","arguments":"{\"path\":"}}
        ]}}]}"#;

        let deltas: Vec<ModelDelta> = parser
            .parse_line(line)
            .into_iter()
            .map(|r| r.expect("delta"))
            .collect();

        assert!(
            deltas
                .iter()
                .all(|d| !matches!(d, ModelDelta::ToolCallArgumentsDelta { .. }))
        );
    }

    #[test]
    fn sse_parser_parses_cache_usage_and_cost() {
        let mut parser = SseParser::new(["edit_file"]);
        let line = r#"data: {"choices":[],"usage":{"prompt_tokens":10339,"completion_tokens":60,"cost":0.0123,"prompt_tokens_details":{"cached_tokens":10318,"cache_write_tokens":0}}}"#;

        let deltas: Vec<ModelDelta> = parser
            .parse_line(line)
            .into_iter()
            .map(|r| r.expect("delta"))
            .collect();

        let usage = deltas
            .iter()
            .find_map(|d| match d {
                ModelDelta::Usage(u) => Some(u.clone()),
                _ => None,
            })
            .expect("usage delta");
        assert_eq!(usage.input_tokens, 10339);
        assert_eq!(usage.output_tokens, 60);
        assert_eq!(usage.cache_read_tokens, Some(10318));
        assert_eq!(usage.cache_write_tokens, Some(0));
        assert_eq!(usage.cost_usd, Some(0.0123));
    }

    #[test]
    fn model_entry_detects_image_input_modality() {
        let model = ModelEntry {
            id: "google/gemini".into(),
            name: "Gemini".into(),
            context_length: Some(1_000_000),
            pricing: ModelPricing {
                prompt: "0.0".into(),
                completion: "0.0".into(),
            },
            architecture: ModelArchitecture {
                input_modalities: vec!["text".into(), "image".into()],
                output_modalities: vec!["text".into()],
            },
            supported_parameters: vec!["tools".into()],
        };

        assert!(model_supports_image_input(&model));
        assert!(model_supports_tools(&model));
        assert!(model_outputs_text(&model));
    }

    #[test]
    fn serialize_message_keeps_text_only_content_as_string() {
        let message = ModelMessage {
            role: ModelMessageRole::User,
            content: ModelMessageContent::text("hello"),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        };

        let value = serialize_message(&message);
        assert_eq!(value["content"], "hello");
    }

    #[test]
    fn serialize_message_emits_image_parts() {
        let message = ModelMessage {
            role: ModelMessageRole::User,
            content: ModelMessageContent::Parts(vec![
                ModelContentPart::Text {
                    text: "what is this?".into(),
                },
                ModelContentPart::ImageUrl {
                    url: "data:image/png;base64,abc".into(),
                    mime_type: "image/png".into(),
                },
            ]),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        };

        let value = serialize_message(&message);
        assert_eq!(value["content"][0]["type"], "text");
        assert_eq!(value["content"][0]["text"], "what is this?");
        assert_eq!(value["content"][1]["type"], "image_url");
        assert_eq!(
            value["content"][1]["image_url"]["url"],
            "data:image/png;base64,abc"
        );
    }
}
