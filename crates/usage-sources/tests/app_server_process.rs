//! Codex app-server child ownership and bounded restart tests.

#![allow(clippy::expect_used)]

use std::{os::unix::fs::PermissionsExt, path::Path, process::Stdio, time::Duration};

use tempfile::tempdir;
use tokio::{sync::mpsc, time::timeout};
use tokio_util::sync::CancellationToken;
use usage_core::ConnectionStatus;
use usage_sources::{
    SourceEvent,
    codex::{
        app_server::{AppServerConfig, AppServerSource},
        discover::DiscoveryOptions,
    },
    scheduler::{Scheduler, SchedulerConfig},
};

async fn write_executable(path: &Path, contents: &str) {
    tokio::fs::write(path, contents)
        .await
        .expect("fixture should be written");
    let mut permissions = tokio::fs::metadata(path)
        .await
        .expect("fixture metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(path, permissions)
        .await
        .expect("fixture permissions should update");
}

fn test_config(path: &Path, failures: u8) -> AppServerConfig {
    let discovery = DiscoveryOptions::testing(
        Some(path.to_path_buf()),
        Vec::new(),
        Path::new("/usr/bin/false").to_path_buf(),
        Duration::from_secs(1),
    );
    AppServerConfig::new(discovery, "1.0.0").testing(
        Duration::from_secs(1),
        Duration::from_millis(10),
        Duration::from_millis(20),
        failures,
    )
}

#[tokio::test]
async fn source_records_pid_emits_reading_and_reaps_child_on_cancel() {
    let directory = tempdir().expect("tempdir should be created");
    let binary = directory.path().join("codex");
    write_executable(
        &binary,
        r#"#!/bin/sh
IFS= read -r initialize || exit 1
printf '%s\n' '{"id":1,"result":{}}'
IFS= read -r initialized || exit 1
IFS= read -r limits || exit 1
printf '%s\n' '{"id":2,"result":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":7,"windowDurationMins":10080,"resetsAt":1790725056},"planType":"pro"}}}'
IFS= read -r usage || exit 1
printf '%s\n' '{"id":3,"result":{"dailyUsageBuckets":[]}}'
while IFS= read -r line; do :; done
"#,
    )
    .await;

    let cancel = CancellationToken::new();
    let (scheduler, scheduler_actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 41);
    let scheduler_task = tokio::spawn(scheduler_actor.run(cancel.child_token()));
    let (source, control) = AppServerSource::new(test_config(&binary, 10), scheduler);
    let mut pid_changes = control.pid_receiver();
    let (events, mut event_rx) = mpsc::channel(16);
    let source_task = tokio::spawn(source.run(events, cancel.child_token()));

    timeout(Duration::from_secs(1), pid_changes.changed())
        .await
        .expect("PID should appear promptly")
        .expect("PID sender should remain open");
    let pid = control.pid().expect("source should expose its child PID");
    let reading = timeout(Duration::from_secs(1), async {
        loop {
            if let Some(SourceEvent::Reading(reading)) = event_rx.recv().await {
                return reading;
            }
        }
    })
    .await
    .expect("initial reading should arrive");
    assert!((reading.windows[0].used.get() - 7.0).abs() < f64::EPSILON);

    cancel.cancel();
    timeout(Duration::from_secs(1), source_task)
        .await
        .expect("source should stop promptly")
        .expect("source task should not panic")
        .expect("source should shut down cleanly");
    scheduler_task
        .await
        .expect("scheduler task should not panic");
    assert_eq!(control.pid(), None);
    let status = tokio::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .await
        .expect("process probe should run");
    assert!(!status.success(), "child PID must be reaped after shutdown");
}

#[tokio::test]
async fn restart_limit_waits_for_explicit_retry() {
    let directory = tempdir().expect("tempdir should be created");
    let binary = directory.path().join("failing-codex");
    write_executable(&binary, "#!/bin/sh\nprintf x >> \"${0}.count\"\nexit 1\n").await;
    let count_path = directory.path().join("failing-codex.count");

    let cancel = CancellationToken::new();
    let (scheduler, scheduler_actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 43);
    let scheduler_task = tokio::spawn(scheduler_actor.run(cancel.child_token()));
    let (source, control) = AppServerSource::new(test_config(&binary, 2), scheduler);
    let (events, _event_rx) = mpsc::channel(16);
    let source_task = tokio::spawn(source.run(events, cancel.child_token()));

    timeout(Duration::from_secs(1), async {
        loop {
            let count = tokio::fs::read(&count_path)
                .await
                .map_or(0, |contents| contents.len());
            if count == 2 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("two failed starts should occur");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        tokio::fs::read(&count_path)
            .await
            .expect("count should exist")
            .len(),
        2
    );

    control.retry().expect("retry should be accepted");
    timeout(Duration::from_secs(1), async {
        loop {
            let count = tokio::fs::read(&count_path)
                .await
                .map_or(0, |contents| contents.len());
            if count == 3 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("explicit retry should resume spawning");

    cancel.cancel();
    source_task
        .await
        .expect("source task should not panic")
        .expect("source should stop cleanly");
    scheduler_task
        .await
        .expect("scheduler task should not panic");
}

#[tokio::test]
async fn unsupported_method_stops_until_retry_or_cancellation() {
    let directory = tempdir().expect("tempdir should be created");
    let binary = directory.path().join("unsupported-codex");
    write_executable(
        &binary,
        r#"#!/bin/sh
IFS= read -r initialize || exit 1
printf '%s\n' '{"id":1,"result":{}}'
IFS= read -r initialized || exit 1
IFS= read -r limits || exit 1
printf '%s\n' '{"id":2,"error":{"code":-32601,"message":"method not found"}}'
while IFS= read -r line; do :; done
"#,
    )
    .await;

    let cancel = CancellationToken::new();
    let (scheduler, scheduler_actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 47);
    let scheduler_task = tokio::spawn(scheduler_actor.run(cancel.child_token()));
    let (source, control) = AppServerSource::new(test_config(&binary, 10), scheduler);
    let (events, mut event_rx) = mpsc::channel(16);
    let source_task = tokio::spawn(source.run(events, cancel.child_token()));

    let status = timeout(Duration::from_secs(1), async {
        loop {
            if let Some(SourceEvent::Status { status, .. }) = event_rx.recv().await {
                return status;
            }
        }
    })
    .await
    .expect("unsupported status should arrive");
    assert!(matches!(status, ConnectionStatus::Unsupported { .. }));
    assert_eq!(control.pid(), None);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        control.pid(),
        None,
        "unsupported source must not auto-restart"
    );

    cancel.cancel();
    source_task
        .await
        .expect("source task should not panic")
        .expect("source should stop cleanly");
    scheduler_task
        .await
        .expect("scheduler task should not panic");
}
