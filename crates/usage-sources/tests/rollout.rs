//! Codex rollout-source tests over synthetic session trees.

#![allow(clippy::expect_used)]

use std::{
    fs::FileTimes,
    path::Path,
    time::{Duration, SystemTime},
};

use tempfile::tempdir;
use tokio::{io::AsyncWriteExt, sync::mpsc, time::timeout};
use tokio_util::sync::CancellationToken;
use usage_core::{ConnectionStatus, SourceKind};
use usage_sources::{
    SourceEvent,
    codex::rollout::{RolloutConfig, RolloutSource},
    scheduler::{Scheduler, SchedulerConfig},
};

const ROLLOUT_FIXTURE: &str = include_str!("../../usage-core/tests/fixtures/codex_rollout.jsonl");

const PREMIUM_WITH_TOKENS: &str = r#"{"timestamp":"2026-09-23T01:14:09.852Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":12157261},"last_token_usage":{"total_tokens":1000}},"rate_limits":{"limit_id":"premium","primary":{"used_percent":88,"window_minutes":300,"resets_at":1790725056}}}}"#;

fn first_fixture_line() -> &'static str {
    ROLLOUT_FIXTURE
        .lines()
        .next()
        .expect("fixture should contain a line")
}

async fn write_line(path: &Path, line: &str) {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .expect("rollout fixture should open");
    file.write_all(line.as_bytes())
        .await
        .expect("line should append");
    file.write_all(b"\n").await.expect("newline should append");
    file.flush().await.expect("fixture should flush");
}

#[tokio::test]
async fn missing_sessions_root_emits_not_configured() {
    let directory = tempdir().expect("tempdir should be created");
    let missing = directory.path().join("missing-sessions");
    let (scheduler, _actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 53);
    let source = RolloutSource::new(RolloutConfig::new(missing), scheduler);
    let (events, mut event_rx) = mpsc::channel(4);

    source
        .run(events, CancellationToken::new())
        .await
        .expect("missing root should be a configured status, not a task failure");
    assert!(matches!(
        event_rx.recv().await,
        Some(SourceEvent::Status {
            source: SourceKind::CodexRollout,
            status: ConnectionStatus::NotConfigured { .. },
            ..
        })
    ));
}

#[tokio::test]
async fn recent_scan_and_watched_append_emit_limits_tokens_activity_and_trigger() {
    let directory = tempdir().expect("tempdir should be created");
    let sessions = directory.path().join("sessions");
    let day = sessions.join("2026/09/23");
    tokio::fs::create_dir_all(&day)
        .await
        .expect("session tree should be created");
    let recent = day.join("rollout-recent.jsonl");
    write_line(&recent, first_fixture_line()).await;
    let old = day.join("rollout-old.jsonl");
    write_line(&old, first_fixture_line()).await;
    let old_file = std::fs::OpenOptions::new()
        .write(true)
        .open(&old)
        .expect("old fixture should open");
    old_file
        .set_times(FileTimes::new().set_modified(SystemTime::now() - Duration::from_hours(216)))
        .expect("old fixture mtime should update");
    let old_age = SystemTime::now()
        .duration_since(
            std::fs::metadata(&old)
                .expect("old fixture metadata should exist")
                .modified()
                .expect("old fixture should have an mtime"),
        )
        .expect("old fixture should be in the past");
    assert!(old_age > Duration::from_hours(192));

    let cancel = CancellationToken::new();
    let (scheduler, scheduler_actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 59);
    let scheduler_task = tokio::spawn(scheduler_actor.run(cancel.child_token()));
    timeout(
        Duration::from_secs(1),
        scheduler.next_due(SourceKind::CodexAppServer),
    )
    .await
    .expect("initial scheduler deadline should be prompt")
    .expect("scheduler should accept app-server source");
    let source = RolloutSource::new(RolloutConfig::new(&sessions), scheduler.clone());
    let (events, mut event_rx) = mpsc::channel(16);
    let source_task = tokio::spawn(source.run(events, cancel.child_token()));

    let (initial_readings, initial_tokens) = timeout(Duration::from_secs(2), async {
        let mut readings = 0;
        let mut tokens = Vec::new();
        loop {
            match timeout(Duration::from_millis(700), event_rx.recv()).await {
                Ok(Some(SourceEvent::Reading(_))) => readings += 1,
                Ok(Some(SourceEvent::Tokens(events))) => {
                    tokens.extend(events.into_iter().map(|event| event.tokens.0));
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => return (readings, tokens),
            }
        }
    })
    .await
    .expect("recent initial scan should emit promptly");
    assert_eq!(initial_readings, 1, "the nine-day-old file must be skipped");
    assert_eq!(initial_tokens, vec![211_554]);
    let idle_due = scheduler
        .scheduled_for(SourceKind::CodexAppServer)
        .await
        .expect("idle deadline should be available");

    write_line(&recent, PREMIUM_WITH_TOKENS).await;
    let (activity, appended_tokens, appended_readings) = timeout(Duration::from_secs(3), async {
        let mut activity = false;
        let mut tokens = Vec::new();
        let mut readings = 0;
        while !activity || tokens.is_empty() {
            match event_rx.recv().await {
                Some(SourceEvent::Activity { .. }) => activity = true,
                Some(SourceEvent::Tokens(events)) => {
                    tokens.extend(events.into_iter().map(|event| event.tokens.0));
                }
                Some(SourceEvent::Reading(_)) => readings += 1,
                Some(_) => {}
                None => return (activity, tokens, readings),
            }
        }
        (activity, tokens, readings)
    })
    .await
    .expect("debounced append should be processed");
    assert!(activity);
    assert_eq!(appended_tokens, vec![1_000]);
    assert_eq!(appended_readings, 0, "premium limits must be filtered");
    let triggered_due = scheduler
        .scheduled_for(SourceKind::CodexAppServer)
        .await
        .expect("triggered deadline should be available");
    assert!(triggered_due < idle_due);

    cancel.cancel();
    timeout(Duration::from_secs(1), source_task)
        .await
        .expect("source should stop promptly")
        .expect("source task should not panic")
        .expect("source should stop cleanly");
    scheduler_task
        .await
        .expect("scheduler task should not panic");
}
