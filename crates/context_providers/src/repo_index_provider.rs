use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use project_index::RepoIndex;

use crate::{
    ContextBlock, ContextHit, ContextNodeId, ContextProvider, ContextQuery, ContextSummary,
    ProviderKind,
};

const ID_PREFIX: &str = "repo:";

/// Wraps a shared [`RepoIndex`] for search, open, and cached summaries.
pub struct RepoIndexProvider {
    index: Arc<Mutex<RepoIndex>>,
}

impl RepoIndexProvider {
    pub fn new(index: Arc<Mutex<RepoIndex>>) -> Self {
        Self { index }
    }

    fn strip_prefix(id: &str) -> &str {
        id.strip_prefix(ID_PREFIX).unwrap_or(id)
    }

    fn wrap_id(raw: &str) -> ContextNodeId {
        if raw.starts_with(ID_PREFIX) {
            ContextNodeId::new(raw)
        } else {
            ContextNodeId::new(format!("{ID_PREFIX}{raw}"))
        }
    }

    fn lock_index(&self) -> Result<std::sync::MutexGuard<'_, RepoIndex>, String> {
        self.index
            .lock()
            .map_err(|e| format!("repo index lock poisoned: {e}"))
    }
}

#[async_trait]
impl ContextProvider for RepoIndexProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::RepoIndex
    }

    async fn search(&self, query: ContextQuery) -> Result<Vec<ContextHit>, String> {
        let q = query.text.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let limit = query.limit.max(1);
        let hits = self.lock_index()?.rank(q, limit);
        Ok(hits
            .into_iter()
            .map(|h| {
                let detail = Some(format!("{}:{}-{}", h.kind, h.start_line, h.end_line));
                ContextHit {
                    id: Self::wrap_id(&h.symbol_id),
                    provider: ProviderKind::RepoIndex,
                    label: format!("{}:{}", h.path, h.name),
                    detail,
                    score: h.score,
                }
            })
            .collect())
    }

    async fn open(&self, id: ContextNodeId) -> Result<ContextBlock, String> {
        let raw = Self::strip_prefix(&id.0);
        let content = self.lock_index()?.open_node(raw)?;
        Ok(ContextBlock {
            id,
            provider: ProviderKind::RepoIndex,
            content,
        })
    }

    async fn summarize(&self, id: ContextNodeId) -> Result<ContextSummary, String> {
        let raw = Self::strip_prefix(&id.0);
        let summary = self.lock_index()?.summarize_node(raw)?;
        Ok(ContextSummary {
            id,
            provider: ProviderKind::RepoIndex,
            summary,
        })
    }
}
