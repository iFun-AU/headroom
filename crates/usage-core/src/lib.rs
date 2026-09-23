//! Pure domain logic for How Is It usage data.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod alerts;
mod domain;
mod history;
mod history_time;
mod merge;
mod projection;
mod window;

/// Lenient external DTO parsers that produce strict domain values.
pub mod parse;

pub use alerts::{Alert, AlertKind, AlertTracker};
pub use domain::{
    Bucket, ConnectionStatus, History, HistoryScope, LimitWindow, Percent, PercentError,
    Projection, Provider, ProviderUsage, Series, SourceHealth, SourceKind, TokenCount, UnixSeconds,
    UsageSnapshot, WindowKind, WindowMinutes, log_trunc,
};
pub use history::{DailyBuckets, HistoryMetrics, HistoryStore, TokenEvent};
pub use merge::{
    Reading, SourceState, State, derive_provider_usage, derive_snapshot, ingest_reading,
    ingest_status,
};
pub use projection::project_weekly;
pub use window::classify;
