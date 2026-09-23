//! Claude Code local JSONL history scan and watch source.

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use thiserror::Error;
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::{
    ConnectionStatus, Provider, SourceKind,
    parse::{ClaudeLogRecord, ClaudeSession, parse_claude_log_record},
};

use crate::{
    SourceEvent,
    paths::Paths,
    recent::{MAX_TRACKED_FILES, is_recent_file, recent_jsonl},
    tail::{TailError, Tailer},
    watch::{DirectoryWatcher, WatchError, WatchFilter, path_channel},
};

use super::effectiveness::EffectivenessHandle;

const USAGE_MARKER: &[u8] = b"\"usage\"";
const ASSISTANT_MARKER: &[u8] = b"\"assistant\"";

/// Claude local-project history root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySourceConfig {
    /// `<claude_dir>/projects` root derived by [`Paths`].
    pub projects_root: PathBuf,
}

impl HistorySourceConfig {
    /// Creates an explicit source configuration.
    #[must_use]
    pub fn new(projects_root: impl Into<PathBuf>) -> Self {
        Self {
            projects_root: projects_root.into(),
        }
    }

    /// Uses centralized application paths.
    #[must_use]
    pub fn from_paths(paths: &Paths) -> Self {
        Self::new(&paths.claude_projects)
    }
}

/// Fatal setup or event-delivery failure for Claude local history.
#[derive(Debug, Error)]
pub enum HistorySourceError {
    /// The projects root could not be inspected or scanned.
    #[error("could not inspect Claude projects root: {0}")]
    Root(#[source] io::Error),
    /// The native watcher could not start.
    #[error(transparent)]
    Watch(#[from] WatchError),
    /// The central source-event receiver stopped unexpectedly.
    #[error("usage event channel is closed")]
    EventChannelClosed,
}

/// Owns bounded file offsets and watches local Claude conversation history.
pub struct HistorySource {
    config: HistorySourceConfig,
    effectiveness: EffectivenessHandle,
}

impl HistorySource {
    /// Creates an unspawned history source.
    #[must_use]
    pub const fn new(config: HistorySourceConfig, effectiveness: EffectivenessHandle) -> Self {
        Self {
            config,
            effectiveness,
        }
    }

    /// Scans recent files once, then tails debounced JSONL changes.
    ///
    /// # Errors
    ///
    /// Returns setup or event-channel failures. Per-file I/O and parse errors
    /// become source status events and remain retryable.
    pub async fn run(
        self,
        events: mpsc::Sender<SourceEvent>,
        cancel: CancellationToken,
    ) -> Result<(), HistorySourceError> {
        let metadata = match tokio::fs::metadata(&self.config.projects_root).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                emit_status(
                    &events,
                    ConnectionStatus::NotConfigured {
                        hint: "Run Claude Code once to create local history".to_owned(),
                    },
                    &cancel,
                )
                .await?;
                return Ok(());
            }
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(HistorySourceError::Root(error));
            }
        };
        if !metadata.is_dir() {
            emit_error(&events, &cancel).await?;
            return Err(HistorySourceError::Root(io::Error::new(
                io::ErrorKind::NotADirectory,
                "projects root is not a directory",
            )));
        }
        let root = match tokio::fs::canonicalize(&self.config.projects_root).await {
            Ok(root) => root,
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(HistorySourceError::Root(error));
            }
        };

        let (path_tx, mut paths) = path_channel();
        let watcher = match DirectoryWatcher::new(&root, WatchFilter::JsonLines, path_tx) {
            Ok(watcher) => watcher,
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(error.into());
            }
        };
        let mut processor = HistoryProcessor::new(self.effectiveness.clone());
        let initial = match recent_jsonl(&root, SystemTime::now(), &cancel, "Claude history").await
        {
            Ok(initial) => initial,
            Err(error) => {
                emit_error(&events, &cancel).await?;
                return Err(HistorySourceError::Root(error));
            }
        };
        for path in initial {
            if cancel.is_cancelled() {
                return Ok(());
            }
            process_or_report(&mut processor, &path, false, &events, &cancel).await?;
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
                    process_or_report(&mut processor, &path, true, &events, &cancel).await?;
                }
            }
        }
    }
}

struct HistoryProcessor {
    tailer: Tailer,
    last_seen: BTreeMap<PathBuf, Instant>,
    effectiveness: EffectivenessHandle,
}

