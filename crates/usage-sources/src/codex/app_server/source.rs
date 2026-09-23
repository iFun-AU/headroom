//! Owned Codex child-process lifecycle and restart policy.

use std::time::Duration;

use thiserror::Error;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::{ConnectionStatus, Provider, SourceKind, log_trunc};

use crate::{SourceEvent, scheduler::Scheduler};

use super::{
    super::discover::{DiscoveryOptions, discover},
    send_event,
};

mod process;

use process::{AttemptError, AttemptResult, run_attempt};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const INITIAL_RESTART_DELAY: Duration = Duration::from_secs(5);
const MAXIMUM_RESTART_DELAY: Duration = Duration::from_mins(5);
const MAX_CONSECUTIVE_FAILURES: u8 = 10;
const RETRY_CAPACITY: usize = 1;

/// Runtime configuration for the Codex app-server source.
#[derive(Debug, Clone)]
pub struct AppServerConfig {
    discovery: DiscoveryOptions,
    app_version: String,
    request_timeout: Duration,
    initial_restart_delay: Duration,
    maximum_restart_delay: Duration,
    max_consecutive_failures: u8,
}

impl AppServerConfig {
    /// Creates the production source policy around explicit discovery inputs.
    #[must_use]
    pub fn new(discovery: DiscoveryOptions, app_version: impl Into<String>) -> Self {
        Self {
            discovery,
            app_version: app_version.into(),
            request_timeout: REQUEST_TIMEOUT,
            initial_restart_delay: INITIAL_RESTART_DELAY,
            maximum_restart_delay: MAXIMUM_RESTART_DELAY,
            max_consecutive_failures: MAX_CONSECUTIVE_FAILURES,
        }
    }

    /// Replaces timing policy for deterministic process-lifecycle tests.
    #[doc(hidden)]
    #[must_use]
    pub fn testing(
        mut self,
        request_timeout: Duration,
        initial_restart_delay: Duration,
        maximum_restart_delay: Duration,
        max_consecutive_failures: u8,
    ) -> Self {
        self.request_timeout = request_timeout;
        self.initial_restart_delay = initial_restart_delay;
        self.maximum_restart_delay = maximum_restart_delay;
        self.max_consecutive_failures = max_consecutive_failures;
        self
    }
}

/// Failure to control a running app-server source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AppServerControlError {
    /// A retry request is already queued.
    #[error("Codex app-server retry is already queued")]
    Busy,
    /// The source has stopped.
    #[error("Codex app-server source is not running")]
    Closed,
}

/// Cloneable PID observation and explicit retry handle.
#[derive(Clone)]
pub struct AppServerControl {
    pid: watch::Receiver<Option<u32>>,
    retry: mpsc::Sender<()>,
}

impl AppServerControl {
    /// Returns the currently owned child PID, if any.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        *self.pid.borrow()
    }

    /// Subscribes to child PID changes for diagnostics and shutdown tests.
    #[must_use]
    pub fn pid_receiver(&self) -> watch::Receiver<Option<u32>> {
        self.pid.clone()
    }

    /// Resumes discovery or restart after a stopped source.
    ///
    /// # Errors
    ///
    /// Returns an error if one retry is already queued or the source stopped.
    pub fn retry(&self) -> Result<(), AppServerControlError> {
        self.retry.try_send(()).map_err(|error| match error {
            mpsc::error::TrySendError::Full(()) => AppServerControlError::Busy,
            mpsc::error::TrySendError::Closed(()) => AppServerControlError::Closed,
        })
    }
}

/// Fatal failure of the app-server source owner.
#[derive(Debug, Error)]
pub enum AppServerSourceError {
    /// The central source-event receiver stopped unexpectedly.
    #[error("usage event channel is closed")]
    EventChannelClosed,
}

/// Owns discovery, one child at a time, and bounded restart state.
pub struct AppServerSource {
    config: AppServerConfig,
    scheduler: Scheduler,
    pid: watch::Sender<Option<u32>>,
    retry: mpsc::Receiver<()>,
    retry_closed: bool,
}

impl AppServerSource {
    /// Creates an unspawned source and its control handle.
    #[must_use]
    pub fn new(config: AppServerConfig, scheduler: Scheduler) -> (Self, AppServerControl) {
        let (pid, pid_rx) = watch::channel(None);
        let (retry_tx, retry) = mpsc::channel(RETRY_CAPACITY);
        let source = Self {
            config,
            scheduler,
            pid,
            retry,
            retry_closed: false,
        };
        let control = AppServerControl {
            pid: pid_rx,
            retry: retry_tx,
        };
        (source, control)
    }

