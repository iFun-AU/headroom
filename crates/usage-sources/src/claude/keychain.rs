//! Read-only access to Claude Code's OAuth credential (DEVELOPMENT.md §8.4).
//!
//! The credential is only ever read. It is never refreshed, rotated, written,
//! logged, or persisted, because doing so could invalidate Claude Code's own
//! session. Error values deliberately carry no provider text, since a JSON
//! error message could quote part of the secret.

use std::fmt;

use serde::Deserialize;
use thiserror::Error;
use usage_core::UnixSeconds;

/// Keychain generic-password service that Claude Code writes.
pub const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// macOS `errSecItemNotFound`.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25_300;
/// macOS `errSecUserCanceled` and `errSecAuthFailed`: the user denied access.
const ERR_SEC_DENIED: [i32; 2] = [-128, -25_293];

/// A bearer token whose value never appears in `Debug` output or logs.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(String);

impl AccessToken {
    /// Wraps a raw token value.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns the raw value for the `Authorization` header only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken(<redacted>)")
    }
}

/// The parts of Claude Code's credential this app uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCredentials {
    /// OAuth bearer token.
    pub access_token: AccessToken,
    /// Local expiry, converted from epoch milliseconds.
    pub expires_at: Option<UnixSeconds>,
    /// Subscription name, for example `max`.
    pub subscription_type: Option<String>,
}

/// A credential could not be obtained. Variants carry no secret material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CredentialError {
    /// Claude Code has not stored an OAuth credential.
    #[error("Claude Code OAuth credential was not found")]
    NotFound,
    /// The user denied the Keychain access prompt.
    #[error("Keychain access to the Claude Code credential was denied")]
    Denied,
    /// The stored value was not the expected JSON shape.
    #[error("Claude Code OAuth credential has an unexpected shape")]
    Malformed,
    /// Another Keychain failure, by `OSStatus` code.
    #[error("Keychain read failed with status {0}")]
    Keychain(i32),
}

#[derive(Deserialize)]
struct CredentialFileDto {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuthDto>,
}

#[derive(Deserialize)]
struct OAuthDto {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

/// Parses the Keychain value Claude Code stores.
///
/// # Errors
///
/// Returns [`CredentialError::Malformed`] for invalid JSON or a missing or
/// empty access token, without echoing any part of the input.
pub fn parse_credentials(bytes: &[u8]) -> Result<ClaudeCredentials, CredentialError> {
    let dto: CredentialFileDto =
        serde_json::from_slice(bytes).map_err(|_| CredentialError::Malformed)?;
    let oauth = dto.claude_ai_oauth.ok_or(CredentialError::Malformed)?;
    let token = oauth
        .access_token
        .filter(|token| !token.is_empty())
        .ok_or(CredentialError::Malformed)?;
    Ok(ClaudeCredentials {
        access_token: AccessToken::new(token),
        expires_at: oauth
            .expires_at
            .map(|millis| UnixSeconds(millis.div_euclid(1_000))),
        subscription_type: oauth.subscription_type.filter(|value| !value.is_empty()),
    })
}

/// Source of Claude Code credentials, replaceable in tests.
pub trait CredentialStore: Send + Sync + 'static {
    /// Reads the current credential. May block on a macOS Keychain prompt.
    ///
    /// # Errors
    ///
    /// Returns a secret-free [`CredentialError`].
    fn read(&self) -> Result<ClaudeCredentials, CredentialError>;
}

/// The login Keychain item written by Claude Code.
#[derive(Debug, Clone, Default)]
pub struct KeychainCredentials {
    account: Option<String>,
}

impl KeychainCredentials {
    /// Uses the macOS user name as the Keychain account, as Claude Code does.
    #[must_use]
    pub fn new() -> Self {
        Self {
            account: std::env::var("USER").ok().filter(|user| !user.is_empty()),
        }
    }
}

impl CredentialStore for KeychainCredentials {
    fn read(&self) -> Result<ClaudeCredentials, CredentialError> {
        let bytes = match &self.account {
            Some(account) => {
                match security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, account)
                {
                    Ok(bytes) => bytes,
                    Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => search_by_service()?,
                    Err(error) => return Err(map_status(error.code())),
                }
            }
            None => search_by_service()?,
        };
        parse_credentials(&bytes)
    }
}

/// Falls back to a service-only search when the account differs.
fn search_by_service() -> Result<Vec<u8>, CredentialError> {
    use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};

    let results = ItemSearchOptions::new()
        .class(ItemClass::generic_password())
        .service(KEYCHAIN_SERVICE)
        .load_data(true)
        .limit(Limit::Max(1))
        .search()
        .map_err(|error| map_status(error.code()))?;
    results
        .into_iter()
        .find_map(|result| match result {
            SearchResult::Data(bytes) => Some(bytes),
            _ => None,
        })
        .ok_or(CredentialError::NotFound)
}

fn map_status(code: i32) -> CredentialError {
    if code == ERR_SEC_ITEM_NOT_FOUND {
        CredentialError::NotFound
    } else if ERR_SEC_DENIED.contains(&code) {
        CredentialError::Denied
    } else {
        CredentialError::Keychain(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_expected_shape_and_converts_milliseconds() {
        let credentials = parse_credentials(
            br#"{"claudeAiOauth":{"accessToken":"secret-value","refreshToken":"other","expiresAt":1790000000999,"subscriptionType":"max"}}"#,
        )
        .unwrap();
        assert_eq!(credentials.access_token.expose(), "secret-value");
        assert_eq!(credentials.expires_at, Some(UnixSeconds(1_790_000_000)));
        assert_eq!(credentials.subscription_type.as_deref(), Some("max"));
    }

    #[test]
    fn debug_output_and_errors_never_contain_the_token() {
        let credentials =
            parse_credentials(br#"{"claudeAiOauth":{"accessToken":"secret-value"}}"#).unwrap();
        assert!(!format!("{credentials:?}").contains("secret-value"));

        // A type error would make serde quote the value; it must not escape.
        let error = parse_credentials(
            br#"{"claudeAiOauth":{"accessToken":"secret-value","expiresAt":"secret-value"}}"#,
        )
        .unwrap_err();
        assert_eq!(error, CredentialError::Malformed);
        assert!(!error.to_string().contains("secret-value"));
    }

    #[test]
    fn missing_or_empty_token_is_malformed() {
        for input in [
            &b"{}"[..],
            br#"{"claudeAiOauth":{}}"#,
            br#"{"claudeAiOauth":{"accessToken":""}}"#,
            b"not json",
        ] {
            assert_eq!(parse_credentials(input), Err(CredentialError::Malformed));
        }
    }

    #[test]
    fn keychain_status_codes_map_to_secret_free_errors() {
        assert_eq!(map_status(-25_300), CredentialError::NotFound);
        assert_eq!(map_status(-128), CredentialError::Denied);
        assert_eq!(map_status(-25_293), CredentialError::Denied);
        assert_eq!(map_status(-1), CredentialError::Keychain(-1));
    }
}
