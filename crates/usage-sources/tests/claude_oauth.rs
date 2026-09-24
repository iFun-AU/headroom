//! Claude OAuth poller integration tests with fake HTTP and credentials.

#![cfg(feature = "claude-oauth")]
#![allow(clippy::expect_used)]

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::{
    sync::{mpsc, watch},
    time::{Instant, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use usage_core::{
    ConnectionStatus, LimitWindow, Percent, Provider, Reading, SourceKind, State, UnixSeconds,
    UsageSnapshot, WindowKind, classify, derive_snapshot, ingest_reading,
};
use usage_sources::{
    SourceEvent,
    claude::{
        keychain::{AccessToken, ClaudeCredentials, CredentialError, CredentialStore},
        oauth::{HttpError, HttpResponse, OAuthSource, UsageHttp},
    },
    scheduler::{Scheduler, SchedulerConfig},
};

const TOKEN: &str = "test-token";
const USAGE: &str = r#"{"five_hour":{"utilization":23.0,"resets_at":"2026-09-23T15:00:00Z"},"seven_day":{"utilization":41.0,"resets_at":null},"future":null}"#;

struct FakeHttp {
    responses: Mutex<VecDeque<HttpResponse>>,
    calls: Arc<AtomicUsize>,
}

impl UsageHttp for FakeHttp {
    async fn get_usage(&self, token: &AccessToken) -> Result<HttpResponse, HttpError> {
        assert_eq!(token.expose(), TOKEN);
        self.calls.fetch_add(1, Ordering::SeqCst);
        let next = self.responses.lock().expect("fake lock").pop_front();
        Ok(next.expect("test supplied too few responses"))
    }
}

struct FakeCredentials {
    result: Result<ClaudeCredentials, CredentialError>,
    reads: Arc<AtomicUsize>,
}

impl FakeCredentials {
    fn new(result: Result<ClaudeCredentials, CredentialError>) -> Self {
        Self {
            result,
            reads: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl CredentialStore for FakeCredentials {
    fn read(&self) -> Result<ClaudeCredentials, CredentialError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

fn now() -> UnixSeconds {
    UnixSeconds(chrono::Utc::now().timestamp())
}

fn credentials(expires_at: Option<UnixSeconds>) -> FakeCredentials {
    FakeCredentials::new(Ok(ClaudeCredentials {
        access_token: AccessToken::new(TOKEN.to_owned()),
        expires_at,
        subscription_type: Some("max".to_owned()),
    }))
}

fn response(status: u16, retry_after: Option<Duration>, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        retry_after,
        body: body.as_bytes().to_vec(),
    }
}

fn snapshot(statusline_at: Option<UnixSeconds>) -> watch::Receiver<Arc<UsageSnapshot>> {
    let mut state = State::new();
    if let Some(at) = statusline_at {
        let ingested = ingest_reading(
            &mut state,
            Reading {
                provider: Provider::Claude,
                source: SourceKind::ClaudeStatusline,
                observed_at: at,
                plan: None,
                windows: vec![LimitWindow {
                    kind: classify(None, Some(WindowKind::Session)),
                    used: Percent::new(10.0).expect("finite percent"),
                    resets_at: None,
                    reset_pending: false,
                    source: SourceKind::ClaudeStatusline,
                    observed_at: at,
                }],
                partial: false,
            },
        );
        assert!(ingested);
    }
    watch::channel(Arc::new(derive_snapshot(&state, now()))).1
}

struct Harness {
    events: mpsc::Receiver<SourceEvent>,
    calls: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    scheduler: Scheduler,
    cancel: CancellationToken,
}

fn start(
    responses: Vec<HttpResponse>,
    credentials: FakeCredentials,
    statusline_at: Option<UnixSeconds>,
) -> Harness {
    start_with(
        SchedulerConfig::default(),
        responses,
        credentials,
        statusline_at,
    )
}

fn start_with(
    config: SchedulerConfig,
    responses: Vec<HttpResponse>,
    credentials: FakeCredentials,
    statusline_at: Option<UnixSeconds>,
) -> Harness {
    let cancel = CancellationToken::new();
    let (scheduler, actor) = Scheduler::channel_with_seed(config, 5);
    let reads = Arc::clone(&credentials.reads);
    tokio::spawn(actor.run(cancel.child_token()));
    let calls = Arc::new(AtomicUsize::new(0));
    let http = FakeHttp {
        responses: Mutex::new(responses.into()),
        calls: Arc::clone(&calls),
    };
    let source = OAuthSource::new(
        http,
        credentials,
        scheduler.clone(),
        snapshot(statusline_at),
    );
    let (events_tx, events) = mpsc::channel(8);
    tokio::spawn(source.run(events_tx, cancel.child_token()));
    Harness {
        events,
        calls,
        reads,
        scheduler,
        cancel,
    }
}

async fn next_event(harness: &mut Harness) -> SourceEvent {
    timeout(Duration::from_secs(2), harness.events.recv())
        .await
        .expect("source should emit")
        .expect("event channel should stay open")
}

async fn settle() {
    sleep(Duration::from_millis(150)).await;
}

#[tokio::test]
async fn successful_poll_emits_a_full_oauth_reading() {
    let mut harness = start(vec![response(200, None, USAGE)], credentials(None), None);
    let reading = match next_event(&mut harness).await {
        SourceEvent::Reading(reading) => Some(reading),
        _ => None,
    }
    .expect("expected a reading");
    assert_eq!(reading.source, SourceKind::ClaudeOAuth);
    assert_eq!(reading.plan.as_deref(), Some("Max"));
    assert_eq!(reading.windows.len(), 2);
    assert_eq!(harness.calls.load(Ordering::SeqCst), 1);
    harness.cancel.cancel();
}

#[tokio::test]
async fn forbidden_and_not_found_report_unsupported_and_stop_polling() {
    for status in [403, 404] {
        let mut harness = start(vec![response(status, None, "")], credentials(None), None);
        assert!(matches!(
            next_event(&mut harness).await,
            SourceEvent::Status {
                source: SourceKind::ClaudeOAuth,
                status: ConnectionStatus::Unsupported { .. },
                ..
            }
        ));
        harness
            .scheduler
            .trigger(SourceKind::ClaudeOAuth)
            .expect("trigger should enqueue");
        settle().await;
        assert_eq!(harness.calls.load(Ordering::SeqCst), 1, "HTTP {status}");
        harness.cancel.cancel();
    }
}

#[tokio::test]
async fn unauthorized_and_locally_expired_tokens_report_auth_expired() {
    let mut harness = start(vec![response(401, None, "")], credentials(None), None);
    assert!(matches!(
        next_event(&mut harness).await,
        SourceEvent::Status {
            status: ConnectionStatus::AuthExpired { .. },
            ..
        }
    ));
    harness.cancel.cancel();

    let mut expired = start(
        Vec::new(),
        credentials(Some(UnixSeconds(now().0 - 1))),
        None,
    );
    assert!(matches!(
        next_event(&mut expired).await,
        SourceEvent::Status {
            status: ConnectionStatus::AuthExpired { .. },
            ..
        }
    ));
    assert_eq!(expired.calls.load(Ordering::SeqCst), 0);
    expired.cancel.cancel();
}

#[tokio::test]
async fn rate_limit_honors_retry_after_without_changing_status() {
    let mut harness = start(
        vec![response(429, Some(Duration::from_mins(10)), "")],
        credentials(None),
        None,
    );
    timeout(Duration::from_secs(2), async {
        while harness.calls.load(Ordering::SeqCst) == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("source should poll");
    settle().await;
    let due = harness
        .scheduler
        .scheduled_for(SourceKind::ClaudeOAuth)
        .await
        .expect("scheduler should answer");
    assert!(due >= Instant::now() + Duration::from_secs(590));
    assert!(harness.events.try_recv().is_err());
    harness.cancel.cancel();
}

#[tokio::test]
async fn fresh_status_line_data_skips_the_request() {
    let mut harness = start(Vec::new(), credentials(None), Some(now()));
    settle().await;
    assert_eq!(harness.calls.load(Ordering::SeqCst), 0);
    assert!(harness.events.try_recv().is_err());
    harness.cancel.cancel();
}

#[tokio::test]
async fn denied_keychain_access_stops_without_reprompting() {
    let mut harness = start(
        Vec::new(),
        FakeCredentials::new(Err(CredentialError::Denied)),
        None,
    );
    assert!(matches!(
        next_event(&mut harness).await,
        SourceEvent::Status {
            status: ConnectionStatus::Error { .. },
            ..
        }
    ));
    assert_eq!(harness.calls.load(Ordering::SeqCst), 0);
    harness.cancel.cancel();
}

#[tokio::test]
async fn credential_is_cached_until_rejected() {
    let fast = SchedulerConfig {
        claude_active: Duration::from_millis(50),
        idle: Duration::from_millis(50),
        minimum_gap: Duration::from_millis(10),
        initial_backoff: Duration::from_millis(10),
        ..SchedulerConfig::default()
    };
    let mut harness = start_with(
        fast,
        vec![
            response(200, None, USAGE),
            response(200, None, USAGE),
            response(401, None, ""),
            response(200, None, USAGE),
        ],
        credentials(None),
        None,
    );
    for expected in ["reading", "reading", "expired", "reading"] {
        let event = next_event(&mut harness).await;
        let actual = match event {
            SourceEvent::Reading(_) => "reading",
            SourceEvent::Status {
                status: ConnectionStatus::AuthExpired { .. },
                ..
            } => "expired",
            _ => "other",
        };
        assert_eq!(actual, expected);
    }
    // One read for the first three polls, one more after the 401.
    assert_eq!(harness.reads.load(Ordering::SeqCst), 2);
    harness.cancel.cancel();
}
