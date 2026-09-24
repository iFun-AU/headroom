//! Cross-source race and ordering contract tests.

#![allow(clippy::expect_used, clippy::too_many_lines)]

use usage_core::{
    ConnectionStatus, LimitWindow, Percent, Provider, ProviderUsage, Reading, SourceKind, State,
    UnixSeconds, WindowKind, derive_provider_usage, ingest_reading, ingest_status,
};

const NOW: UnixSeconds = UnixSeconds(20_000);

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
        credits: None,
    }
}

#[test]
fn race_a_full_read_without_secondary_removes_it() {
    let mut state = State::new();
    let windows = vec![
        window(
            WindowKind::Session,
            1.0,
            None,
            SourceKind::CodexAppServer,
            10,
        ),
        window(
            WindowKind::Weekly,
            2.0,
            None,
            SourceKind::CodexAppServer,
            10,
        ),
    ];
    assert!(ingest_reading(
        &mut state,
        reading(SourceKind::CodexAppServer, 10, None, windows, false),
    ));
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            11,
            None,
            vec![window(
                WindowKind::Weekly,
                3.0,
                None,
                SourceKind::CodexAppServer,
                11,
            )],
            false,
        ),
    ));

    let stored = state
        .get(&(Provider::Codex, SourceKind::CodexAppServer))
        .expect("app-server state should exist");
    assert_eq!(
        stored.windows.keys().copied().collect::<Vec<_>>(),
        vec![WindowKind::Weekly]
    );
}

#[test]
fn race_b_rollout_error_does_not_override_healthy_app_server() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(SourceKind::CodexAppServer, NOW.0, None, Vec::new(), false,),
    ));
    ingest_status(
        &mut state,
        Provider::Codex,
        SourceKind::CodexRollout,
        ConnectionStatus::Error {
            message: "rollout failed".to_owned(),
        },
    );

    assert_eq!(
        derive_provider_usage(&state, Provider::Codex, NOW).status,
        ConnectionStatus::Connected
    );
}

#[test]
fn race_c_five_event_permutations_produce_the_same_snapshot() {
    let events = vec![
        reading(
            SourceKind::CodexAppServer,
            100,
            Some("Old"),
            vec![window(
                WindowKind::Weekly,
                10.0,
                Some(1_000),
                SourceKind::CodexAppServer,
                100,
            )],
            false,
        ),
        reading(
            SourceKind::CodexAppServer,
            102,
            Some("Current"),
            vec![window(
                WindowKind::Weekly,
                20.0,
                Some(1_000),
                SourceKind::CodexAppServer,
                102,
            )],
            false,
        ),
        reading(
            SourceKind::CodexAppServer,
            101,
            Some("Middle"),
            vec![window(
                WindowKind::Weekly,
                15.0,
                Some(1_000),
                SourceKind::CodexAppServer,
                101,
            )],
            false,
        ),
        reading(
            SourceKind::CodexRollout,
            103,
            None,
            vec![window(
                WindowKind::Weekly,
                21.0,
                Some(1_000),
                SourceKind::CodexRollout,
                103,
            )],
            false,
        ),
        reading(
            SourceKind::CodexRollout,
            104,
            None,
            vec![window(
                WindowKind::Weekly,
                22.0,
                Some(1_000),
                SourceKind::CodexRollout,
                104,
            )],
            false,
        ),
    ];
    let mut orderings = Vec::new();
    permute(&mut events.clone(), 0, &mut orderings);
    assert_eq!(orderings.len(), 120);

    let expected = derived_for_events(&orderings[0]);
    for ordering in orderings.iter().skip(1) {
        assert_eq!(derived_for_events(ordering), expected);
    }
}

#[test]
fn race_d_sparse_secondary_keeps_primary_and_plan() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            10,
            Some("Pro"),
            vec![window(
                WindowKind::Session,
                10.0,
                Some(500),
                SourceKind::CodexAppServer,
                10,
            )],
            false,
        ),
    ));
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            11,
            None,
            vec![window(
                WindowKind::Weekly,
                20.0,
                Some(900),
                SourceKind::CodexAppServer,
                11,
            )],
            true,
        ),
    ));

    let stored = state
        .get(&(Provider::Codex, SourceKind::CodexAppServer))
        .expect("app-server state should exist");
    assert!(stored.windows.contains_key(&WindowKind::Session));
    assert!(stored.windows.contains_key(&WindowKind::Weekly));
    assert_eq!(stored.plan.as_deref(), Some("Pro"));
}

#[test]
fn race_e_stale_app_server_yields_to_fresh_rollout() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            NOW.0 - 1_200,
            None,
            Vec::new(),
            false,
        ),
    ));
    assert!(ingest_reading(
        &mut state,
        reading(SourceKind::CodexRollout, NOW.0, None, Vec::new(), false,),
    ));

    let usage = derive_provider_usage(&state, Provider::Codex, NOW);
    assert_eq!(usage.authoritative_source, Some(SourceKind::CodexRollout));
    assert!(matches!(usage.status, ConnectionStatus::Degraded { .. }));
}

fn permute(events: &mut [Reading], start: usize, output: &mut Vec<Vec<Reading>>) {
    if start == events.len() {
        output.push(events.to_vec());
        return;
    }

    for index in start..events.len() {
        events.swap(start, index);
        permute(events, start + 1, output);
        events.swap(start, index);
    }
}

fn derived_for_events(events: &[Reading]) -> ProviderUsage {
    let mut state = State::new();
    for event in events {
        let _applied = ingest_reading(&mut state, event.clone());
    }
    derive_provider_usage(&state, Provider::Codex, UnixSeconds(105))
}
