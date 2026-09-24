//! T9.1 shape check: prints only JSON key paths and value types, never values.

use std::error::Error;

#[cfg(not(feature = "claude-oauth"))]
#[allow(clippy::unused_async, reason = "matches the feature-enabled signature")]
pub async fn dump_keys() -> Result<(), Box<dyn Error>> {
    Err(std::io::Error::other("--claude-oauth-dump-keys requires --features claude-oauth").into())
}

#[cfg(feature = "claude-oauth")]
pub async fn dump_keys() -> Result<(), Box<dyn Error>> {
    use std::{
        fmt::Write as _,
        io::{self, Write as _},
    };

    use usage_sources::claude::{
        keychain::{CredentialStore, KeychainCredentials},
        oauth::{ReqwestUsageHttp, UsageHttp},
    };

    let presence = |present: bool| if present { "present" } else { "absent" };
    let credentials = tokio::task::spawn_blocking(|| KeychainCredentials::new().read()).await??;
    let now = chrono::Utc::now().timestamp();
    let expiry = match credentials.expires_at {
        None => "absent",
        Some(at) if at.0 < now => "in the past",
        Some(_) => "in the future",
    };
    let mut report = String::new();
    writeln!(
        report,
        "credential: accessToken present, expiresAt {expiry}, subscriptionType {}",
        presence(credentials.subscription_type.is_some()),
    )?;
    let response = ReqwestUsageHttp::new(env!("CARGO_PKG_VERSION"))?
        .get_usage(&credentials.access_token)
        .await?;
    writeln!(
        report,
        "status: {}\nretry-after: {}",
        response.status,
        presence(response.retry_after.is_some()),
    )?;
    match serde_json::from_slice::<serde_json::Value>(&response.body) {
        Ok(value) => key_paths("$", &value, &mut report)?,
        Err(_) => writeln!(report, "body: not JSON ({} bytes)", response.body.len())?,
    }
    io::stdout().write_all(report.as_bytes())?;
    Ok(())
}

#[cfg(feature = "claude-oauth")]
fn key_paths(path: &str, value: &serde_json::Value, report: &mut String) -> std::fmt::Result {
    use std::fmt::Write as _;

    use serde_json::Value;

    let kind = match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "float",
        Value::String(text) if chrono::DateTime::parse_from_rfc3339(text).is_ok() => {
            "string (RFC 3339)"
        }
        Value::String(_) => "string",
        Value::Array(items) => {
            writeln!(report, "{path}: array (len {})", items.len())?;
            return items.first().map_or(Ok(()), |first| {
                key_paths(&format!("{path}[0]"), first, report)
            });
        }
        Value::Object(map) => {
            writeln!(report, "{path}: object")?;
            for (key, child) in map {
                key_paths(&format!("{path}.{key}"), child, report)?;
            }
            return Ok(());
        }
    };
    writeln!(report, "{path}: {kind}")
}
