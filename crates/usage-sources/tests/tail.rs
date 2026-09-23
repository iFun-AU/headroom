//! Incremental JSONL tailer tests using temporary files only.

#![allow(clippy::expect_used)]

use std::time::{Duration, SystemTime};

use tempfile::tempdir;
use tokio::io::AsyncWriteExt;
use usage_sources::tail::{MAX_PARTIAL_LINE_BYTES, Tailer};

#[tokio::test]
async fn appended_lines_are_emitted_once_and_partial_lines_are_completed() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("rollout.jsonl");
    tokio::fs::write(&path, b"one\npartial")
        .await
        .expect("fixture should be written");
    let mut tailer = Tailer::new();

    let first = tailer
        .read_new(&path)
        .await
        .expect("initial read should work");
    assert_eq!(first.lines, vec![b"one".to_vec()]);

    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .await
        .expect("fixture should open for append");
    file.write_all(b"-done\nthree\n")
        .await
        .expect("append should work");
    file.flush().await.expect("append should flush");

    let second = tailer
        .read_new(&path)
        .await
        .expect("append read should work");
    assert_eq!(
        second.lines,
        vec![b"partial-done".to_vec(), b"three".to_vec()]
    );
    assert!(
        tailer
            .read_new(&path)
            .await
            .expect("repeat read should work")
            .lines
            .is_empty()
    );
}

#[tokio::test]
async fn truncation_resets_offset_and_reads_replacement_content() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("rollout.jsonl");
    tokio::fs::write(&path, b"first-long\nsecond\n")
        .await
        .expect("fixture should be written");
    let mut tailer = Tailer::new();
    let initial = tailer
        .read_new(&path)
        .await
        .expect("initial read should work");
    assert_eq!(initial.lines.len(), 2);

    tokio::fs::write(&path, b"new\n")
        .await
        .expect("fixture should be truncated");
    let replacement = tailer
        .read_new(&path)
        .await
        .expect("replacement read should work");
    assert_eq!(replacement.lines, vec![b"new".to_vec()]);
}

#[tokio::test]
async fn oversized_line_is_skipped_without_exceeding_buffer_cap() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("rollout.jsonl");
    let mut fixture = vec![b'x'; MAX_PARTIAL_LINE_BYTES + 1];
    fixture.extend_from_slice(b"\nok\n");
    tokio::fs::write(&path, fixture)
        .await
        .expect("fixture should be written");
    let mut tailer = Tailer::new();

    let batch = tailer.read_new(&path).await.expect("read should work");

    assert_eq!(batch.lines, vec![b"ok".to_vec()]);
    assert_eq!(batch.skipped_oversized, 1);
    assert!(tailer.metrics().buffered_bytes <= MAX_PARTIAL_LINE_BYTES);
}

#[tokio::test]
async fn stale_file_state_is_forgotten_after_eight_days() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("rollout.jsonl");
    tokio::fs::write(&path, b"one\n")
        .await
        .expect("fixture should be written");
    let mut tailer = Tailer::new();
    tailer.read_new(&path).await.expect("read should work");
    assert_eq!(tailer.metrics().tracked_files, 1);

    let future = SystemTime::now() + Duration::from_hours(216);
    assert_eq!(tailer.prune_stale(future), 1);
    assert_eq!(tailer.metrics().tracked_files, 0);
}

#[tokio::test]
async fn completed_line_batches_are_bounded_and_continue_without_loss() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("rollout.jsonl");
    let fixture = "line\n".repeat(5_000);
    tokio::fs::write(&path, fixture)
        .await
        .expect("fixture should be written");
    let mut tailer = Tailer::new();

    let first = tailer
        .read_new(&path)
        .await
        .expect("first read should work");
    assert!(first.has_more);
    assert!(first.lines.len() < 5_000);
    let second = tailer
        .read_new(&path)
        .await
        .expect("continuation read should work");
    assert!(!second.has_more);
    assert_eq!(first.lines.len() + second.lines.len(), 5_000);
    assert!(
        first
            .lines
            .into_iter()
            .chain(second.lines)
            .all(|line| line == b"line")
    );
}
