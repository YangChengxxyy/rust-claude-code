//! The `ModelClient` trait for abstracting API access.
//!
//! This trait is placed in the `api` crate so that both `cli` (QueryLoop)
//! and `tools` (AgentTool) can reference it without circular dependencies.

use async_trait::async_trait;

use crate::error::ApiError;
use crate::streaming::MessageStream;
use crate::types::{CreateMessageRequest, CreateMessageResponse};

/// Trait abstracting the Anthropic API client for testability and
/// type-erasure (e.g. `Arc<dyn ModelClient>` in AgentContext).
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ApiError>;

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<MessageStream, ApiError>;
}

/// Blanket impl for references to a ModelClient.
#[async_trait]
impl<C: ModelClient> ModelClient for &C {
    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ApiError> {
        (*self).create_message(request).await
    }

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<MessageStream, ApiError> {
        (*self).create_message_stream(request).await
    }
}

/// Blanket impl for Arc<dyn ModelClient>.
#[async_trait]
impl ModelClient for std::sync::Arc<dyn ModelClient> {
    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ApiError> {
        (**self).create_message(request).await
    }

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<MessageStream, ApiError> {
        (**self).create_message_stream(request).await
    }
}

/// Impl for the real Anthropic client.
#[async_trait]
impl ModelClient for crate::AnthropicClient {
    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse, ApiError> {
        crate::AnthropicClient::create_message(self, request).await
    }

    async fn create_message_stream(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<MessageStream, ApiError> {
        crate::AnthropicClient::create_message_stream(self, request).await
    }
}
