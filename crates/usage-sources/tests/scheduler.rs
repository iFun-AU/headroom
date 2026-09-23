//! Adaptive scheduler tests with Tokio's clock paused.

#![allow(clippy::expect_used)]

use std::time::{Duration, SystemTime};

use tokio::time::{Instant, advance, timeout};
use tokio_util::sync::CancellationToken;
use usage_core::{Provider, SourceKind};
use usage_sources::scheduler::{Scheduler, SchedulerConfig, WakeDetector};

#[tokio::test(start_paused = true)]
async fn codex_uses_idle_then_active_interval_and_returns_to_idle() {
    let (scheduler, actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 7);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(actor.run(cancel.child_token()));

    let first = scheduler
        .next_due(SourceKind::CodexAppServer)
        .await
        .expect("initial read should be due");
    scheduler
        .record_success(SourceKind::CodexAppServer)
        .expect("success should be accepted");
    assert_eq!(
        scheduler
            .scheduled_for(SourceKind::CodexAppServer)
            .await
            .expect("schedule should be available")
            .duration_since(first),
        Duration::from_mins(10)
    );

    scheduler
        .record_activity(Provider::Codex)
        .expect("activity should be accepted");
    assert_eq!(
        scheduler
            .scheduled_for(SourceKind::CodexAppServer)
            .await
            .expect("active schedule should be available")
            .duration_since(first),
        Duration::from_mins(2)
    );

    advance(Duration::from_secs(601)).await;
    scheduler
        .next_due(SourceKind::CodexAppServer)
        .await
        .expect("overdue read should start");
    scheduler
        .record_success(SourceKind::CodexAppServer)
        .expect("success should be accepted");
    let now = Instant::now();
    assert_eq!(
        scheduler
            .scheduled_for(SourceKind::CodexAppServer)
            .await
            .expect("idle schedule should be available")
            .duration_since(now),
        Duration::from_mins(10)
    );

    cancel.cancel();
    task.await.expect("scheduler task should not panic");
}

#[tokio::test(start_paused = true)]
async fn triggered_read_respects_fifteen_second_minimum_gap() {
    let (scheduler, actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 11);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(actor.run(cancel.child_token()));
    scheduler
        .next_due(SourceKind::CodexAppServer)
        .await
        .expect("initial read should be due");
    scheduler
        .record_success(SourceKind::CodexAppServer)
        .expect("success should be accepted");
    scheduler
        .trigger(SourceKind::CodexAppServer)
        .expect("trigger should be accepted");

    let waiter_scheduler = scheduler.clone();
    let waiter =
        tokio::spawn(async move { waiter_scheduler.next_due(SourceKind::CodexAppServer).await });
    advance(Duration::from_secs(14)).await;
    tokio::task::yield_now().await;
    assert!(!waiter.is_finished());
    advance(Duration::from_secs(1)).await;
    assert_eq!(
        waiter
            .await
            .expect("waiter should not panic")
            .expect("triggered read should become due"),
        Instant::now()
    );

    cancel.cancel();
    task.await.expect("scheduler task should not panic");
}

#[tokio::test(start_paused = true)]
async fn failure_backoff_doubles_with_jitter_caps_and_honors_retry_after() {
    let (scheduler, actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 17);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(actor.run(cancel.child_token()));
    scheduler
        .next_due(SourceKind::CodexAppServer)
        .await
        .expect("initial read should be due");

    let base_seconds = [30_u64, 60, 120, 240, 480, 960, 1_800, 1_800];
    for base in base_seconds {
        scheduler
            .record_failure(SourceKind::CodexAppServer, None)
            .expect("failure should be accepted");
        let now = Instant::now();
        let due = scheduler
            .scheduled_for(SourceKind::CodexAppServer)
            .await
            .expect("backoff schedule should be available");
        let delay = due.duration_since(now);
        let minimum = Duration::from_secs(base * 8 / 10);
        let maximum = Duration::from_secs(base * 12 / 10).min(Duration::from_mins(30));
        assert!(delay >= minimum, "{delay:?} was below {minimum:?}");
        assert!(delay <= maximum, "{delay:?} exceeded {maximum:?}");
        advance(delay).await;
        scheduler
            .next_due(SourceKind::CodexAppServer)
            .await
            .expect("retry should become due");
    }

    scheduler
        .record_failure(SourceKind::CodexAppServer, Some(Duration::from_secs(2_000)))
        .expect("rate-limit failure should be accepted");
    let retry_after_delay = scheduler
        .scheduled_for(SourceKind::CodexAppServer)
        .await
        .expect("retry-after schedule should be available")
        .duration_since(Instant::now());
    assert!(retry_after_delay >= Duration::from_secs(2_000));

    cancel.cancel();
    task.await.expect("scheduler task should not panic");
}

#[tokio::test(start_paused = true)]
async fn stale_visible_ui_triggers_and_cancellation_is_prompt() {
    let (scheduler, actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 23);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(actor.run(cancel.child_token()));
    scheduler
        .next_due(SourceKind::CodexAppServer)
        .await
        .expect("initial read should be due");
    scheduler
        .record_success(SourceKind::CodexAppServer)
        .expect("success should be accepted");

    advance(Duration::from_secs(31)).await;
    scheduler
        .ui_visible(SourceKind::CodexAppServer)
        .expect("visibility should be accepted");
    assert!(
        scheduler
            .scheduled_for(SourceKind::CodexAppServer)
            .await
            .expect("visible schedule should be available")
            <= Instant::now()
    );

    cancel.cancel();
    timeout(Duration::from_millis(100), task)
        .await
        .expect("scheduler should stop promptly")
        .expect("scheduler task should not panic");
}

#[tokio::test(start_paused = true)]
async fn validated_settings_can_replace_active_and_idle_intervals() {
    let (scheduler, actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 31);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(actor.run(cancel.child_token()));
    let first = scheduler
        .next_due(SourceKind::CodexAppServer)
        .await
        .expect("initial read should be due");
    scheduler
        .record_success(SourceKind::CodexAppServer)
        .expect("success should be accepted");

    let config = SchedulerConfig {
        codex_active: Duration::from_secs(61),
        idle: Duration::from_secs(121),
        ..SchedulerConfig::default()
    };
    scheduler
        .update_config(config)
        .await
        .expect("configuration should be accepted");
    scheduler
        .record_activity(Provider::Codex)
        .expect("activity should be accepted");
    assert_eq!(
        scheduler
            .scheduled_for(SourceKind::CodexAppServer)
            .await
            .expect("updated active schedule should be available")
            .duration_since(first),
        Duration::from_secs(61)
    );

    cancel.cancel();
    task.await.expect("scheduler task should not panic");
}

#[test]
fn wake_detector_flags_wall_and_monotonic_drift_over_sixty_seconds() {
    let monotonic = Instant::now();
    let wall = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let mut detector = WakeDetector::new(wall, monotonic);
    assert!(!detector.sample(
        wall + Duration::from_secs(30),
        monotonic + Duration::from_secs(30)
    ));
    assert!(detector.sample(
        wall + Duration::from_secs(200),
        monotonic + Duration::from_secs(40)
    ));
}
