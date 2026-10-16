//! Local JSONL transport. No shell, TCP listener, or credential copying.
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex, oneshot},
};

type Reply = Result<Value, String>;
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Reply>>>>;
pub struct Client {
    child: Child,
    input: Mutex<ChildStdin>,
    pending: Pending,
    next_id: AtomicU64,
    broken: Arc<AtomicBool>,
    pub incoming: flume::Receiver<Reply>,
    reader: tokio::task::JoinHandle<()>,
}
impl Client {
    pub async fn launch(binary: &Path) -> Result<Self, String> {
        let mut command = Command::new(binary);
        // npm-installed launchers use `env node`; Finder's PATH may lack Node too.
        if let Some(parent) = binary.parent().filter(|p| !p.as_os_str().is_empty()) {
            let mut paths = vec![parent.to_path_buf()];
            if let Some(path) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&path));
            }
            if let Ok(path) = std::env::join_paths(paths) {
                command.env("PATH", path);
            }
        }
        let mut child = command
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                format!("Cannot start Codex: {e}. Install Codex CLI or set VORTEX_CODEX_BIN.")
            })?;
        let input = child.stdin.take().ok_or("Codex stdin unavailable")?;
        let output = child.stdout.take().ok_or("Codex stdout unavailable")?;
        let pending: Pending = Arc::default();
        let replies = pending.clone();
        let broken = Arc::new(AtomicBool::new(false));
        let reader_broken = broken.clone();
        let (tx, incoming) = flume::bounded(512);
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(output).lines();
            let failure = loop {
                match lines.next_line().await {
                    Ok(Some(line)) => match serde_json::from_str::<Value>(&line) {
                        Ok(message) => {
                            if message.get("method").is_none() {
                                if let Some(id) = message["id"].as_u64()
                                    && let Some(reply) = replies.lock().await.remove(&id)
                                {
                                    let result = if message.get("error").is_some() {
                                        Err(format!(
                                            "Codex: {}",
                                            message["error"]["message"]
                                                .as_str()
                                                .unwrap_or("request failed")
                                        ))
                                    } else {
                                        Ok(message["result"].clone())
                                    };
                                    let _ = reply.send(result);
                                }
                            } else if tx.send_async(Ok(message)).await.is_err() {
                                return;
                            }
                        }
                        Err(_) => break "Codex returned invalid JSON".to_string(),
                    },
                    Ok(None) => break "Codex process disconnected".to_string(),
                    Err(e) => break format!("Codex transport failed: {e}"),
                }
            };
            reader_broken.store(true, Ordering::Relaxed);
            for (_, reply) in replies.lock().await.drain() {
                let _ = reply.send(Err(failure.clone()));
            }
            let _ = tx.send_async(Err(failure)).await;
        });
        let mut client = Self {
            child,
            input: Mutex::new(input),
            pending,
            next_id: AtomicU64::new(1),
            broken,
            incoming,
            reader,
        };
        if let Err(error) = client.request("initialize", json!({"clientInfo":{"name":"vortex","title":"Vortex","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}})).await {
            let exit = client.child.try_wait().ok().flatten().map(|s| format!(" ({s})")).unwrap_or_default();
            return Err(format!("Codex at {} could not initialize{exit}: {error}. Check `codex --version` and run `codex app-server` in a terminal to diagnose CLI configuration; update the CLI if needed.", binary.display()));
        }
        client
            .send(json!({"method":"initialized","params":{}}))
            .await?;
        Ok(client)
    }
    pub fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Relaxed)
    }
    pub async fn send(&self, value: Value) -> Result<(), String> {
        let mut input = self.input.lock().await;
        let mut bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        input.write_all(&bytes).await.map_err(|e| e.to_string())?;
        input.flush().await.map_err(|e| e.to_string())
    }
    pub async fn request(&self, method: &str, params: Value) -> Reply {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        if let Err(e) = self
            .send(json!({"id":id,"method":method,"params":params}))
            .await
        {
            self.pending.lock().await.remove(&id);
            self.broken.store(true, Ordering::Relaxed);
            return Err(e);
        }
        let result = tokio::time::timeout(Duration::from_secs(20), rx).await;
        self.pending.lock().await.remove(&id);
        match result {
            Ok(Ok(reply)) => reply,
            Ok(Err(_)) => Err("Codex response channel closed".into()),
            Err(_) => {
                self.broken.store(true, Ordering::Relaxed);
                Err(format!(
                    "Codex {method} timed out; the request was not automatically retried"
                ))
            }
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.reader.abort();
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn missing_cli_reports_installation_guidance() {
        let dir = tempfile::tempdir().unwrap();
        let result = Client::launch(&dir.path().join("missing-codex")).await;
        assert!(matches!(result,Err(e) if e.contains("VORTEX_CODEX_BIN")));
    }
}
