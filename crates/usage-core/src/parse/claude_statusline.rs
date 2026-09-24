use serde::Deserialize;

use super::Result;
use crate::{
    LimitWindow, Percent, Provider, Reading, SourceKind, UnixSeconds, WindowKind, classify,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatuslineDto {
    #[serde(rename = "writtenAt")]
    written_at: Option<i64>,
    #[serde(alias = "rateLimits")]
    rate_limits: Option<RateLimitsDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RateLimitsDto {
    five_hour: Option<RateLimitWindowDto>,
    seven_day: Option<RateLimitWindowDto>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
struct RateLimitWindowDto {
    used_percentage: Option<f64>,
    resets_at: Option<i64>,
}

/// Parses Claude Code status-line input or the persisted bridge-file envelope.
///
/// Raw status-line input uses `fallback_observed_at`; a bridge envelope's
/// `writtenAt` takes precedence. Missing limit windows produce no reading.
///
/// # Errors
///
/// Returns an error for invalid JSON or a percentage that cannot form a strict
/// domain value.
pub fn parse_claude_statusline(
    input: &str,
    fallback_observed_at: UnixSeconds,
) -> Result<Option<Reading>> {
    let dto: StatuslineDto = serde_json::from_str(input)?;
    let Some(rate_limits) = dto.rate_limits else {
        return Ok(None);
    };
    let observed_at = dto.written_at.map_or(fallback_observed_at, UnixSeconds);
    let windows = [
        (rate_limits.five_hour, WindowKind::Session),
        (rate_limits.seven_day, WindowKind::Weekly),
    ]
    .into_iter()
    .filter_map(|(window, hint)| window.map(|value| (value, hint)))
    .map(|(window, hint)| window_from_dto(window, hint, observed_at))
    .collect::<Result<Vec<_>>>()?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if windows.is_empty() {
        return Ok(None);
    }

    Ok(Some(Reading {
        provider: Provider::Claude,
        source: SourceKind::ClaudeStatusline,
        observed_at,
        plan: None,
        windows,
        partial: false,
        credits: None,
    }))
}

fn window_from_dto(
    dto: RateLimitWindowDto,
    hint: WindowKind,
    observed_at: UnixSeconds,
) -> Result<Option<LimitWindow>> {
    let Some(used_percentage) = dto.used_percentage else {
        return Ok(None);
    };

    Ok(Some(LimitWindow {
        kind: classify(None, Some(hint)),
        used: Percent::new(used_percentage)?,
        resets_at: dto.resets_at.map(UnixSeconds),
        reset_pending: false,
        source: SourceKind::ClaudeStatusline,
        observed_at,
    }))
}
