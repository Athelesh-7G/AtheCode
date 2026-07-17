//! Core sampler types.
//!
//! [`RequestId`] now lives in `xai-grok-sampling-types` so it can be
//! referenced by the shared `SamplingBackend` trait and event types.
//! It is re-exported here to preserve the `crate::types::RequestId`
//! path used throughout this crate.

pub use xai_grok_sampling_types::RequestId;
