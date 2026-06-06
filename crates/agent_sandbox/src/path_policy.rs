use std::path::{Path, PathBuf};

use agent_protocol::{ApprovalDecision, RiskLevel};

pub struct PathPolicy {
    pub project_root: PathBuf,
}

impl PathPolicy {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    pub fn validate_read(&self, path: &Path) -> Result<PathBuf, String> {
        self.canonicalize_within_root(path)
    }

    pub fn validate_write(&self, path: &Path) -> Result<PathBuf, String> {
        let root = self
            .project_root
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };

        let resolved = if joined.exists() {
            joined.canonicalize().map_err(|e| e.to_string())?
        } else {
            let parent = joined.parent().ok_or_else(|| "invalid path".to_string())?;
            let parent_canonical = if parent.exists() {
                parent.canonicalize().map_err(|e| e.to_string())?
            } else {
                self.canonicalize_existing_ancestor(parent)?
            };
            if !parent_canonical.starts_with(&root) {
                return Err(format!("path `{}` escapes project root", path.display()));
            }
            joined
        };

        if !resolved.starts_with(&root) {
            return Err(format!("path `{}` escapes project root", path.display()));
        }
        if is_protected_file(&resolved) {
            return Err(format!(
                "edits to protected file `{}` are blocked by default",
                resolved.display()
            ));
        }
        Ok(resolved)
    }

    fn canonicalize_within_root(&self, path: &Path) -> Result<PathBuf, String> {
        let root = self
            .project_root
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let joined = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        let canonical = joined.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(&root) {
            return Err(format!("path `{}` escapes project root", path.display()));
        }
        Ok(canonical)
    }

    fn canonicalize_existing_ancestor(&self, path: &Path) -> Result<PathBuf, String> {
        let root = self
            .project_root
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let mut current = path.to_path_buf();
        loop {
            if current.exists() {
                let canonical = current.canonicalize().map_err(|e| e.to_string())?;
                if !canonical.starts_with(&root) {
                    return Err(format!("path `{}` escapes project root", path.display()));
                }
                return Ok(canonical);
            }
            current = current
                .parent()
                .ok_or_else(|| format!("path `{}` is outside project root", path.display()))?
                .to_path_buf();
        }
    }
}

pub fn is_protected_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if file_name == ".env" || file_name.starts_with(".env.") {
        return true;
    }
    if file_name == "id_rsa" || file_name == "id_ed25519" {
        return true;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("pem" | "key" | "p12" | "pfx")
    ) || file_name == ".npmrc"
        || file_name == ".pypirc"
}

pub fn classify_tool_name(name: &str) -> ApprovalDecision {
    match name {
        "read_file" | "list_files" | "search_project" | "git_status" | "git_diff" => {
            ApprovalDecision::Allow
        }
        "bash_virtual" => ApprovalDecision::Allow,
        "propose_patch" => ApprovalDecision::Allow,
        "apply_patch" => ApprovalDecision::AskUser {
            risk: RiskLevel::Medium,
            reason: "This will modify files in the project".into(),
        },
        "run_real_command" => ApprovalDecision::AskUser {
            risk: RiskLevel::Medium,
            reason: "This command executes code on your machine".into(),
        },
        "delegate" => ApprovalDecision::AskUser {
            risk: RiskLevel::Medium,
            reason: "This delegates work to a nested subagent run".into(),
        },
        "delete_file" => ApprovalDecision::AskUser {
            risk: RiskLevel::Critical,
            reason: "This deletes files".into(),
        },
        _ => ApprovalDecision::Deny {
            reason: format!("Unknown tool: {name}"),
        },
    }
}
