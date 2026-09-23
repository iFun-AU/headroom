use serde::Deserialize;

use super::{Error, Result, display_plan, parse_timestamp};
use crate::{
    LimitWindow, Percent, Provider, Reading, SourceKind, TokenCount, TokenEvent, UnixSeconds,
    classify,
};

const EVENT_MESSAGE_TYPE: &str = "event_msg";
const TOKEN_COUNT_TYPE: &str = "token_count";
const CODEX_LIMIT_ID: &str = "codex";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RolloutLineDto {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    payload: Option<PayloadDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PayloadDto {
    #[serde(rename = "type")]
    kind: Option<String>,
    info: Option<UsageInfoDto>,
    rate_limits: Option<RateLimitsDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UsageInfoDto {
    total_token_usage: Option<TokenUsageDto>,
    last_token_usage: Option<TokenUsageDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TokenUsageDto {
    total_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RateLimitsDto {
    limit_id: Option<String>,
    primary: Option<RateLimitWindowDto>,
    secondary: Option<RateLimitWindowDto>,
    plan_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
struct RateLimitWindowDto {
    used_percent: Option<f64>,
    window_minutes: Option<i64>,
    resets_at: Option<i64>,
    resets_in_seconds: Option<i64>,
}

/// Normalized events produced by one relevant rollout JSONL record.
#[derive(Debug, Default, PartialEq)]
pub struct CodexRolloutLine {
    /// Full limit reading when the line contains eligible Codex windows.
    pub reading: Option<Reading>,
    /// Fork-safe token delta when the line contains token usage information.
    pub token: Option<TokenEvent>,
}

/// Stateful parser for one Codex rollout file.
///
/// Construct one parser per file so cumulative token counters from separate or
/// forked sessions cannot affect one another.
#[derive(Debug, Default)]
pub struct CodexRolloutParser {
    previous_total: Option<u64>,
}

impl CodexRolloutParser {
    /// Creates a parser with no prior cumulative token total.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            previous_total: None,
        }
    }

    /// Parses one rollout JSONL record and updates this file's token counter.
    ///
    /// Unknown record types are ignored. Token deltas are retained for every
    /// limit ID, while limit readings accept only `codex` or an absent ID.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid JSON, an invalid timestamp on a relevant
    /// record, or a percentage that cannot form a strict domain value.
    pub fn parse_line(&mut self, input: &str) -> Result<CodexRolloutLine> {
        let dto: RolloutLineDto = serde_json::from_str(input)?;
        if dto.kind.as_deref() != Some(EVENT_MESSAGE_TYPE) {
            return Ok(CodexRolloutLine::default());
        }

        let Some(payload) = dto.payload else {
            return Ok(CodexRolloutLine::default());
        };
        if payload.kind.as_deref() != Some(TOKEN_COUNT_TYPE) {
            return Ok(CodexRolloutLine::default());
        }

        let timestamp = dto
            .timestamp
            .as_deref()
            .ok_or(Error::MissingField("timestamp"))?;
        let observed_at = parse_timestamp(timestamp)?;

        Ok(CodexRolloutLine {
            reading: payload
                .rate_limits
                .as_ref()
                .map(|limits| reading_from_limits(limits, observed_at))
                .transpose()?
                .flatten(),
            token: payload
                .info
                .as_ref()
                .and_then(|info| self.token_from_info(info, observed_at)),
        })
    }

    fn token_from_info(&mut self, info: &UsageInfoDto, at: UnixSeconds) -> Option<TokenEvent> {
        let total = info.total_token_usage.as_ref()?.total_tokens?;
        let last = info
            .last_token_usage
            .as_ref()
            .and_then(|usage| usage.total_tokens)
            .unwrap_or_default();
        let delta = self
            .previous_total
            .replace(total)
            .map_or(last, |previous| total.checked_sub(previous).unwrap_or(last));

        Some(TokenEvent {
            provider: Provider::Codex,
            source: SourceKind::CodexRollout,
            at,
            tokens: TokenCount(delta),
            dedupe_key: None,
        })
    }
}

fn reading_from_limits(
    limits: &RateLimitsDto,
    observed_at: UnixSeconds,
) -> Result<Option<Reading>> {
    if !is_codex_limit(limits.limit_id.as_deref()) {
        return Ok(None);
    }

    let windows = [limits.primary, limits.secondary]
        .into_iter()
        .flatten()
        .map(|window| window_from_dto(window, observed_at))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if windows.is_empty() {
        return Ok(None);
    }

    Ok(Some(Reading {
        provider: Provider::Codex,
        source: SourceKind::CodexRollout,
        observed_at,
        plan: limits.plan_type.as_deref().and_then(display_plan),
        windows,
        partial: false,
    }))
}

fn is_codex_limit(limit_id: Option<&str>) -> bool {
    limit_id.is_none_or(|value| value == CODEX_LIMIT_ID)
}

fn window_from_dto(
    dto: RateLimitWindowDto,
    observed_at: UnixSeconds,
) -> Result<Option<LimitWindow>> {
    let Some(used_percent) = dto.used_percent else {
        return Ok(None);
    };
    let resets_at = dto.resets_at.or_else(|| {
        dto.resets_in_seconds
            .map(|seconds| observed_at.0.saturating_add(seconds))
    });

    Ok(Some(LimitWindow {
        kind: classify(dto.window_minutes, None),
        used: Percent::new(used_percent)?,
        resets_at: resets_at.map(UnixSeconds),
        reset_pending: false,
        source: SourceKind::CodexRollout,
        observed_at,
    }))
}
