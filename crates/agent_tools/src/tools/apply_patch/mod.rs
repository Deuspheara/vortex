use std::path::PathBuf;

use agent_protocol::{
    CheckpointId, FileCheckpoint, IconToken, NetworkAccess, RiskLevel, ToolAssessment,
    ToolCapabilities, ToolCategory, ToolContext, ToolModeGate, ToolPack, ToolPackPolicy,
    ToolPolicy, ToolResult, WorkspaceCheckpoint,
};
use agent_sandbox::PathPolicy;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};

use crate::shared::{
    apply_unified_diff, current_git_head, normalize_unified_diff, parse_patch_files,
    patch_diff_from_args, patch_diff_preview, validate_patch_applicable,
};
use crate::tool::{AgentTool, default_finish_summary};

pub struct ApplyPatchTool {
    pub checkpoint_dir: PathBuf,
}

#[async_trait]
impl AgentTool for ApplyPatchTool {
    fn name(&self) -> &'static str {
        "apply_patch"
    }

    fn description(&self) -> &'static str {
        "Apply a previously proposed unified diff patch"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unified_diff": { "type": "string" }
            },
            "required": ["unified_diff"]
        })
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            category: ToolCategory::PatchApply,
            ..ToolCapabilities::default()
        }
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ApplyPatches,
            pack_policy: ToolPackPolicy::Only(vec![
                ToolPack::Dependency,
                ToolPack::CodeEdit,
                ToolPack::UiBrowser,
                ToolPack::GitCi,
                ToolPack::General,
            ]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Pencil
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Applying patch".into()
        } else {
            "Applied patch".into()
        }
    }

    fn finish_summary(&self, _args: &Value, _output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), _output, true);
        }
        "Applied patch".into()
    }

    fn args_preview(&self, args: &Value) -> String {
        if let Ok(diff) = patch_diff_from_args(args) {
            return patch_diff_preview(&diff);
        }
        String::new()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        if !ctx.mode.can_apply_patches() {
            return Ok(ToolAssessment {
                risk: RiskLevel::Critical,
                requires_approval: false,
                reason: "patch apply is not allowed in current mode".into(),
                affected_paths: vec![],
                network_access: NetworkAccess::Disabled,
                writes_to_disk: false,
                runs_real_process: false,
                denied: true,
            });
        }
        let diff = patch_diff_from_args(args).unwrap_or_default();
        let files = parse_patch_files(&diff, &ctx.project_root)?;
        Ok(ToolAssessment {
            risk: RiskLevel::Medium,
            requires_approval: true,
            reason: "writes files on disk".into(),
            affected_paths: files.iter().map(|f| f.path.clone()).collect(),
            network_access: NetworkAccess::Disabled,
            writes_to_disk: true,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let diff = patch_diff_from_args(&args)?;
        let normalized = normalize_unified_diff(&diff);
        let files = parse_patch_files(&normalized, &ctx.project_root)?;
        let policy = PathPolicy::new(&ctx.project_root);

        std::fs::create_dir_all(&self.checkpoint_dir).map_err(|e| e.to_string())?;
        validate_patch_applicable(&ctx.project_root, &normalized)?;

        let checkpoint_id = CheckpointId::new(uuid_simple());
        let mut snapshots = Vec::new();

        for file in &files {
            let resolved = policy.validate_write(&file.path)?;
            if resolved.exists() {
                let content = std::fs::read(&resolved).map_err(|e| e.to_string())?;
                let hash = blake3::hash(&content).to_hex().to_string();
                let backup_path = self.checkpoint_dir.join(format!(
                    "{}-{}",
                    checkpoint_id.0,
                    file.path.display().to_string().replace('/', "_")
                ));
                std::fs::copy(&resolved, &backup_path).map_err(|e| e.to_string())?;
                snapshots.push(FileCheckpoint {
                    path: file.path.clone(),
                    old_hash: hash,
                    old_content_path: backup_path,
                    new_hash: None,
                });
            }
        }

        apply_unified_diff(
            &ctx.project_root,
            &normalized,
            &self.checkpoint_dir,
            ctx.output_sink.as_ref(),
        )?;

        let checkpoint = WorkspaceCheckpoint {
            id: checkpoint_id.clone(),
            project_id: ctx.project_id.clone(),
            run_id: ctx.run_id.clone(),
            git_head: current_git_head(&ctx.project_root),
            dirty_files_before: files.iter().map(|f| f.path.clone()).collect(),
            file_snapshots: snapshots.clone(),
            created_at: Utc::now(),
        };

        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output: serde_json::json!({
                "checkpoint_id": checkpoint.id.0,
                "checkpoint": checkpoint,
                "patch_id": checkpoint_id.0,
                "files": files.iter().map(|f| f.path.display().to_string()).collect::<Vec<_>>(),
            })
            .to_string(),
            is_error: false,
        })
    }
}

fn uuid_simple() -> String {
    uuid::Uuid::new_v4().to_string()
}
