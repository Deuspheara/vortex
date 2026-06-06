use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

const DEFAULT_BROWSER_TIMEOUT_MS: u64 = 30_000;
const IPC_TIMEOUT_MARGIN_MS: u64 = 15_000;

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    id: String,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    id: String,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

pub struct BrowserSidecarClient {
    child: Mutex<Option<BrowserSidecarProcess>>,
    sidecar_path: PathBuf,
}

struct BrowserSidecarProcess {
    _child: Child,
    stdin: ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
}

impl BrowserSidecarClient {
    pub fn new(sidecar_entry: PathBuf) -> Self {
        Self {
            child: Mutex::new(None),
            sidecar_path: sidecar_entry,
        }
    }

    pub async fn snapshot(&self, params: Value) -> Result<Value, String> {
        self.call("browser.snapshot", params, DEFAULT_BROWSER_TIMEOUT_MS)
            .await
    }

    pub async fn screenshot(&self, params: Value) -> Result<Value, String> {
        self.call("browser.screenshot", params, DEFAULT_BROWSER_TIMEOUT_MS)
            .await
    }

    async fn ensure_running(&self) -> Result<(), String> {
        let mut guard = self.child.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let mut cmd = Command::new("bun");
        cmd.arg(&self.sidecar_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn browser sidecar: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        *guard = Some(BrowserSidecarProcess {
            _child: child,
            stdin,
            reader: BufReader::new(stdout),
        });
        Ok(())
    }

    fn reset_sidecar(guard: &mut Option<BrowserSidecarProcess>) {
        *guard = None;
    }

    async fn call(&self, method: &str, params: Value, timeout_ms: u64) -> Result<Value, String> {
        self.ensure_running().await?;
        let request = JsonRpcRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            params: json!({
                "timeout_ms": timeout_ms,
                "params": params
            }),
        };
        let request_id = request.id.clone();
        let ipc_timeout = Duration::from_millis(timeout_ms.saturating_add(IPC_TIMEOUT_MARGIN_MS));
        let mut guard = self.child.lock().await;
        let process = guard.as_mut().ok_or("browser sidecar not running")?;
        let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        process
            .stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        process.stdin.flush().await.map_err(|e| e.to_string())?;

        loop {
            let mut response_line = String::new();
            let read =
                tokio::time::timeout(ipc_timeout, process.reader.read_line(&mut response_line))
                    .await;
            match read {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return Err(e.to_string()),
                Err(_) => {
                    Self::reset_sidecar(&mut guard);
                    return Err(format!(
                        "browser sidecar response timeout after {timeout_ms}ms"
                    ));
                }
            }
            let trimmed = response_line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let response: JsonRpcResponse =
                serde_json::from_str(trimmed).map_err(|e| e.to_string())?;
            if response.id != request_id {
                continue;
            }
            if let Some(error) = response.error {
                return Err(error.message);
            }
            return Ok(response.result.unwrap_or(Value::Null));
        }
    }
}
