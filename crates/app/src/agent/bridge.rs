use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_core::{AgentRuntime, AgentRuntimeConfig, ReadCacheStats};
use agent_models::{MockProvider, OpenRouterModelInfo, OpenRouterProvider};
use agent_protocol::{AgentCommand, AgentEvent, AgentRunLimits, ProjectId, SessionId};
use agent_store::{EventStore, SqliteEventStore, StoredProject, StoredSession};
use chrono::{DateTime, Utc};
use project_index::{
    IndexPhase as CoreIndexPhase, IndexSnapshot, RepoIndex, load_index_snapshot, mark_index_failed,
    mark_index_phase, mark_index_stale, project_db_path,
};

use crate::agent::paths::{canonical_project_path, git_head_branch};
use crate::agent::{
    new_project_id, new_session_id, sidecar_entry, vortex_data_dir, workspace_root,
};
use crate::features::shell::state::{
    IndexPhase, ProjectId as UiProjectId, ProjectIndexStats, ProjectIndexStatus, ReadCacheRecap,
};

pub struct AgentBridge {
    pub runtime: Arc<AgentRuntime>,
    pub event_rx: flume::Receiver<AgentEvent>,
    pub uses_mock: bool,
    /// Populated asynchronously on the agent tokio runtime when OpenRouter is configured.
    pub openrouter_models_rx: Option<flume::Receiver<Result<Vec<OpenRouterModelInfo>, String>>>,
    index_workers: Arc<Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>>,
    _tokio: tokio::runtime::Runtime,
}

impl AgentBridge {
    pub fn new(use_mock: bool) -> Self {
        let tokio = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("vortex-agent")
            .build()
            .expect("failed to start agent tokio runtime");

        let db_path = vortex_data_dir().join("vortex.db");
        let checkpoint_dir = vortex_data_dir().join("checkpoints");
        let store: Arc<dyn EventStore> =
            Arc::new(SqliteEventStore::open(db_path).expect("open vortex.db"));

        let config = AgentRuntimeConfig {
            checkpoint_dir,
            sidecar_entry: sidecar_entry(),
            limits: AgentRunLimits::default(),
        };

        let (uses_mock, provider): (bool, Arc<dyn agent_models::ModelProvider>) = if use_mock {
            (true, Arc::new(MockProvider::default()))
        } else if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
            if key.trim().is_empty() {
                (true, Arc::new(MockProvider::default()))
            } else {
                (false, Arc::new(OpenRouterProvider::new(key)))
            }
        } else {
            (true, Arc::new(MockProvider::default()))
        };

        let (runtime, event_rx, command_rx) = AgentRuntime::new(store.clone(), provider, config);
        let runtime = Arc::new(runtime);

        #[cfg(feature = "demo_mode")]
        {
            runtime
                .ensure_seed_data(&workspace_root())
                .expect("seed agent store");
        }

        let openrouter_api_key = if uses_mock {
            None
        } else {
            std::env::var("OPENROUTER_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty())
        };

        let openrouter_models_rx = openrouter_api_key.as_ref().map(|key| {
            let (tx, rx) = flume::bounded(1);
            let key = key.clone();
            tokio.handle().spawn(async move {
                let result = OpenRouterProvider::new(key)
                    .list_models()
                    .await
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
            });
            rx
        });

        runtime.clone().spawn(tokio.handle().clone(), command_rx);
        Self {
            runtime,
            event_rx,
            uses_mock,
            openrouter_models_rx,
            index_workers: Arc::new(Mutex::new(std::collections::HashMap::new())),
            _tokio: tokio,
        }
    }

    pub fn send(&self, command: AgentCommand) -> Result<(), String> {
        match &command {
            AgentCommand::StartRun {
                session_id, model, ..
            } => {
                tracing::info!(
                    command = "StartRun",
                    session_id = %session_id.0,
                    model = %model.0,
                    "agent command sent"
                );
            }
            AgentCommand::CancelRun { run_id } => {
                tracing::info!(command = "CancelRun", run_id = %run_id.0, "agent command sent");
            }
            AgentCommand::ApproveTool { approval_id } => {
                tracing::info!(
                    command = "ApproveTool",
                    approval_id = %approval_id.0,
                    "agent command sent"
                );
            }
            AgentCommand::ApproveToolAlways { approval_id } => {
                tracing::info!(
                    command = "ApproveToolAlways",
                    approval_id = %approval_id.0,
                    "agent command sent"
                );
            }
            AgentCommand::RejectTool { approval_id, .. } => {
                tracing::info!(
                    command = "RejectTool",
                    approval_id = %approval_id.0,
                    "agent command sent"
                );
            }
            AgentCommand::RollbackCheckpoint { checkpoint_id } => {
                tracing::info!(
                    command = "RollbackCheckpoint",
                    checkpoint_id = %checkpoint_id.0,
                    "agent command sent"
                );
            }
            _ => tracing::info!(command = "other", "agent command sent"),
        }
        self.runtime
            .send_command(command)
            .map_err(|e| e.to_string())
    }

