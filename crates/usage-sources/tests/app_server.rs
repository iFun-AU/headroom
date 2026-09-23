//! Codex app-server session tests over the bounded JSON-RPC transport.

#![allow(clippy::expect_used)]

use std::time::Duration;

use serde_json::{Value, json};
use tokio::{
    io::{
        AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf, duplex, split,
    },
    sync::mpsc,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use usage_core::{Provider, SourceKind, WindowKind};
use usage_sources::{
    SourceEvent,
    codex::{
        app_server::{AppServerError, AppServerSession},
        rpc::RpcClient,
    },
    scheduler::{Scheduler, SchedulerConfig},
};

const RATE_FIXTURE: &str =
    include_str!("../../usage-core/tests/fixtures/codex_rate_limits_read.json");
const UPDATE_FIXTURE: &str =
    include_str!("../../usage-core/tests/fixtures/codex_rate_limits_updated.json");

type ServerLines = tokio::io::Lines<BufReader<ReadHalf<DuplexStream>>>;

async fn next_message(lines: &mut ServerLines) -> Value {
    let line = lines
        .next_line()
        .await
        .expect("server read should work")
        .expect("client message should exist");
    serde_json::from_str(&line).expect("client message should be JSON")
}

async fn respond(writer: &mut WriteHalf<DuplexStream>, request: &Value, mut response: Value) {
    response["id"] = request["id"].clone();
    writer
        .write_all(format!("{response}\n").as_bytes())
        .await
        .expect("server response should write");
    writer.flush().await.expect("server response should flush");
}

async fn run_happy_server(
    server_read: ReadHalf<DuplexStream>,
    mut server_write: WriteHalf<DuplexStream>,
    cancel: CancellationToken,
) {
    let mut lines = BufReader::new(server_read).lines();
    let initialize = next_message(&mut lines).await;
    assert_eq!(initialize["method"], "initialize");
    assert_eq!(initialize["params"]["clientInfo"]["name"], "how_is_it");
    assert_eq!(initialize["params"]["clientInfo"]["version"], "1.0.0");
    respond(
        &mut server_write,
        &initialize,
        json!({ "result": { "userAgent": "fake" } }),
    )
    .await;

    let initialized = next_message(&mut lines).await;
    assert_eq!(initialized, json!({ "method": "initialized" }));
    let rate_request = next_message(&mut lines).await;
    assert_eq!(rate_request["method"], "account/rateLimits/read");
    assert_eq!(rate_request["params"]["excludeResetCreditDetails"], true);
    respond(
        &mut server_write,
        &rate_request,
        serde_json::from_str(RATE_FIXTURE).expect("rate fixture should parse"),
    )
    .await;

    let usage_request = next_message(&mut lines).await;
    assert_eq!(usage_request["method"], "account/usage/read");
    assert_eq!(usage_request["params"], Value::Null);
    respond(
        &mut server_write,
        &usage_request,
        json!({
            "result": {
                "dailyUsageBuckets": [
                    { "startDate": "2026-09-21", "tokens": 123 },
                    { "startDate": "2026-09-22", "tokens": 456 }
                ]
            }
        }),
    )
    .await;
    server_write
        .write_all(format!("{UPDATE_FIXTURE}\n").as_bytes())
        .await
        .expect("notification should write");
    server_write
        .flush()
        .await
        .expect("notification should flush");
    cancel.cancelled().await;
}

#[tokio::test]
async fn handshake_limits_daily_usage_and_sparse_push_become_source_events() {
    let (client_io, server_io) = duplex(128 * 1_024);
    let (client_read, client_write) = split(client_io);
    let (server_read, server_write) = split(server_io);
    let (client, notifications, driver) =
        RpcClient::new(client_read, client_write, Duration::from_secs(15));
    let (scheduler, scheduler_actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 31);
    let session = AppServerSession::new(client, notifications, scheduler, "1.0.0");
    let (events, mut event_rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    let driver_task = tokio::spawn(driver.run(cancel.child_token()));
    let scheduler_task = tokio::spawn(scheduler_actor.run(cancel.child_token()));
    let session_task = tokio::spawn(session.run(events, cancel.child_token()));
    let server_cancel = cancel.child_token();
    let server_task = tokio::spawn(run_happy_server(server_read, server_write, server_cancel));

    let initial = timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .expect("initial reading should arrive promptly")
        .expect("event channel should remain open");
    assert!(matches!(&initial, SourceEvent::Reading(_)));
    let SourceEvent::Reading(initial) = initial else {
        return;
    };
    assert_eq!(initial.provider, Provider::Codex);
    assert!(!initial.partial);
    assert_eq!(initial.plan.as_deref(), Some("Pro"));
    assert_eq!(initial.windows[0].kind, WindowKind::Weekly);

    let daily = event_rx.recv().await.expect("daily event should arrive");
    assert!(matches!(&daily, SourceEvent::Daily(_)));
    let SourceEvent::Daily(daily) = daily else {
        return;
    };
    assert_eq!(daily.source, SourceKind::CodexAppServer);
    assert_eq!(daily.days[0].1.0, 123);
    assert_eq!(daily.days[1].1.0, 456);

    let pushed = event_rx.recv().await.expect("push reading should arrive");
    assert!(matches!(&pushed, SourceEvent::Reading(_)));
    let SourceEvent::Reading(pushed) = pushed else {
        return;
    };
    assert!(pushed.partial);
    assert_eq!(pushed.windows[0].kind, WindowKind::Session);
    assert!(matches!(
        event_rx.recv().await,
        Some(SourceEvent::Activity {
            provider: Provider::Codex
        })
    ));

    cancel.cancel();
    assert!(
        session_task
            .await
            .expect("session task should not panic")
            .is_ok()
    );
    assert!(
        driver_task
            .await
            .expect("driver task should not panic")
            .is_ok()
    );
    scheduler_task
        .await
        .expect("scheduler task should not panic");
    server_task.await.expect("server task should not panic");
}

#[tokio::test]
async fn unsupported_rate_limit_method_is_a_typed_stop() {
    let (client_io, server_io) = duplex(64 * 1_024);
    let (client_read, client_write) = split(client_io);
    let (server_read, mut server_write) = split(server_io);
    let (client, notifications, driver) =
        RpcClient::new(client_read, client_write, Duration::from_secs(15));
    let (scheduler, scheduler_actor) = Scheduler::channel_with_seed(SchedulerConfig::default(), 37);
    let session = AppServerSession::new(client, notifications, scheduler, "1.0.0");
    let (events, _event_rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    let driver_task = tokio::spawn(driver.run(cancel.child_token()));
    let scheduler_task = tokio::spawn(scheduler_actor.run(cancel.child_token()));
    let server_task = tokio::spawn(async move {
        let mut lines = BufReader::new(server_read).lines();
        let initialize = next_message(&mut lines).await;
        respond(&mut server_write, &initialize, json!({ "result": {} })).await;
        assert_eq!(next_message(&mut lines).await["method"], "initialized");
        let rate_request = next_message(&mut lines).await;
        respond(
            &mut server_write,
            &rate_request,
            json!({ "error": { "code": -32601, "message": "method not found" } }),
        )
        .await;
    });

    let result = session.run(events, cancel.child_token()).await;
    assert!(matches!(result, Err(AppServerError::Unsupported { .. })));

    cancel.cancel();
    let _ = driver_task.await;
    scheduler_task
        .await
        .expect("scheduler task should not panic");
    server_task.await.expect("server task should not panic");
}
