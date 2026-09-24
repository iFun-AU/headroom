//! Provider-view derivation-rule contract tests.

#![allow(clippy::expect_used, clippy::too_many_lines)]

use usage_core::{
    ConnectionStatus, CreditAmount, Credits, LimitWindow, Percent, Provider, Reading, SourceKind,
    State, UnixSeconds, WindowKind, derive_provider_usage, derive_snapshot, ingest_reading,
    ingest_status,
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
) -> Reading {
    Reading {
        provider: Provider::Codex,
        source,
        observed_at: UnixSeconds(observed_at),
        plan: plan.map(str::to_owned),
        windows,
        partial: false,
        credits: None,
    }
}

#[test]
fn d1_selects_fresh_priority_then_newest_when_all_are_stale() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            NOW.0 - 800,
            None,
            vec![window(
                WindowKind::Weekly,
                10.0,
                Some(NOW.0 + 1_000),
                SourceKind::CodexAppServer,
                NOW.0 - 800,
            )],
        ),
    ));
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexRollout,
            NOW.0 - 10,
            None,
            vec![window(
                WindowKind::Weekly,
                20.0,
                Some(NOW.0 + 1_000),
                SourceKind::CodexRollout,
                NOW.0 - 10,
            )],
        ),
    ));

    let fresh = derive_provider_usage(&state, Provider::Codex, NOW);
    assert_eq!(fresh.authoritative_source, Some(SourceKind::CodexAppServer));

    let stale_usage = derive_provider_usage(&state, Provider::Codex, UnixSeconds(NOW.0 + 901));
    assert_eq!(
        stale_usage.authoritative_source,
        Some(SourceKind::CodexRollout)
    );
}

#[test]
fn d2_refines_only_existing_authoritative_windows_with_non_regressing_resets() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            NOW.0 - 20,
            None,
            vec![window(
                WindowKind::Weekly,
                10.0,
                Some(NOW.0 + 1_000),
                SourceKind::CodexAppServer,
                NOW.0 - 20,
            )],
        ),
    ));
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexRollout,
            NOW.0 - 10,
            None,
            vec![
                window(
                    WindowKind::Weekly,
                    12.0,
                    Some(NOW.0 + 1_000),
                    SourceKind::CodexRollout,
                    NOW.0 - 10,
                ),
                window(
                    WindowKind::Session,
                    70.0,
                    Some(NOW.0 + 500),
                    SourceKind::CodexRollout,
                    NOW.0 - 10,
                ),
            ],
        ),
    ));

    let usage = derive_provider_usage(&state, Provider::Codex, NOW);
    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.windows[0].kind, WindowKind::Weekly);
    assert_eq!(usage.windows[0].used, percent(12.0));
    assert_eq!(usage.windows[0].source, SourceKind::CodexRollout);

    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexRollout,
            NOW.0 - 5,
            None,
            vec![window(
                WindowKind::Weekly,
                99.0,
                Some(NOW.0 + 999),
                SourceKind::CodexRollout,
                NOW.0 - 5,
            )],
        ),
    ));
    let guarded = derive_provider_usage(&state, Provider::Codex, NOW);
    assert_eq!(guarded.windows[0].used, percent(10.0));
    assert_eq!(guarded.windows[0].source, SourceKind::CodexAppServer);
}

#[test]
fn d3_marks_recent_resets_and_drops_day_old_resets() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            NOW.0,
            None,
            vec![
                window(
                    WindowKind::Session,
                    95.0,
                    Some(NOW.0),
                    SourceKind::CodexAppServer,
                    NOW.0,
                ),
                window(
                    WindowKind::Weekly,
                    99.0,
                    Some(NOW.0 - 86_401),
                    SourceKind::CodexAppServer,
                    NOW.0,
                ),
            ],
        ),
    ));

    let usage = derive_provider_usage(&state, Provider::Codex, NOW);
    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.windows[0].kind, WindowKind::Session);
    assert_eq!(usage.windows[0].used, Percent::ZERO);
    assert!(usage.windows[0].reset_pending);
}

#[test]
fn d4_evaluates_provider_status_in_contract_order() {
    let mut no_data = State::new();
    ingest_status(
        &mut no_data,
        Provider::Codex,
        SourceKind::CodexAppServer,
        ConnectionStatus::NotConfigured {
            hint: "Enable real-time updates".to_owned(),
        },
    );
    ingest_status(
        &mut no_data,
        Provider::Codex,
        SourceKind::CodexRollout,
        ConnectionStatus::Error {
            message: "ignored fallback error".to_owned(),
        },
    );
    assert_eq!(
        derive_provider_usage(&no_data, Provider::Codex, NOW).status,
        ConnectionStatus::NotConfigured {
            hint: "Enable real-time updates".to_owned()
        }
    );

    let mut stale_state = State::new();
    assert!(ingest_reading(
        &mut stale_state,
        reading(SourceKind::CodexAppServer, NOW.0 - 900, None, Vec::new()),
    ));
    assert_eq!(
        derive_provider_usage(&stale_state, Provider::Codex, NOW).status,
        ConnectionStatus::Stale
    );

    let mut connected = State::new();
    assert!(ingest_reading(
        &mut connected,
        reading(SourceKind::CodexAppServer, NOW.0, None, Vec::new()),
    ));
    ingest_status(
        &mut connected,
        Provider::Codex,
        SourceKind::CodexRollout,
        ConnectionStatus::Error {
            message: "lower priority".to_owned(),
        },
    );
    assert_eq!(
        derive_provider_usage(&connected, Provider::Codex, NOW).status,
        ConnectionStatus::Connected
    );

    let mut degraded = State::new();
    ingest_status(
        &mut degraded,
        Provider::Codex,
        SourceKind::CodexAppServer,
        ConnectionStatus::Error {
            message: "app-server stopped".to_owned(),
        },
    );
    assert!(ingest_reading(
        &mut degraded,
        reading(SourceKind::CodexRollout, NOW.0, None, Vec::new()),
    ));
    assert_eq!(
        derive_provider_usage(&degraded, Provider::Codex, NOW).status,
        ConnectionStatus::Degraded {
            reason: "app-server stopped".to_owned()
        }
    );
}

