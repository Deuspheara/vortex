use std::collections::HashSet;

use crate::{
    ContextBlock, ContextHit, ContextNodeId, ContextProvider, ContextQuery, ContextSummary,
    ProviderKind,
};

/// Fans out search across registered providers and merges ranked hits.
pub struct ContextService {
    providers: Vec<Box<dyn ContextProvider>>,
}

impl ContextService {
    pub fn new(providers: Vec<Box<dyn ContextProvider>>) -> Self {
        Self { providers }
    }

    pub fn provider_kinds(&self) -> Vec<ProviderKind> {
        self.providers.iter().map(|p| p.kind()).collect()
    }

    /// Search all providers; PageIndex errors are ignored when network is off.
    pub async fn search(&self, query: ContextQuery) -> Result<Vec<ContextHit>, String> {
        let mut merged: Vec<ContextHit> = Vec::new();
        for provider in &self.providers {
            match provider.search(query.clone()).await {
                Ok(hits) => merged.extend(hits),
                Err(err) if provider.kind() == ProviderKind::PageIndex => {
                    let _ = err;
                }
                Err(err) => return Err(err),
            }
        }
        merged.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        merged.truncate(query.limit.max(1));
        Ok(merged)
    }

    /// Open a node by id, routing to the provider implied by the id prefix.
    pub async fn open(&self, id: ContextNodeId) -> Result<ContextBlock, String> {
        let provider = self
            .provider_for_id(&id.0)
            .ok_or_else(|| format!("no provider for id {}", id.0))?;
        provider.open(id).await
    }

    pub async fn summarize(&self, id: ContextNodeId) -> Result<ContextSummary, String> {
        let provider = self
            .provider_for_id(&id.0)
            .ok_or_else(|| format!("no provider for id {}", id.0))?;
        provider.summarize(id).await
    }

    /// Open many nodes, skipping duplicates and failures.
    pub async fn open_many(&self, ids: &[ContextNodeId]) -> Vec<ContextBlock> {
        let mut seen = HashSet::new();
        let mut blocks = Vec::new();
        for id in ids {
            if !seen.insert(id.0.clone()) {
                continue;
            }
            if let Ok(block) = self.open(id.clone()).await {
                blocks.push(block);
            }
        }
        blocks
    }

    fn provider_for_id(&self, id: &str) -> Option<&dyn ContextProvider> {
        let kind = if id.starts_with("git:") {
            ProviderKind::Git
        } else if id.starts_with("page:") {
            ProviderKind::PageIndex
        } else {
            ProviderKind::RepoIndex
        };
        self.providers
            .iter()
            .find(|p| p.kind() == kind)
            .map(|p| p.as_ref())
    }
}
