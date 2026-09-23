//! Codex app-server protocol session and normalized source events.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use usage_core::{
    ConnectionStatus, Provider, SourceKind, UnixSeconds,
    parse::{parse_codex_rate_limits_notification, parse_codex_rate_limits_response},
};

use crate::{
    SourceEvent,
    scheduler::{Scheduler, SchedulerError},
};

use super::rpc::{RpcClient, RpcError, RpcNotification};

mod daily;
mod source;

use daily::parse_daily;

pub use source::{
    AppServerConfig, AppServerControl, AppServerControlError, AppServerSource, AppServerSourceError,
};

const DAILY_REFRESH_INTERVAL: Duration = Duration::from_mins(15);
const UNSUPPORTED_REASON: &str = "Codex app-server does not support rate-limit reads";

/// A Codex protocol session could not continue.
#[derive(Debug, Error)]
pub enum AppServerError {
    /// The bounded JSON-RPC layer failed.
    #[error(transparent)]
    Rpc(#[from] RpcError),
    /// A provider rate-limit payload was invalid.
    #[error(transparent)]
    RateParse(#[from] usage_core::parse::Error),
    /// A daily-usage payload was invalid JSON.
    #[error("invalid Codex daily-usage JSON: {0}")]
    DailyJson(#[from] serde_json::Error),
    /// A daily bucket contained an invalid UTC date.
    #[error("invalid Codex daily date `{value}`: {source}")]
    DailyDate {
        /// Bounded provider value.
        value: String,
        /// Date parsing failure.
        #[source]
        source: chrono::ParseError,
    },
    /// The app-server explicitly lacks the required RPC method.
    #[error("{reason}")]
    Unsupported {
        /// Short actionable source status.
        reason: String,
    },
    /// Scheduler actor stopped.
    #[error(transparent)]
    Scheduler(#[from] SchedulerError),
    /// Store/source event receiver stopped.
    #[error("usage event channel is closed")]
    EventChannelClosed,
    /// RPC driver stopped forwarding notifications.
    #[error("Codex RPC notification stream closed")]
    NotificationStreamClosed,
}

/// One live app-server protocol session over an already-running RPC driver.
pub struct AppServerSession {
    client: RpcClient,
    notifications: mpsc::Receiver<RpcNotification>,
    scheduler: Scheduler,
    app_version: String,
    last_daily_read: Option<Instant>,
    ready: Option<oneshot::Sender<()>>,
}

impl AppServerSession {
    /// Creates a protocol session without spawning tasks.
    #[must_use]
    pub fn new(
        client: RpcClient,
        notifications: mpsc::Receiver<RpcNotification>,
        scheduler: Scheduler,
        app_version: impl Into<String>,
    ) -> Self {
        Self {
            client,
            notifications,
            scheduler,
            app_version: app_version.into(),
            last_daily_read: None,
            ready: None,
        }
    }

    pub(super) fn with_ready(mut self, ready: oneshot::Sender<()>) -> Self {
        self.ready = Some(ready);
        self
    }

    /// Handshakes, performs initial reads, then handles push and poll signals.
    ///
    /// # Errors
    ///
    /// Returns a typed protocol, parser, scheduler, or channel error. A remote
    /// `-32601` is returned as [`AppServerError::Unsupported`].
    pub async fn run(
        mut self,
        events: mpsc::Sender<SourceEvent>,
        cancel: CancellationToken,
    ) -> Result<(), AppServerError> {
        let started = tokio::select! {
            biased;
            () = cancel.cancelled() => false,
            result = self.start(&events, &cancel) => {
                result?;
                true
            }
        };
        if !started {
            return Ok(());
        }
        if let Some(ready) = self.ready.take() {
            let _ = ready.send(());
        }

        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => return Ok(()),
                notification = self.notifications.recv() => {
                    let Some(notification) = notification else {
                        return Err(AppServerError::NotificationStreamClosed);
                    };
                    self.handle_notification(notification, &events, &cancel).await?;
                }
                due = self.scheduler.next_due(SourceKind::CodexAppServer) => {
                    due?;
                    self.poll(&events, &cancel).await?;
                }
            }
        }
    }

    async fn start(
        &mut self,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerError> {
        let initialize = self
            .client
            .request(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "how_is_it",
                        "title": "How Is It",
                        "version": self.app_version,
                    }
                }),
            )
            .await?;
        if initialize.get("result").is_none() {
            return Err(AppServerError::Unsupported {
                reason: "Codex app-server initialization failed".to_owned(),
            });
        }
        self.client.notify("initialized", None).await?;
        self.scheduler.next_due(SourceKind::CodexAppServer).await?;
        self.read_limits(events, cancel).await?;
        if let Err(error) = self.read_daily(events, cancel).await {
            warn!(error = %usage_core::log_trunc(&error.to_string()), "initial Codex daily-usage read failed");
        }
        Ok(())
    }

