use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    AgentMode, ApprovalId, CheckpointId, EventId, ModelId, PatchId, ProjectId, RunId, SessionId,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AgentCommand {
    StartRun {
        project_id: ProjectId,
        session_id: SessionId,
        prompt: String,
        model: ModelId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent_model: Option<ModelId>,
        mode: AgentMode,
        attachments: Vec<ContextAttachment>,
    },
    CancelRun {
        run_id: RunId,
    },
    ApproveTool {
        approval_id: ApprovalId,
    },
    ApproveToolAlways {
        approval_id: ApprovalId,
    },
    SubmitChoice {
        choice_id: String,
        option_id: String,
    },
    RejectTool {
        approval_id: ApprovalId,
        reason: Option<String>,
    },
    ApprovePatch {
        patch_id: PatchId,
        scope: crate::PatchApprovalScope,
    },
    RejectPatch {
        patch_id: PatchId,
        reason: Option<String>,
    },
    CompactSession {
        session_id: SessionId,
    },
    RetryFromEvent {
        session_id: SessionId,
        event_id: EventId,
    },
    SwitchModel {
        session_id: SessionId,
        model: ModelId,
    },
    RollbackCheckpoint {
        checkpoint_id: CheckpointId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextAttachment {
    pub source: AttachmentSource,
    pub kind: AttachmentKind,
    pub mime_type: Option<String>,
    pub display_name: Option<String>,
    pub size_bytes: Option<u64>,
}

impl ContextAttachment {
    pub fn file(path: PathBuf) -> Self {
        Self {
            source: AttachmentSource::Path(path),
            kind: AttachmentKind::File,
            mime_type: None,
            display_name: None,
            size_bytes: None,
        }
    }

    pub fn image_file(path: PathBuf, mime_type: impl Into<String>, size_bytes: u64) -> Self {
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string());
        Self {
            source: AttachmentSource::Path(path),
            kind: AttachmentKind::Image,
            mime_type: Some(mime_type.into()),
            display_name,
            size_bytes: Some(size_bytes),
        }
    }

    pub fn image_bytes(
        bytes: Vec<u8>,
        mime_type: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let size_bytes = bytes.len() as u64;
        Self {
            source: AttachmentSource::Bytes(bytes),
            kind: AttachmentKind::Image,
            mime_type: Some(mime_type.into()),
            display_name: Some(display_name.into()),
            size_bytes: Some(size_bytes),
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match &self.source {
            AttachmentSource::Path(path) => Some(path),
            AttachmentSource::Bytes(_) => None,
        }
    }

    pub fn display_label(&self) -> String {
        self.display_name
            .clone()
            .or_else(|| {
                self.path()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
            })
            .unwrap_or_else(|| "attachment".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AttachmentSource {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttachmentKind {
    File,
    Image,
    Selection,
    Diagnostic,
}
