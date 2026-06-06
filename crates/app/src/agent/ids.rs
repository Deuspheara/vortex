use std::path::PathBuf;

use agent_protocol::{ProjectId as ProtoProjectId, SessionId as ProtoSessionId};

use crate::features::shell::state::{ConversationId, ProjectId};

pub fn new_project_id() -> ProjectId {
    ProjectId(format!("proj-{}", uuid::Uuid::new_v4()))
}

pub fn new_session_id() -> ConversationId {
    ConversationId(format!("sess-{}", uuid::Uuid::new_v4()))
}

pub fn proto_project_id(id: &ProjectId) -> ProtoProjectId {
    ProtoProjectId::new(id.0.clone())
}

pub fn proto_session_id(id: &ConversationId) -> ProtoSessionId {
    ProtoSessionId::new(id.0.clone())
}

pub fn ui_project_id(id: &ProtoProjectId) -> ProjectId {
    ProjectId(id.0.clone())
}

pub fn ui_conversation_id(session: &ProtoSessionId) -> ConversationId {
    ConversationId(session.0.clone())
}

pub fn vortex_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".config").join("vortex"))
        .unwrap_or_else(|| PathBuf::from(".vortex"))
}

pub fn workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn sidecar_entry() -> PathBuf {
    workspace_root().join("sidecars/browser_worker/src/main.ts")
}
