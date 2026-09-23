//! Recursive filesystem watching with bounded channels and per-path debounce.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::log_trunc;

/// Capacity of both raw callback events and consumer-facing path events.
pub const PATH_EVENT_CAPACITY: usize = 256;
const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

/// Selects files relevant to one concrete source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchFilter {
    /// Codex and Claude local history files.
    JsonLines,
    /// One exact file, such as the Claude bridge output.
    Exact(PathBuf),
}

impl WatchFilter {
    fn matches(&self, path: &Path) -> bool {
        match self {
            Self::JsonLines => path.extension() == Some(OsStr::new("jsonl")),
            Self::Exact(expected) => path == expected,
        }
    }

    fn normalize(self) -> io::Result<Self> {
        match self {
            Self::JsonLines => Ok(Self::JsonLines),
            Self::Exact(path) => normalize_future_path(&path).map(Self::Exact),
        }
    }
}

/// Failure to create or register a native filesystem watcher.
#[derive(Debug, Error)]
pub enum WatchError {
    /// The platform watcher could not be constructed.
    #[error("could not create filesystem watcher: {0}")]
    Create(#[source] notify::Error),
    /// The requested root could not be watched.
    #[error("could not watch {path}: {source}")]
    Register {
        /// Root that failed registration.
        path: PathBuf,
        /// Platform watcher error.
        #[source]
        source: notify::Error,
    },
    /// A watched or matched path could not be normalized.
    #[error("could not resolve watcher path {path}: {source}")]
    Resolve {
        /// Path that failed normalization.
        path: PathBuf,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
}

/// Creates the standard bounded consumer-facing path channel.
#[must_use]
pub fn path_channel() -> (mpsc::Sender<PathBuf>, mpsc::Receiver<PathBuf>) {
    mpsc::channel(PATH_EVENT_CAPACITY)
}

/// Owns a registered recursive watcher until [`Self::run`] exits.
pub struct DirectoryWatcher {
    watcher: RecommendedWatcher,
    raw_events: mpsc::Receiver<PathBuf>,
    output: mpsc::Sender<PathBuf>,
}

impl DirectoryWatcher {
    /// Registers a recursive watcher before returning, so callers may safely
    /// create files immediately after construction.
    ///
    /// # Errors
    ///
    /// Returns [`WatchError`] if the platform watcher cannot be created or the
    /// root cannot be registered.
    pub fn new(
        root: impl Into<PathBuf>,
        filter: WatchFilter,
        output: mpsc::Sender<PathBuf>,
    ) -> Result<Self, WatchError> {
        let root = root.into();
        let root = std::fs::canonicalize(&root)
            .map_err(|source| WatchError::Resolve { path: root, source })?;
        let filter_path = match &filter {
            WatchFilter::Exact(path) => Some(path.clone()),
            WatchFilter::JsonLines => None,
        };
        let filter = filter.normalize().map_err(|source| WatchError::Resolve {
            path: filter_path.unwrap_or_else(|| root.clone()),
            source,
        })?;
        let (raw_tx, raw_events) = mpsc::channel(PATH_EVENT_CAPACITY);
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<Event>| match result {
            Ok(event) => forward_relevant(&raw_tx, &filter, event),
            Err(error) => {
                warn!(error = %log_trunc(&error.to_string()), "filesystem watcher callback failed");
            }
        })
            .map_err(WatchError::Create)?;
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|source| WatchError::Register { path: root, source })?;
        Ok(Self {
            watcher,
            raw_events,
            output,
        })
    }

    /// Coalesces bursts per path and forwards them until cancellation.
    pub async fn run(mut self, cancel: CancellationToken) {
        let mut pending = BTreeMap::<PathBuf, Instant>::new();

        loop {
            let _watcher_guard = &self.watcher;
            if let Some(deadline) = pending.values().copied().min() {
                tokio::select! {
                    () = cancel.cancelled() => return,
                    event = self.raw_events.recv() => {
                        let Some(path) = event else { return; };
                        remember(&mut pending, path);
                    }
                    () = tokio::time::sleep_until(deadline) => {
                        if !emit_due(&mut pending, &self.output, &cancel).await {
                            return;
                        }
                    }
                }
            } else {
                tokio::select! {
                    () = cancel.cancelled() => return,
                    event = self.raw_events.recv() => {
                        let Some(path) = event else { return; };
                        remember(&mut pending, path);
                    }
                }
            }
        }
    }
}

fn forward_relevant(sender: &mpsc::Sender<PathBuf>, filter: &WatchFilter, event: Event) {
    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
        return;
    }
    for path in event.paths.into_iter().filter(|path| filter.matches(path)) {
        if let Err(error) = sender.try_send(path) {
            match error {
                mpsc::error::TrySendError::Full(path) => {
                    warn!(path = %log_trunc(&path.to_string_lossy()), "filesystem event queue is full; dropping path");
                }
                mpsc::error::TrySendError::Closed(_) => return,
            }
        }
    }
}

fn normalize_future_path(path: &Path) -> io::Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path);
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    Ok(std::fs::canonicalize(parent)?.join(file_name))
}

fn remember(pending: &mut BTreeMap<PathBuf, Instant>, path: PathBuf) {
    if pending.contains_key(&path) || pending.len() < PATH_EVENT_CAPACITY {
        pending.insert(path, Instant::now() + DEBOUNCE_DURATION);
    } else {
        warn!(path = %log_trunc(&path.to_string_lossy()), "filesystem debounce set is full; dropping path");
    }
}

async fn emit_due(
    pending: &mut BTreeMap<PathBuf, Instant>,
    output: &mpsc::Sender<PathBuf>,
    cancel: &CancellationToken,
) -> bool {
    let now = Instant::now();
    let due: Vec<_> = pending
        .iter()
        .filter(|(_, deadline)| **deadline <= now)
        .map(|(path, _)| path.clone())
        .collect();
    for path in due {
        pending.remove(&path);
        tokio::select! {
            () = cancel.cancelled() => return false,
            result = output.send(path) => {
                if result.is_err() {
                    return false;
                }
            }
        }
    }
    true
}
