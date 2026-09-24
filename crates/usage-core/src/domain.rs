use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

const MAX_EXTERNAL_LOG_BYTES: usize = 2 * 1_024;

/// A usage provider supported by the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Provider {
    /// Anthropic Claude.
    Claude,
    /// `OpenAI` Codex.
    Codex,
}

/// Classified limit window. Classification uses duration rather than response position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum WindowKind {
    /// Approximately five hours (240 through 360 minutes).
    Session,
    /// Approximately seven days (9,000 through 11,000 minutes).
    Weekly,
    /// An unrecognized duration, retained so no provider data is silently dropped.
    Other {
        /// The reported duration in minutes.
        minutes: u32,
    },
}

/// An invalid percentage value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PercentError {
    /// The percentage was NaN or infinite.
    #[error("percentage must be finite")]
    NonFinite,
}

/// A finite usage percentage clamped to the inclusive range 0 through 100.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, TS)]
#[serde(transparent)]
#[ts(export, type = "number")]
pub struct Percent(f64);

impl Percent {
    /// Zero percent.
    pub const ZERO: Self = Self(0.0);

    /// Validates and constructs a percentage, clamping finite values into range.
    ///
    /// # Errors
    ///
    /// Returns [`PercentError::NonFinite`] for NaN and positive or negative infinity.
    pub fn new(value: f64) -> Result<Self, PercentError> {
        if !value.is_finite() {
            return Err(PercentError::NonFinite);
        }

        Ok(Self(value.clamp(0.0, 100.0)))
    }

    /// Returns the validated percentage value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Seconds since the Unix epoch in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export, type = "number")]
pub struct UnixSeconds(
    /// The timestamp value.
    pub i64,
);

/// A non-negative token count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(transparent)]
#[ts(export, type = "number")]
pub struct TokenCount(
    /// The number of tokens.
    pub u64,
);

/// A provider window duration measured in whole minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowMinutes(
    /// The duration value.
    pub u32,
);

/// A mechanism that provides usage information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceKind {
    /// Codex's local app-server RPC interface.
    CodexAppServer,
    /// Codex session rollout files on this Mac.
    CodexRollout,
    /// Claude Code's status-line integration.
    ClaudeStatusline,
    /// Claude's optional OAuth usage endpoint.
    ClaudeOAuth,
    /// Claude Code's local conversation logs.
    ClaudeLocalLogs,
}

impl SourceKind {
    /// Returns the explicit authority order for limit readings; lower is stronger.
    ///
    /// Claude local logs never produce limit readings and therefore sort last.
    #[must_use]
    pub const fn limit_priority(self) -> u8 {
        match self {
            Self::CodexAppServer | Self::ClaudeStatusline => 0,
            Self::CodexRollout | Self::ClaudeOAuth => 1,
            Self::ClaudeLocalLogs => u8::MAX,
        }
    }
}

/// The usage and reset state for one classified provider limit window.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LimitWindow {
    /// The classified duration.
    pub kind: WindowKind,
    /// The consumed percentage.
    pub used: Percent,
    /// The provider-reported reset time, when known.
    pub resets_at: Option<UnixSeconds>,
    /// Whether the reset has passed without a fresh provider reading.
    pub reset_pending: bool,
    /// The source that supplied the displayed values.
    pub source: SourceKind,
    /// When the source observed this window.
    pub observed_at: UnixSeconds,
}

/// An amount of money or provider credits, kept in exact minor units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreditAmount {
    /// The amount in minor units; the value is `minor / 10^exponent`.
    #[ts(type = "number")]
    pub minor: i64,
    /// Decimal places represented in `minor`.
    pub exponent: u8,
    /// ISO 4217 currency code such as `USD`, or `None` for provider credits.
    pub currency: Option<String>,
}

/// Paid usage beyond plan limits: Claude extra usage or Codex credits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Credits {
    /// Claude: extra usage is turned on. Codex: the account has credits.
    pub enabled: bool,
    /// Codex plans whose credits are unlimited.
    pub unlimited: bool,
    /// Amount spent in the current billing period (Claude).
    pub used: Option<CreditAmount>,
    /// Spending cap for the current billing period (Claude).
    pub limit: Option<CreditAmount>,
    /// Remaining prepaid balance (Codex).
    pub balance: Option<CreditAmount>,
    /// The source that reported these credits.
    pub source: SourceKind,
    /// When the source observed these credits.
    pub observed_at: UnixSeconds,
}