    async fn poll(
        &mut self,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerError> {
        if let Err(error) = self.read_limits(events, cancel).await {
            self.record_failure();
            if matches!(error, AppServerError::Unsupported { .. }) {
                return Err(error);
            }
            self.emit_error(events, cancel).await?;
            return match error {
                AppServerError::Rpc(RpcError::Disconnected | RpcError::Closed) => Err(error),
                _ => Ok(()),
            };
        }
        if self
            .last_daily_read
            .is_none_or(|last| last.elapsed() >= DAILY_REFRESH_INTERVAL)
            && self.read_daily(events, cancel).await.is_err()
        {
            self.emit_error(events, cancel).await?;
        }
        Ok(())
    }

    async fn read_limits(
        &self,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerError> {
        let response = self
            .client
            .request(
                "account/rateLimits/read",
                json!({ "excludeResetCreditDetails": true }),
            )
            .await
            .map_err(map_rate_rpc_error)?;
        log_filtered_limit_ids(&response);
        let encoded = serde_json::to_string(&response)?;
        if let Some(reading) = parse_codex_rate_limits_response(&encoded, unix_now())? {
            send_event(events, SourceEvent::Reading(reading), cancel).await?;
        }
        self.record_success();
        Ok(())
    }

    async fn read_daily(
        &mut self,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerError> {
        let response = self
            .client
            .request("account/usage/read", Value::Null)
            .await?;
        let daily = parse_daily(response, unix_now())?;
        send_event(events, SourceEvent::Daily(daily), cancel).await?;
        self.last_daily_read = Some(Instant::now());
        Ok(())
    }

    async fn handle_notification(
        &self,
        notification: RpcNotification,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerError> {
        if notification.method != "account/rateLimits/updated" {
            return Ok(());
        }
        log_filtered_limit_ids(&notification.message);
        let encoded = serde_json::to_string(&notification.message)?;
        if let Some(reading) = parse_codex_rate_limits_notification(&encoded, unix_now())? {
            send_event(events, SourceEvent::Reading(reading), cancel).await?;
            send_event(
                events,
                SourceEvent::Activity {
                    provider: Provider::Codex,
                },
                cancel,
            )
            .await?;
            self.record_success();
            if let Err(error) = self.scheduler.record_activity(Provider::Codex) {
                warn!(%error, "could not record Codex push activity");
            }
        }
        Ok(())
    }

    async fn emit_error(
        &self,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerError> {
        send_event(
            events,
            SourceEvent::Status {
                provider: Provider::Codex,
                source: SourceKind::CodexAppServer,
                status: ConnectionStatus::Error {
                    message: "Codex app-server read failed; retrying".to_owned(),
                },
            },
            cancel,
        )
        .await
    }

    fn record_success(&self) {
        if let Err(error) = self.scheduler.record_success(SourceKind::CodexAppServer) {
            warn!(%error, "could not record successful Codex read");
        }
    }

    fn record_failure(&self) {
        if let Err(error) = self
            .scheduler
            .record_failure(SourceKind::CodexAppServer, None)
        {
            warn!(%error, "could not record failed Codex read");
        }
    }
}

fn map_rate_rpc_error(error: RpcError) -> AppServerError {
    match error {
        RpcError::Remote { code: -32601, .. } => AppServerError::Unsupported {
            reason: UNSUPPORTED_REASON.to_owned(),
        },
        other => AppServerError::Rpc(other),
    }
}

fn log_filtered_limit_ids(message: &Value) {
    let result = message.get("result");
    if let Some(by_id) = result
        .and_then(|value| value.get("rateLimitsByLimitId"))
        .and_then(Value::as_object)
    {
        for limit_id in by_id.keys().filter(|limit_id| limit_id.as_str() != "codex") {
            debug!(limit_id = %usage_core::log_trunc(limit_id), "ignoring non-Codex rate limit");
        }
    }
    let snapshot = result
        .and_then(|value| value.get("rateLimits"))
        .or_else(|| message.pointer("/params/rateLimits"));
    if let Some(limit_id) = snapshot
        .and_then(|value| value.get("limitId"))
        .and_then(Value::as_str)
        .filter(|limit_id| *limit_id != "codex")
    {
        debug!(limit_id = %usage_core::log_trunc(limit_id), "ignoring non-Codex rate limit");
    }
}

async fn send_event(
    events: &mpsc::Sender<SourceEvent>,
    event: SourceEvent,
    cancel: &CancellationToken,
) -> Result<(), AppServerError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Ok(()),
        result = events.send(event) => result.map_err(|_| AppServerError::EventChannelClosed),
    }
}

fn unix_now() -> UnixSeconds {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    UnixSeconds(i64::try_from(seconds).unwrap_or(i64::MAX))
}
