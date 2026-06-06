use std::time::Duration;

use agent_protocol::{AgentError, CancellationToken, ModelDelta, ModelRequest, ModelUsage};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use tokio::time::sleep;

use crate::{ModelProvider, ModelStream};

pub struct MockProvider {
    pub response: String,
    pub chunk_size: usize,
    pub delay_ms: u64,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self {
            response: "Hello from the Vortex mock agent runtime.\n\nThis is a streamed response from the event-sourced agent core.".into(),
            chunk_size: 8,
            delay_ms: 30,
        }
    }
}

#[async_trait]
impl ModelProvider for MockProvider {
    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, AgentError> {
        let _ = request;
        let response = self.response.clone();
        let chunk_size = self.chunk_size.max(1);
        let delay_ms = self.delay_ms;

        let mut chunks = Vec::new();
        let mut offset = 0usize;
        while offset < response.len() {
            if cancel.is_cancelled() {
                return Err(AgentError::Cancelled);
            }
            let end = (offset + chunk_size).min(response.len());
            chunks.push(Ok(ModelDelta::Text(response[offset..end].to_string())));
            offset = end;
        }
        chunks.push(Ok(ModelDelta::Usage(ModelUsage {
            input_tokens: 42,
            output_tokens: (response.len() / 4) as u64,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd: None,
        })));
        chunks.push(Ok(ModelDelta::Done));

        let stream = stream::iter(chunks).then(move |item| async move {
            sleep(Duration::from_millis(delay_ms)).await;
            item
        });

        Ok(Box::pin(stream))
    }
}
