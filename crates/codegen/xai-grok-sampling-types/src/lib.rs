//! Pure data types for the xAI sampling / chat-completion API layer.
//!
//! This crate contains the API-agnostic conversation types, chat completion
//! request/response types, streaming types, and error types used across the
//! xAI agent stack.  It intentionally contains **no I/O** (no HTTP clients,
//! no file system access) so it can be depended on by downstream crates
//! (e.g., `xai-chat-state`) without pulling in the full `xai-grok-shell`.

pub mod backend;
pub mod conversation;
pub mod doom_loop;
pub mod error;
pub mod events;
pub mod messages;
pub mod metrics;
pub mod request_id;
pub mod serde_helpers;
pub mod types;

pub use self::backend::SamplingBackend;
pub use self::conversation::*;
pub use self::events::{SamplingChannel, SamplingErrorInfo, SamplingErrorKind, SamplingEvent};
pub use self::metrics::{InferenceLatencyStats, compute_percentiles};
pub use self::request_id::RequestId;
pub use self::doom_loop::{
    DOOM_LOOP_CHECK_EVENT_TYPE, DOOM_LOOP_CHECK_HEADER, DoomLoopPeek, DoomLoopRecoveryPolicy,
    DoomLoopSignal, DoomLoopSignalKind, is_check_event, peek_doom_loop,
};
pub use self::error::{
    EmptyReason, EmptyResponseContext, ResponseModelMetadata, Result, SamplingError,
    is_context_length_error,
};
pub use self::types::*;

// Re-export async-openai crate Responses API types under `rs` namespace
pub use async_openai::types::responses as rs;
