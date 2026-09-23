//! Claude source integration tests over temporary directories and native watchers.

#![allow(clippy::expect_used)]

use std::{
    io::Write,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tempfile::{NamedTempFile, tempdir};
use tokio::{io::AsyncWriteExt, sync::mpsc, time::timeout};
use tokio_util::sync::CancellationToken;
use usage_core::{
    ConnectionStatus, Provider, SourceKind, UnixSeconds, parse::parse_claude_log_line,
};
use usage_sources::{
    SourceEvent,
    claude::{
        bridge_source::{BridgeSource, BridgeSourceConfig},
        effectiveness::{Effectiveness, EffectivenessHandle},
        history::{HistorySource, HistorySourceConfig},
    },
};

const CLAUDE_LOG_FIXTURE: &str = include_str!("../../usage-core/tests/fixtures/claude_log.jsonl");

fn unix_now() -> UnixSeconds {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be after the Unix epoch")
        .as_secs();
    UnixSeconds(i64::try_from(seconds).expect("test timestamp should fit"))
}

fn bridge_envelope(written_at: UnixSeconds, used: u8) -> String {
    format!(
        r#"{{"schema":1,"writtenAt":{},"sessionId":"test","rateLimits":{{"five_hour":{{"used_percentage":{used},"resets_at":{}}}}}}}"#,
        written_at.0,
        written_at.0 + 3_600,
    )
}

fn atomic_replace(path: &Path, contents: &str) {
    let parent = path.parent().expect("bridge target should have a parent");
    let mut temporary = NamedTempFile::new_in(parent).expect("temporary bridge file should open");
    temporary
        .write_all(contents.as_bytes())
        .expect("bridge envelope should write");
    temporary.flush().expect("bridge envelope should flush");
    temporary
        .persist(path)
        .expect("bridge envelope should atomically replace the target");
}

async fn append_line(path: &Path, line: &str) {
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .await
        .expect("history fixture should open");
    file.write_all(line.as_bytes())
        .await
        .expect("history line should append");
    file.write_all(b"\n")
        .await
        .expect("history newline should append");
    file.flush().await.expect("history fixture should flush");
}

#[tokio::test]
async fn disabled_bridge_emits_not_configured() {
    let directory = tempdir().expect("tempdir should be created");
    let target = directory.path().join("claude-rate-limits.json");
    let (effectiveness, _actor) = EffectivenessHandle::channel(None);
    let source = BridgeSource::new(
        BridgeSourceConfig::new(directory.path(), target, None),
        effectiveness,
    );
    let (events, mut event_rx) = mpsc::channel(4);

    source
        .run(events, CancellationToken::new())
        .await
        .expect("a disabled bridge should be a status, not a task failure");
    assert!(matches!(
        event_rx.recv().await,
        Some(SourceEvent::Status {
            provider: Provider::Claude,
            source: SourceKind::ClaudeStatusline,
            status: ConnectionStatus::NotConfigured { ref hint },
        }) if hint == "Enable real-time updates"
    ));
}

#[tokio::test]
async fn oversized_bridge_file_emits_error_without_unbounded_read() {
    let directory = tempdir().expect("tempdir should be created");
    let target = directory.path().join("claude-rate-limits.json");
    tokio::fs::write(&target, vec![b'x'; 4 * 1_024 * 1_024 + 1])
        .await
        .expect("oversized bridge fixture should be written");

    let installed_at = unix_now();
    let (effectiveness, _actor) = EffectivenessHandle::channel(Some(installed_at));
    let source = BridgeSource::new(
        BridgeSourceConfig::new(directory.path(), target, Some(installed_at)),
        effectiveness,
    );
    let cancel = CancellationToken::new();
    let (events, mut event_rx) = mpsc::channel(4);
    let source_task = tokio::spawn(source.run(events, cancel.child_token()));

    assert!(matches!(
        timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("oversized bridge status should arrive"),
        Some(SourceEvent::Status {
            provider: Provider::Claude,
            source: SourceKind::ClaudeStatusline,
            status: ConnectionStatus::Error { .. },
        })
    ));

    cancel.cancel();
    timeout(Duration::from_secs(1), source_task)
        .await
        .expect("bridge source should cancel promptly")
        .expect("bridge source task should not panic")
        .expect("bridge source should stop cleanly");
}

#[tokio::test]
async fn bridge_initial_read_and_atomic_replacement_emit_full_updates() {
    let directory = tempdir().expect("tempdir should be created");
    let target = directory.path().join("claude-rate-limits.json");
    let installed_at = unix_now();
    atomic_replace(&target, &bridge_envelope(installed_at, 23));

    let cancel = CancellationToken::new();
    let (effectiveness, actor) = EffectivenessHandle::channel(Some(installed_at));
    let mut effectiveness_rx = effectiveness.subscribe();
    let actor_task = tokio::spawn(actor.run(cancel.child_token()));
    let source = BridgeSource::new(
        BridgeSourceConfig::new(directory.path(), &target, Some(installed_at)),
        effectiveness,
    );
    let (events, mut event_rx) = mpsc::channel(16);
    let source_task = tokio::spawn(source.run(events, cancel.child_token()));

    let first = timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("initial bridge reading should arrive")
        .expect("source event channel should stay open");
    let first = match first {
        SourceEvent::Reading(reading) => Some(reading),
        _ => None,
    }
    .expect("initial bridge event should be a reading");
    assert!(!first.partial);
    assert_eq!(first.source, SourceKind::ClaudeStatusline);
    assert_eq!(first.observed_at, installed_at);
    assert!(matches!(
        event_rx.recv().await,
        Some(SourceEvent::Activity { .. })
    ));
    timeout(Duration::from_secs(1), effectiveness_rx.changed())
        .await
        .expect("bridge confirmation should arrive")
        .expect("effectiveness actor should stay open");
    assert_eq!(
        effectiveness_rx.borrow().effective,
        Effectiveness::Confirmed
    );

    let replacement_at = UnixSeconds(installed_at.0 + 1);
    atomic_replace(&target, &bridge_envelope(replacement_at, 41));
    let replacement = timeout(Duration::from_secs(3), async {
        loop {
            if let Some(SourceEvent::Reading(reading)) = event_rx.recv().await
                && reading.observed_at == replacement_at
            {
                return reading;
            }
        }
    })
    .await
    .expect("atomic replacement should trigger the native watcher");
    assert!(!replacement.partial);
    assert!((replacement.windows[0].used.get() - 41.0).abs() < f64::EPSILON);

    cancel.cancel();
    timeout(Duration::from_secs(1), source_task)
        .await
        .expect("bridge source should cancel promptly")
        .expect("bridge source task should not panic")
        .expect("bridge source should stop cleanly");
    actor_task
        .await
        .expect("effectiveness actor should not panic");
}

#[tokio::test]
async fn history_scan_and_watched_append_emit_dedupable_tokens_and_activity() {
    let directory = tempdir().expect("tempdir should be created");
    let projects = directory.path().join("projects");
    let project = projects.join("example");
    tokio::fs::create_dir_all(&project)
        .await
        .expect("history tree should be created");
    let log = project.join("session.jsonl");
    tokio::fs::write(&log, CLAUDE_LOG_FIXTURE)
        .await
        .expect("history fixture should be written");
    let first_event = parse_claude_log_line(
        CLAUDE_LOG_FIXTURE
            .lines()
            .next()
            .expect("fixture should contain an event"),
    )
    .expect("fixture should parse")
    .expect("fixture should contain token usage");
    let installed_at = UnixSeconds(first_event.at.0 - 601);

    let cancel = CancellationToken::new();
    let (effectiveness, actor) = EffectivenessHandle::channel(Some(installed_at));
    let mut effectiveness_rx = effectiveness.subscribe();
    let actor_task = tokio::spawn(actor.run(cancel.child_token()));
    let source = HistorySource::new(HistorySourceConfig::new(&projects), effectiveness);
    let (events, mut event_rx) = mpsc::channel(16);
    let source_task = tokio::spawn(source.run(events, cancel.child_token()));

    let initial = timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("initial history scan should emit")
        .expect("source event channel should stay open");
    let initial = match initial {
        SourceEvent::Tokens(events) => Some(events),
        _ => None,
    }
    .expect("initial history event should contain tokens");
    assert_eq!(initial.len(), 2);
    assert_eq!(initial[0].dedupe_key, initial[1].dedupe_key);
    assert!(initial[0].dedupe_key.is_some());
    timeout(Duration::from_secs(1), effectiveness_rx.changed())
        .await
        .expect("override evidence should arrive")
        .expect("effectiveness actor should stay open");
    assert_eq!(
        effectiveness_rx.borrow().effective,
        Effectiveness::LikelyOverridden
    );

    let watched_line = r#"{"type":"assistant","timestamp":"2026-08-26T10:25:32.875Z","requestId":"req-watched","message":{"id":"msg-watched","usage":{"input_tokens":7,"output_tokens":5}}}"#;
    append_line(&log, watched_line).await;
    let (watched_tokens, activity) = timeout(Duration::from_secs(3), async {
        let mut tokens = None;
        let mut activity = false;
        while tokens.is_none() || !activity {
            match event_rx.recv().await {
                Some(SourceEvent::Tokens(events)) => tokens = Some(events),
                Some(SourceEvent::Activity {
                    provider: Provider::Claude,
                }) => activity = true,
                Some(_) => {}
                None => break,
            }
        }
        (tokens, activity)
    })
    .await
    .expect("watched append should be processed");
    let watched_tokens = watched_tokens.expect("watched append should emit tokens");
    assert!(activity);
    assert_eq!(watched_tokens.len(), 1);
    assert_eq!(watched_tokens[0].tokens.0, 12);
    assert_eq!(
        watched_tokens[0].dedupe_key.as_deref(),
        Some("msg-watched:req-watched")
    );

    cancel.cancel();
    timeout(Duration::from_secs(1), source_task)
        .await
        .expect("history source should cancel promptly")
        .expect("history source task should not panic")
        .expect("history source should stop cleanly");
    actor_task
        .await
        .expect("effectiveness actor should not panic");
}
