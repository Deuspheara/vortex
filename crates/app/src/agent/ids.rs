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
    #[cfg(all(feature = "demo_mode", debug_assertions))]
    {
        let path = PathBuf::from(
            std::env::var_os("VORTEX_DEMO_DATA_DIR")
                .expect("demo_mode requires an isolated VORTEX_DEMO_DATA_DIR"),
        );
        assert!(path.is_absolute(), "Demo directory must be absolute");
        assert!(
            path.join(".portfolio-demo").is_file(),
            "Create a dedicated .portfolio-demo marker before launch"
        );
        path
    }

    #[cfg(not(all(feature = "demo_mode", debug_assertions)))]
    {
        dirs::home_dir()
            .map(|h| h.join(".config").join("vortex"))
            .unwrap_or_else(|| PathBuf::from(".vortex"))
    }
}

pub fn workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
