//! Provider-agnostic sampling backend trait.
//!
//! [`SamplingBackend`] is the seam that lets multiple LLM providers
//! (xAI's `SamplingClient`, the Bedrock `BedrockClient`, …) be selected
//! at runtime behind a single interface. It is defined here, in the
//! shared types crate, so that both provider implementations can depend
//! on it without depending on each other.
//!
//! The trait speaks only in the provider-agnostic
//! [`ConversationRequest`] / [`ConversationResponse`] representation and
//! the shared [`SamplingEvent`] stream, so callers never need to know
//! which backend is behind the `dyn SamplingBackend`.

use async_trait::async_trait;
use futures_util::stream::BoxStream;

use crate::events::SamplingEvent;
use crate::{ConversationRequest, ConversationResponse, SamplingError};

/// A pluggable LLM inference backend.
///
/// Implementations translate the provider-agnostic
/// [`ConversationRequest`] into their own wire format, perform the
/// request, and translate the result back into a
/// [`ConversationResponse`] (non-streaming) or a stream of
/// [`SamplingEvent`]s (streaming).
///
/// Must be `Send + Sync` so a `dyn SamplingBackend` can be shared across
/// tasks and stored behind an `Arc`.
#[async_trait]
pub trait SamplingBackend: Send + Sync {
    /// Perform a single non-streaming completion.
    async fn chat_completion(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, SamplingError>;

    /// Perform a streaming completion, yielding [`SamplingEvent`]s as the
    /// response is produced.
    async fn chat_completion_stream(
        &self,
        request: ConversationRequest,
    ) -> Result<BoxStream<'static, SamplingEvent>, SamplingError>;
}
