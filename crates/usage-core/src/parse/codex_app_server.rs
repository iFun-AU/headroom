use std::collections::BTreeMap;

use serde::Deserialize;

use super::{Result, display_plan};
use crate::{LimitWindow, Percent, Provider, Reading, SourceKind, UnixSeconds, classify};

const RATE_LIMITS_UPDATED_METHOD: &str = "account/rateLimits/updated";
const CODEX_LIMIT_ID: &str = "codex";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ResponseDto {
    result: Option<ResponseResultDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ResponseResultDto {
    rate_limits: Option<RateLimitSnapshotDto>,
    rate_limits_by_limit_id: Option<BTreeMap<String, RateLimitSnapshotDto>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct NotificationDto {
    method: Option<String>,
    params: Option<NotificationParamsDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct NotificationParamsDto {
    rate_limits: Option<RateLimitSnapshotDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RateLimitSnapshotDto {
    limit_id: Option<String>,
    primary: Option<RateLimitWindowDto>,
    secondary: Option<RateLimitWindowDto>,
    plan_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RateLimitWindowDto {
    used_percent: Option<f64>,
    window_duration_mins: Option<i64>,
    resets_at: Option<i64>,
}

/// Parses an `account/rateLimits/read` response into one complete source reading.
///
/// A payload with no eligible `codex` snapshot returns `Ok(None)`. Unknown JSON
/// fields are ignored so compatible app-server schema additions remain harmless.
///
/// # Errors
///
/// Returns an error for invalid JSON or a percentage that cannot form a strict
/// domain value.
pub fn parse_codex_rate_limits_response(
    input: &str,
    observed_at: UnixSeconds,
) -> Result<Option<Reading>> {
    let dto: ResponseDto = serde_json::from_str(input)?;
    let Some(result) = dto.result else {
        return Ok(None);
    };
    let snapshot = select_codex_snapshot(result);
    snapshot
        .as_ref()
        .map(|value| reading_from_snapshot(value, observed_at, false))
        .transpose()
}

/// Parses an app-server notification into one sparse source reading.
///
/// Unknown notification methods and non-`codex` snapshots return `Ok(None)`.
///
/// # Errors
///
/// Returns an error for invalid JSON or a percentage that cannot form a strict
/// domain value.
pub fn parse_codex_rate_limits_notification(
    input: &str,
    observed_at: UnixSeconds,
) -> Result<Option<Reading>> {
    let dto: NotificationDto = serde_json::from_str(input)?;
    if dto.method.as_deref() != Some(RATE_LIMITS_UPDATED_METHOD) {
        return Ok(None);
    }

    let snapshot = dto
        .params
        .and_then(|params| params.rate_limits)
        .filter(is_codex_snapshot);
    snapshot
        .as_ref()
        .map(|value| reading_from_snapshot(value, observed_at, true))
        .transpose()
}

fn select_codex_snapshot(mut result: ResponseResultDto) -> Option<RateLimitSnapshotDto> {
    if let Some(snapshot) = result
        .rate_limits_by_limit_id
        .as_mut()
        .and_then(|by_id| by_id.remove(CODEX_LIMIT_ID))
        .filter(is_codex_snapshot)
    {
        return Some(snapshot);
    }

    result.rate_limits.filter(is_codex_snapshot)
}

fn is_codex_snapshot(snapshot: &RateLimitSnapshotDto) -> bool {
    snapshot
        .limit_id
        .as_deref()
        .is_none_or(|limit_id| limit_id == CODEX_LIMIT_ID)
}

fn reading_from_snapshot(
    snapshot: &RateLimitSnapshotDto,
    observed_at: UnixSeconds,
    partial: bool,
) -> Result<Reading> {
    let windows = [snapshot.primary, snapshot.secondary]
        .into_iter()
        .flatten()
        .map(|window| window_from_dto(window, observed_at))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    Ok(Reading {
        provider: Provider::Codex,
        source: SourceKind::CodexAppServer,
        observed_at,
        plan: snapshot.plan_type.as_deref().and_then(display_plan),
        windows,
        partial,
    })
}

fn window_from_dto(
    dto: RateLimitWindowDto,
    observed_at: UnixSeconds,
) -> Result<Option<LimitWindow>> {
    let Some(used_percent) = dto.used_percent else {
        return Ok(None);
    };

    Ok(Some(LimitWindow {
        kind: classify(dto.window_duration_mins, None),
        used: Percent::new(used_percent)?,
        resets_at: dto.resets_at.map(UnixSeconds),
        reset_pending: false,
        source: SourceKind::CodexAppServer,
        observed_at,
    }))
}
