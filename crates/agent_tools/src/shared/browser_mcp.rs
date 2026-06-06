use std::collections::HashSet;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

const DEFAULT_MCP_TIMEOUT_MS: u64 = 30_000;
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserMcpConfig {
    pub command: String,
    pub args: Vec<String>,
    pub snapshot_tool: String,
    pub screenshot_tool: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserMcpConfigState {
    Configured(BrowserMcpConfig),
    Unconfigured,
    Invalid(String),
}

impl BrowserMcpConfigState {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
        let Some(command) = lookup("VORTEX_BROWSER_MCP_COMMAND")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Self::Unconfigured;
        };
        let args = match lookup("VORTEX_BROWSER_MCP_ARGS") {
            Some(raw) if !raw.trim().is_empty() => {
                match serde_json::from_str::<Vec<String>>(&raw) {
                    Ok(args) => args,
                    Err(err) => {
                        return Self::Invalid(format!(
                            "VORTEX_BROWSER_MCP_ARGS must be a JSON array of strings: {err}"
                        ));
                    }
                }
            }
            _ => Vec::new(),
        };
        let Some(snapshot_tool) = lookup("VORTEX_BROWSER_MCP_SNAPSHOT_TOOL")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Self::Invalid("VORTEX_BROWSER_MCP_SNAPSHOT_TOOL is required".into());
        };
        let Some(screenshot_tool) = lookup("VORTEX_BROWSER_MCP_SCREENSHOT_TOOL")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Self::Invalid("VORTEX_BROWSER_MCP_SCREENSHOT_TOOL is required".into());
        };
        Self::Configured(BrowserMcpConfig {
            command,
            args,
            snapshot_tool,
            screenshot_tool,
        })
    }

    fn config(&self) -> Result<BrowserMcpConfig, String> {
        match self {
            Self::Configured(config) => Ok(config.clone()),
            Self::Unconfigured => Err("browser MCP server is not configured".into()),
            Self::Invalid(reason) => Err(format!("browser MCP server config is invalid: {reason}")),
        }
    }
}

pub struct BrowserMcpClient {
    config: BrowserMcpConfigState,
    session: Mutex<Option<McpSession>>,
}

struct McpSession {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    tools: Option<HashSet<String>>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    id: Option<Value>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ToolsListResult {
    #[serde(default)]
    tools: Vec<McpTool>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McpTool {
    name: String,
}

#[derive(Debug, Serialize)]
struct ToolCallParams<'a> {
    name: &'a str,
    arguments: Value,
}

impl BrowserMcpClient {
    pub fn new(config: BrowserMcpConfigState) -> Self {
        Self {
            config,
            session: Mutex::new(None),
        }
    }

    pub fn unavailable_reason(&self) -> Option<String> {
        match &self.config {
            BrowserMcpConfigState::Configured(_) => None,
            BrowserMcpConfigState::Unconfigured => {
                Some("browser MCP server is not configured".into())
            }
            BrowserMcpConfigState::Invalid(reason) => {
                Some(format!("browser MCP server config is invalid: {reason}"))
            }
        }
    }

    pub async fn snapshot(&self, args: Value) -> Result<Value, String> {
        let config = self.config.config()?;
        self.call_tool(config.snapshot_tool, args).await
    }

    pub async fn screenshot(&self, args: Value) -> Result<Value, String> {
        let config = self.config.config()?;
        self.call_tool(config.screenshot_tool, args).await
    }

    async fn call_tool(&self, tool_name: String, args: Value) -> Result<Value, String> {
        let config = self.config.config()?;
        let mut guard = self.session.lock().await;
        if guard.is_none() {
            *guard = Some(start_session(&config).await?);
        }
        let session = guard.as_mut().ok_or("browser MCP session not running")?;
        ensure_tool_list(session).await?;
        if !session
            .tools
            .as_ref()
            .is_some_and(|tools| tools.contains(&tool_name))
        {
            return Err(format!("browser MCP tool `{tool_name}` was not advertised"));
        }
        let result = session
            .request(
                "tools/call",
                json!(ToolCallParams {
                    name: &tool_name,
                    arguments: args,
                }),
                DEFAULT_MCP_TIMEOUT_MS,
            )
            .await?;
        if let Some(message) = mcp_tool_error_message(&result) {
            return Err(message);
        }
        Ok(result)
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

async fn start_session(config: &BrowserMcpConfig) -> Result<McpSession, String> {
    let mut command = Command::new(&config.command);
    command
        .args(&config.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to spawn browser MCP server: {err}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or("browser MCP server has no stdin")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("browser MCP server has no stdout")?;
    let mut session = McpSession {
        child,
        stdin,
        reader: BufReader::new(stdout),
        tools: None,
    };
    session
        .request(
            "initialize",
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "vortex",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
            DEFAULT_MCP_TIMEOUT_MS,
        )
        .await?;
    session
        .notify("notifications/initialized", json!({}))
        .await?;
    Ok(session)
}

async fn ensure_tool_list(session: &mut McpSession) -> Result<(), String> {
    if session.tools.is_some() {
        return Ok(());
    }
    let mut tools = HashSet::new();
    let mut cursor: Option<String> = None;
    loop {
        let params = cursor
            .as_ref()
            .map(|cursor| json!({ "cursor": cursor }))
            .unwrap_or_else(|| json!({}));
        let result = session
            .request("tools/list", params, DEFAULT_MCP_TIMEOUT_MS)
            .await?;
        let listed: ToolsListResult = serde_json::from_value(result).map_err(|err| {
            format!("browser MCP server returned an invalid tools/list response: {err}")
        })?;
        tools.extend(listed.tools.into_iter().map(|tool| tool.name));
        cursor = listed.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    session.tools = Some(tools);
    Ok(())
}

impl McpSession {
    async fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let message = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&message).await
    }

    async fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout_ms: u64,
    ) -> Result<Value, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&message).await?;
        let timeout = Duration::from_millis(timeout_ms);
        loop {
            let mut line = String::new();
            let read = tokio::time::timeout(timeout, self.reader.read_line(&mut line)).await;
            match read {
                Ok(Ok(0)) => return Err("browser MCP server closed stdout".into()),
                Ok(Ok(_)) => {}
                Ok(Err(err)) => return Err(err.to_string()),
                Err(_) => {
                    return Err(format!(
                        "browser MCP server response timeout after {timeout_ms}ms"
                    ));
                }
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let response: JsonRpcResponse =
                serde_json::from_str(trimmed).map_err(|err| err.to_string())?;
            if response.id.as_ref() != Some(&Value::String(id.clone())) {
                continue;
            }
            if let Some(error) = response.error {
                return Err(error.message);
            }
            return Ok(response.result.unwrap_or(Value::Null));
        }
    }

