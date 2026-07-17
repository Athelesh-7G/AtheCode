//! Outbound events emitted by the sampler.
//!
//! These types now live in `xai-grok-sampling-types` so the shared
//! `SamplingBackend` trait can reference [`SamplingEvent`] directly.
//! They are re-exported here to preserve the `crate::events::*` paths
//! used throughout this crate.

pub use xai_grok_sampling_types::{
    SamplingChannel, SamplingErrorInfo, SamplingErrorKind, SamplingEvent,
};
