//! Claude bridge effectiveness state-machine tests with injected time.

#![allow(clippy::expect_used)]

use std::time::Duration;

use tokio::time::{Instant, advance, timeout};
use tokio_util::sync::CancellationToken;
use usage_core::UnixSeconds;
use usage_sources::claude::effectiveness::{
    Effectiveness, EffectivenessHandle, EffectivenessTracker, LIKELY_OVERRIDDEN_HINT,
};

#[test]
fn pure_tracker_uses_strict_ten_minute_boundary_and_recovers_on_write() {
    let installed_at = UnixSeconds(1_000);
    let mut tracker = EffectivenessTracker::new(Some(installed_at));

    assert!(tracker.assistant_activity(UnixSeconds(1_600)));
    assert_eq!(tracker.snapshot().effective, Effectiveness::Unverified);
    assert!(tracker.assistant_activity(UnixSeconds(1_601)));
    assert_eq!(
        tracker.snapshot().effective,
        Effectiveness::LikelyOverridden
    );
    assert!(LIKELY_OVERRIDDEN_HINT.contains("project or organization"));
    assert!(LIKELY_OVERRIDDEN_HINT.contains("plan doesn't report limits"));

    assert!(tracker.bridge_written(UnixSeconds(1_602)));
    assert_eq!(tracker.snapshot().effective, Effectiveness::Confirmed);
    assert!(!tracker.bridge_written(UnixSeconds(999)));
    assert_eq!(tracker.snapshot().effective, Effectiveness::Confirmed);
}

#[tokio::test(start_paused = true)]
async fn actor_moves_unverified_to_likely_overridden_then_confirmed() {
    let installed_at = UnixSeconds(10_000);
    let monotonic_start = Instant::now();
    let cancel = CancellationToken::new();
    let (handle, actor) = EffectivenessHandle::channel(Some(installed_at));
    let actor_task = tokio::spawn(actor.run(cancel.child_token()));
    let mut snapshots = handle.subscribe();

    advance(Duration::from_mins(11)).await;
    let elapsed = Instant::now().duration_since(monotonic_start).as_secs();
    let activity_at = UnixSeconds(
        installed_at
            .0
            .saturating_add(i64::try_from(elapsed).expect("elapsed test time should fit")),
    );
    handle
        .assistant_activity(activity_at)
        .await
        .expect("activity should enqueue");
    timeout(Duration::from_secs(1), snapshots.changed())
        .await
        .expect("activity snapshot should arrive")
        .expect("snapshot actor should remain open");
    assert_eq!(
        snapshots.borrow().effective,
        Effectiveness::LikelyOverridden
    );

    handle
        .bridge_written(UnixSeconds(activity_at.0 + 1))
        .await
        .expect("bridge write should enqueue");
    timeout(Duration::from_secs(1), snapshots.changed())
        .await
        .expect("confirmation snapshot should arrive")
        .expect("snapshot actor should remain open");
    assert_eq!(snapshots.borrow().effective, Effectiveness::Confirmed);

    cancel.cancel();
    actor_task.await.expect("actor should stop without panic");
}

#[tokio::test]
async fn bounded_actor_applies_bursts_without_dropping_latest_evidence() {
    let cancel = CancellationToken::new();
    let (handle, actor) = EffectivenessHandle::channel(None);
    let actor_task = tokio::spawn(actor.run(cancel.child_token()));
    let mut snapshots = handle.subscribe();
    handle
        .install(UnixSeconds(1_000))
        .await
        .expect("install should enqueue");

    let final_activity = UnixSeconds(1_704);
    for seconds in (1_011..=final_activity.0).step_by(11) {
        handle
            .assistant_activity(UnixSeconds(seconds))
            .await
            .expect("every activity should enqueue with backpressure");
    }
    timeout(Duration::from_secs(1), async {
        while snapshots.borrow().last_activity != Some(final_activity) {
            snapshots
                .changed()
                .await
                .expect("effectiveness actor should stay open");
        }
    })
    .await
    .expect("latest burst evidence should be applied");
    assert_eq!(
        snapshots.borrow().effective,
        Effectiveness::LikelyOverridden
    );

    handle.uninstall().await.expect("uninstall should enqueue");
    timeout(Duration::from_secs(1), async {
        while snapshots.borrow().installed_at.is_some() {
            snapshots
                .changed()
                .await
                .expect("effectiveness actor should stay open");
        }
    })
    .await
    .expect("uninstall snapshot should arrive");
    cancel.cancel();
    actor_task.await.expect("actor should stop without panic");
}
