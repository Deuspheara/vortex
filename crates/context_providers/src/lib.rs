mod git_provider;
mod page_index_provider;
mod repo_index_provider;
mod service;

pub use git_provider::GitProvider;
pub use page_index_provider::PageIndexProvider;
pub use repo_index_provider::RepoIndexProvider;
pub use service::ContextService;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Which backend produced a context hit or block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    RepoIndex,
    Git,
    PageIndex,
}

/// Stable id for a node in a provider namespace (`repo:`, `git:`, `page:` prefixes).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextNodeId(pub String);

impl ContextNodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Search request shared across providers.
#[derive(Clone, Debug)]
pub struct ContextQuery {
    pub text: String,
    pub limit: usize,
    /// When false, [`PageIndexProvider`] returns no hits (network disabled by default).
    pub network_allowed: bool,
}

impl ContextQuery {
    pub fn new(text: impl Into<String>, limit: usize) -> Self {
        Self {
            text: text.into(),
            limit,
            network_allowed: false,
        }
    }
}

/// A ranked search hit from a provider (scores are internal).
#[derive(Clone, Debug)]
pub struct ContextHit {
    pub id: ContextNodeId,
    pub provider: ProviderKind,
    pub label: String,
    pub detail: Option<String>,
    pub score: f64,
}

/// Compact model-facing content for an opened node.
#[derive(Clone, Debug)]
pub struct ContextBlock {
    pub id: ContextNodeId,
    pub provider: ProviderKind,
    pub content: String,
}

/// Short summary for a node (from cache or heuristics).
#[derive(Clone, Debug)]
pub struct ContextSummary {
    pub id: ContextNodeId,
    pub provider: ProviderKind,
    pub summary: String,
}

/// Pluggable context source (repo index, git, optional PageIndex).
#[async_trait]
pub trait ContextProvider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    async fn search(&self, query: ContextQuery) -> Result<Vec<ContextHit>, String>;

    async fn open(&self, id: ContextNodeId) -> Result<ContextBlock, String>;

    async fn summarize(&self, id: ContextNodeId) -> Result<ContextSummary, String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct FakeProvider {
        hits: Vec<ContextHit>,
        block: String,
        summary: String,
    }

    #[async_trait]
    impl ContextProvider for FakeProvider {
        fn kind(&self) -> ProviderKind {
            ProviderKind::RepoIndex
        }

        async fn search(&self, query: ContextQuery) -> Result<Vec<ContextHit>, String> {
            let limit = query.limit.max(1);
            Ok(self.hits.iter().take(limit).cloned().collect())
        }

        async fn open(&self, id: ContextNodeId) -> Result<ContextBlock, String> {
            Ok(ContextBlock {
                id,
                provider: ProviderKind::RepoIndex,
                content: self.block.clone(),
            })
        }

        async fn summarize(&self, id: ContextNodeId) -> Result<ContextSummary, String> {
            Ok(ContextSummary {
                id,
                provider: ProviderKind::RepoIndex,
                summary: self.summary.clone(),
            })
        }
    }

    #[tokio::test]
    async fn context_service_merges_fake_provider_hits() {
        let hit = ContextHit {
            id: ContextNodeId::new("repo:fake.rs"),
            provider: ProviderKind::RepoIndex,
            label: "fake.rs".into(),
            detail: None,
            score: 1.0,
        };
        let provider = FakeProvider {
            hits: vec![hit.clone()],
            block: "<file_slice>fake</file_slice>".into(),
            summary: "stub summary".into(),
        };
        let service = ContextService::new(vec![Box::new(provider)]);
        let merged = service
            .search(ContextQuery::new("fake", 5))
            .await
            .expect("search");
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].label, "fake.rs");

        let block = service.open(hit.id.clone()).await.expect("open");
        assert!(block.content.contains("file_slice"));

        let summary = service.summarize(hit.id).await.expect("summarize");
        assert_eq!(summary.summary, "stub summary");
    }
}
