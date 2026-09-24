//! Snapshot-forwarder timing tests with Tokio's clock paused.

#![allow(clippy::expect_used)]

use std::sync::Arc;

use headroom::forwarder::forward_snapshots;
use tokio::{sync::mpsc, time::Duration};
use tokio_util::sync::CancellationToken;
use usage_core::{State, UnixSeconds, UsageSnapshot, derive_snapshot};

fn snapshot(at: i64) -> Arc<UsageSnapshot> {
    Arc::new(derive_snapshot(&State::new(), UnixSeconds(at)))
}

#[tokio::test(start_paused = true)]
async fn burst_is_throttled_and_latest_trailing_snapshot_is_guaranteed() {
    let (snapshots, snapshot_rx) = tokio::sync::watch::channel(snapshot(0));
    let (emitted, mut emitted_rx) = mpsc::channel(4);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(forward_snapshots(
        snapshot_rx,
        cancel.child_token(),
        move |snapshot| {
            let _ = emitted.try_send(snapshot);
        },
    ));

    snapshots.send_replace(snapshot(1));
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(249)).await;
    assert!(emitted_rx.try_recv().is_err());
    snapshots.send_replace(snapshot(2));
    tokio::time::advance(Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        emitted_rx
            .recv()
            .await
            .expect("trailing snapshot should emit")
            .generated_at,
        UnixSeconds(2)
    );

    snapshots.send_replace(snapshot(3));
    tokio::time::advance(Duration::from_millis(250)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        emitted_rx
            .recv()
            .await
            .expect("next window should emit")
            .generated_at,
        UnixSeconds(3)
    );

    cancel.cancel();
    task.await.expect("forwarder should not panic");
}
