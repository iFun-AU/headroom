//! Bounded recursive discovery of recently modified JSONL history files.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, VecDeque},
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::log_trunc;

const RECENT_AGE: Duration = Duration::from_hours(192);
const MAX_SCAN_ENTRIES: usize = 32_768;
const MAX_PENDING_DIRECTORIES: usize = 4_096;
pub(crate) const MAX_TRACKED_FILES: usize = 4_096;

pub(crate) async fn recent_jsonl(
    root: &Path,
    now: SystemTime,
    cancel: &CancellationToken,
    context: &'static str,
) -> io::Result<Vec<PathBuf>> {
    let mut directories = VecDeque::from([root.to_path_buf()]);
    let mut recent = BinaryHeap::<Reverse<(SystemTime, PathBuf)>>::new();
    let mut inspected = 0_usize;

    while let Some(directory) = directories.pop_back() {
        if cancel.is_cancelled() {
            return Ok(sorted_paths(recent));
        }
        let mut entries = match tokio::fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) => {
                warn!(%context, path = %log_trunc(&directory.to_string_lossy()), error = %log_trunc(&error.to_string()), "skipping unreadable history directory");
                continue;
            }
        };
        while let Some(entry) = entries.next_entry().await? {
            if cancel.is_cancelled() {
                return Ok(sorted_paths(recent));
            }
            inspected = inspected.saturating_add(1);
            if inspected > MAX_SCAN_ENTRIES {
                warn!(%context, limit = MAX_SCAN_ENTRIES, "history scan reached its entry limit");
                return Ok(sorted_paths(recent));
            }
            let file_type = match entry.file_type().await {
                Ok(file_type) => file_type,
                Err(error) => {
                    warn!(%context, error = %log_trunc(&error.to_string()), "skipping unreadable history entry");
                    continue;
                }
            };
            if file_type.is_dir() {
                if directories.len() < MAX_PENDING_DIRECTORIES {
                    directories.push_back(entry.path());
                } else {
                    warn!(%context, limit = MAX_PENDING_DIRECTORIES, "history scan reached its directory limit");
                }
                continue;
            }
            if !file_type.is_file() || entry.path().extension() != Some(OsStr::new("jsonl")) {
                continue;
            }
            let metadata = match entry.metadata().await {
                Ok(metadata) => metadata,
                Err(error) => {
                    warn!(%context, error = %log_trunc(&error.to_string()), "skipping unreadable history metadata");
                    continue;
                }
            };
            let modified = metadata.modified().unwrap_or(now);
            if !modified_is_recent(modified, now) {
                continue;
            }
            retain_newest(&mut recent, modified, entry.path());
        }
    }

    Ok(sorted_paths(recent))
}

pub(crate) async fn is_recent_file(path: &Path, now: SystemTime) -> io::Result<bool> {
    let metadata = tokio::fs::metadata(path).await?;
    if !metadata.is_file() {
        return Ok(false);
    }
    Ok(modified_is_recent(metadata.modified().unwrap_or(now), now))
}

fn modified_is_recent(modified: SystemTime, now: SystemTime) -> bool {
    !now.duration_since(modified)
        .is_ok_and(|age| age > RECENT_AGE)
}

fn retain_newest(
    files: &mut BinaryHeap<Reverse<(SystemTime, PathBuf)>>,
    modified: SystemTime,
    path: PathBuf,
) {
    let candidate = Reverse((modified, path));
    if files.len() < MAX_TRACKED_FILES {
        files.push(candidate);
    } else if files.peek().is_some_and(|oldest| candidate.0 > oldest.0) {
        files.pop();
        files.push(candidate);
    }
}

fn sorted_paths(files: BinaryHeap<Reverse<(SystemTime, PathBuf)>>) -> Vec<PathBuf> {
    let mut entries = files
        .into_iter()
        .map(|Reverse(entry)| entry)
        .collect::<Vec<_>>();
    entries.sort();
    entries.into_iter().map(|(_, path)| path).collect()
}
