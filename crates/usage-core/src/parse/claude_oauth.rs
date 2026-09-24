use serde::Deserialize;
use serde_json::Value;

use super::{Result, display_plan, parse_timestamp};
use crate::{
    CreditAmount, Credits, LimitWindow, Percent, Provider, Reading, SourceKind, UnixSeconds,
    WindowKind, classify,
};

/// Numeric `resets_at` values above this are epoch milliseconds, not seconds.
const MILLISECOND_THRESHOLD: i64 = 100_000_000_000;
/// Larger exponents are not plausible currency precision and are ignored.
const MAX_EXPONENT: u8 = 6;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UsageDto {
    five_hour: Option<WindowDto>,
    seven_day: Option<WindowDto>,
    /// Kept untyped: it is undocumented, so a shape change must drop only the
    /// credits and never the limit windows.
    spend: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WindowDto {
    utilization: Option<f64>,
    resets_at: Option<Value>,
}

/// Parses a Claude OAuth usage response (verified shape: decision D-024).
///
/// Only `five_hour` (Session), `seven_day` (Weekly) and the extra-usage
/// `spend` object (credits, decision D-030) are mapped; every other key is
/// ignored. `utilization` is a 0–100 percentage and `resets_at`
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
    let credits = dto
        .spend
        .as_ref()
        .and_then(|spend| credits_from_spend(spend, observed_at));
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
        credits,
    }))
}

/// Reads `spend: {enabled, used, limit}`, where each amount is
/// `{amount_minor, currency, exponent}`. Anything unexpected yields `None`.
fn credits_from_spend(spend: &Value, observed_at: UnixSeconds) -> Option<Credits> {
    let enabled = spend.get("enabled")?.as_bool()?;
    Some(Credits {
        enabled,
        unlimited: false,
        used: spend.get("used").and_then(money),
        limit: spend.get("limit").and_then(money),
        balance: None,
        source: SourceKind::ClaudeOAuth,
        observed_at,
    })
}

fn money(value: &Value) -> Option<CreditAmount> {
    let exponent = u8::try_from(value.get("exponent")?.as_u64()?)
        .ok()
        .filter(|exponent| *exponent <= MAX_EXPONENT)?;
    Some(CreditAmount {
        minor: value.get("amount_minor")?.as_i64()?,
        exponent,
        currency: Some(value.get("currency")?.as_str()?.trim().to_ascii_uppercase())
            .filter(|code| !code.is_empty()),
    })
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