    async fn write_message(&mut self, message: &Value) -> Result<(), String> {
        let line = serde_json::to_string(message).map_err(|err| err.to_string())?;
        self.stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(|err| err.to_string())?;
        self.stdin.flush().await.map_err(|err| err.to_string())
    }
}

fn mcp_tool_error_message(result: &Value) -> Option<String> {
    if result.get("isError").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let content = result
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|message| !message.trim().is_empty());
    Some(content.unwrap_or_else(|| "browser MCP tool returned an error".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn config(command: impl Into<String>, args: Vec<String>) -> BrowserMcpConfigState {
        BrowserMcpConfigState::Configured(BrowserMcpConfig {
            command: command.into(),
            args,
            snapshot_tool: "snap".into(),
            screenshot_tool: "shot".into(),
        })
    }

    #[test]
    fn env_config_absent_is_unconfigured() {
        let state = BrowserMcpConfigState::from_lookup(|_| None);
        assert_eq!(state, BrowserMcpConfigState::Unconfigured);
    }

    #[test]
    fn env_config_rejects_invalid_args_json() {
        let state = BrowserMcpConfigState::from_lookup(|key| match key {
            "VORTEX_BROWSER_MCP_COMMAND" => Some("server".into()),
            "VORTEX_BROWSER_MCP_ARGS" => Some("not-json".into()),
            "VORTEX_BROWSER_MCP_SNAPSHOT_TOOL" => Some("snap".into()),
            "VORTEX_BROWSER_MCP_SCREENSHOT_TOOL" => Some("shot".into()),
            _ => None,
        });
        assert!(matches!(state, BrowserMcpConfigState::Invalid(_)));
    }

    #[tokio::test]
    async fn maps_browser_calls_to_configured_mcp_tools() {
        let dir = tempfile::tempdir().expect("tempdir");
        let script_path = dir.path().join("fake-mcp.sh");
        let mut script = std::fs::File::create(&script_path).expect("script");
        writeln!(
            script,
            r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"method":"initialize"'*)
      printf '{{"jsonrpc":"2.0","id":"%s","result":{{"protocolVersion":"2025-06-18","capabilities":{{}}}}}}\n' "$id"
      ;;
    *'"method":"tools/list"'*)
      printf '{{"jsonrpc":"2.0","id":"%s","result":{{"tools":[{{"name":"snap","inputSchema":{{}}}},{{"name":"shot","inputSchema":{{}}}}]}}}}\n' "$id"
      ;;
    *'"name":"snap"'*)
      printf '{{"jsonrpc":"2.0","id":"%s","result":{{"content":[{{"type":"text","text":"snap-ok"}}],"tool":"snap"}}}}\n' "$id"
      ;;
    *'"name":"shot"'*)
      printf '{{"jsonrpc":"2.0","id":"%s","result":{{"content":[{{"type":"text","text":"shot-ok"}}],"tool":"shot"}}}}\n' "$id"
      ;;
  esac
done
"#
        )
        .expect("write script");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).expect("chmod");

        let client = BrowserMcpClient::new(config(
            "/bin/sh",
            vec![script_path.to_string_lossy().to_string()],
        ));
        let snapshot = client
            .snapshot(json!({ "url": "https://example.com" }))
            .await;
        assert_eq!(snapshot.expect("snapshot")["tool"], "snap");
        let screenshot = client
            .screenshot(json!({
                "url": "https://example.com",
                "output_path": "/tmp/fake.png"
            }))
            .await;
        assert_eq!(screenshot.expect("screenshot")["tool"], "shot");
    }
}
