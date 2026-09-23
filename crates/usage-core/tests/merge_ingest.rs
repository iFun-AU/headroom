//! Per-source ingest-rule contract tests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use usage_core::{
    ConnectionStatus, LimitWindow, Percent, Provider, Reading, SourceKind, SourceState, State,
    UnixSeconds, WindowKind, ingest_reading, ingest_status,
};

fn percent(value: f64) -> Percent {
    Percent::new(value).expect("test percentages are finite")
}

fn window(
    kind: WindowKind,
    used: f64,
    resets_at: Option<i64>,
    source: SourceKind,
    observed_at: i64,
) -> LimitWindow {
    LimitWindow {
        kind,
        used: percent(used),
        resets_at: resets_at.map(UnixSeconds),
        reset_pending: false,
        source,
        observed_at: UnixSeconds(observed_at),
    }
}

fn reading(
    source: SourceKind,
    observed_at: i64,
    plan: Option<&str>,
    windows: Vec<LimitWindow>,
    partial: bool,
) -> Reading {
    Reading {
        provider: Provider::Codex,
        source,
        observed_at: UnixSeconds(observed_at),
        plan: plan.map(str::to_owned),
        windows,
        partial,
    }
}

fn source_state(state: &State, source: SourceKind) -> &SourceState {
    state
        .get(&(Provider::Codex, source))
        .expect("source state should exist")
}

#[test]
fn i1_full_reading_replaces_windows_and_preserves_absent_plan() {
    let mut state = State::new();
    let first = reading(
        SourceKind::CodexAppServer,
        100,
        Some("Pro"),
        vec![
            window(
                WindowKind::Session,
                10.0,
                Some(500),
                SourceKind::CodexAppServer,
                100,
            ),
            window(
                WindowKind::Weekly,
                20.0,
                Some(900),
                SourceKind::CodexAppServer,
                100,
            ),
        ],
        false,
    );
    let second = reading(
        SourceKind::CodexAppServer,
        101,
        None,
        vec![window(
            WindowKind::Session,
            11.0,
            Some(500),
            SourceKind::CodexAppServer,
            101,
        )],
        false,
    );

    assert!(ingest_reading(&mut state, first));
    assert!(ingest_reading(&mut state, second));

    let stored = source_state(&state, SourceKind::CodexAppServer);
    assert_eq!(stored.windows.len(), 1);
    assert!(stored.windows.contains_key(&WindowKind::Session));
    assert_eq!(stored.plan.as_deref(), Some("Pro"));
    assert_eq!(stored.status, ConnectionStatus::Connected);
    assert_eq!(stored.observed_at, Some(UnixSeconds(101)));
}

#[test]
fn i2_partial_reading_upserts_and_rejects_pre_rollover_data() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            100,
            Some("Pro"),
            vec![window(
                WindowKind::Session,
                40.0,
                Some(600),
                SourceKind::CodexAppServer,
                100,
            )],
            false,
        ),
    ));

    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            110,
            None,
            vec![
                window(
                    WindowKind::Session,
                    5.0,
                    Some(500),
                    SourceKind::CodexAppServer,
                    110,
                ),
                window(
                    WindowKind::Weekly,
                    60.0,
                    Some(900),
                    SourceKind::CodexAppServer,
                    110,
                ),
            ],
            true,
        ),
    ));

    let stored = source_state(&state, SourceKind::CodexAppServer);
    assert_eq!(stored.windows.len(), 2);
    assert_eq!(
        stored
            .windows
            .get(&WindowKind::Session)
            .expect("session remains")
            .used,
        percent(40.0)
    );
    assert_eq!(stored.plan.as_deref(), Some("Pro"));
}

#[test]
fn i2_partial_null_reset_does_not_clear_the_stored_reset() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            100,
            None,
            vec![window(
                WindowKind::Weekly,
                10.0,
                Some(700),
                SourceKind::CodexAppServer,
                100,
            )],
            false,
        ),
    ));
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            101,
            None,
            vec![window(
                WindowKind::Weekly,
                12.0,
                None,
                SourceKind::CodexAppServer,
                101,
            )],
            true,
        ),
    ));

    let weekly = &source_state(&state, SourceKind::CodexAppServer).windows[&WindowKind::Weekly];
    assert_eq!(weekly.resets_at, Some(UnixSeconds(700)));
    assert_eq!(weekly.used, percent(12.0));
}

#[test]
fn i3_status_event_keeps_last_good_data() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexRollout,
            100,
            Some("Pro"),
            vec![window(
                WindowKind::Weekly,
                22.0,
                Some(900),
                SourceKind::CodexRollout,
                100,
            )],
            false,
        ),
    ));
    ingest_status(
        &mut state,
        Provider::Codex,
        SourceKind::CodexRollout,
        ConnectionStatus::Error {
            message: "rollout unavailable".to_owned(),
        },
    );

    let stored = source_state(&state, SourceKind::CodexRollout);
    assert_eq!(stored.windows.len(), 1);
    assert_eq!(stored.observed_at, Some(UnixSeconds(100)));
    assert_eq!(stored.plan.as_deref(), Some("Pro"));
    assert!(matches!(stored.status, ConnectionStatus::Error { .. }));
}

#[test]
fn i4_older_full_reading_is_ignored_entirely() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            200,
            Some("New"),
            vec![window(
                WindowKind::Weekly,
                30.0,
                Some(900),
                SourceKind::CodexAppServer,
                200,
            )],
            false,
        ),
    ));
    assert!(!ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            199,
            Some("Old"),
            Vec::new(),
            false,
        ),
    ));

    let stored = source_state(&state, SourceKind::CodexAppServer);
    assert_eq!(stored.observed_at, Some(UnixSeconds(200)));
    assert_eq!(stored.plan.as_deref(), Some("New"));
    assert_eq!(stored.windows.len(), 1);
}
