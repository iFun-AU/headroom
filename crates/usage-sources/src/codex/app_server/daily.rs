//! Lenient Codex account-daily DTO normalization.

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use usage_core::{DailyBuckets, Provider, SourceKind, TokenCount, UnixSeconds};

use super::AppServerError;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DailyResponseDto {
    result: Option<DailyResultDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct DailyResultDto {
    daily_usage_buckets: Vec<DailyBucketDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct DailyBucketDto {
    start_date: Option<String>,
    tokens: Option<u64>,
}

pub(super) fn parse_daily(
    message: Value,
    observed_at: UnixSeconds,
) -> Result<DailyBuckets, AppServerError> {
    let response: DailyResponseDto = serde_json::from_value(message)?;
    let buckets = response
        .result
        .map_or_else(Vec::new, |result| result.daily_usage_buckets);
    let mut days = Vec::with_capacity(buckets.len());
    for bucket in buckets {
        let (Some(date), Some(tokens)) = (bucket.start_date, bucket.tokens) else {
            continue;
        };
        let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|source| {
            AppServerError::DailyDate {
                value: usage_core::log_trunc(&date).into_owned(),
                source,
            }
        })?;
        days.push((parsed, TokenCount(tokens)));
    }
    Ok(DailyBuckets {
        provider: Provider::Codex,
        source: SourceKind::CodexAppServer,
        observed_at,
        days,
    })
}
