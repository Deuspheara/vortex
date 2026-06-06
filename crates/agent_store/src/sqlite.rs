use std::path::Path;
use std::sync::Mutex;

use agent_protocol::{
    AgentEvent, AgentMode, CheckpointId, EventId, PatchId, ProjectId, RiskLevel, RunId, RunStatus,
    SessionId, ToolCallId, ToolStatus, WorkspaceCheckpoint,
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde_json;

use crate::{
    EventStore, StoredApprovalRule, StoredEvent, StoredPatchProposal, StoredProject, StoredRun,
    StoredSession, StoredToolCall,
};

pub struct SqliteEventStore {
    conn: Mutex<Connection>,
}

impl SqliteEventStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init()?;
        Ok(store)
    }
}

const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  root_path TEXT NOT NULL,
  name TEXT NOT NULL,
  trusted INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  title TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  parent_run_id TEXT,
  depth INTEGER NOT NULL DEFAULT 0,
  model TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT
);

CREATE TABLE IF NOT EXISTS events (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(run_id, sequence)
);

CREATE TABLE IF NOT EXISTS tool_calls (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  name TEXT NOT NULL,
  args_json TEXT NOT NULL,
  risk TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT
);

CREATE TABLE IF NOT EXISTS patch_proposals (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  base_git_sha TEXT,
  diff TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS approval_rules (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  command_pattern TEXT,
  path_prefix TEXT,
  max_risk TEXT NOT NULL,
  expires_at TEXT
);

CREATE TABLE IF NOT EXISTS checkpoints (
  id TEXT PRIMARY KEY,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
"#;

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn format_dt(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

fn ensure_runs_column(conn: &Connection, name: &str, ddl: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(runs)")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let col_name: String = row.get(1).map_err(|e| e.to_string())?;
        if col_name == name {
            return Ok(());
        }
    }
    conn.execute(&format!("ALTER TABLE runs ADD COLUMN {name} {ddl}"), [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn serialize_event(event: &AgentEvent) -> Result<(String, String), String> {
    let event_type = match event {
        AgentEvent::RunStarted { .. } => "RunStarted",
        AgentEvent::ContextBuilt { .. } => "ContextBuilt",
        AgentEvent::AssistantTextDelta { .. } => "AssistantTextDelta",
        AgentEvent::ReasoningDelta { .. } => "ReasoningDelta",
        AgentEvent::ToolCallStarted { .. } => "ToolCallStarted",
        AgentEvent::ToolCallUpdated { .. } => "ToolCallUpdated",
        AgentEvent::ApprovalRequested { .. } => "ApprovalRequested",
        AgentEvent::ChoiceRequested { .. } => "ChoiceRequested",
        AgentEvent::TodoUpdated { .. } => "TodoUpdated",
        AgentEvent::ContextTrace { .. } => "ContextTrace",
        AgentEvent::PlanUpdated { .. } => "PlanUpdated",
        AgentEvent::AndroidSessionUpdated { .. } => "AndroidSessionUpdated",
        AgentEvent::AndroidObservationUpdated { .. } => "AndroidObservationUpdated",
        AgentEvent::AndroidActionPreviewed { .. } => "AndroidActionPreviewed",
        AgentEvent::AndroidActionCompleted { .. } => "AndroidActionCompleted",
        AgentEvent::AndroidJourneyUpdated { .. } => "AndroidJourneyUpdated",
        AgentEvent::SubagentStarted { .. } => "SubagentStarted",
        AgentEvent::SubagentFinished { .. } => "SubagentFinished",
        AgentEvent::ToolOutputDelta { .. } => "ToolOutputDelta",
        AgentEvent::ToolCallFinished { .. } => "ToolCallFinished",
        AgentEvent::PatchPreviewUpdated { .. } => "PatchPreviewUpdated",
        AgentEvent::PatchProposed { .. } => "PatchProposed",
        AgentEvent::PatchApplied { .. } => "PatchApplied",
        AgentEvent::UsageUpdated { .. } => "UsageUpdated",
        AgentEvent::RunFinished { .. } => "RunFinished",
        AgentEvent::RunFailed { .. } => "RunFailed",
        AgentEvent::CommandFailed { .. } => "CommandFailed",
    };
    let payload = serde_json::to_string(event).map_err(|e| e.to_string())?;
    Ok((event_type.to_string(), payload))
}

fn deserialize_event(_event_type: &str, payload: &str) -> rusqlite::Result<AgentEvent> {
    serde_json::from_str(payload).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn delete_runs_for_session(conn: &Connection, session_id: &SessionId) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id FROM runs WHERE session_id = ?1")
        .map_err(|e| e.to_string())?;
    let run_ids: Vec<String> = stmt
        .query_map(params![session_id.0], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    for run_id in run_ids {
        conn.execute("DELETE FROM events WHERE run_id = ?1", params![run_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM tool_calls WHERE run_id = ?1", params![run_id])
            .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM patch_proposals WHERE run_id = ?1",
            params![run_id],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "DELETE FROM runs WHERE session_id = ?1",
        params![session_id.0],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

impl EventStore for SqliteEventStore {
    fn init(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(MIGRATIONS).map_err(|e| e.to_string())?;
        ensure_runs_column(&conn, "parent_run_id", "TEXT")?;
        ensure_runs_column(&conn, "depth", "INTEGER NOT NULL DEFAULT 0")?;
        Ok(())
    }

    fn upsert_project(&self, project: &StoredProject) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO projects (id, root_path, name, trusted, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               root_path=excluded.root_path,
               name=excluded.name,
               trusted=excluded.trusted,
               updated_at=excluded.updated_at",
            params![
                project.id.0,
                project.root_path,
                project.name,
                project.trusted as i32,
                format_dt(&project.created_at),
                format_dt(&project.updated_at),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn list_projects(&self) -> Result<Vec<StoredProject>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, root_path, name, trusted, created_at, updated_at FROM projects ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(StoredProject {
                    id: ProjectId(row.get(0)?),
                    root_path: row.get(1)?,
                    name: row.get(2)?,
                    trusted: row.get::<_, i32>(3)? != 0,
                    created_at: parse_dt(&row.get::<_, String>(4)?),
                    updated_at: parse_dt(&row.get::<_, String>(5)?),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn get_project(&self, id: &ProjectId) -> Result<Option<StoredProject>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, root_path, name, trusted, created_at, updated_at FROM projects WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id.0]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Ok(Some(StoredProject {
                id: ProjectId(row.get(0).map_err(|e| e.to_string())?),
                root_path: row.get(1).map_err(|e| e.to_string())?,
                name: row.get(2).map_err(|e| e.to_string())?,
                trusted: row.get::<_, i32>(3).map_err(|e| e.to_string())? != 0,
                created_at: parse_dt(&row.get::<_, String>(4).map_err(|e| e.to_string())?),
                updated_at: parse_dt(&row.get::<_, String>(5).map_err(|e| e.to_string())?),
            }))
        } else {
            Ok(None)
        }
    }

    fn create_session(&self, session: &StoredSession) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO sessions (id, project_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session.id.0,
                session.project_id.0,
                session.title,
                format_dt(&session.created_at),
                format_dt(&session.updated_at),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn update_session_title(&self, id: &SessionId, title: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, format_dt(&Utc::now()), id.0],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn delete_session(&self, session_id: &SessionId) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        delete_runs_for_session(&tx, session_id)?;
        tx.execute("DELETE FROM sessions WHERE id = ?1", params![session_id.0])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn list_sessions(&self, project_id: &ProjectId) -> Result<Vec<StoredSession>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, title, created_at, updated_at FROM sessions WHERE project_id = ?1 ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![project_id.0], |row| {
                Ok(StoredSession {
                    id: SessionId(row.get(0)?),
                    project_id: ProjectId(row.get(1)?),
                    title: row.get(2)?,
                    created_at: parse_dt(&row.get::<_, String>(3)?),
                    updated_at: parse_dt(&row.get::<_, String>(4)?),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn get_session(&self, id: &SessionId) -> Result<Option<StoredSession>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, title, created_at, updated_at FROM sessions WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id.0]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Ok(Some(StoredSession {
                id: SessionId(row.get(0).map_err(|e| e.to_string())?),
                project_id: ProjectId(row.get(1).map_err(|e| e.to_string())?),
                title: row.get(2).map_err(|e| e.to_string())?,
                created_at: parse_dt(&row.get::<_, String>(3).map_err(|e| e.to_string())?),
                updated_at: parse_dt(&row.get::<_, String>(4).map_err(|e| e.to_string())?),
            }))
        } else {
            Ok(None)
        }
    }

    fn delete_project(&self, project_id: &ProjectId) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        let mut stmt = tx
            .prepare("SELECT id FROM sessions WHERE project_id = ?1")
            .map_err(|e| e.to_string())?;
        let session_ids: Vec<String> = stmt
            .query_map(params![project_id.0], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        for session_id in session_ids {
            delete_runs_for_session(&tx, &SessionId::new(session_id.clone()))?;
            tx.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])
                .map_err(|e| e.to_string())?;
        }

        tx.execute(
            "DELETE FROM approval_rules WHERE project_id = ?1",
            params![project_id.0],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM projects WHERE id = ?1", params![project_id.0])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn create_run(&self, run: &StoredRun) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO runs (id, session_id, parent_run_id, depth, model, mode, status, started_at, finished_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id.0,
                run.session_id.0,
                run.parent_run_id.as_ref().map(|id| id.0.clone()),
                run.depth,
                run.model.0,
                serde_json::to_string(&run.mode).map_err(|e| e.to_string())?,
                serde_json::to_string(&run.status).map_err(|e| e.to_string())?,
                format_dt(&run.started_at),
                run.finished_at.as_ref().map(format_dt),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn update_run_status(
        &self,
        id: &RunId,
        status: RunStatus,
        finished_at: Option<DateTime<Utc>>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE runs SET status = ?1, finished_at = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(&status).map_err(|e| e.to_string())?,
                finished_at.as_ref().map(format_dt),
                id.0
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get_run(&self, id: &RunId) -> Result<Option<StoredRun>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, parent_run_id, depth, model, mode, status, started_at, finished_at FROM runs WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id.0]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let mode: AgentMode =
                serde_json::from_str(&row.get::<_, String>(5).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let status: RunStatus =
                serde_json::from_str(&row.get::<_, String>(6).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let finished_at = row
                .get::<_, Option<String>>(8)
                .map_err(|e| e.to_string())?
                .map(|s| parse_dt(&s));
            Ok(Some(StoredRun {
                id: RunId(row.get(0).map_err(|e| e.to_string())?),
                session_id: SessionId(row.get(1).map_err(|e| e.to_string())?),
                parent_run_id: row
                    .get::<_, Option<String>>(2)
                    .map_err(|e| e.to_string())?
                    .map(RunId),
                depth: row.get(3).map_err(|e| e.to_string())?,
                model: agent_protocol::ModelId(row.get(4).map_err(|e| e.to_string())?),
                mode,
                status,
                started_at: parse_dt(&row.get::<_, String>(7).map_err(|e| e.to_string())?),
                finished_at,
            }))
        } else {
            Ok(None)
        }
    }

    fn active_run_for_session(&self, session_id: &SessionId) -> Result<Option<StoredRun>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, parent_run_id, depth, model, mode, status, started_at, finished_at FROM runs
                 WHERE session_id = ?1 AND status IN ('\"Running\"', '\"PausedForApproval\"')
                 ORDER BY started_at DESC LIMIT 1",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query(params![session_id.0])
            .map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let mode: AgentMode =
                serde_json::from_str(&row.get::<_, String>(5).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let status: RunStatus =
                serde_json::from_str(&row.get::<_, String>(6).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let finished_at = row
                .get::<_, Option<String>>(8)
                .map_err(|e| e.to_string())?
                .map(|s| parse_dt(&s));
            Ok(Some(StoredRun {
                id: RunId(row.get(0).map_err(|e| e.to_string())?),
                session_id: SessionId(row.get(1).map_err(|e| e.to_string())?),
                parent_run_id: row
                    .get::<_, Option<String>>(2)
                    .map_err(|e| e.to_string())?
                    .map(RunId),
                depth: row.get(3).map_err(|e| e.to_string())?,
                model: agent_protocol::ModelId(row.get(4).map_err(|e| e.to_string())?),
                mode,
                status,
                started_at: parse_dt(&row.get::<_, String>(7).map_err(|e| e.to_string())?),
                finished_at,
            }))
        } else {
            Ok(None)
        }
    }

    fn append_event(&self, event: &StoredEvent) -> Result<(), String> {
        let (event_type, payload) = serialize_event(&event.event)?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO events (id, run_id, sequence, event_type, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.id.0,
                event.run_id.0,
                event.sequence,
                event_type,
                payload,
                format_dt(&event.created_at),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn record_event(&self, run_id: &RunId, event: AgentEvent) -> Result<StoredEvent, String> {
        let (event_type, payload) = serialize_event(&event)?;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let sequence: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE run_id = ?1",
                params![run_id.0],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let stored = StoredEvent {
            id: EventId::new(uuid::Uuid::new_v4().to_string()),
            run_id: run_id.clone(),
            sequence,
            event,
            created_at: Utc::now(),
        };
        conn.execute(
            "INSERT INTO events (id, run_id, sequence, event_type, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                stored.id.0,
                stored.run_id.0,
                stored.sequence,
                event_type,
                payload,
                format_dt(&stored.created_at),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(stored)
    }

    fn load_run_events(&self, run_id: &RunId) -> Result<Vec<StoredEvent>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, sequence, event_type, payload_json, created_at FROM events WHERE run_id = ?1 ORDER BY sequence ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![run_id.0], |row| {
                let event_type: String = row.get(3)?;
                let payload: String = row.get(4)?;
                let event = deserialize_event(&event_type, &payload)?;
                Ok(StoredEvent {
                    id: EventId(row.get(0)?),
                    run_id: RunId(row.get(1)?),
                    sequence: row.get(2)?,
                    event,
                    created_at: parse_dt(&row.get::<_, String>(5)?),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn load_session_events(&self, session_id: &SessionId) -> Result<Vec<StoredEvent>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT e.id, e.run_id, e.sequence, e.event_type, e.payload_json, e.created_at
                 FROM events e
                 JOIN runs r ON r.id = e.run_id
                 WHERE r.session_id = ?1
                 ORDER BY e.sequence ASC, e.created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![session_id.0], |row| {
                let event_type: String = row.get(3)?;
                let payload: String = row.get(4)?;
                let event = deserialize_event(&event_type, &payload)?;
                Ok(StoredEvent {
                    id: EventId(row.get(0)?),
                    run_id: RunId(row.get(1)?),
                    sequence: row.get(2)?,
                    event,
                    created_at: parse_dt(&row.get::<_, String>(5)?),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn next_event_sequence(&self, run_id: &RunId) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT COALESCE(MAX(sequence), 0) + 1 FROM events WHERE run_id = ?1")
            .map_err(|e| e.to_string())?;
        let seq: i64 = stmt
            .query_row(params![run_id.0], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(seq)
    }

    fn record_tool_call(&self, call: &StoredToolCall) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO tool_calls (id, run_id, name, args_json, risk, status, started_at, finished_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               args_json = excluded.args_json,
               risk = excluded.risk,
               status = excluded.status,
               started_at = excluded.started_at,
               finished_at = excluded.finished_at
             WHERE tool_calls.run_id = excluded.run_id",
            params![
                call.id.0,
                call.run_id.0,
                call.name,
                call.args_json,
                serde_json::to_string(&call.risk).map_err(|e| e.to_string())?,
                serde_json::to_string(&call.status).map_err(|e| e.to_string())?,
                format_dt(&call.started_at),
                call.finished_at.as_ref().map(format_dt),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn update_tool_call(
        &self,
        id: &ToolCallId,
        status: ToolStatus,
        finished_at: Option<DateTime<Utc>>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE tool_calls SET status = ?1, finished_at = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(&status).map_err(|e| e.to_string())?,
                finished_at.as_ref().map(format_dt),
                id.0
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn record_patch_proposal(&self, proposal: &StoredPatchProposal) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO patch_proposals (id, run_id, base_git_sha, diff, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                proposal.id.0,
                proposal.run_id.0,
                proposal.base_git_sha,
                proposal.diff,
                proposal.status,
                format_dt(&proposal.created_at),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn update_patch_status(&self, id: &PatchId, status: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE patch_proposals SET status = ?1 WHERE id = ?2",
            params![status, id.0],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn save_approval_rule(&self, rule: &StoredApprovalRule) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO approval_rules (project_id, tool_name, command_pattern, path_prefix, max_risk, expires_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6
             WHERE NOT EXISTS (
               SELECT 1 FROM approval_rules
               WHERE project_id = ?1
                 AND tool_name = ?2
                 AND COALESCE(command_pattern, '') = COALESCE(?3, '')
                 AND COALESCE(path_prefix, '') = COALESCE(?4, '')
             )",
            params![
                rule.project_id.0,
                rule.tool_name,
                rule.command_pattern,
                rule.path_prefix,
                serde_json::to_string(&rule.max_risk).map_err(|e| e.to_string())?,
                rule.expires_at.as_ref().map(format_dt),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn list_approval_rules(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<StoredApprovalRule>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT project_id, tool_name, command_pattern, path_prefix, max_risk, expires_at
                 FROM approval_rules WHERE project_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![project_id.0], |row| {
                let max_risk: RiskLevel =
                    serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or(RiskLevel::Medium);
                let expires_at = row.get::<_, Option<String>>(5)?.map(|s| parse_dt(&s));
                Ok(StoredApprovalRule {
                    project_id: ProjectId(row.get(0)?),
                    tool_name: row.get(1)?,
                    command_pattern: row.get(2)?,
                    path_prefix: row.get(3)?,
                    max_risk,
                    expires_at,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn save_checkpoint(&self, checkpoint: &WorkspaceCheckpoint) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let payload = serde_json::to_string(checkpoint).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO checkpoints (id, payload_json, created_at) VALUES (?1, ?2, ?3)",
            params![checkpoint.id.0, payload, format_dt(&checkpoint.created_at),],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get_checkpoint(&self, id: &CheckpointId) -> Result<Option<WorkspaceCheckpoint>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT payload_json FROM checkpoints WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(params![id.0]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let payload: String = row.get(0).map_err(|e| e.to_string())?;
            let checkpoint: WorkspaceCheckpoint =
                serde_json::from_str(&payload).map_err(|e| e.to_string())?;
            Ok(Some(checkpoint))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::AgentMode;

    #[test]
    fn round_trip_event() {
        let store = SqliteEventStore::in_memory().unwrap();
        let run_id = RunId::new("run-1");
        let session_id = SessionId::new("sess-1");
        let project = StoredProject {
            id: ProjectId::new("proj-1"),
            root_path: "/tmp".into(),
            name: "Test".into(),
            trusted: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        store.upsert_project(&project).unwrap();
        store
            .create_session(&StoredSession {
                id: session_id.clone(),
                project_id: project.id.clone(),
                title: "Session".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .unwrap();
        store
            .create_run(&StoredRun {
                id: run_id.clone(),
                session_id,
                parent_run_id: None,
                depth: 0,
                model: agent_protocol::ModelId::new("mock"),
                mode: AgentMode::ApplyWithApproval,
                status: RunStatus::Running,
                started_at: Utc::now(),
                finished_at: None,
            })
            .unwrap();

        let event = StoredEvent {
            id: EventId::new("evt-1"),
            run_id: run_id.clone(),
            sequence: 1,
            event: AgentEvent::AssistantTextDelta {
                run_id: run_id.clone(),
                text: "hello".into(),
            },
            created_at: Utc::now(),
        };
        store.append_event(&event).unwrap();
        let loaded = store.load_run_events(&run_id).unwrap();
        assert_eq!(loaded.len(), 1);
        match &loaded[0].event {
            AgentEvent::AssistantTextDelta { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("wrong event type"),
        }
    }

    fn seed_session_with_run_data(store: &SqliteEventStore) -> (ProjectId, SessionId, RunId) {
        let project_id = ProjectId::new("proj-1");
        let session_id = SessionId::new("sess-1");
        let run_id = RunId::new("run-1");
        let now = Utc::now();

        store
            .upsert_project(&StoredProject {
                id: project_id.clone(),
                root_path: "/tmp".into(),
                name: "Test".into(),
                trusted: false,
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        store
            .create_session(&StoredSession {
                id: session_id.clone(),
                project_id: project_id.clone(),
                title: "Session".into(),
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        store
            .create_run(&StoredRun {
                id: run_id.clone(),
                session_id: session_id.clone(),
                parent_run_id: None,
                depth: 0,
                model: agent_protocol::ModelId::new("mock"),
                mode: AgentMode::ApplyWithApproval,
                status: RunStatus::Running,
                started_at: now,
                finished_at: None,
            })
            .unwrap();
        store
            .append_event(&StoredEvent {
                id: EventId::new("evt-1"),
                run_id: run_id.clone(),
                sequence: 1,
                event: AgentEvent::AssistantTextDelta {
                    run_id: run_id.clone(),
                    text: "hello".into(),
                },
                created_at: now,
            })
            .unwrap();
        store
            .record_tool_call(&StoredToolCall {
                id: ToolCallId::new("tool-1"),
                run_id: run_id.clone(),
                name: "read_file".into(),
                args_json: "{}".into(),
                risk: agent_protocol::RiskLevel::SafeRead,
                status: agent_protocol::ToolStatus::Running,
                started_at: now,
                finished_at: None,
            })
            .unwrap();
        store
            .record_patch_proposal(&StoredPatchProposal {
                id: PatchId::new("patch-1"),
                run_id: run_id.clone(),
                base_git_sha: None,
                diff: "diff".into(),
                status: "pending".into(),
                created_at: now,
            })
            .unwrap();
        store
            .save_approval_rule(&StoredApprovalRule {
                project_id: project_id.clone(),
                tool_name: "bash".into(),
                command_pattern: Some("git status".into()),
                path_prefix: None,
                max_risk: agent_protocol::RiskLevel::Low,
                expires_at: None,
            })
            .unwrap();

        (project_id, session_id, run_id)
    }

    #[test]
    fn delete_session_cascades_related_rows() {
        let store = SqliteEventStore::in_memory().unwrap();
        let (project_id, session_id, run_id) = seed_session_with_run_data(&store);

        store.delete_session(&session_id).unwrap();

        assert!(store.get_session(&session_id).unwrap().is_none());
        assert!(store.get_run(&run_id).unwrap().is_none());
        assert!(store.load_run_events(&run_id).unwrap().is_empty());
        assert_eq!(store.list_projects().unwrap().len(), 1);
        assert_eq!(store.list_approval_rules(&project_id).unwrap().len(), 1);
    }

    #[test]
    fn delete_project_cascades_sessions_and_rules() {
        let store = SqliteEventStore::in_memory().unwrap();
        let (project_id, session_id, run_id) = seed_session_with_run_data(&store);

        store.delete_project(&project_id).unwrap();

        assert!(store.get_project(&project_id).unwrap().is_none());
        assert!(store.get_session(&session_id).unwrap().is_none());
        assert!(store.get_run(&run_id).unwrap().is_none());
        assert!(store.list_projects().unwrap().is_empty());
        assert!(store.list_approval_rules(&project_id).unwrap().is_empty());
    }
}
