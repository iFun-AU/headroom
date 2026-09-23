//! Codex JSON-RPC transport tests over in-memory duplex streams.

#![allow(clippy::expect_used)]

use std::time::Duration;

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, duplex, split},
    time::advance,
};
use tokio_util::sync::CancellationToken;
use usage_sources::codex::rpc::{RpcClient, RpcError, RpcTransportError};

#[tokio::test]
async fn concurrent_requests_are_correlated_when_responses_arrive_out_of_order() {
    let (client_io, server_io) = duplex(64 * 1_024);
    let (client_read, client_write) = split(client_io);
    let (server_read, mut server_write) = split(server_io);
    let (client, _notifications, driver) =
        RpcClient::new(client_read, client_write, Duration::from_secs(15));
    let cancel = CancellationToken::new();
    let driver_task = tokio::spawn(driver.run(cancel.child_token()));

    let first_client = client.clone();
    let first = tokio::spawn(async move { first_client.request("first", json!({})).await });
    let second_client = client.clone();
    let second = tokio::spawn(async move { second_client.request("second", json!({})).await });

    let mut lines = BufReader::new(server_read).lines();
    let mut requests = Vec::new();
    for _ in 0..2 {
        let line = lines
            .next_line()
            .await
            .expect("server read should work")
            .expect("request line should exist");
        requests.push(serde_json::from_str::<Value>(&line).expect("request should be JSON"));
    }
    requests.reverse();
    for request in requests {
        let response = json!({
            "id": request["id"],
            "result": { "method": request["method"] }
        });
        server_write
            .write_all(format!("{response}\n").as_bytes())
            .await
            .expect("response should write");
    }
    server_write.flush().await.expect("responses should flush");

    assert_eq!(
        first
            .await
            .expect("first task should not panic")
            .expect("first request should succeed")["result"]["method"],
        "first"
    );
    assert_eq!(
        second
            .await
            .expect("second task should not panic")
            .expect("second request should succeed")["result"]["method"],
        "second"
    );
    assert_eq!(client.pending_count(), 0);

    cancel.cancel();
    assert!(
        driver_task
            .await
            .expect("driver task should not panic")
            .is_ok()
    );
}

#[tokio::test(start_paused = true)]
async fn request_timeout_removes_the_pending_entry() {
    let (client_io, _server_io) = duplex(4 * 1_024);
    let (client_read, client_write) = split(client_io);
    let (client, _notifications, driver) =
        RpcClient::new(client_read, client_write, Duration::from_secs(5));
    let cancel = CancellationToken::new();
    let driver_task = tokio::spawn(driver.run(cancel.child_token()));
    let request_client = client.clone();
    let request = tokio::spawn(async move { request_client.request("never", Value::Null).await });

    while client.pending_count() == 0 {
        tokio::task::yield_now().await;
    }
    advance(Duration::from_secs(4)).await;
    tokio::task::yield_now().await;
    assert!(!request.is_finished());
    advance(Duration::from_secs(1)).await;
    assert!(matches!(
        request.await.expect("request task should not panic"),
        Err(RpcError::Timeout)
    ));
    assert_eq!(client.pending_count(), 0);

    cancel.cancel();
    assert!(
        driver_task
            .await
            .expect("driver task should not panic")
            .is_ok()
    );
}

#[tokio::test]
async fn server_requests_are_rejected_and_notifications_are_forwarded() {
    let (client_io, server_io) = duplex(64 * 1_024);
    let (client_read, client_write) = split(client_io);
    let (server_read, mut server_write) = split(server_io);
    let (_client, mut notifications, driver) =
        RpcClient::new(client_read, client_write, Duration::from_secs(15));
    let cancel = CancellationToken::new();
    let driver_task = tokio::spawn(driver.run(cancel.child_token()));

    server_write
        .write_all(
            b"{\"id\":\"server-1\",\"method\":\"server/question\",\"params\":{}}\n\
              {\"method\":\"account/rateLimits/updated\",\"params\":{\"value\":1}}\n",
        )
        .await
        .expect("server messages should write");
    server_write
        .flush()
        .await
        .expect("server messages should flush");

    let mut reply_line = String::new();
    BufReader::new(server_read)
        .read_line(&mut reply_line)
        .await
        .expect("reply should read");
    let reply: Value = serde_json::from_str(&reply_line).expect("reply should be JSON");
    assert_eq!(reply["id"], "server-1");
    assert_eq!(reply["error"]["code"], -32601);
    assert_eq!(reply["error"]["message"], "not supported");

    let notification = notifications
        .recv()
        .await
        .expect("notification should be forwarded");
    assert_eq!(notification.method, "account/rateLimits/updated");
    assert_eq!(notification.message["params"]["value"], 1);

    cancel.cancel();
    assert!(
        driver_task
            .await
            .expect("driver task should not panic")
            .is_ok()
    );
}

#[tokio::test]
async fn disconnect_drains_pending_requests() {
    let (client_io, server_io) = duplex(4 * 1_024);
    let (client_read, client_write) = split(client_io);
    let (server_read, server_write) = split(server_io);
    let (client, _notifications, driver) =
        RpcClient::new(client_read, client_write, Duration::from_secs(15));
    let cancel = CancellationToken::new();
    let driver_task = tokio::spawn(driver.run(cancel.child_token()));
    let request_client = client.clone();
    let request = tokio::spawn(async move { request_client.request("pending", Value::Null).await });

    let mut request_line = String::new();
    BufReader::new(server_read)
        .read_line(&mut request_line)
        .await
        .expect("request should read");
    drop(server_write);

    assert!(matches!(
        request.await.expect("request task should not panic"),
        Err(RpcError::Disconnected)
    ));
    assert_eq!(client.pending_count(), 0);
    assert!(matches!(
        driver_task.await.expect("driver task should not panic"),
        Err(RpcTransportError::Disconnected)
    ));
}
