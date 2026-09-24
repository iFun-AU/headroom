//! Bounded usage-store actor integration tests.

#![allow(clippy::expect_used)]

use std::{sync::Arc, time::Duration};

use tokio::{task::JoinSet, time::timeout};
use tokio_util::sync::CancellationToken;
use usage_core::{
    LimitWindow, Percent, Provider, Reading, SourceKind, TokenCount, TokenEvent, UnixSeconds,
    WindowKind,
};
use usage_sources::{
    SourceEvent,
    scheduler::{Scheduler, SchedulerConfig},
    store::{ALERT_CAPACITY, SOURCE_EVENT_CAPACITY, UsageStore},
};

fn reading(used: f64, observed_at: UnixSeconds) -> Reading {
    Reading {
        provider: Provider::Claude,
        source: SourceKind::ClaudeStatusline,
        observed_at,
        plan: Some("Max".to_owned()),
        windows: vec![LimitWindow {
            kind: WindowKind::Weekly,
            used: Percent::new(used).expect("percentage should be valid"),
            resets_at: Some(UnixSeconds(observed_at.0 + 604_800)),
            reset_pending: false,
            source: SourceKind::ClaudeStatusline,
            observed_at,
        }],
        partial: false,
        credits: None,
    }
}

fn unix_now() -> UnixSeconds {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock should follow Unix epoch")
        .as_secs();
    UnixSeconds(i64::try_from(seconds).expect("current epoch should fit i64"))
}

#[tokio::test]
async fn readings_history_and_alerts_flow_through_the_actor() {
    assert_eq!(SOURCE_EVENT_CAPACITY, 256);
    assert_eq!(ALERT_CAPACITY, 32);
    let now = unix_now();
    let (events, mut handle, actor) = UsageStore::channel(vec![75, 90, 100]);
    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(actor.run(cancel.clone()));

    events
        .send(SourceEvent::Reading(reading(70.0, now)))
        .await
        .expect("event channel should be open");
    timeout(Duration::from_millis(100), handle.snapshot.changed())
        .await
        .expect("snapshot should change promptly")
        .expect("snapshot sender should remain open");
    let first = Arc::clone(&handle.snapshot.borrow());
    assert_eq!(first.claude.plan.as_deref(), Some("Max"));
    assert!((first.claude.windows[0].used.get() - 70.0).abs() < f64::EPSILON);

    events
        .send(SourceEvent::Tokens(vec![TokenEvent {
            provider: Provider::Claude,
            source: SourceKind::ClaudeLocalLogs,
            at: now,
            tokens: TokenCount(123),
            dedupe_key: Some("message:request".to_owned()),
        }]))
        .await
        .expect("token event should be accepted");
    events
        .send(SourceEvent::Reading(reading(80.0, UnixSeconds(now.0 + 1))))
        .await
        .expect("second reading should be accepted");
    timeout(Duration::from_millis(100), handle.snapshot.changed())
        .await
        .expect("second snapshot should change promptly")
        .expect("snapshot sender should remain open");

    let history = handle
        .history(Provider::Claude)
        .await
        .expect("history command should reply");
    assert_eq!(
        history
            .hourly
            .buckets
            .iter()
            .map(|bucket| bucket.tokens.0)
            .sum::<u64>(),
        123
    );

    let alert = timeout(Duration::from_millis(100), handle.alerts.recv())
        .await
        .expect("alert should arrive promptly")
        .expect("alert channel should remain open");
    assert_eq!(alert.title, "Claude weekly limit at 75%");

    cancel.cancel();
    timeout(Duration::from_millis(100), tasks.join_next())
        .await
        .expect("store should stop promptly")
        .expect("store task should exist")
        .expect("store task should not panic");
}

#[tokio::test]
async fn cancellation_ends_an_idle_actor_within_one_hundred_milliseconds() {
    let (_events, _handle, actor) = UsageStore::channel(vec![75, 90, 100]);
    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(actor.run(cancel.clone()));

    cancel.cancel();
    timeout(Duration::from_millis(100), tasks.join_next())
        .await
        .expect("store should stop promptly")
        .expect("store task should exist")
        .expect("store task should not panic");
}

#[tokio::test]
async fn split_handle_keeps_commands_and_alerts_independently_owned() {
    let now = unix_now();
    let (events, handle, actor) = UsageStore::channel(vec![75]);
    let (mut client, mut alerts) = handle.split();
    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(actor.run(cancel.child_token()));

    events
        .send(SourceEvent::Reading(reading(70.0, now)))
        .await
        .expect("baseline should send");
    client
        .snapshot
        .changed()
        .await
        .expect("baseline snapshot should publish");
    events
        .send(SourceEvent::Reading(reading(80.0, UnixSeconds(now.0 + 1))))
        .await
        .expect("crossing should send");
    client
        .snapshot
        .changed()
        .await
        .expect("crossing snapshot should publish");

    client
        .refresh_now()
        .await
        .expect("split client should retain commands");
    assert_eq!(
        timeout(Duration::from_millis(100), alerts.recv())
            .await
            .expect("split alerts should arrive")
            .expect("alert sender should remain open")
            .title,
        "Claude weekly limit at 75%"
    );

    cancel.cancel();
    tasks.join_all().await;
}

#[tokio::test]
async fn updated_alert_preferences_suppress_reset_notifications() {
    let now = unix_now();
    let (events, handle, actor) = UsageStore::channel(vec![75]);
    let (mut client, mut alerts) = handle.split();
    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(actor.run(cancel.child_token()));
    client
        .update_alert_settings(vec![90], false)
        .await
        .expect("alert preferences should update");

    events
        .send(SourceEvent::Reading(reading(95.0, now)))
        .await
        .expect("baseline should send");
    client
        .snapshot
        .changed()
        .await
        .expect("baseline should publish");
    let mut reset = reading(0.0, UnixSeconds(now.0 + 1));
    reset.windows[0].resets_at = Some(UnixSeconds(now.0 + 700_000));
    events
        .send(SourceEvent::Reading(reset))
        .await
        .expect("reset should send");
    client
        .snapshot
        .changed()
        .await
        .expect("reset should publish");

    assert!(
        timeout(Duration::from_millis(20), alerts.recv())
            .await
            .is_err()
    );
    cancel.cancel();
    tasks.join_all().await;
}

#[tokio::test(start_paused = true)]
async fn activity_events_reach_the_scheduler_without_shared_mutable_state() {
    let (scheduler, scheduler_actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 29);
    let (events, mut handle, store) =
        UsageStore::channel_with_scheduler(vec![75, 90, 100], scheduler.clone());
    let cancel = CancellationToken::new();
    let mut tasks = JoinSet::new();
    tasks.spawn(scheduler_actor.run(cancel.child_token()));
    tasks.spawn(store.run(cancel.child_token()));
    let first = scheduler
        .next_due(SourceKind::CodexAppServer)
        .await
        .expect("initial read should be due");
    scheduler
        .record_success(SourceKind::CodexAppServer)
        .expect("success should be accepted");

    events
        .send(SourceEvent::Activity {
            provider: Provider::Codex,
        })
        .await
        .expect("activity should be accepted");
    events
        .send(SourceEvent::Reading(reading(20.0, unix_now())))
        .await
        .expect("barrier reading should be accepted");
    handle
        .snapshot
        .changed()
        .await
        .expect("barrier snapshot should publish");

    assert_eq!(
        scheduler
            .scheduled_for(SourceKind::CodexAppServer)
            .await
            .expect("schedule should be available")
            .duration_since(first),
        Duration::from_mins(2)
    );

    cancel.cancel();
    tasks.join_all().await;
}