/// Connection and data-health state for a provider source.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum ConnectionStatus {
    /// The authoritative source is healthy and current.
    Connected,
    /// The newest available reading is older than 15 minutes.
    Stale,
    /// A fallback source works but has reduced capability.
    Degraded {
        /// A short user-facing explanation of the degraded mode.
        reason: String,
    },
    /// The source has not been configured.
    NotConfigured {
        /// A short action the user can take.
        hint: String,
    },
    /// Authentication needs user attention.
    AuthExpired {
        /// A short action the user can take.
        hint: String,
    },
    /// The source is reachable but does not offer the requested data.
    Unsupported {
        /// A short provider-facing explanation.
        reason: String,
    },
    /// The source failed unexpectedly.
    Error {
        /// A short actionable error message.
        message: String,
    },
}

/// Health information for one configured source.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceHealth {
    /// The source being described.
    pub source: SourceKind,
    /// The source's own status, independent of provider-level fallback logic.
    pub status: ConnectionStatus,
    /// The last successful reading time.
    pub last_success: Option<UnixSeconds>,
}

/// The complete derived usage view for one provider.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderUsage {
    /// The provider represented by this view.
    pub provider: Provider,
    /// The display plan, such as `Pro` or `Max`, when known.
    pub plan: Option<String>,
    /// Derived windows, sorted as session, weekly, then other duration ascending.
    pub windows: Vec<LimitWindow>,
    /// Provider-level health derived from source authority and freshness.
    pub status: ConnectionStatus,
    /// The source whose window set defines the derived view.
    pub authoritative_source: Option<SourceKind>,
    /// The authoritative source's last successful update time.
    pub last_updated: Option<UnixSeconds>,
    /// One health entry per source configured for this provider.
    pub sources: Vec<SourceHealth>,
    /// The newest credit information any source reported, when available.
    pub credits: Option<Credits>,
}

/// A point-in-time usage view for every supported provider.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageSnapshot {
    /// Claude usage.
    pub claude: ProviderUsage,
    /// Codex usage.
    pub codex: ProviderUsage,
    /// When this derived snapshot was generated.
    pub generated_at: UnixSeconds,
}

/// One token-history bucket.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Bucket {
    /// Bucket start time.
    pub start: UnixSeconds,
    /// Tokens counted in the bucket.
    pub tokens: TokenCount,
}

/// Where a history series' numbers originate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HistoryScope {
    /// Only tool activity recorded in local files on this Mac.
    ThisMac,
    /// Account-wide usage reported by the provider across devices and clients.
    Account,
}

/// A chart series from one source and one scope.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Series {
    /// The sole source used for every bucket in this series.
    pub source: SourceKind,
    /// Whether the data represents this Mac or the complete account.
    pub scope: HistoryScope,
    /// Human-readable scope text shown beneath the chart.
    pub scope_label: String,
    /// When the underlying source was last refreshed.
    pub observed_at: Option<UnixSeconds>,
    /// Ordered, zero-filled chart buckets.
    pub buckets: Vec<Bucket>,
}

/// Usage history for one provider.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct History {
    /// The provider represented by this history.
    pub provider: Provider,
    /// Exactly 24 local-hour-aligned buckets, oldest first.
    pub hourly: Series,
    /// Exactly seven daily buckets, oldest first, subject to Codex UTC account rules.
    pub daily: Series,
    /// Weekly percentage-based pace projection, when sufficiently stable.
    pub projection: Option<Projection>,
}

/// A linear forecast for a weekly usage window.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Projection {
    /// Projected percentage at the weekly reset; this may exceed 100.
    #[ts(type = "number")]
    pub projected_percent_at_reset: f64,
    /// When 100 percent is projected to be reached, if before reset.
    pub hits_limit_at: Option<UnixSeconds>,
}

/// Bounds external text before it is written to a log.
///
/// Text at or below 2 KiB is returned unchanged. Longer text is cut at the final
/// complete UTF-8 character whose bytes fit inside the limit.
#[must_use]
pub fn log_trunc(input: &str) -> Cow<'_, str> {
    if input.len() <= MAX_EXTERNAL_LOG_BYTES {
        return Cow::Borrowed(input);
    }

    let mut end = MAX_EXTERNAL_LOG_BYTES;
    while !input.is_char_boundary(end) {
        end -= 1;
    }

    Cow::Borrowed(&input[..end])
}
