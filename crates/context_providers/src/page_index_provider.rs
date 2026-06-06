use async_trait::async_trait;

use crate::{
    ContextBlock, ContextHit, ContextNodeId, ContextProvider, ContextQuery, ContextSummary,
    ProviderKind,
};

const STUB_MESSAGE: &str = "PageIndex not configured / network disabled. Set PAGEINDEX_API_KEY and pass network_allowed=true.";

/// Optional document/PDF provider (stub until MCP/API integration).
pub struct PageIndexProvider;

impl PageIndexProvider {
    pub fn new() -> Self {
        Self
    }

    fn api_key_configured() -> bool {
        std::env::var("PAGEINDEX_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .is_some()
    }

    fn enabled(query: &ContextQuery) -> bool {
        query.network_allowed && Self::api_key_configured()
    }
}

#[async_trait]
impl ContextProvider for PageIndexProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::PageIndex
    }

    async fn search(&self, query: ContextQuery) -> Result<Vec<ContextHit>, String> {
        if Self::enabled(&query) {
            // TODO: MCP/API integration for hosted PageIndex search.
            return Ok(Vec::new());
        }
        Err(STUB_MESSAGE.to_string())
    }

    async fn open(&self, id: ContextNodeId) -> Result<ContextBlock, String> {
        let _ = id;
        Err(STUB_MESSAGE.to_string())
    }

    async fn summarize(&self, id: ContextNodeId) -> Result<ContextSummary, String> {
        let _ = id;
        Err(STUB_MESSAGE.to_string())
    }
}
