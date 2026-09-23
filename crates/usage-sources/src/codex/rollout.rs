//! Codex rollout-file scan, watch, tail, and normalization source.

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use thiserror::Error;
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use usage_core::{ConnectionStatus, Provider, SourceKind, parse::CodexRolloutParser};

use crate::{
    SourceEvent,
    recent::{MAX_TRACKED_FILES, is_recent_file, recent_jsonl},
    scheduler::Scheduler,
    tail::{TailError, Tailer},
    watch::{DirectoryWatcher, WatchError, WatchFilter, path_channel},
};

const TOKEN_COUNT_MARKER: &[u8] = b"\"token_count\"";

/// Filesystem location used by the Codex rollout source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloutConfig {
    /// `<codex_home>/sessions` root derived by [`crate::paths::Paths`].
    pub sessions_root: PathBuf,
}

impl RolloutConfig {
    /// Creates a source configuration for one derived sessions root.
    #[must_use]
    pub fn new(sessions_root: impl Into<PathBuf>) -> Self {
        Self {
            sessions_root: sessions_root.into(),
        }
    }
}

/// Fatal setup or delivery failure for the rollout source.
#[derive(Debug, Error)]
pub enum RolloutSourceError {
    /// The session root could not be inspected.
    #[error("could not inspect Codex sessions root: {0}")]
    Root(#[source] io::Error),
    /// The recursive native watcher could not start.
    #[error(transparent)]
    Watch(#[from] WatchError),
    /// The central source-event receiver stopped unexpectedly.
    #[error("usage event channel is closed")]
    EventChannelClosed,
}

/// Owns bounded per-file parser state and a native recursive watcher.
pub struct RolloutSource {
    config: RolloutConfig,
    scheduler: Scheduler,
}

impl RolloutSource {
    /// Creates an unspawned rollout source.
    #[must_use]
    pub const fn new(config: RolloutConfig, scheduler: Scheduler) -> Self {
        Self { config, scheduler }
    }

    /// Scans recent files once, then tails debounced create/modify events.
    ///
    /// # Errors
    ///
    /// Returns a typed setup or event-channel failure. Per-file I/O and parse
    /// failures become source status events and do not stop other files.
    pub async fn run(
        self,
        events: mpsc::Sender<SourceEvent>,
        cancel: CancellationToken,
    ) -> Result<(), RolloutSourceError> {
        let metadata = match tokio::fs::metadata(&self.config.sessions_root).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                emit_status(
                    &events,
                    ConnectionStatus::NotConfigured {
                        hint: "Run Codex CLI once to create session history".to_owned(),
                    },
                    &cancel,
                )
                .await?;
                return Ok(());
            }
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(RolloutSourceError::Root(error));
            }
        };
        if !metadata.is_dir() {
            emit_error(&events, &cancel).await?;
            return Err(RolloutSourceError::Root(io::Error::new(
                io::ErrorKind::NotADirectory,
                "sessions root is not a directory",
            )));
        }
        let sessions_root = match tokio::fs::canonicalize(&self.config.sessions_root).await {
            Ok(path) => path,
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(RolloutSourceError::Root(error));
            }
        };

        let (path_tx, mut paths) = path_channel();
        let watcher = match DirectoryWatcher::new(&sessions_root, WatchFilter::JsonLines, path_tx) {
            Ok(watcher) => watcher,
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(error.into());
            }
        };
        let mut processor = RolloutProcessor::new();
        let initial =
            match recent_jsonl(&sessions_root, SystemTime::now(), &cancel, "Codex rollout").await {
                Ok(initial) => initial,
                Err(error) => {
                    emit_error(&events, &cancel).await?;
                    return Err(RolloutSourceError::Root(error));
                }
            };
        for path in initial {
            if cancel.is_cancelled() {
                return Ok(());
            }
            process_or_report(&mut processor, &path, &events, &cancel).await?;
        }

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
                    send_event(
                        &events,
                        SourceEvent::Activity { provider: Provider::Codex },
                        &cancel,
                    )
                    .await?;
                    process_or_report(&mut processor, &path, &events, &cancel).await?;
                    if let Err(error) = self.scheduler.trigger(SourceKind::CodexAppServer) {
                        warn!(%error, "could not trigger Codex app-server after rollout activity");
                    }
                }
            }
        }
    }
}

struct TrackedParser {
    parser: CodexRolloutParser,
    last_seen: Instant,
}

struct RolloutProcessor {
    tailer: Tailer,
    parsers: BTreeMap<PathBuf, TrackedParser>,
}