    /// Ensures at least one project + session exist in the store (workspace folder by default).
    pub fn ensure_workspace_session(&self) -> Result<(StoredProject, StoredSession), String> {
        let projects = self.list_projects()?;
        if let Some(project) = projects.into_iter().next() {
            let sessions = self.list_sessions(&project.id)?;
            if let Some(session) = sessions.into_iter().next() {
                return Ok((project, session));
            }
            let session = self.create_session(&project.id, "New Conversation")?;
            return Ok((project, session));
        }

        let root = workspace_root();
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        let project = self.upsert_project(&root, name)?;
        let session = self.create_session(&project.id, "New Conversation")?;
        Ok((project, session))
    }

    pub fn list_projects(&self) -> Result<Vec<StoredProject>, String> {
        self.runtime.store.list_projects()
    }

    pub fn list_sessions(&self, project_id: &ProjectId) -> Result<Vec<StoredSession>, String> {
        self.runtime.store.list_sessions(project_id)
    }

    pub fn find_project_by_path(&self, path: &Path) -> Result<Option<StoredProject>, String> {
        let canonical = canonical_project_path(&path.display().to_string())?;
        let canonical_str = canonical.display().to_string();
        Ok(self.runtime.store.list_projects()?.into_iter().find(|p| {
            canonical_project_path(&p.root_path)
                .map(|c| c.display().to_string() == canonical_str)
                .unwrap_or(false)
        }))
    }

    pub fn upsert_project(&self, root_path: &Path, name: &str) -> Result<StoredProject, String> {
        let canonical = canonical_project_path(&root_path.display().to_string())?;
        let now = Utc::now();
        let existing = self.find_project_by_path(&canonical)?;

        let project = StoredProject {
            id: existing
                .as_ref()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| ProjectId::new(new_project_id().0)),
            root_path: canonical.display().to_string(),
            name: name.to_string(),
            trusted: existing.as_ref().map(|p| p.trusted).unwrap_or(false),
            created_at: existing.as_ref().map(|p| p.created_at).unwrap_or(now),
            updated_at: now,
        };
        self.runtime.store.upsert_project(&project)?;
        Ok(project)
    }

    pub fn set_project_trusted(
        &self,
        project_id: &ProjectId,
        trusted: bool,
    ) -> Result<StoredProject, String> {
        let Some(mut project) = self.runtime.store.get_project(project_id)? else {
            return Err(format!("project {} not found", project_id.0));
        };
        project.trusted = trusted;
        project.updated_at = Utc::now();
        self.runtime.store.upsert_project(&project)?;
        Ok(project)
    }

    pub fn update_session_title(&self, session_id: &SessionId, title: &str) -> Result<(), String> {
        self.runtime.store.update_session_title(session_id, title)
    }

    pub fn create_session(
        &self,
        project_id: &ProjectId,
        title: &str,
    ) -> Result<StoredSession, String> {
        let now = Utc::now();
        let session = StoredSession {
            id: SessionId::new(new_session_id().0),
            project_id: project_id.clone(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
        };
        self.runtime.store.create_session(&session)?;
        Ok(session)
    }

    pub fn delete_session(&self, session_id: &SessionId) -> Result<(), String> {
        self.runtime.store.delete_session(session_id)
    }

    pub fn delete_project(&self, project_id: &ProjectId) -> Result<(), String> {
        self.runtime.store.delete_project(project_id)
    }

    pub fn git_branch_for_path(&self, root_path: &str) -> String {
        canonical_project_path(root_path)
            .map(|p| git_head_branch(&p))
            .unwrap_or_else(|_| "main".into())
    }

    pub fn ensure_project_indexing(&self, project_id: &ProjectId, root_path: &str) {
        let Ok(root_path) = canonical_project_path(root_path) else {
            return;
        };
        let key = project_id.0.clone();
        let mut workers = self.index_workers.lock().unwrap();
        if workers.contains_key(&key) {
            return;
        }
        let alive = Arc::new(AtomicBool::new(true));
        workers.insert(key.clone(), alive.clone());
        let db_path = project_db_path(&vortex_data_dir(), &project_id.0);
        std::thread::Builder::new()
            .name(format!("vortex-index-{}", project_id.0))
            .spawn(move || {
                let _ = mark_index_phase(&db_path, CoreIndexPhase::Queued);
                loop {
                    let index = match RepoIndex::build(root_path.clone(), &db_path) {
                        Ok(index) => index,
                        Err(err) => {
                            let _ = mark_index_failed(&db_path, &err);
                            return;
                        }
                    };
                    let Ok((_watcher, rx)) = index.watch() else {
                        return;
                    };
                    while alive.load(Ordering::Relaxed) {
                        let event = rx.recv_timeout(Duration::from_secs(30));
                        if event.is_err() {
                            continue;
                        }
                        let _ = mark_index_stale(&db_path);
                        std::thread::sleep(Duration::from_millis(750));
                        while rx.recv_timeout(Duration::from_millis(50)).is_ok() {}
                        let _ = mark_index_phase(&db_path, CoreIndexPhase::Queued);
                        break;
                    }
                    if !alive.load(Ordering::Relaxed) {
                        return;
                    }
                }
            })
            .ok();
    }

    pub fn project_index_status(&self, project_id: &UiProjectId) -> ProjectIndexStatus {
        let db_path = project_db_path(&vortex_data_dir(), &project_id.0);
        let snapshot = load_index_snapshot(&db_path).unwrap_or_default();
        map_index_snapshot(snapshot)
    }

    pub fn read_cache_recap(&self, session_id: &SessionId) -> ReadCacheRecap {
        let stats = self
            ._tokio
            .block_on(self.runtime.read_cache_stats_for_session(session_id));
        map_read_cache_stats(stats)
    }

    pub fn page_index_configured(&self) -> bool {
        std::env::var("PAGEINDEX_API_KEY")
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
    }
}

