//! Lenient provider DTO parsing.

mod claude_log;
mod claude_oauth;
mod claude_statusline;
mod codex_app_server;
mod codex_rollout;

use chrono::DateTime;
use thiserror::Error;

pub use claude_log::{
    ClaudeLogRecord, ClaudeSession, parse_claude_log_line, parse_claude_log_record,
};
pub use claude_oauth::parse_claude_oauth_usage;
pub use claude_statusline::parse_claude_statusline;
pub use codex_app_server::{
    parse_codex_rate_limits_notification, parse_codex_rate_limits_response,
};
pub use codex_rollout::{CodexRolloutLine, CodexRolloutParser};

use crate::{PercentError, UnixSeconds};

/// A provider payload could not be converted into strict domain data.
#[derive(Debug, Error)]
pub enum Error {
    /// The external payload was not valid JSON.
    #[error("invalid provider JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A provider supplied a non-finite percentage.
    #[error(transparent)]
    Percent(#[from] PercentError),
    /// An RFC 3339 timestamp was invalid.
    #[error("invalid provider timestamp `{value}`: {source}")]
    Timestamp {
        /// Original provider value.
        value: String,
        /// Parsing failure.
        #[source]
        source: chrono::ParseError,
    },
    /// A relevant provider record omitted a required field.
    #[error("provider record is missing required field `{0}`")]
    MissingField(&'static str),
    /// Provider token fields overflowed a domain token count.
    #[error("provider token count overflow")]
    TokenOverflow,
}

/// Result type used by provider parsers.
pub type Result<T> = std::result::Result<T, Error>;

fn parse_timestamp(value: &str) -> Result<UnixSeconds> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| UnixSeconds(timestamp.timestamp()))
        .map_err(|source| Error::Timestamp {
            value: value.to_owned(),
            source,
        })
}

fn display_plan(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    let display = match normalized.as_str() {
        "" | "unknown" => return None,
        "free" => "Free",
        "plus" => "Plus",
        "pro" => "Pro",
        "max" => "Max",
        "team" => "Team",
        "business" => "Business",
        "enterprise" => "Enterprise",
        "edu" => "Edu",
        _ => value.trim(),
    };
    Some(display.to_owned())
}
