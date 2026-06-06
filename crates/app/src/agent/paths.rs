use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct GitBranchInfo {
    pub current: String,
    pub branches: Vec<String>,
}

/// Expand a leading `~` and canonicalize to an absolute path suitable for persistence.
pub fn canonical_project_path(raw: &str) -> Result<PathBuf, String> {
    let expanded = expand_home(raw);
    expanded
        .canonicalize()
        .map_err(|e| format!("invalid project path {}: {e}", expanded.display()))
}

pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        dirs::home_dir()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| PathBuf::from(path))
    } else if path == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    }
}

/// Current git branch for a project directory, or `"main"` when not a repo.
pub fn git_head_branch(project_root: &Path) -> String {
    Command::new("git")
        .args([
            "-C",
            &project_root.display().to_string(),
            "rev-parse",
            "--abbrev-ref",
            "HEAD",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD")
        .unwrap_or_else(|| "main".into())
}

/// Current branch plus local branch names for a project directory.
pub fn git_branch_info(project_root: &Path) -> GitBranchInfo {
    let current = git_head_branch(project_root);
    let mut branches = git_local_branches_without_head_fallback(project_root);

    if branches.is_empty() {
        branches.push(current.clone());
    } else if !branches.iter().any(|branch| branch == &current) {
        branches.insert(0, current.clone());
    }

    GitBranchInfo { current, branches }
}

/// Local branch names for the composer branch picker.
pub fn git_local_branches(project_root: &Path) -> Vec<String> {
    git_branch_info(project_root).branches
}

fn git_local_branches_without_head_fallback(project_root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args([
            "-C",
            &project_root.display().to_string(),
            "branch",
            "--format=%(refname:short)",
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    }
}
