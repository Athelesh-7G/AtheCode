//! Per-response inference latency metrics.
//!
//! These types now live in `xai-grok-sampling-types` (so the shared
//! `SamplingBackend` trait and event types can reference them without a
//! dependency cycle). They are re-exported here to preserve the
//! `crate::metrics::*` paths used throughout this crate.

pub use xai_grok_sampling_types::{InferenceLatencyStats, compute_percentiles};
