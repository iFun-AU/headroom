//! Native watcher source for atomic Claude bridge envelopes.

use std::{io, path::PathBuf};

use thiserror::Error;
use tokio::{io::AsyncReadExt, sync::mpsc};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::{
    ConnectionStatus, Provider, SourceKind, UnixSeconds, parse::parse_claude_statusline,
};

use crate::{
    SourceEvent,
    paths::Paths,
    watch::{DirectoryWatcher, WatchError, WatchFilter, path_channel},
};

use super::effectiveness::EffectivenessHandle;

const MAX_BRIDGE_FILE_BYTES: u64 = 4 * 1_024 * 1_024;

/// Paths and installation epoch for the Claude bridge source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeSourceConfig {
    /// Directory watched for atomic target replacement.
    pub app_support: PathBuf,
    /// Exact bridge envelope path.
    pub rate_limits_file: PathBuf,
    /// Current install timestamp; `None` means the bridge is disabled.
    pub installed_at: Option<UnixSeconds>,
}

impl BridgeSourceConfig {
    /// Creates an explicit source configuration.
    #[must_use]
    pub fn new(
        app_support: impl Into<PathBuf>,
        rate_limits_file: impl Into<PathBuf>,
        installed_at: Option<UnixSeconds>,
    ) -> Self {
        Self {
            app_support: app_support.into(),
            rate_limits_file: rate_limits_file.into(),
            installed_at,
        }
    }

    /// Uses centralized application paths.
    #[must_use]
    pub fn from_paths(paths: &Paths, installed_at: Option<UnixSeconds>) -> Self {
        Self::new(&paths.app_support, &paths.bridge_rate_limits, installed_at)
    }
}

/// Fatal setup or event-delivery failure for the bridge source.
#[derive(Debug, Error)]
pub enum BridgeSourceError {
    /// The app-support directory could not be resolved.
    #[error("could not resolve bridge directory: {0}")]
    Root(#[source] io::Error),
    /// The native watcher could not start.
    #[error(transparent)]
    Watch(#[from] WatchError),
    /// The central source-event receiver stopped unexpectedly.
    #[error("usage event channel is closed")]
    EventChannelClosed,
}

/// Owns the exact-file watcher and sends full Claude limit readings.
pub struct BridgeSource {
    config: BridgeSourceConfig,
    effectiveness: EffectivenessHandle,
}

impl BridgeSource {
    /// Creates an unspawned bridge source.
    #[must_use]
    pub const fn new(config: BridgeSourceConfig, effectiveness: EffectivenessHandle) -> Self {
        Self {
            config,
            effectiveness,
        }
    }

    /// Reads existing evidence once, then handles atomic file changes.
    ///
    /// # Errors
    ///
    /// Returns a typed setup or event-channel failure. Per-write parse and I/O
    /// errors become source status events and remain retryable on the next write.
    pub async fn run(
        self,
        events: mpsc::Sender<SourceEvent>,
        cancel: CancellationToken,
    ) -> Result<(), BridgeSourceError> {
        let Some(installed_at) = self.config.installed_at else {
            emit_status(
                &events,
                ConnectionStatus::NotConfigured {
                    hint: "Enable real-time updates".to_owned(),
                },
                &cancel,
            )
            .await?;
            return Ok(());
        };
        let root = match tokio::fs::canonicalize(&self.config.app_support).await {
            Ok(root) => root,
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(BridgeSourceError::Root(error));
            }
        };
        let target = root.join(
            self.config
                .rate_limits_file
                .file_name()
                .ok_or_else(|| BridgeSourceError::Root(invalid_path()))?,
        );
        let (path_tx, mut paths) = path_channel();
        let watcher =
            match DirectoryWatcher::new(&root, WatchFilter::Exact(target.clone()), path_tx) {
                Ok(watcher) => watcher,
                Err(error) => {
                    emit_error(&events, &cancel).await?;
                    return Err(error.into());
                }
            };

        self.process(&target, installed_at, &events, &cancel)
            .await?;
        let watcher_run = watcher.run(cancel.child_token());
        tokio::pin!(watcher_run);
        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => return Ok(()),
                () = &mut watcher_run => {
                    if !cancel.is_cancelled() {
                        emit_error(&events, &cancel).await?;
                    }
                    return Ok(());
                }
                path = paths.recv() => {
                    let Some(path) = path else {
                        emit_error(&events, &cancel).await?;
                        return Ok(());
                    };
                    self.process(&path, installed_at, &events, &cancel).await?;
                }
            }
        }
    }

    async fn process(
        &self,
        path: &std::path::Path,
        installed_at: UnixSeconds,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), BridgeSourceError> {
        let bytes = match read_bounded(path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                warn!(error = %usage_core::log_trunc(&error.to_string()), "Claude bridge read failed");
                emit_error(events, cancel).await?;
                return Ok(());
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(error) => {
                warn!(error = %usage_core::log_trunc(&error.to_string()), "Claude bridge file is not UTF-8");
                emit_error(events, cancel).await?;
                return Ok(());
            }
        };
        let reading = match parse_claude_statusline(text, unix_now()) {
            Ok(Some(reading)) if reading.observed_at >= installed_at => reading,
            Ok(_) => return Ok(()),
            Err(error) => {
                warn!(error = %usage_core::log_trunc(&error.to_string()), "Claude bridge file is invalid");
                emit_error(events, cancel).await?;
                return Ok(());
            }
        };
        let observed_at = reading.observed_at;
        send_event(events, SourceEvent::Reading(reading), cancel).await?;
        send_event(
            events,
            SourceEvent::Activity {
                provider: Provider::Claude,
            },
            cancel,
        )
        .await?;
        if let Err(error) = self.effectiveness.bridge_written(observed_at).await {
            warn!(%error, "could not record Claude bridge effectiveness");
        }
        Ok(())
    }
}

async fn read_bounded(path: &std::path::Path) -> io::Result<Vec<u8>> {
    let file = tokio::fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    if metadata.len() > MAX_BRIDGE_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "bridge file exceeds 4 MiB",
        ));
    }

    // The metadata check avoids normal oversized reads. `take` also bounds a
    // file that grows between the metadata call and the actual read.
    let mut bytes = Vec::new();
    file.take(MAX_BRIDGE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BRIDGE_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "bridge file exceeds 4 MiB",
        ));
    }
    Ok(bytes)
}

async fn emit_error(
    events: &mpsc::Sender<SourceEvent>,
    cancel: &CancellationToken,
) -> Result<(), BridgeSourceError> {
    emit_status(
        events,
        ConnectionStatus::Error {
            message: "Claude bridge update could not be read".to_owned(),
        },
        cancel,
    )
    .await
}

async fn emit_status(
    events: &mpsc::Sender<SourceEvent>,
    status: ConnectionStatus,
    cancel: &CancellationToken,
) -> Result<(), BridgeSourceError> {
    send_event(
        events,
        SourceEvent::Status {
            provider: Provider::Claude,
            source: SourceKind::ClaudeStatusline,
            status,
        },
        cancel,
    )
    .await
}

async fn send_event(
    events: &mpsc::Sender<SourceEvent>,
    event: SourceEvent,
    cancel: &CancellationToken,
) -> Result<(), BridgeSourceError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Ok(()),
        result = events.send(event) => result.map_err(|_| BridgeSourceError::EventChannelClosed),
    }
}

fn unix_now() -> UnixSeconds {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    UnixSeconds(i64::try_from(seconds).unwrap_or(i64::MAX))
}

fn invalid_path() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "bridge file has no name")
}
