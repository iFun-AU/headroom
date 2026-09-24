//! Opt-in poller for Claude's unofficial OAuth usage endpoint
//! (DEVELOPMENT.md §§2.4 and 8.4, decision D-023).

use std::{future::Future, sync::Arc, time::Duration};

use thiserror::Error;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use usage_core::{
    ConnectionStatus, Provider, Reading, SourceKind, UnixSeconds, UsageSnapshot,
    parse::parse_claude_oauth_usage,
};

use super::keychain::{AccessToken, ClaudeCredentials, CredentialError, CredentialStore};
use crate::{
    SourceEvent,
    scheduler::{Scheduler, SchedulerError},
};

/// Undocumented endpoint used internally by Claude Code.
pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA_HEADER: &str = "oauth-2025-04-20";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Upper bound on a usage response body; the expected body is a few KiB.
pub const MAX_BODY_BYTES: usize = 256 * 1_024;
/// A status-line reading at most this old makes a poll unnecessary (§8.4).
const STATUSLINE_FRESH_SECS: i64 = 180;
const AUTH_EXPIRED_HINT: &str = "Open Claude Code to refresh sign-in";
/// Longest honored `Retry-After`; larger values would stall polling for good
/// or overflow the scheduler's deadline arithmetic.
const MAX_RETRY_AFTER: Duration = Duration::from_hours(1);

/// Status, rate-limit hint, and bounded body of one usage response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// `Retry-After` in delta-seconds form, when present.
    pub retry_after: Option<Duration>,
    /// Response body, at most [`MAX_BODY_BYTES`].
    pub body: Vec<u8>,
}

/// A usage request did not produce a response. Messages never contain the
/// token, and transport messages have the URL removed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HttpError {
    /// The HTTP client could not be constructed.
    #[error("could not create the HTTP client: {0}")]
    Client(String),
    /// Connection, TLS, timeout, or body-read failure.
    #[error("usage request failed: {0}")]
    Transport(String),
    /// The body exceeded [`MAX_BODY_BYTES`].
    #[error("usage response is larger than {MAX_BODY_BYTES} bytes")]
    TooLarge,
}

/// Minimal HTTP seam so the poller can be tested without the network.
pub trait UsageHttp: Send + Sync + 'static {
    /// Performs one authenticated `GET` of [`USAGE_URL`].
    fn get_usage(
        &self,
        token: &AccessToken,
    ) -> impl Future<Output = Result<HttpResponse, HttpError>> + Send;
}

/// Production HTTPS client (rustls with the macOS platform verifier).
#[derive(Debug, Clone)]
pub struct ReqwestUsageHttp {
    client: reqwest::Client,
}