impl HistoryProcessor {
    fn new(effectiveness: EffectivenessHandle) -> Self {
        Self {
            tailer: Tailer::new(),
            last_seen: BTreeMap::new(),
            effectiveness,
        }
    }

    async fn process(
        &mut self,
        path: &Path,
        emit_activity: bool,
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
            self.last_seen.insert(path.to_path_buf(), Instant::now());
            let mut records = Vec::with_capacity(batch.lines.len());
            let mut parse_failed = false;
            for line in batch.lines.into_iter().filter(|line| relevant(line)) {
                if cancel.is_cancelled() {
                    return Ok(());
                }
                match std::str::from_utf8(&line) {
                    Ok(text) => match parse_claude_log_record(text) {
                        Ok(Some(record)) => records.push(record),
                        Ok(None) => {}
                        Err(error) => {
                            warn!(error = %usage_core::log_trunc(&error.to_string()), "invalid Claude history record");
                            parse_failed = true;
                        }
                    },
                    Err(error) => {
                        warn!(error = %usage_core::log_trunc(&error.to_string()), "invalid UTF-8 in Claude history");
                        parse_failed = true;
                    }
                }
            }
            self.emit_tokens(records, emit_activity, events, cancel)
                .await?;
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

    async fn emit_tokens(
        &self,
        records: Vec<ClaudeLogRecord>,
        emit_activity: bool,
        events: &mpsc::Sender<SourceEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), ProcessError> {
        if records.is_empty() {
            return Ok(());
        }
        // Latest activity per session kind; a batch normally comes from one file.
        let activity = [ClaudeSession::Headless, ClaudeSession::Interactive].map(|session| {
            let latest = records
                .iter()
                .filter(|record| record.session == session)
                .map(|record| record.event.at)
                .max();
            (session, latest)
        });
        let tokens = records.into_iter().map(|record| record.event).collect();
        send_event(events, SourceEvent::Tokens(tokens), cancel)
            .await
            .map_err(|_| ProcessError::EventChannelClosed)?;
        if emit_activity {
            send_event(
                events,
                SourceEvent::Activity {
                    provider: Provider::Claude,
                },
                cancel,
            )
            .await
            .map_err(|_| ProcessError::EventChannelClosed)?;
        }
        for (session, at) in activity {
            if let Some(at) = at
                && let Err(error) = self.effectiveness.assistant_activity(at, session).await
            {
                warn!(%error, "could not record Claude bridge effectiveness activity");
            }
        }
        Ok(())
    }

    fn track(&mut self, path: &Path) {
        if self.last_seen.contains_key(path) {
            return;
        }
        if self.last_seen.len() == MAX_TRACKED_FILES
            && let Some(oldest) = self
                .last_seen
                .iter()
                .min_by_key(|(_, seen)| *seen)
                .map(|(path, _)| path.clone())
        {
            self.last_seen.remove(&oldest);
            self.tailer.forget(&oldest);
        }
        self.last_seen.insert(path.to_path_buf(), Instant::now());
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
    processor: &mut HistoryProcessor,
    path: &Path,
    emit_activity: bool,
    events: &mpsc::Sender<SourceEvent>,
    cancel: &CancellationToken,
) -> Result<(), HistorySourceError> {
    match processor.process(path, emit_activity, events, cancel).await {
        Ok(()) => Ok(()),
        Err(ProcessError::EventChannelClosed) => Err(HistorySourceError::EventChannelClosed),
        Err(error) => {
            warn!(error = %usage_core::log_trunc(&error.to_string()), "Claude history file processing failed");
            emit_error(events, cancel).await
        }
    }
}

async fn emit_error(
    events: &mpsc::Sender<SourceEvent>,
    cancel: &CancellationToken,
) -> Result<(), HistorySourceError> {
    emit_status(
        events,
        ConnectionStatus::Error {
            message: "Claude local history could not be read".to_owned(),
        },
        cancel,
    )
    .await
}

async fn emit_status(
    events: &mpsc::Sender<SourceEvent>,
    status: ConnectionStatus,
    cancel: &CancellationToken,
) -> Result<(), HistorySourceError> {
    send_event(
        events,
        SourceEvent::Status {
            provider: Provider::Claude,
            source: SourceKind::ClaudeLocalLogs,
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
) -> Result<(), HistorySourceError> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Ok(()),
        result = events.send(event) => result.map_err(|_| HistorySourceError::EventChannelClosed),
    }
}

fn relevant(line: &[u8]) -> bool {
    contains(line, USAGE_MARKER) && contains(line, ASSISTANT_MARKER)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
