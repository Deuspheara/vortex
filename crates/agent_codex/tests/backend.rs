use agent_codex::{CodexBackend, ConnectionState};
use agent_protocol::*;
use agent_store::{EventStore, SqliteEventStore, StoredProject, StoredSession};
use chrono::Utc;
use std::{path::PathBuf, sync::Arc, time::Duration};

struct Fixture {
    dir: tempfile::TempDir,
    store: Arc<SqliteEventStore>,
    binary: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("codex");
        std::fs::write(&binary, include_str!("fake_server.py")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let store = Arc::new(SqliteEventStore::open(dir.path().join("db.sqlite")).unwrap());
        let now = Utc::now();
        store
            .upsert_project(&StoredProject {
                id: ProjectId::new("p"),
                root_path: dir.path().display().to_string(),
                name: "test".into(),
                trusted: false,
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        store
            .create_session(&StoredSession {
                id: SessionId::new("s"),
                project_id: ProjectId::new("p"),
                title: "test".into(),
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        Self { dir, store, binary }
    }
    fn spawn(&self) -> (CodexBackend, flume::Receiver<AgentEvent>) {
        let (tx, rx) = flume::unbounded();
        (
            CodexBackend::spawn(
                self.store.clone(),
                tx,
                &tokio::runtime::Handle::current(),
                self.binary.clone(),
            ),
            rx,
        )
    }
}
async fn ready(b: &CodexBackend) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if b.status().state == ConnectionState::Ready {
                return;
            }
            b.updates.recv_async().await.unwrap();
        }
    })
    .await
    .unwrap();
}
fn start(b: &CodexBackend, prompt: &str) {
    b.send(AgentCommand::StartRun {
        project_id: ProjectId::new("p"),
        session_id: SessionId::new("s"),
        prompt: prompt.into(),
        model: ModelId::new("test-model"),
        subagent_model: None,
        mode: AgentMode::ApplyWithApproval,
        attachments: vec![],
    })
    .unwrap();
}
async fn until(
    rx: &flume::Receiver<AgentEvent>,
    predicate: impl Fn(&AgentEvent) -> bool,
) -> Vec<AgentEvent> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut events = Vec::new();
        loop {
            let e = rx.recv_async().await.unwrap();
            let done = predicate(&e);
            events.push(e);
            if done {
                return events;
            }
        }
    })
    .await
    .unwrap()
}
async fn finished(rx: &flume::Receiver<AgentEvent>) -> Vec<AgentEvent> {
    until(rx, |e| matches!(e, AgentEvent::RunFinished { .. })).await
}
fn text(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|e| {
            if let AgentEvent::AssistantTextDelta { text, .. } = e {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect()
}
#[tokio::test]
async fn streams_persists_and_resumes_without_duplicate_text() {
    let f = Fixture::new();
    let (b, rx) = f.spawn();
    ready(&b).await;
    start(&b, "first");
    assert_eq!(text(&finished(&rx).await), "hello world");
    start(&b, "second");
    assert_eq!(text(&finished(&rx).await), "hello world");
    assert!(f.dir.path().join("resumes").exists());
    let events = f.store.load_session_events(&SessionId::new("s")).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e.event, AgentEvent::UserMessage { .. }))
            .count(),
        2
    );
    assert_eq!(
        f.store
            .session_backend(&SessionId::new("s"))
            .unwrap()
            .unwrap()
            .thread_id
            .as_deref(),
        Some("thread-1")
    );
}
#[tokio::test]
async fn approval_accept_and_reject_use_correlated_request_id() {
    for accept in [true, false] {
        let f = Fixture::new();
        let (b, rx) = f.spawn();
        ready(&b).await;
        start(&b, "approval");
        let events = until(&rx, |e| matches!(e, AgentEvent::ApprovalRequested { .. })).await;
        let AgentEvent::ApprovalRequested { approval_id, .. } = events.last().unwrap() else {
            panic!()
        };
        b.send(if accept {
            AgentCommand::ApproveTool {
                approval_id: approval_id.clone(),
            }
        } else {
            AgentCommand::RejectTool {
                approval_id: approval_id.clone(),
                reason: None,
            }
        })
        .unwrap();
        assert_eq!(
            text(&finished(&rx).await),
            if accept { "approved" } else { "denied" }
        );
        let reply: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(f.dir.path().join("decision.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(reply["id"], "approval-1");
    }
}
#[tokio::test]
async fn cancellation_interrupts_turn() {
    let f = Fixture::new();
    let (b, rx) = f.spawn();
    ready(&b).await;
    start(&b, "wait");
    let events = until(&rx, |e| matches!(e, AgentEvent::RunStarted { .. })).await;
    let AgentEvent::RunStarted { run_id, .. } = events.last().unwrap() else {
        panic!()
    };
    b.send(AgentCommand::CancelRun {
        run_id: run_id.clone(),
    })
    .unwrap();
    assert!(finished(&rx).await.iter().any(|e| matches!(
        e,
        AgentEvent::RunFinished {
            status: RunStatus::Cancelled,
            ..
        }
    )));
}
#[tokio::test]
async fn reconnect_reconciles_suffix_without_resending_turn() {
    let f = Fixture::new();
    let (b, rx) = f.spawn();
    ready(&b).await;
    start(&b, "crash");
    assert_eq!(text(&finished(&rx).await), "hello world");
    let history: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(f.dir.path().join("history.json")).unwrap())
            .unwrap();
    assert_eq!(history.as_array().unwrap().len(), 1);
}
#[tokio::test]
async fn unknown_permission_request_is_rejected() {
    let f = Fixture::new();
    let (b, rx) = f.spawn();
    ready(&b).await;
    start(&b, "unknown-request");
    finished(&rx).await;
    let reply: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(f.dir.path().join("decision.json")).unwrap())
            .unwrap();
    assert_eq!(reply["error"]["code"], -32601);
}
#[tokio::test]
async fn signed_out_does_not_start_login_or_turn() {
    let f = Fixture::new();
    std::fs::write(f.dir.path().join("signed-out"), "").unwrap();
    let (b, _) = f.spawn();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if b.updates.recv_async().await.unwrap().state == ConnectionState::SignInRequired {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(!f.dir.path().join("starts").exists());
}
#[tokio::test]
async fn unavailable_model_fails_without_starting_thread() {
    let f = Fixture::new();
    let (b, rx) = f.spawn();
    ready(&b).await;
    b.send(AgentCommand::StartRun {
        project_id: ProjectId::new("p"),
        session_id: SessionId::new("s"),
        prompt: "test".into(),
        model: ModelId::new("missing"),
        subagent_model: None,
        mode: AgentMode::ApplyWithApproval,
        attachments: vec![],
    })
    .unwrap();
    until(&rx, |e| {
        matches!(e, AgentEvent::SessionCommandFailed { .. })
    })
    .await;
    assert!(!f.dir.path().join("starts").exists());
}

#[tokio::test]
#[ignore = "uses the installed Codex CLI and signed-in account for two tiny inference turns"]
async fn live_codex_login_and_conversation_survive_backend_restart() {
    let mut f = Fixture::new();
    f.binary = std::env::var_os("VORTEX_CODEX_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let (b, rx) = f.spawn();
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let status = b.updates.recv_async().await.unwrap();
            assert_ne!(
                status.state,
                ConnectionState::SignInRequired,
                "Codex login required"
            );
            assert_ne!(
                status.state,
                ConnectionState::Unavailable,
                "{}",
                status.detail
            );
            if status.state == ConnectionState::Ready {
                break;
            }
        }
    })
    .await
    .unwrap();
    let models = b.status().models;
    let model = models
        .iter()
        .find(|m| m.id.contains("sol"))
        .unwrap_or(&models[0])
        .id
        .clone();
    let send = |b: &CodexBackend, prompt: &str| {
        b.send(AgentCommand::StartRun {
            project_id: ProjectId::new("p"),
            session_id: SessionId::new("s"),
            prompt: prompt.into(),
            model: ModelId::new(model.clone()),
            subagent_model: None,
            mode: AgentMode::ReadOnlyInspect,
            attachments: vec![],
        })
        .unwrap()
    };
    send(
        &b,
        "Remember the marker VORTEX_COBALT_731. Do not use any tools. Reply only ACK.",
    );
    let first = tokio::time::timeout(Duration::from_secs(90), async {
        let mut events = Vec::new();
        loop {
            let e = rx.recv_async().await.unwrap();
            let done = matches!(
                e,
                AgentEvent::RunFinished { .. } | AgentEvent::SessionCommandFailed { .. }
            );
            events.push(e);
            if done {
                break events;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        first.iter().any(|e| matches!(
            e,
            AgentEvent::RunFinished {
                status: RunStatus::Completed,
                ..
            }
        )),
        "{first:?}"
    );
    drop(b);
    tokio::time::timeout(Duration::from_secs(10), async {
        while rx.recv_async().await.is_ok() {}
    })
    .await
    .unwrap();
    let (b, rx) = f.spawn();
    ready(&b).await;
    send(
        &b,
        "What marker did I ask you to remember? Reply only with that marker. Do not use tools.",
    );
    let second = tokio::time::timeout(Duration::from_secs(90), async {
        let mut events = Vec::new();
        loop {
            let e = rx.recv_async().await.unwrap();
            let done = matches!(
                e,
                AgentEvent::RunFinished { .. } | AgentEvent::SessionCommandFailed { .. }
            );
            events.push(e);
            if done {
                break events;
            }
        }
    })
    .await
    .unwrap();
    assert!(
        second.iter().any(|e| matches!(
            e,
            AgentEvent::RunFinished {
                status: RunStatus::Completed,
                ..
            }
        )),
        "{second:?}"
    );
    assert!(text(&second).contains("VORTEX_COBALT_731"), "{second:?}");
    println!(
        "Local Codex login reused; conversation resumed after process restart; context preserved."
    );
}

#[tokio::test]
#[ignore = "uses the local Codex account and approves one edit inside a disposable directory"]
async fn live_codex_edit_requires_approval_and_reports_completed_diff() {
    let mut f = Fixture::new();
    f.binary = std::env::var_os("VORTEX_CODEX_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let target = f.dir.path().join("vortex-smoke.txt");
    std::fs::write(&target, "before\n").unwrap();
    let (b, rx) = f.spawn();
    ready(&b).await;
    let models = b.status().models;
    let model = models
        .iter()
        .find(|m| m.id.contains("sol"))
        .unwrap_or(&models[0])
        .id
        .clone();
    b.send(AgentCommand::StartRun{project_id:ProjectId::new("p"),session_id:SessionId::new("s"),prompt:"Use apply_patch to change vortex-smoke.txt from before to after. Request approval if needed. Do not run shell commands or change any other files.".into(),model:ModelId::new(model),subagent_model:None,mode:AgentMode::ApplyWithApproval,attachments:vec![]}).unwrap();
    let mut approved = false;
    let mut observed = false;
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let event = rx.recv_async().await.unwrap();
            match event {
                AgentEvent::ApprovalRequested { approval_id, .. } => {
                    assert_eq!(
                        std::fs::read_to_string(&target).unwrap(),
                        "before\n",
                        "edit happened before approval"
                    );
                    approved = true;
                    b.send(AgentCommand::ApproveTool { approval_id }).unwrap();
                }
                AgentEvent::FileChangesObserved { applied: true, .. } => observed = true,
                AgentEvent::RunFinished { status, .. } => {
                    assert_eq!(status, RunStatus::Completed);
                    break;
                }
                AgentEvent::RunFailed { error, .. } => panic!("{}", error.message),
                AgentEvent::SessionCommandFailed { message, .. } => panic!("{message}"),
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(approved, "Codex did not request approval");
    assert!(observed, "No completed file diff was normalized");
    assert_eq!(std::fs::read_to_string(target).unwrap(), "after\n");
}
