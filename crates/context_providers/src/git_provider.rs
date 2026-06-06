use std::path::PathBuf;

use async_trait::async_trait;
use project_index::git_changed_files;

use crate::{
    ContextBlock, ContextHit, ContextNodeId, ContextProvider, ContextQuery, ContextSummary,
    ProviderKind,
};

const ID_PREFIX: &str = "git:";

/// Surfaces git working-tree changes as searchable context hits.
pub struct GitProvider {
    root: PathBuf,
}

impl GitProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn strip_prefix(id: &str) -> &str {
        id.strip_prefix(ID_PREFIX).unwrap_or(id)
    }

    fn wrap_id(path: &str) -> ContextNodeId {
        ContextNodeId::new(format!("{ID_PREFIX}{path}"))
    }

    fn changed_paths(&self) -> Vec<String> {
        git_changed_files(&self.root)
            .into_iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn git_diff_snippet(&self, path: &str) -> String {
        let mut args = vec!["diff", "-U3", "--"];
        args.push(path);
        let output = std::process::Command::new("git")
            .args(&args)
            .current_dir(&self.root)
            .output();
        let Ok(output) = output else {
            return format!("(git diff unavailable for {path})");
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let stderr = stderr.trim();
            return if stderr.is_empty() {
                format!("(git diff failed for {path})")
            } else {
                format!("(git diff error: {stderr})")
            };
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            format!("(no diff for {path})")
        } else {
            trimmed.chars().take(4000).collect()
        }
    }

    fn git_status_line(&self, path: &str) -> String {
        let output = std::process::Command::new("git")
            .args(["status", "--short", "--", path])
            .current_dir(&self.root)
            .output();
        let Ok(output) = output else {
            return format!("? {path}");
        };
        let line = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if line.is_empty() {
            format!("? {path}")
        } else {
            line
        }
    }
}

#[async_trait]
impl ContextProvider for GitProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Git
    }

    async fn search(&self, query: ContextQuery) -> Result<Vec<ContextHit>, String> {
        let q = query.text.trim().to_lowercase();
        let limit = query.limit.max(1);
        let mut hits: Vec<ContextHit> = self
            .changed_paths()
            .into_iter()
            .filter(|path| {
                if q.is_empty() {
                    true
                } else {
                    path.to_lowercase().contains(&q)
                }
            })
            .map(|path| {
                let score = if q.is_empty() {
                    0.5
                } else if path.to_lowercase() == q {
                    1.0
                } else if path.to_lowercase().contains(&q) {
                    0.8
                } else {
                    0.5
                };
                ContextHit {
                    id: Self::wrap_id(&path),
                    provider: ProviderKind::Git,
                    label: path.clone(),
                    detail: Some("changed in git working tree".into()),
                    score,
                }
            })
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(limit);
        Ok(hits)
    }

    async fn open(&self, id: ContextNodeId) -> Result<ContextBlock, String> {
        let path = Self::strip_prefix(&id.0);
        let status = self.git_status_line(path);
        let diff = self.git_diff_snippet(path);
        let content = format!(
            "<git_context path=\"{path}\">\nstatus: {status}\n<diff>\n{diff}\n</diff>\n</git_context>\n"
        );
        Ok(ContextBlock {
            id,
            provider: ProviderKind::Git,
            content,
        })
    }

    async fn summarize(&self, id: ContextNodeId) -> Result<ContextSummary, String> {
        let path = Self::strip_prefix(&id.0);
        let status = self.git_status_line(path);
        Ok(ContextSummary {
            id,
            provider: ProviderKind::Git,
            summary: format!("Git change: {status}"),
        })
    }
}