    /// Runs until cancellation, with at most one owned child process.
    ///
    /// # Errors
    ///
    /// Returns an error only when the central event channel closes.
    pub async fn run(
        mut self,
        events: mpsc::Sender<SourceEvent>,
        cancel: CancellationToken,
    ) -> Result<(), AppServerSourceError> {
        let mut failures = 0_u8;
        loop {
            let discovered = tokio::select! {
                biased;
                () = cancel.cancelled() => return Ok(()),
                result = discover(&self.config.discovery) => result,
            };
            let binary = match discovered {
                Ok(Some(binary)) => binary,
                Ok(None) => {
                    self.emit_status(
                        &events,
                        ConnectionStatus::NotConfigured {
                            hint: "Install Codex CLI or set its path in Settings".to_owned(),
                        },
                        &cancel,
                    )
                    .await?;
                    if !self.wait_for_retry(&cancel).await {
                        return Ok(());
                    }
                    failures = 0;
                    continue;
                }
                Err(error) => {
                    warn!(error = %log_trunc(&error.to_string()), "Codex CLI discovery failed");
                    self.emit_status(
                        &events,
                        ConnectionStatus::Error {
                            message: "Codex CLI discovery failed; retrying".to_owned(),
                        },
                        &cancel,
                    )
                    .await?;
                    if !self.after_failure(&cancel, &mut failures).await {
                        return Ok(());
                    }
                    continue;
                }
            };

            let result = run_attempt(
                &binary,
                events.clone(),
                cancel.child_token(),
                self.scheduler.clone(),
                &self.config.app_version,
                self.config.request_timeout,
                &self.pid,
            )
            .await;
            match result {
                Ok(AttemptResult::Cancelled) => return Ok(()),
                Ok(AttemptResult::Unsupported(reason)) => {
                    self.emit_status(&events, ConnectionStatus::Unsupported { reason }, &cancel)
                        .await?;
                    if !self.wait_for_retry(&cancel).await {
                        return Ok(());
                    }
                    failures = 0;
                }
                Ok(AttemptResult::Stopped { became_ready }) => {
                    if became_ready {
                        failures = 0;
                    }
                    self.emit_degraded(&events, &cancel).await?;
                    if !self.after_failure(&cancel, &mut failures).await {
                        return Ok(());
                    }
                }
                Err(AttemptError::EventChannelClosed) => {
                    return Err(AppServerSourceError::EventChannelClosed);
                }
                Err(error) => {
                    warn!(error = %log_trunc(&error.to_string()), "Codex app-server attempt failed");
                    self.emit_degraded(&events, &cancel).await?;
                    if !self.after_failure(&cancel, &mut failures).await {
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn emit_degraded(
        &self,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerSourceError> {
        self.emit_status(
            events,
            ConnectionStatus::Degraded {
                reason: "Codex app-server stopped; using session files".to_owned(),
            },
            cancel,
        )
        .await
    }

    async fn emit_status(
        &self,
        events: &mpsc::Sender<SourceEvent>,
        status: ConnectionStatus,
        cancel: &CancellationToken,
    ) -> Result<(), AppServerSourceError> {
        send_event(
            events,
            SourceEvent::Status {
                provider: Provider::Codex,
                source: SourceKind::CodexAppServer,
                status,
            },
            cancel,
        )
        .await
        .map_err(|_| AppServerSourceError::EventChannelClosed)
    }

    async fn after_failure(&mut self, cancel: &CancellationToken, failures: &mut u8) -> bool {
        *failures = failures.saturating_add(1);
        if *failures >= self.config.max_consecutive_failures {
            let resumed = self.wait_for_retry(cancel).await;
            if resumed {
                *failures = 0;
            }
            return resumed;
        }
        let delay = restart_delay(
            self.config.initial_restart_delay,
            self.config.maximum_restart_delay,
            *failures,
        );
        match self.wait(delay, cancel).await {
            WaitResult::Cancelled => false,
            WaitResult::Elapsed => true,
            WaitResult::Retry => {
                *failures = 0;
                true
            }
        }
    }

    async fn wait_for_retry(&mut self, cancel: &CancellationToken) -> bool {
        loop {
            if self.retry_closed {
                cancel.cancelled().await;
                return false;
            }
            tokio::select! {
                biased;
                () = cancel.cancelled() => return false,
                retry = self.retry.recv() => {
                    if retry.is_some() {
                        return true;
                    }
                    self.retry_closed = true;
                }
            }
        }
    }

    async fn wait(&mut self, delay: Duration, cancel: &CancellationToken) -> WaitResult {
        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => return WaitResult::Cancelled,
                retry = self.retry.recv(), if !self.retry_closed => {
                    if retry.is_some() {
                        return WaitResult::Retry;
                    }
                    self.retry_closed = true;
                }
                () = &mut sleep => return WaitResult::Elapsed,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitResult {
    Cancelled,
    Elapsed,
    Retry,
}

fn restart_delay(initial: Duration, maximum: Duration, failures: u8) -> Duration {
    let exponent = u32::from(failures.saturating_sub(1)).min(31);
    initial.saturating_mul(1_u32 << exponent).min(maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_backoff_doubles_and_caps_at_five_minutes() {
        assert_eq!(
            restart_delay(INITIAL_RESTART_DELAY, MAXIMUM_RESTART_DELAY, 1),
            Duration::from_secs(5)
        );
        assert_eq!(
            restart_delay(INITIAL_RESTART_DELAY, MAXIMUM_RESTART_DELAY, 2),
            Duration::from_secs(10)
        );
        assert_eq!(
            restart_delay(
                INITIAL_RESTART_DELAY,
                MAXIMUM_RESTART_DELAY,
                MAX_CONSECUTIVE_FAILURES
            ),
            Duration::from_mins(5)
        );
    }
}
