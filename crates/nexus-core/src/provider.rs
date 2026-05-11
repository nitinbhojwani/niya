//! Provider adapter trait.
//!
//! Every LLM backend (Anthropic, OpenAI, Ollama, etc.) implements this trait.
//! Adapters are thin: they translate between the canonical Nexus message format
//! and the provider's API, manage the HTTP connection, and stream responses back.

use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::types::{ChatRequest, ChatResponseChunk, ProviderError};

/// A provider adapter that communicates with an LLM backend.
///
/// Implementations must be `Send + Sync` so they can be shared across async tasks.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Human-readable name for this provider (e.g., "openai", "anthropic", "ollama").
    fn name(&self) -> &str;

    /// Validate that the provider is reachable and credentials are valid.
    ///
    /// Called once at session startup. Should make a lightweight API call
    /// (e.g., list models) to verify connectivity and authentication.
    async fn validate(&self) -> Result<(), ProviderError>;

    /// Send a chat request and return a stream of response chunks.
    ///
    /// The stream yields `ChatResponseChunk` variants as they arrive:
    /// text deltas, tool call blocks, usage info, and a final `Done`.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'_, ChatResponseChunk>, ProviderError>;

    /// Return the context window size (in tokens) for the configured model.
    fn context_window_size(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    /// A mock provider for testing the orchestrator.
    pub struct MockProvider {
        pub name: String,
        pub context_window: usize,
        pub responses: std::sync::Mutex<Vec<Vec<ChatResponseChunk>>>,
    }

    impl MockProvider {
        pub fn new(responses: Vec<Vec<ChatResponseChunk>>) -> Self {
            Self {
                name: "mock".to_string(),
                context_window: 200_000,
                responses: std::sync::Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl ProviderAdapter for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        async fn validate(&self) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn chat(
            &self,
            _request: ChatRequest,
        ) -> Result<BoxStream<'_, ChatResponseChunk>, ProviderError> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Ok(Box::pin(futures::stream::once(async {
                    ChatResponseChunk::Done
                })));
            }
            let chunks = responses.remove(0);
            Ok(Box::pin(futures::stream::iter(chunks)))
        }

        fn context_window_size(&self) -> usize {
            self.context_window
        }
    }

    #[test]
    fn mock_provider_implements_trait() {
        let provider = MockProvider::new(vec![]);
        assert_eq!(provider.name(), "mock");
        assert_eq!(provider.context_window_size(), 200_000);
    }
}
