use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{PatchId, RiskLevel, RunId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchProposal {
    pub id: PatchId,
    pub run_id: RunId,
    pub base_git_sha: Option<String>,
    pub files: Vec<PatchFile>,
    pub unified_diff: String,
    pub summary: String,
    pub risk: RiskLevel,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchFile {
    pub path: PathBuf,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileCheckpoint {
    pub path: PathBuf,
    pub old_hash: String,
    pub old_content_path: PathBuf,
    pub new_hash: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceCheckpoint {
    pub id: crate::CheckpointId,
    pub project_id: crate::ProjectId,
    pub run_id: RunId,
    pub git_head: Option<String>,
    pub dirty_files_before: Vec<PathBuf>,
    pub file_snapshots: Vec<FileCheckpoint>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalRule {
    pub project_id: crate::ProjectId,
    pub tool_name: crate::ToolName,
    pub command_pattern: Option<String>,
    pub path_prefix: Option<PathBuf>,
    pub max_risk: RiskLevel,
    pub expires_at: Option<DateTime<Utc>>,
}