#[test]
fn d5_prefers_authoritative_plan_then_priority_fallback() {
    let mut state = State::new();
    assert!(ingest_reading(
        &mut state,
        reading(SourceKind::CodexAppServer, NOW.0, None, Vec::new()),
    ));
    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexRollout,
            NOW.0,
            Some("Fallback"),
            Vec::new(),
        ),
    ));
    assert_eq!(
        derive_provider_usage(&state, Provider::Codex, NOW)
            .plan
            .as_deref(),
        Some("Fallback")
    );

    assert!(ingest_reading(
        &mut state,
        reading(
            SourceKind::CodexAppServer,
            NOW.0 + 1,
            Some("Authoritative"),
            Vec::new(),
        ),
    ));
    assert_eq!(
        derive_provider_usage(&state, Provider::Codex, UnixSeconds(NOW.0 + 1))
            .plan
            .as_deref(),
        Some("Authoritative")
    );
}

#[test]
fn complete_snapshot_derives_both_providers_at_the_requested_time() {
    let now = UnixSeconds(42);
    let snapshot = derive_snapshot(&State::new(), now);

    assert_eq!(snapshot.generated_at, now);
    assert_eq!(snapshot.claude.provider, Provider::Claude);
    assert_eq!(snapshot.codex.provider, Provider::Codex);
    assert!(matches!(
        snapshot.claude.status,
        ConnectionStatus::NotConfigured { .. }
    ));
    assert!(matches!(
        snapshot.codex.status,
        ConnectionStatus::NotConfigured { .. }
    ));
}

fn credits(source: SourceKind, observed_at: i64, spent: i64) -> Credits {
    Credits {
        enabled: true,
        unlimited: false,
        used: Some(CreditAmount {
            minor: spent,
            exponent: 2,
            currency: Some("USD".to_owned()),
        }),
        limit: None,
        balance: None,
        source,
        observed_at: UnixSeconds(observed_at),
    }
}

fn claude(
    source: SourceKind,
    observed_at: i64,
    credits: Option<Credits>,
    partial: bool,
) -> Reading {
    Reading {
        provider: Provider::Claude,
        credits,
        partial,
        ..reading(
            source,
            observed_at,
            None,
            vec![window(
                WindowKind::Session,
                10.0,
                Some(NOW.0 + 3_600),
                source,
                observed_at,
            )],
        )
    }
}

#[test]
fn credits_come_from_any_source_while_another_supplies_the_windows() {
    let mut state = State::new();
    let oauth = credits(SourceKind::ClaudeOAuth, NOW.0 - 120, 1_240);
    assert!(ingest_reading(
        &mut state,
        claude(
            SourceKind::ClaudeOAuth,
            NOW.0 - 120,
            Some(oauth.clone()),
            false
        ),
    ));
    assert!(ingest_reading(
        &mut state,
        claude(SourceKind::ClaudeStatusline, NOW.0 - 10, None, false),
    ));

    let usage = derive_provider_usage(&state, Provider::Claude, NOW);
    assert_eq!(
        usage.authoritative_source,
        Some(SourceKind::ClaudeStatusline)
    );
    assert_eq!(usage.credits, Some(oauth));
}

#[test]
fn sparse_readings_keep_credits_and_complete_readings_replace_them() {
    let mut state = State::new();
    let first = credits(SourceKind::ClaudeOAuth, NOW.0 - 300, 100);
    assert!(ingest_reading(
        &mut state,
        claude(
            SourceKind::ClaudeOAuth,
            NOW.0 - 300,
            Some(first.clone()),
            false
        ),
    ));
    assert!(ingest_reading(
        &mut state,
        claude(SourceKind::ClaudeOAuth, NOW.0 - 200, None, true),
    ));
    assert_eq!(
        derive_provider_usage(&state, Provider::Claude, NOW).credits,
        Some(first)
    );

    assert!(ingest_reading(
        &mut state,
        claude(SourceKind::ClaudeOAuth, NOW.0 - 100, None, false),
    ));
    assert_eq!(
        derive_provider_usage(&state, Provider::Claude, NOW).credits,
        None
    );
}
