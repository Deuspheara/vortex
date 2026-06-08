use std::pin::Pin;

use agent_protocol::{
    AgentError, CancellationToken, ModelDelta, ModelProviderCapabilities, ModelRequest,
};
use async_trait::async_trait;
use futures::Stream;

pub type ModelStream = Pin<Box<dyn Stream<Item = Result<ModelDelta, AgentError>> + Send>>;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn capabilities(&self) -> ModelProviderCapabilities {
        ModelProviderCapabilities::default()
    }

    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, AgentError>;
}
