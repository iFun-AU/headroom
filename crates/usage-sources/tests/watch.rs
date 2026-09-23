//! Filesystem watcher integration tests using temporary directories only.

#![allow(clippy::expect_used)]

use std::time::Duration;

use tempfile::tempdir;
use tokio::{io::AsyncWriteExt, task::JoinSet, time::timeout};
use tokio_util::sync::CancellationToken;
use usage_sources::watch::{DirectoryWatcher, WatchFilter, path_channel};

#[tokio::test]
async fn burst_writes_yield_one_debounced_jsonl_path() {
    let directory = tempdir().expect("tempdir should be created");
    let expected_path = directory.path().join("rollout.jsonl");
    let expected_event_path = directory
        .path()
        .canonicalize()
        .expect("tempdir should resolve")
        .join("rollout.jsonl");
    let (paths, mut events) = path_channel();
    let watcher = DirectoryWatcher::new(directory.path(), WatchFilter::JsonLines, paths)
        .expect("watcher should start");
    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(watcher.run(cancel.child_token()));
    tokio::time::sleep(Duration::from_millis(250)).await;

    tokio::fs::write(&expected_path, b"one\n")
        .await
        .expect("fixture should be created");
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&expected_path)
        .await
        .expect("fixture should open");
    file.write_all(b"two\n")
        .await
        .expect("fixture should be appended");
    file.write_all(b"three\n")
        .await
        .expect("fixture should be appended again");
    file.flush().await.expect("fixture should flush");

    let event = timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("debounced event should arrive")
        .expect("event stream should remain open");
    assert_eq!(event, expected_event_path);
    assert!(
        timeout(Duration::from_millis(650), events.recv())
            .await
            .is_err()
    );

    cancel.cancel();
    timeout(Duration::from_millis(100), tasks.join_all())
        .await
        .expect("watcher should stop promptly");
    assert_eq!(events.recv().await, None);
}

#[tokio::test]
async fn irrelevant_files_are_filtered_and_exact_file_matching_is_supported() {
    let directory = tempdir().expect("tempdir should be created");
    let exact_path = directory.path().join("claude-rate-limits.json");
    let expected_event_path = directory
        .path()
        .canonicalize()
        .expect("tempdir should resolve")
        .join("claude-rate-limits.json");
    let (paths, mut events) = path_channel();
    let watcher = DirectoryWatcher::new(
        directory.path(),
        WatchFilter::Exact(exact_path.clone()),
        paths,
    )
    .expect("watcher should start");
    let cancel = CancellationToken::new();
    let task = tokio::spawn(watcher.run(cancel.child_token()));
    tokio::time::sleep(Duration::from_millis(250)).await;

    tokio::fs::write(directory.path().join("ignored.jsonl"), b"ignored")
        .await
        .expect("irrelevant fixture should be written");
    tokio::fs::write(&exact_path, b"{}")
        .await
        .expect("exact fixture should be written");

    assert_eq!(
        timeout(Duration::from_secs(2), events.recv())
            .await
            .expect("exact event should arrive"),
        Some(expected_event_path)
    );
    assert!(
        timeout(Duration::from_millis(650), events.recv())
            .await
            .is_err()
    );

    cancel.cancel();
    timeout(Duration::from_millis(100), task)
        .await
        .expect("watcher should stop promptly")
        .expect("watcher task should not panic");
}
