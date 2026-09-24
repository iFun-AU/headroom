use serde::Deserialize;
use serde_json::Value;

use super::{Result, display_plan, parse_timestamp};
use crate::{
    LimitWindow, Percent, Provider, Reading, SourceKind, UnixSeconds, WindowKind, classify,
};

/// Numeric `resets_at` values above this are epoch milliseconds, not seconds.
const MILLISECOND_THRESHOLD: i64 = 100_000_000_000;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UsageDto {
    five_hour: Option<WindowDto>,
    seven_day: Option<WindowDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WindowDto {
    utilization: Option<f64>,
    resets_at: Option<Value>,
}

/// Parses a Claude OAuth usage response (verified shape: decision D-024).
///
/// Only `five_hour` (Session) and `seven_day` (Weekly) are mapped; every
/// other key is ignored. `utilization` is a 0–100 percentage and `resets_at`
/// may be an RFC 3339 string, an epoch number, or null. A response without
/// either window produces no reading.
///
/// # Errors
///
/// Returns an error for invalid JSON, a wrongly typed known field, or a
/// non-finite utilization.
pub fn parse_claude_oauth_usage(
    input: &str,
    observed_at: UnixSeconds,
    subscription_type: Option<&str>,
) -> Result<Option<Reading>> {
    let dto: UsageDto = serde_json::from_str(input)?;
    let windows = [
        (dto.five_hour, WindowKind::Session),
        (dto.seven_day, WindowKind::Weekly),
    ]
    .into_iter()
    .filter_map(|(window, hint)| window.map(|window| (window, hint)))
    .filter_map(|(window, hint)| {
        window.utilization.map(|used| {
            Ok(LimitWindow {
                kind: classify(None, Some(hint)),
                used: Percent::new(used)?,
                resets_at: window.resets_at.as_ref().and_then(reset_time),
                reset_pending: false,
                source: SourceKind::ClaudeOAuth,
                observed_at,
            })
        })
    })
    .collect::<Result<Vec<_>>>()?;

    if windows.is_empty() {
        return Ok(None);
    }

    Ok(Some(Reading {
        provider: Provider::Claude,
        source: SourceKind::ClaudeOAuth,
        observed_at,
        plan: subscription_type.and_then(display_plan),
        windows,
        partial: false,
    }))
}

fn reset_time(value: &Value) -> Option<UnixSeconds> {
    match value {
        Value::String(text) => parse_timestamp(text).ok(),
        Value::Number(number) => number.as_i64().map(|raw| {
            UnixSeconds(if raw > MILLISECOND_THRESHOLD {
                raw.div_euclid(1_000)
            } else {
                raw
            })
        }),
        _ => None,
    }
}
