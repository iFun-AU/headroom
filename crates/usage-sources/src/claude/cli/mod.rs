//! Manual Claude CLI refreshes, serialized across app windows and cancelled on exit.

mod probe;
mod screen;

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use usage_core::{Reading, parse::ClaudeCliError};

pub use probe::capture;

/// All filesystem and process inputs for one isolated CLI query.
#[derive(Debug, Clone)]
pub struct ProbeConfig {
    /// Resolved executable, invoked directly without a shell.
    pub binary: PathBuf,
    /// Explicit configuration override. None preserves Claude's default sign-in profile;
    /// setting even the default directory explicitly changes Claude's auth selection.
    pub claude_dir: Option<PathBuf>,
    /// App-owned, empty directory for the probe, outside any user project.
    pub working_directory: PathBuf,
    /// Maximum duration of startup and capture together.
    pub timeout: Duration,
}

/// Safe public failures; never contains terminal output or credential material.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CliError {
    /// No Claude executable was found.
    #[error("Claude Code was not found. Install it and sign in once, then refresh again.")]
    NotInstalled,
    /// Another window already owns the active refresh.
    #[error("Claude usage is already refreshing.")]
    Busy,
    /// A prior attempt is still within its minimum retry interval.
    #[error("Wait {0} seconds before refreshing Claude again.")]
    Cooldown(u64),
    /// Startup, PTY creation or I/O failed.
    #[error("Could not run Claude Code. Check that it opens normally in a terminal.")]
    Process,
    /// The bounded capture produced no complete usage panel.
    #[error("Claude usage refresh timed out. Open Claude Code and check /usage, then retry.")]
    Timeout,
    /// An unexpected trust dialog cannot safely be answered automatically.
    #[error("Claude Code needs setup. Open it once and complete setup, then retry.")]
    Setup,
    /// Application exit or configuration changes cancelled the request.
    #[error("Claude usage refresh was cancelled.")]
    Cancelled,
    /// Terminal output exceeded the capture budget.
    #[error("Claude Code returned too much output for a usage refresh.")]
    OutputLimit,
    /// A recognized provider failure.
    #[error(transparent)]
    Provider(#[from] ClaudeCliError),
}

struct Request {
    config: ProbeConfig,
    reply: oneshot::Sender<Result<Reading, CliError>>,
    _permit: OwnedSemaphorePermit,
}

/// Cloneable entry point for manual refresh. Never starts work on its own.
#[derive(Clone)]
pub struct CliRefresh {
    requests: mpsc::Sender<Request>,
    slot: Arc<Semaphore>,
}

impl CliRefresh {
    /// Creates the client and its lifecycle-owned actor.
    #[must_use]
    pub fn channel() -> (Self, CliRefreshActor) {
        let (requests, incoming) = mpsc::channel(1);
        (
            Self {
                requests,
                slot: Arc::new(Semaphore::new(1)),
            },
            CliRefreshActor { incoming },
        )
    }

    /// Completes only after the owned process has been cleaned up.
    ///
    /// # Errors
    /// Returns a safe diagnostic on busy, cooldown, cancellation, or provider failure.
    pub async fn refresh(&self, config: ProbeConfig) -> Result<Reading, CliError> {
        let permit = self
            .slot
            .clone()
            .try_acquire_owned()
            .map_err(|_| CliError::Busy)?;
        let (reply, received) = oneshot::channel();
        self.requests
            .send(Request {
                config,
                reply,
                _permit: permit,
            })
            .await
            .map_err(|_| CliError::Cancelled)?;
        received.await.map_err(|_| CliError::Cancelled)?
    }
}

/// Serial worker owned by the application supervisor, including during shutdown.
pub struct CliRefreshActor {
    incoming: mpsc::Receiver<Request>,
}

impl CliRefreshActor {
    /// Runs manual requests with a 30-second floor and five-minute rate-limit cooldown.
    pub async fn run(mut self, cancel: CancellationToken) {
        let mut next_allowed = Instant::now();
        loop {
            let request = tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                request = self.incoming.recv() => match request { Some(request) => request, None => return },
            };
            if next_allowed > Instant::now() {
                let seconds = next_allowed
                    .saturating_duration_since(Instant::now())
                    .as_secs()
                    + 1;
                let _ = request.reply.send(Err(CliError::Cooldown(seconds)));
                continue;
            }
            let token = cancel.child_token();
            let result = tokio::task::spawn_blocking(move || capture(&request.config, &token))
                .await
                .unwrap_or(Err(CliError::Process));
            let delay = if matches!(result, Err(CliError::Provider(ClaudeCliError::RateLimited))) {
                300
            } else {
                30
            };
            next_allowed = Instant::now() + Duration::from_secs(delay);
            let _ = request.reply.send(result);
            // The permit remains owned by this request until capture and cleanup finish,
            // even if the caller closed its window or dropped its response future.
        }
    }
}
