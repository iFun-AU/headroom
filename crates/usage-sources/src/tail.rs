//! Incremental file tailing with a strict per-file partial-line memory bound.

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
};
use tracing::warn;
use usage_core::log_trunc;

/// Maximum retained unterminated line size for each tracked file.
pub const MAX_PARTIAL_LINE_BYTES: usize = 16 * 1024 * 1024;
const READ_CHUNK_BYTES: usize = 64 * 1024;
const MAX_BATCH_BYTES: usize = MAX_PARTIAL_LINE_BYTES + READ_CHUNK_BYTES;
const MAX_BATCH_LINES: usize = 4_096;
const STALE_AFTER: Duration = Duration::from_hours(192);

/// Failure to inspect or read one tailed file.
#[derive(Debug, Error)]
#[error("could not tail {path}: {source}")]
pub struct TailError {
    path: PathBuf,
    #[source]
    source: io::Error,
}

impl TailError {
    fn new(path: &Path, source: io::Error) -> Self {
        Self {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// Complete lines discovered during one incremental read.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TailRead {
    /// Lines without their trailing newline bytes.
    pub lines: Vec<Vec<u8>>,
    /// Unterminated lines discarded after exceeding the configured cap.
    pub skipped_oversized: usize,
    /// More bytes remain and should be read in another bounded call.
    pub has_more: bool,
}

/// Bounded-state diagnostics for tests and operational metrics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TailMetrics {
    /// Number of files with remembered offsets.
    pub tracked_files: usize,
    /// Total bytes currently retained across partial lines.
    pub buffered_bytes: usize,
    /// Number of files currently discarding an oversized line.
    pub discarding_files: usize,
}

#[derive(Debug, Default)]
struct TailState {
    offset: u64,
    partial: Vec<u8>,
    discarding_oversized: bool,
    modified_at: Option<SystemTime>,
    file_identity: Option<FileIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

/// Stateful incremental tailer. One instance may safely track many files.
#[derive(Debug, Default)]
pub struct Tailer {
    files: BTreeMap<PathBuf, TailState>,
}

impl Tailer {
    /// Creates an empty tailer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads only bytes appended since the previous successful call.
    ///
    /// File replacement or truncation resets the remembered offset. A line
    /// larger than [`MAX_PARTIAL_LINE_BYTES`] is discarded through its next
    /// newline without retaining the remainder in memory.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be inspected, opened, sought, or
    /// read. No file contents are included in the error.
    pub async fn read_new(&mut self, path: &Path) -> Result<TailRead, TailError> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|error| TailError::new(path, error))?;
        let identity = file_identity(&metadata);
        let state = self.files.entry(path.to_path_buf()).or_default();
        if metadata.len() < state.offset || state.file_identity.is_some_and(|old| old != identity) {
            state.reset();
        }
        state.file_identity = Some(identity);

        let mut file = File::open(path)
            .await
            .map_err(|error| TailError::new(path, error))?;
        file.seek(SeekFrom::Start(state.offset))
            .await
            .map_err(|error| TailError::new(path, error))?;

        let mut result = TailRead::default();
        let mut chunk = vec![0_u8; READ_CHUNK_BYTES].into_boxed_slice();
        let mut processed = 0_usize;
        while processed < MAX_BATCH_BYTES && result.lines.len() < MAX_BATCH_LINES {
            let available = (MAX_BATCH_BYTES - processed).min(chunk.len());
            let read = file
                .read(&mut chunk[..available])
                .await
                .map_err(|error| TailError::new(path, error))?;
            if read == 0 {
                break;
            }
            let consumed = consume_bytes(state, &chunk[..read], path, &mut result);
            state.offset = state.offset.saturating_add(consumed as u64);
            processed = processed.saturating_add(consumed);
            if consumed < read {
                break;
            }
        }
        result.has_more = state.offset < metadata.len();
        state.modified_at = metadata.modified().ok().or_else(|| Some(SystemTime::now()));
        Ok(result)
    }

    /// Returns current bounded-state diagnostics.
    #[must_use]
    pub fn metrics(&self) -> TailMetrics {
        TailMetrics {
            tracked_files: self.files.len(),
            buffered_bytes: self.files.values().map(|state| state.partial.len()).sum(),
            discarding_files: self
                .files
                .values()
                .filter(|state| state.discarding_oversized)
                .count(),
        }
    }

    /// Forgets files whose last modification is more than eight days old.
    pub fn prune_stale(&mut self, now: SystemTime) -> usize {
        let before = self.files.len();
        self.files.retain(|_, state| {
            state
                .modified_at
                .and_then(|modified| now.duration_since(modified).ok())
                .is_none_or(|age| age <= STALE_AFTER)
        });
        before.saturating_sub(self.files.len())
    }
}

impl TailState {
    fn reset(&mut self) {
        self.offset = 0;
        self.partial.clear();
        self.discarding_oversized = false;
    }
}

fn consume_bytes(state: &mut TailState, bytes: &[u8], path: &Path, result: &mut TailRead) -> usize {
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            if state.discarding_oversized {
                state.discarding_oversized = false;
            } else {
                result.lines.push(std::mem::take(&mut state.partial));
            }
        } else if !state.discarding_oversized {
            if state.partial.len() == MAX_PARTIAL_LINE_BYTES {
                state.partial.clear();
                state.discarding_oversized = true;
                result.skipped_oversized = result.skipped_oversized.saturating_add(1);
                warn!(path = %log_trunc(&path.to_string_lossy()), "skipping oversized unterminated line");
            } else {
                state.partial.push(byte);
            }
        }
        if result.lines.len() == MAX_BATCH_LINES {
            return index + 1;
        }
    }
    bytes.len()
}

#[cfg(unix)]
fn file_identity(metadata: &std::fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt;

    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(not(unix))]
fn file_identity(metadata: &std::fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: 0,
        inode: metadata.len(),
    }
}