fn map_read_cache_stats(stats: ReadCacheStats) -> ReadCacheRecap {
    ReadCacheRecap {
        entries: stats.entries,
        bytes: stats.bytes,
        hits: stats.hits,
    }
}

fn map_index_snapshot(snapshot: IndexSnapshot) -> ProjectIndexStatus {
    ProjectIndexStatus {
        phase: match snapshot.phase {
            CoreIndexPhase::Unindexed => IndexPhase::Unindexed,
            CoreIndexPhase::Queued => IndexPhase::Queued,
            CoreIndexPhase::Scanning => IndexPhase::Scanning,
            CoreIndexPhase::Parsing => IndexPhase::Parsing,
            CoreIndexPhase::Summarizing => IndexPhase::Summarizing,
            CoreIndexPhase::Ready => IndexPhase::Ready,
            CoreIndexPhase::Stale => IndexPhase::Stale,
            CoreIndexPhase::Failed => IndexPhase::Failed,
        },
        last_indexed_at: snapshot.last_indexed_unix_secs.map(format_index_age),
        last_error: snapshot.last_error,
        stale: snapshot.stale,
        active_ignore_sources: snapshot.active_ignore_sources,
        stats: ProjectIndexStats {
            files_indexed: snapshot.stats.files_indexed,
            skipped_ignore: snapshot.stats.skipped_ignore,
            skipped_hidden: snapshot.stats.skipped_hidden,
            skipped_binary: snapshot.stats.skipped_binary,
            skipped_large: snapshot.stats.skipped_large,
            skipped_policy: snapshot.stats.skipped_policy,
            symbols_indexed: snapshot.symbols_indexed,
            summaries_cached: snapshot.summaries_cached,
        },
    }
}

fn format_index_age(unix_secs: i64) -> String {
    let updated = DateTime::<Utc>::from_timestamp(unix_secs, 0).unwrap_or_else(Utc::now);
    format_session_age(&updated)
}

pub fn format_session_age(updated_at: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*updated_at);
    let days = duration.num_days();
    if days <= 0 {
        "now".into()
    } else if days == 1 {
        "1d".into()
    } else if days < 30 {
        format!("{days}d")
    } else if days < 365 {
        format!("{}mo", days / 30)
    } else {
        format!("{}y", days / 365)
    }
}