impl ReqwestUsageHttp {
    /// Creates an HTTPS-only client that never follows redirects, so the
    /// bearer token is only ever sent to [`USAGE_URL`].
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Client`] if TLS initialization fails.
    pub fn new(app_version: &str) -> Result<Self, HttpError> {
        let client = reqwest::Client::builder()
            .user_agent(format!("how-is-it/{app_version}"))
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|error| HttpError::Client(error.without_url().to_string()))?;
        Ok(Self { client })
    }
}

impl UsageHttp for ReqwestUsageHttp {
    async fn get_usage(&self, token: &AccessToken) -> Result<HttpResponse, HttpError> {
        use reqwest::header::{ACCEPT, RETRY_AFTER};

        let mut response = self
            .client
            .get(USAGE_URL)
            .bearer_auth(token.expose())
            .header("anthropic-beta", BETA_HEADER)
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(transport)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_retry_after);
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport)? {
            if body.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
                return Err(HttpError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            retry_after,
            body,
        })
    }
}

/// Parses delta-seconds `Retry-After`, capped at [`MAX_RETRY_AFTER`].
fn parse_retry_after(value: &str) -> Option<Duration> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(|seconds| Duration::from_secs(seconds).min(MAX_RETRY_AFTER))
}

fn transport(error: reqwest::Error) -> HttpError {
    HttpError::Transport(error.without_url().to_string())
}

/// Fatal failure that stops the OAuth source task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OAuthSourceError {
    /// The scheduler actor stopped.
    #[error("scheduler is unavailable")]
    SchedulerClosed,
    /// The central source-event receiver stopped.
    #[error("usage event channel is closed")]
    EventChannelClosed,
}

/// What one scheduled poll produced and how polling continues.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// A full reading; clears scheduler backoff.
    Reading(Reading),
    /// Report the status, if any, and retry after scheduler backoff.
    Retry {
        status: Option<ConnectionStatus>,
        retry_after: Option<Duration>,
    },
    /// Report the status and check again at the normal interval.
    Recheck(ConnectionStatus),
    /// Report the status and stop until restart or the setting is toggled.
    Stop(ConnectionStatus),
}

/// State carried between polls of one source generation.
#[derive(Default)]
struct PollState {
    /// Credential reused until it expires or is rejected, so the Keychain
    /// (and any macOS access prompt) is consulted as rarely as possible.
    cached: Option<ClaudeCredentials>,
    logged_keys: bool,
}

/// Scheduler-driven poller for the Claude OAuth usage endpoint.
pub struct OAuthSource<H, C> {
    http: H,
    credentials: Arc<C>,
    scheduler: Scheduler,
    snapshot: watch::Receiver<Arc<UsageSnapshot>>,
}

impl<H: UsageHttp, C: CredentialStore> OAuthSource<H, C> {
    /// Creates an unspawned poller. `snapshot` lets it skip polls while the
    /// status-line bridge is delivering fresher data.
    #[must_use]
    pub fn new(
        http: H,
        credentials: C,
        scheduler: Scheduler,
        snapshot: watch::Receiver<Arc<UsageSnapshot>>,
    ) -> Self {
        Self {
            http,
            credentials: Arc::new(credentials),
            scheduler,
            snapshot,
        }
    }

    /// Polls whenever the scheduler says the source is due.
    ///
    /// # Errors
    ///
    /// Returns an error only when the scheduler or event channel stops.
    pub async fn run(
        self,
        events: mpsc::Sender<SourceEvent>,
        cancel: CancellationToken,
    ) -> Result<(), OAuthSourceError> {
        let mut state = PollState::default();
        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => return Ok(()),
                due = self.scheduler.next_due(SourceKind::ClaudeOAuth) => {
                    due.map_err(|_| OAuthSourceError::SchedulerClosed)?;
                }
            }
            let now = unix_now();
            if self.statusline_fresh(now) {
                report(self.scheduler.record_success(SourceKind::ClaudeOAuth));
                continue;
            }
            let outcome = tokio::select! {
                biased;
                () = cancel.cancelled() => return Ok(()),
                outcome = self.poll(now, &mut state) => outcome,
            };
            match outcome {
                Outcome::Reading(reading) => {
                    send(&events, SourceEvent::Reading(reading), &cancel).await?;
                    report(self.scheduler.record_success(SourceKind::ClaudeOAuth));
                }
                Outcome::Retry {
                    status,
                    retry_after,
                } => {
                    if let Some(status) = status {
                        send(&events, status_event(status), &cancel).await?;
                    }
                    report(
                        self.scheduler
                            .record_failure(SourceKind::ClaudeOAuth, retry_after),
                    );
                }
                Outcome::Recheck(status) => {
                    send(&events, status_event(status), &cancel).await?;
                }
                Outcome::Stop(status) => {
                    send(&events, status_event(status), &cancel).await?;
                    cancel.cancelled().await;
                    return Ok(());
                }
            }
        }
    }

    fn statusline_fresh(&self, now: UnixSeconds) -> bool {
        self.snapshot
            .borrow()
            .claude
            .sources
            .iter()
            .find(|health| health.source == SourceKind::ClaudeStatusline)
            .and_then(|health| health.last_success)
            .is_some_and(|at| now.0.saturating_sub(at.0) < STATUSLINE_FRESH_SECS)
    }

    async fn poll(&self, now: UnixSeconds, state: &mut PollState) -> Outcome {
        let expired =
            |credentials: &ClaudeCredentials| credentials.expires_at.is_some_and(|at| at.0 < now.0);
        let credentials =
            if let Some(cached) = state.cached.take().filter(|cached| !expired(cached)) {
                cached
            } else {
                let store = Arc::clone(&self.credentials);
                match tokio::task::spawn_blocking(move || store.read()).await {
                    Ok(Ok(credentials)) => credentials,
                    Ok(Err(error)) => return credential_outcome(error),
                    Err(_) => return credential_outcome(CredentialError::Keychain(0)),
                }
            };
        if expired(&credentials) {
            return Outcome::Recheck(auth_expired());
        }
        match self.http.get_usage(&credentials.access_token).await {
            Ok(response) => {
                if response.status == 200 && !state.logged_keys {
                    state.logged_keys = true;
                    log_key_names(&response.body);
                }
                let outcome =
                    response_outcome(&response, now, credentials.subscription_type.as_deref());
                // A rejected token is re-read from the Keychain next time.
                if response.status != 401 {
                    state.cached = Some(credentials);
                }
                outcome
            }
            Err(error) => {
                warn!(error = %usage_core::log_trunc(&error.to_string()), "Claude OAuth usage request failed");
                Outcome::Retry {
                    status: Some(ConnectionStatus::Error {
                        message: "Couldn’t reach Claude’s usage API".to_owned(),
                    }),
                    retry_after: None,
                }
            }
        }
    }
}

fn report(result: Result<(), SchedulerError>) {
    if let Err(error) = result {
        warn!(%error, "could not update Claude OAuth schedule");
    }
}

fn credential_outcome(error: CredentialError) -> Outcome {
    warn!(%error, "could not read Claude Code OAuth credential");
    match error {
        CredentialError::NotFound => Outcome::Recheck(ConnectionStatus::NotConfigured {
            hint: "Sign in to Claude Code with a Claude subscription".to_owned(),
        }),
        // Stop so a denied prompt is not shown again on every tick.
        CredentialError::Denied => Outcome::Stop(ConnectionStatus::Error {
            message: "Keychain access was denied. Turn Claude usage API off and on to ask again."
                .to_owned(),
        }),
        CredentialError::Malformed | CredentialError::Keychain(_) => Outcome::Retry {
            status: Some(ConnectionStatus::Error {
                message: "Couldn’t read Claude Code’s sign-in from the Keychain".to_owned(),
            }),
            retry_after: None,
        },
    }
}

fn response_outcome(
    response: &HttpResponse,
    now: UnixSeconds,
    subscription_type: Option<&str>,
) -> Outcome {
    match response.status {
        200 => {
            let parsed = std::str::from_utf8(&response.body)
                .ok()
                .and_then(|body| parse_claude_oauth_usage(body, now, subscription_type).ok())
                .flatten();
            parsed.map_or_else(
                || {
                    Outcome::Stop(ConnectionStatus::Unsupported {
                        reason:
                            "Claude’s usage API returned a response this version doesn’t recognize"
                                .to_owned(),
                    })
                },
                Outcome::Reading,
            )
        }
        401 => Outcome::Recheck(auth_expired()),
        403 | 404 => Outcome::Stop(ConnectionStatus::Unsupported {
            reason: format!(
                "Claude’s usage API isn’t available for this account (HTTP {})",
                response.status
            ),
        }),
        429 => Outcome::Retry {
            status: None,
            retry_after: response.retry_after,
        },
        status => Outcome::Retry {
            status: Some(ConnectionStatus::Error {
                message: format!("Claude’s usage API returned HTTP {status}"),
            }),
            retry_after: None,
        },
    }
}

fn auth_expired() -> ConnectionStatus {
    ConnectionStatus::AuthExpired {
        hint: AUTH_EXPIRED_HINT.to_owned(),
    }
}

/// Logs top-level key names once so shape drift is visible (Appendix A.5).
fn log_key_names(body: &[u8]) {
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_slice(body) {
        let keys = map.keys().map(String::as_str).collect::<Vec<_>>().join(",");
        debug!(keys = %usage_core::log_trunc(&keys), "Claude OAuth usage response keys");
    }
}

fn status_event(status: ConnectionStatus) -> SourceEvent {
    SourceEvent::Status {
        provider: Provider::Claude,
        source: SourceKind::ClaudeOAuth,
        status,
    }
}

async fn send(
    events: &mpsc::Sender<SourceEvent>,
    event: SourceEvent,
    cancel: &CancellationToken,
) -> Result<(), OAuthSourceError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Ok(()),
        result = events.send(event) => result.map_err(|_| OAuthSourceError::EventChannelClosed),
    }
}

fn unix_now() -> UnixSeconds {
    UnixSeconds(chrono::Utc::now().timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            retry_after: None,
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn http_status_codes_follow_the_specified_failure_semantics() {
        let now = UnixSeconds(1_790_000_000);
        assert!(matches!(
            response_outcome(&response(200, r#"{"five_hour":{"utilization":5}}"#), now, Some("max")),
            Outcome::Reading(reading) if reading.plan.as_deref() == Some("Max")
        ));
        assert!(matches!(
            response_outcome(&response(200, "{}"), now, None),
            Outcome::Stop(ConnectionStatus::Unsupported { .. })
        ));
        assert!(matches!(
            response_outcome(&response(200, "<html>"), now, None),
            Outcome::Stop(ConnectionStatus::Unsupported { .. })
        ));
        assert_eq!(
            response_outcome(&response(401, ""), now, None),
            Outcome::Recheck(auth_expired())
        );
        for status in [403, 404] {
            assert!(matches!(
                response_outcome(&response(status, ""), now, None),
                Outcome::Stop(ConnectionStatus::Unsupported { .. })
            ));
        }
        let limited = HttpResponse {
            retry_after: Some(Duration::from_secs(90)),
            ..response(429, "")
        };
        assert_eq!(
            response_outcome(&limited, now, None),
            Outcome::Retry {
                status: None,
                retry_after: Some(Duration::from_secs(90))
            }
        );
        assert!(matches!(
            response_outcome(&response(503, ""), now, None),
            Outcome::Retry {
                status: Some(ConnectionStatus::Error { .. }),
                retry_after: None
            }
        ));
    }

    #[test]
    fn retry_after_is_capped_and_ignores_dates() {
        assert_eq!(parse_retry_after(" 90 "), Some(Duration::from_secs(90)));
        assert_eq!(
            parse_retry_after("18446744073709551615"),
            Some(MAX_RETRY_AFTER)
        );
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT"), None);
    }

    #[test]
    fn credential_errors_map_without_prompt_loops() {
        assert!(matches!(
            credential_outcome(CredentialError::NotFound),
            Outcome::Recheck(ConnectionStatus::NotConfigured { .. })
        ));
        assert!(matches!(
            credential_outcome(CredentialError::Denied),
            Outcome::Stop(ConnectionStatus::Error { .. })
        ));
        assert!(matches!(
            credential_outcome(CredentialError::Keychain(-1)),
            Outcome::Retry { .. }
        ));
    }
}