impl RolloutProcessor {
    fn new() -> Self {
        Self {
            tailer: Tailer::new(),
            parsers: BTreeMap::new(),
        }
    }

    async fn process(
        &mut self,
        path: &Path,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), ProcessError> {
        if !is_recent_file(path, SystemTime::now()).await? {
            return Ok(());
        }
        self.track(path);
        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }
            let batch = self.tailer.read_new(path).await?;
            let Some(tracked) = self.parsers.get_mut(path) else {
                return Ok(());
            };
            tracked.last_seen = Instant::now();
            if batch.reset {
                tracked.parser = CodexRolloutParser::new();
            }
            let mut tokens = Vec::with_capacity(batch.lines.len());
            let mut parse_failed = false;
            for line in batch
                .lines
                .into_iter()
                .filter(|line| contains(line, TOKEN_COUNT_MARKER))
            {
                if cancel.is_cancelled() {
                    return Ok(());
                }
                let text = match std::str::from_utf8(&line) {
                    Ok(text) => text,
                    Err(error) => {
                        warn!(error = %usage_core::log_trunc(&error.to_string()), "invalid UTF-8 in Codex rollout line");
                        parse_failed = true;
                        continue;
                    }
                };
                match tracked.parser.parse_line(text) {
                    Ok(parsed) => {
                        if let Some(limit_id) = parsed.filtered_limit_id {
                            debug!(%limit_id, "ignoring non-Codex rollout rate limit");
                        }
                        if let Some(reading) = parsed.reading {
                            send_event(events, SourceEvent::Reading(reading), cancel)
                                .await
                                .map_err(|_| ProcessError::EventChannelClosed)?;
                        }
                        if let Some(token) = parsed.token {
                            tokens.push(token);
                        }
                    }
                    Err(error) => {
                        warn!(error = %usage_core::log_trunc(&error.to_string()), "invalid Codex rollout record");
                        parse_failed = true;
                    }
                }
            }
            if !tokens.is_empty() {
                send_event(events, SourceEvent::Tokens(tokens), cancel)
                    .await
                    .map_err(|_| ProcessError::EventChannelClosed)?;
            }
            if parse_failed {
                emit_error(events, cancel)
                    .await
                    .map_err(|_| ProcessError::EventChannelClosed)?;
            }
            if !batch.has_more {
                return Ok(());
            }
        }
    }

    fn track(&mut self, path: &Path) {
        if self.parsers.contains_key(path) {
            return;
        }
        if self.parsers.len() == MAX_TRACKED_FILES
            && let Some(oldest) = self
                .parsers
                .iter()
                .min_by_key(|(_, tracked)| tracked.last_seen)
                .map(|(path, _)| path.clone())
        {
            self.parsers.remove(&oldest);
            self.tailer.forget(&oldest);
        }
        self.parsers.insert(
            path.to_path_buf(),
            TrackedParser {
                parser: CodexRolloutParser::new(),
                last_seen: Instant::now(),
            },
        );
    }
}

#[derive(Debug, Error)]
enum ProcessError {
    #[error(transparent)]
    Recent(#[from] io::Error),
    #[error(transparent)]
    Tail(#[from] TailError),
    #[error("usage event channel is closed")]
    EventChannelClosed,
}

async fn process_or_report(
    processor: &mut RolloutProcessor,
    path: &Path,
    events: &mpsc::Sender<SourceEvent>,
    cancel: &CancellationToken,
) -> Result<(), RolloutSourceError> {
    match processor.process(path, events, cancel).await {
        Ok(()) => Ok(()),
        Err(ProcessError::EventChannelClosed) => Err(RolloutSourceError::EventChannelClosed),
        Err(error) => {
            warn!(error = %usage_core::log_trunc(&error.to_string()), "Codex rollout file processing failed");
            emit_error(events, cancel).await
        }
    }
}

async fn emit_error(
    events: &mpsc::Sender<SourceEvent>,
    cancel: &CancellationToken,
) -> Result<(), RolloutSourceError> {
    emit_status(
        events,
        ConnectionStatus::Error {
            message: "Codex session history read failed".to_owned(),
        },
        cancel,
    )
    .await
}

async fn emit_status(
    events: &mpsc::Sender<SourceEvent>,
    status: ConnectionStatus,
    cancel: &CancellationToken,
) -> Result<(), RolloutSourceError> {
    send_event(
        events,
        SourceEvent::Status {
            provider: Provider::Codex,
            source: SourceKind::CodexRollout,
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
) -> Result<(), RolloutSourceError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Ok(()),
        result = events.send(event) => result.map_err(|_| RolloutSourceError::EventChannelClosed),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
