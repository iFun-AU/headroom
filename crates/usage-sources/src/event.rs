use usage_core::{ConnectionStatus, DailyBuckets, Provider, Reading, SourceKind, TokenEvent};

/// A normalized event emitted by an asynchronous provider source.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceEvent {
    /// Drops measurements when the source's configured profile changes.
    ClearSource {
        /// Provider whose source is being replaced.
        provider: Provider,
        /// Exact source whose prior profile must no longer contribute readings.
        source: SourceKind,
    },
    /// A full or sparse provider limit reading.
    Reading(Reading),
    /// One or more local token-history events.
    Tokens(Vec<TokenEvent>),
    /// A complete provider-reported daily account update.
    Daily(DailyBuckets),
    /// A source-specific health transition that preserves last-good data.
    Status {
        /// Provider whose source changed health.
        provider: Provider,
        /// Source whose health changed.
        source: SourceKind,
        /// New source-local status.
        status: ConnectionStatus,
    },
    /// Provider activity used by the adaptive scheduler.
    Activity {
        /// Provider that emitted activity.
        provider: Provider,
    },
}
