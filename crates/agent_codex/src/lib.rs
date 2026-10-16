//! Full Codex agent backend, independent of the native model/tool loop.
mod projection;
mod transport;
use agent_protocol::*;
use agent_store::{EventStore, StoredRun};
use chrono::Utc;
use projection::{RunProjection, string};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use transport::Client;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Starting,
    Ready,
    Recovering,
    SignInRequired,
    Unavailable,
}
#[derive(Clone, Debug)]
pub struct CodexModel {
    pub id: String,
    pub efforts: Vec<String>,
    pub default_effort: String,
    pub images: bool,
}
#[derive(Clone, Debug)]
pub struct BackendStatus {
    pub state: ConnectionState,
    pub detail: String,
    pub models: Vec<CodexModel>,
}
impl Default for BackendStatus {
    fn default() -> Self {
        Self {
            state: ConnectionState::Starting,
            detail: "Starting Codex…".into(),
            models: Vec::new(),
        }
    }
}
#[derive(Clone)]
pub struct CodexBackend {
    commands: flume::Sender<AgentCommand>,
    status: Arc<Mutex<BackendStatus>>,
    pub updates: flume::Receiver<BackendStatus>,
}
impl CodexBackend {
    pub fn spawn(
        store: Arc<dyn EventStore>,
        events: flume::Sender<AgentEvent>,
        runtime: &tokio::runtime::Handle,
        binary: PathBuf,
    ) -> Self {
        let (commands, rx) = flume::bounded(64);
        let (tx, updates) = flume::unbounded();
        let status = Arc::new(Mutex::new(BackendStatus::default()));
        let worker = Worker {
            store,
            events,
            commands: rx,
            status: status.clone(),
            updates: tx,
            runs: HashMap::new(),
            approvals: HashMap::new(),
        };
        runtime.spawn(worker.supervise(binary));
        Self {
            commands,
            status,
            updates,
        }
    }
    pub fn status(&self) -> BackendStatus {
        self.status.lock().unwrap().clone()
    }
    pub fn send(&self, command: AgentCommand) -> Result<(), String> {
        if matches!(command, AgentCommand::StartRun { .. })
            && self.status().state != ConnectionState::Ready
        {
            return Err(self.status().detail);
        }
        self.commands
            .try_send(command)
            .map_err(|e| format!("Codex command queue unavailable: {e}"))
    }
}
struct PendingApproval {
    request_id: Value,
    run_id: RunId,
    method: String,
}
struct Worker {
    store: Arc<dyn EventStore>,
    events: flume::Sender<AgentEvent>,
    commands: flume::Receiver<AgentCommand>,
    status: Arc<Mutex<BackendStatus>>,
    updates: flume::Sender<BackendStatus>,
    runs: HashMap<String, RunProjection>,
    approvals: HashMap<String, PendingApproval>,
}
impl Worker {
    fn status(&self, state: ConnectionState, detail: impl Into<String>, models: Vec<CodexModel>) {
        let status = BackendStatus {
            state,
            detail: detail.into(),
            models,
        };
        *self.status.lock().unwrap() = status.clone();
        let _ = self.updates.send(status);
    }
    fn persist(&self, p: &RunProjection, events: Vec<AgentEvent>) -> Result<(), String> {
        self.store.record_backend_events(
            &p.run_id,
            &events,
            &serde_json::to_string(p).map_err(|e| e.to_string())?,
        )?;
        for event in events {
            let _ = self.events.send(event);
        }
        Ok(())
    }
    async fn supervise(mut self, binary: PathBuf) {
        match self.store.backend_run_states() {
            Ok(states) => {
                for (_, state) in states {
                    if let Ok(p) = serde_json::from_str::<RunProjection>(&state) {
                        self.runs.insert(p.run_id.0.clone(), p);
                    }
                }
            }
            Err(e) => {
                self.status(
                    ConnectionState::Unavailable,
                    format!("Cannot load Codex history: {e}"),
                    vec![],
                );
                return;
            }
        }
        let mut failures = 0u32;
        loop {
            self.status(
                if failures == 0 {
                    ConnectionState::Starting
                } else {
                    ConnectionState::Recovering
                },
                "Connecting to local Codex…",
                vec![],
            );
            let outcome = match Client::launch(&binary).await {
                Ok(client) => self.connected(&client).await,
                Err(e) => Err(e),
            };
            self.approvals.clear();
            match outcome {
                Ok(()) => return,
                Err(e) => {
                    failures += 1;
                    if failures >= 5 {
                        self.status(
                            ConnectionState::Unavailable,
                            format!("{e} Restart Vortex to retry."),
                            vec![],
                        );
                        let ids = self
                            .runs
                            .values()
                            .filter(|p| p.active)
                            .map(|p| p.run_id.0.clone())
                            .collect::<Vec<_>>();
                        for id in ids {
                            let _=self.finish(&id,RunStatus::Failed,Some("Codex disconnected. Review workspace changes before retrying; the turn was not resent.".into()));
                        }
                        return;
                    }
                    self.status(
                        ConnectionState::Recovering,
                        format!("{e} Reconnecting automatically…"),
                        vec![],
                    );
                    tokio::time::sleep(Duration::from_secs(1 << (failures - 1))).await;
                }
            }
        }
    }
    async fn connected(&mut self, client: &Client) -> Result<(), String> {
        let mut poll = tokio::time::interval(Duration::from_secs(15));
        self.refresh_account(client).await?;
        // Recover only this application's persisted threads, never import arbitrary CLI history.
        let threads = self
            .runs
            .values()
            .filter(|p| p.active)
            .map(|p| p.thread_id.clone())
            .collect::<std::collections::HashSet<_>>();
        for thread in threads {
            if let Err(error) = self.resume(client, &thread).await {
                if client.is_broken() {
                    return Err(error);
                }
                let ids = self
                    .runs
                    .values()
                    .filter(|p| p.thread_id == thread && p.active)
                    .map(|p| p.run_id.0.clone())
                    .collect::<Vec<_>>();
                for id in ids {
                    self.finish(&id, RunStatus::Failed, Some(error.clone()))?;
                }
            }
        }
        loop {
            tokio::select! {
                incoming=client.incoming.recv_async() => {
                    let message=incoming.map_err(|_|"Codex event stream closed")??;
                    self.notification(client,message).await?;
                }
                command=self.commands.recv_async() => {
                    let Ok(command)=command else {return Ok(());};
                    let session=match &command {AgentCommand::StartRun{session_id,..}=>Some(session_id.clone()),_=>None};
                    if let Err(e)=self.command(client,command).await {
                        let event=if let Some(session_id)=session{AgentEvent::SessionCommandFailed{session_id,message:e}}else{AgentEvent::CommandFailed{message:e}};
                        let _=self.events.send(event);
                    }
                    if client.is_broken(){return Err("Codex connection lost during request".into());}
                }
                _=poll.tick() => {
                    if self.status.lock().unwrap().state==ConnectionState::SignInRequired {self.refresh_account(client).await?;}
                }
            }
        }
    }
    async fn refresh_account(&self, client: &Client) -> Result<(), String> {
        let account = client
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        if account["account"].is_null() && account["requiresOpenaiAuth"].as_bool().unwrap_or(true) {
            self.status(
                ConnectionState::SignInRequired,
                "Run `codex login` once, then Vortex will connect automatically.",
                vec![],
            );
            return Ok(());
        }
        let mut models = Vec::new();
        let mut cursor = Value::Null;
        loop {
            let response = client
                .request("model/list", json!({"cursor":cursor,"limit":100}))
                .await?;
            for m in response["data"].as_array().into_iter().flatten() {
                models.push(CodexModel {
                    id: string(m, "model").to_string(),
                    efforts: m["supportedReasoningEfforts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|e| e["reasoningEffort"].as_str().map(str::to_string))
                        .collect(),
                    default_effort: string(m, "defaultReasoningEffort").to_string(),
                    images: m["inputModalities"]
                        .as_array()
                        .is_some_and(|v| v.iter().any(|v| v == "image")),
                });
            }
            cursor = response["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
        }
        if models.is_empty() {
            return Err("Codex returned no available models".into());
        }
        self.status(
            ConnectionState::Ready,
            "Codex connected · using your local login",
            models,
        );
        Ok(())
    }
    async fn resume(&mut self, client: &Client, thread: &str) -> Result<(), String> {
        let response=client.request("thread/resume",json!({"threadId":thread,"approvalPolicy":"untrusted","approvalsReviewer":"user","sandbox":"read-only","config":{"web_search":"disabled","sandbox_workspace_write.network_access":false}})).await?;
        let mut turns = response["thread"]["turns"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if turns.is_empty() {
            let mut cursor = Value::Null;
            loop {
                let page=client.request("thread/turns/list",json!({"threadId":thread,"cursor":cursor,"limit":100,"sortDirection":"asc","itemsView":"full"})).await?;
                turns.extend(page["data"].as_array().cloned().unwrap_or_default());
                cursor = page["nextCursor"].clone();
                if cursor.is_null() {
                    break;
                }
            }
        }
        for turn in turns {
            let id = string(&turn, "id");
            let run = self
                .runs
                .values()
                .find(|p| p.thread_id == thread && p.turn_id.as_deref() == Some(id))
                .map(|p| p.run_id.0.clone());
            if let Some(run) = run {
                let p = self.runs.get_mut(&run).unwrap();
                let mut events = Vec::new();
                for item in turn["items"].as_array().into_iter().flatten() {
                    events.extend(p.item(item, true));
                }
                let p = p.clone();
                self.persist(&p, events)?;
                if p.active && string(&turn, "status") != "inProgress" {
                    self.finish_from_turn(&run, &turn)?;
                } else if p.active {
                    // A restarted owned process cannot safely continue an uncertain old turn.
                    let _ = client
                        .request("turn/interrupt", json!({"threadId":thread,"turnId":id}))
                        .await;
                    self.finish(&run,RunStatus::Cancelled,Some("Codex reconnected; the interrupted turn was not resent. Review changes and send a follow-up.".into()))?;
                }
            }
        }
        let unknown = self
            .runs
            .values()
            .filter(|p| p.thread_id == thread && p.active)
            .map(|p| p.run_id.0.clone())
            .collect::<Vec<_>>();
        for id in unknown {
            self.finish(&id,RunStatus::Failed,Some("Connection ended before turn confirmation. The request was not resent; review Codex history and workspace changes.".into()))?;
        }
        Ok(())
    }
    async fn command(&mut self, client: &Client, command: AgentCommand) -> Result<(), String> {
        match command {
            AgentCommand::StartRun {
                project_id,
                session_id,
                prompt,
                model,
                mode,
                attachments,
                ..
            } => {
                if self
                    .runs
                    .values()
                    .any(|p| p.session_id == session_id && p.active)
                {
                    return Err("Codex already has an active turn in this conversation".into());
                }
                let models = self.status.lock().unwrap().models.clone();
                if !models.iter().any(|m| m.id == model.0) {
                    return Err("Selected Codex model is unavailable. Choose a model from the current catalog.".into());
                }
                let project = self
                    .store
                    .get_project(&project_id)?
                    .ok_or("Project not found")?;
                let mut binding = self.store.session_backend(&session_id)?.unwrap_or_default();
                let thread = if let Some(thread) = &binding.thread_id {
                    self.resume(client, thread).await?;
                    thread.clone()
                } else {
                    let response=client.request("thread/start",json!({"cwd":project.root_path,"model":model.0,"approvalPolicy":"untrusted","approvalsReviewer":"user","sandbox":"read-only","config":{"web_search":"disabled","sandbox_workspace_write.network_access":false}})).await?;
                    let id = string(&response["thread"], "id");
                    if id.is_empty() {
                        return Err("Codex did not return a thread id".into());
                    }
                    id.to_string()
                };
                binding.backend = "codex".into();
                binding.thread_id = Some(thread.clone());
                binding.model = Some(model.0.clone());
                self.store.save_session_backend(&session_id, &binding)?;
                let run_id = RunId::new(format!("codex-{}", uuid::Uuid::new_v4()));
                self.store.create_run(&StoredRun {
                    id: run_id.clone(),
                    session_id: session_id.clone(),
                    parent_run_id: None,
                    depth: 0,
                    model: model.clone(),
                    mode: mode.clone(),
                    status: RunStatus::Running,
                    started_at: Utc::now(),
                    finished_at: None,
                })?;
                let mut p = RunProjection::new(run_id.clone(), session_id.clone(), thread.clone());
                // Codex totals are thread-cumulative; retain the previous turn's baseline.
                for previous in self.runs.values().filter(|p| p.thread_id == thread) {
                    for i in 0..3 {
                        p.usage[i] = p.usage[i].max(previous.usage[i]);
                    }
                }
                self.persist(
                    &p,
                    vec![
                        AgentEvent::RunStarted {
                            run_id: run_id.clone(),
                            session_id,
                            model: model.clone(),
                            mode: mode.clone(),
                            depth: 0,
                            parent_run_id: None,
                        },
                        AgentEvent::UserMessage {
                            run_id: run_id.clone(),
                            text: prompt.clone(),
                        },
                    ],
                )?;
                self.runs.insert(run_id.0.clone(), p);
                let mut params = turn_params(
                    &thread,
                    &project.root_path,
                    &model.0,
                    binding.effort.as_deref(),
                    &mode,
                    &prompt,
                );
                for attachment in attachments {
                    let value = match attachment.source {
                        AttachmentSource::Path(path)
                            if matches!(attachment.kind, AttachmentKind::Image) =>
                        {
                            json!({"type":"localImage","path":path})
                        }
                        AttachmentSource::Bytes(bytes)
                            if matches!(attachment.kind, AttachmentKind::Image) =>
                        {
                            use base64::Engine;
                            let mime = attachment.mime_type.unwrap_or_else(|| "image/png".into());
                            json!({"type":"image","url":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))})
                        }
                        AttachmentSource::Path(path) => {
                            json!({"type":"text","text":format!("Attached file: {}",path.display())})
                        }
                        _ => {
                            self.finish(
                                &run_id.0,
                                RunStatus::Failed,
                                Some("Unsupported attachment format".into()),
                            )?;
                            return Ok(());
                        }
                    };
                    params["input"].as_array_mut().unwrap().push(value);
                }
                match client.request("turn/start", params).await {
                    Ok(response) => {
                        let turn_id = string(&response["turn"], "id");
                        let p = self.runs.get_mut(&run_id.0).unwrap();
                        p.turn_id = Some(turn_id.to_string());
                        let p = p.clone();
                        self.persist(&p, vec![])?;
                    }
                    Err(e) => {
                        if client.is_broken() {
                            while let Ok(Ok(message)) = client.incoming.try_recv() {
                                self.notification(client, message).await?;
                            }
                        } else {
                            self.finish(&run_id.0, RunStatus::Failed, Some(e))?;
                        }
                        return Ok(());
                    }
                }
            }
            AgentCommand::CancelRun { run_id } => {
                let p = self
                    .runs
                    .get(&run_id.0)
                    .ok_or("Codex run no longer exists")?;
                let turn = p
                    .turn_id
                    .as_deref()
                    .ok_or("Codex is still starting the turn")?;
                client
                    .request(
                        "turn/interrupt",
                        json!({"threadId":p.thread_id,"turnId":turn}),
                    )
                    .await?;
                // Wait for turn/completed so final tool/file events are preserved.
            }
            AgentCommand::ApproveTool { approval_id }
            | AgentCommand::ApproveToolAlways { approval_id } => {
                self.approve(client, &approval_id.0, true).await?
            }
            AgentCommand::RejectTool { approval_id, .. } => {
                self.approve(client, &approval_id.0, false).await?
            }
            _ => return Err("This action is not supported by the Codex backend.".into()),
        }
        Ok(())
    }
    async fn approve(&mut self, client: &Client, id: &str, accept: bool) -> Result<(), String> {
        let pending = self
            .approvals
            .remove(id)
            .ok_or("Approval expired after reconnect; send a follow-up to continue")?;
        if !self.runs.get(&pending.run_id.0).is_some_and(|p| p.active) {
            return Err("Approval belongs to a finished turn".into());
        }
        let decision = if accept { "accept" } else { "decline" };
        let _method = pending.method;
        client
            .send(json!({"id":pending.request_id,"result":{"decision":decision}}))
            .await?;
        self.store
            .update_run_status(&pending.run_id, RunStatus::Running, None)?;
        Ok(())
    }
    fn finish_from_turn(&mut self, run: &str, turn: &Value) -> Result<(), String> {
        let status = match string(turn, "status") {
            "completed" => RunStatus::Completed,
            "interrupted" => RunStatus::Cancelled,
            _ => RunStatus::Failed,
        };
        let error = turn["error"]["message"].as_str().map(str::to_string);
        self.finish(run, status, error)
    }
    fn finish(&mut self, id: &str, status: RunStatus, error: Option<String>) -> Result<(), String> {
        let Some(p) = self.runs.get_mut(id) else {
            return Ok(());
        };
        if !p.active {
            return Ok(());
        }
        p.active = false;
        let p = p.clone();
        let mut events = Vec::new();
        if let Some(message) = error {
            events.push(AgentEvent::RunFailed {
                run_id: p.run_id.clone(),
                error: AgentErrorView {
                    code: "codex".into(),
                    message,
                    recoverable: true,
                },
            });
        }
        events.push(AgentEvent::RunFinished {
            run_id: p.run_id.clone(),
            status: status.clone(),
        });
        self.persist(&p, events)?;
        self.store
            .update_run_status(&p.run_id, status, Some(Utc::now()))?;
        self.approvals.retain(|_, a| a.run_id != p.run_id);
        Ok(())
    }
    async fn notification(&mut self, client: &Client, message: Value) -> Result<(), String> {
        let method = string(&message, "method");
        let params = &message["params"];
        if method == "account/updated" {
            return self.refresh_account(client).await;
        }
        let thread = string(params, "threadId");
        let turn = params["turnId"]
            .as_str()
            .or_else(|| params["turn"]["id"].as_str());
        let id = self
            .runs
            .values()
            .find(|p| {
                p.thread_id == thread
                    && p.active
                    && (p.turn_id.is_none() || turn.is_none() || p.turn_id.as_deref() == turn)
            })
            .map(|p| p.run_id.0.clone());
        if message.get("id").is_some() {
            if let Some(id) = id
                && matches!(
                    method,
                    "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
                )
            {
                let p = self.runs.get(&id).unwrap();
                let approval_id = ApprovalId::new(format!("codex-{}", uuid::Uuid::new_v4()));
                let item = string(params, "itemId");
                let call = p.call_id(item);
                self.persist(
                    p,
                    vec![AgentEvent::ApprovalRequested {
                        run_id: Some(p.run_id.clone()),
                        approval_id: approval_id.clone(),
                        call_id: call,
                        risk: RiskLevel::High,
                        reason: params["reason"]
                            .as_str()
                            .unwrap_or("Codex requests permission to proceed")
                            .to_string(),
                        command_preview: params["command"].as_str().map(str::to_string),
                        affected_paths: Vec::new(),
                    }],
                )?;
                self.store
                    .update_run_status(&p.run_id, RunStatus::PausedForApproval, None)?;
                self.approvals.insert(
                    approval_id.0,
                    PendingApproval {
                        request_id: message["id"].clone(),
                        run_id: p.run_id.clone(),
                        method: method.into(),
                    },
                );
            } else {
                // Unknown requests must not silently grant permissions or disclose credentials.
                client.send(json!({"id":message["id"],"error":{"code":-32601,"message":"Vortex does not support this request; permission not granted"}})).await?;
            }
            return Ok(());
        }
        let Some(id) = id else {
            return Ok(());
        };
        if method == "turn/completed" {
            return self.finish_from_turn(&id, &params["turn"]);
        }
        let p = self.runs.get_mut(&id).unwrap();
        let mut events = Vec::new();
        match method {
            "turn/started" => {
                p.turn_id = Some(string(&params["turn"], "id").into());
            }
            "item/agentMessage/delta" => {
                if let Some(e) = p.text(
                    string(params, "itemId"),
                    string(params, "delta"),
                    false,
                    false,
                ) {
                    events.push(e);
                }
            }
            "item/reasoning/summaryTextDelta" => {
                if let Some(e) = p.text(
                    &format!("reasoning/{}", string(params, "itemId")),
                    string(params, "delta"),
                    false,
                    true,
                ) {
                    events.push(e);
                }
            }
            "item/started" | "item/completed" => {
                events.extend(p.item(&params["item"], method == "item/completed"))
            }
            "item/commandExecution/outputDelta" => events.push(AgentEvent::ToolOutputDelta {
                run_id: Some(p.run_id.clone()),
                call_id: p.call_id(string(params, "itemId")),
                stream: OutputStreamKind::Stdout,
                chunk: string(params, "delta").into(),
            }),
            "turn/plan/updated" => {
                let markdown = params["plan"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|s| {
                        format!(
                            "- [{}] {}",
                            if string(s, "status") == "completed" {
                                "x"
                            } else {
                                " "
                            },
                            string(s, "step")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                events.push(AgentEvent::PlanUpdated {
                    run_id: p.run_id.clone(),
                    markdown,
                    created_at: Utc::now().to_rfc3339(),
                });
            }
            "thread/tokenUsage/updated" => {
                if let Some(event) = p.usage_update(&params["tokenUsage"]["total"]) {
                    events.push(event);
                }
            }
            "error" if !params["willRetry"].as_bool().unwrap_or(false) => {
                return self.finish(
                    &id,
                    RunStatus::Failed,
                    Some(string(&params["error"], "message").into()),
                );
            }
            _ => {}
        }
        let p = p.clone();
        self.persist(&p, events)
    }
}
/// Never inherit a user's full-access default. Write access is requested through approvals.
pub fn turn_params(
    thread: &str,
    cwd: &str,
    model: &str,
    effort: Option<&str>,
    mode: &AgentMode,
    prompt: &str,
) -> Value {
    let instructions = match mode {
        AgentMode::PlanOnly => "Plan only. Do not edit files or execute mutating commands.\n",
        AgentMode::ReadOnlyInspect | AgentMode::ChatOnly => {
            "Inspect and answer only. Do not modify files.\n"
        }
        AgentMode::SuggestPatch => "Suggest changes for review. Do not apply them.\n",
        _ => "",
    };
    let approval = if mode.can_apply_patches() {
        "untrusted"
    } else {
        "never"
    };
    json!({"threadId":thread,"cwd":cwd,"model":model,"effort":effort,"approvalPolicy":approval,"approvalsReviewer":"user","sandboxPolicy":{"type":"readOnly","networkAccess":false},"input":[{"type":"text","text":format!("{instructions}{prompt}")} ]})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_never_inherit_full_access() {
        for mode in [
            AgentMode::ApplyWithApproval,
            AgentMode::FullAccessDangerous,
            AgentMode::AutoSafe,
        ] {
            let p = turn_params("t", "/tmp", "m", None, &mode, "hello");
            assert_eq!(p["sandboxPolicy"]["type"], "readOnly");
            assert_eq!(p["approvalPolicy"], "untrusted");
        }
        assert_eq!(
            turn_params("t", "/tmp", "m", None, &AgentMode::PlanOnly, "hello")["approvalPolicy"],
            "never"
        );
    }
}
