//! Claude CLI panels: no invented windows, percentages, or reset dates.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use usage_core::{
    SourceKind, UnixSeconds, WindowKind,
    parse::{ClaudeCliError, parse_claude_cli_usage},
};

fn now() -> UnixSeconds {
    UnixSeconds(
        chrono::DateTime::parse_from_rfc3339("2026-09-28T06:00:00Z")
            .unwrap()
            .timestamp(),
    )
}

#[test]
fn fable_has_its_own_weekly_window_and_reset() {
    let screen = "Current session\n2% used\nResets 9am (UTC)\nCurrent week (all models)\n62% used\nResets Sep 30 at 5pm (UTC)\nCurrent week (Fable)\n80% used\nResets Sep 30 at 5pm (UTC)";
    let reading = parse_claude_cli_usage(screen, now()).unwrap().unwrap();
    assert_eq!(
        reading
            .windows
            .iter()
            .map(|window| window.kind)
            .collect::<Vec<_>>(),
        [WindowKind::Session, WindowKind::Weekly, WindowKind::Fable]
    );
    assert_eq!(reading.windows[2].used.get(), 80.0);
    assert_eq!(reading.windows[2].resets_at, reading.windows[1].resets_at);
    assert!(reading.windows[2].resets_at.is_some());
}

#[test]
fn missing_fable_usage_does_not_borrow_a_credit_percentage() {
    let screen = "Current week (Fable)\nClaude Code and Cowork credit\n20% used";
    assert!(parse_claude_cli_usage(screen, now()).unwrap().is_none());
}

#[test]
fn fresh_fable_survives_bridge_updates_and_expires_independently() {
    use usage_core::{Provider, State, derive_provider_usage, ingest_reading};
    let mut state = State::new();
    let cli = parse_claude_cli_usage("Current session\n2% used\nCurrent week (all models)\n62% used\nCurrent week (Fable)\n80% used", now()).unwrap().unwrap();
    let mut bridge = cli.clone();
    bridge.source = SourceKind::ClaudeStatusline;
    bridge.observed_at = UnixSeconds(now().0 + 60);
    bridge
        .windows
        .retain(|window| window.kind != WindowKind::Fable);
    assert!(ingest_reading(&mut state, cli));
    assert!(ingest_reading(&mut state, bridge.clone()));
    let usage = derive_provider_usage(&state, Provider::Claude, UnixSeconds(now().0 + 60));
    assert_eq!(
        usage.authoritative_source,
        Some(SourceKind::ClaudeStatusline)
    );
    let fable = usage
        .windows
        .iter()
        .find(|window| window.kind == WindowKind::Fable)
        .unwrap();
    assert_eq!(fable.source, SourceKind::ClaudeCli);
    assert_eq!(fable.observed_at, now());
    assert_eq!(fable.used.get(), 80.0);
    bridge.observed_at = UnixSeconds(now().0 + 901);
    assert!(ingest_reading(&mut state, bridge));
    let usage = derive_provider_usage(&state, Provider::Claude, UnixSeconds(now().0 + 901));
    assert!(
        !usage
            .windows
            .iter()
            .any(|window| window.kind == WindowKind::Fable)
    );
    assert_eq!(usage.windows.len(), 2);
}

#[test]
fn reads_independent_windows_and_ignores_model_scoped_limits() {
    let screen = "Current session\n████ 24% used\nResets 9am (UTC)\n\nCurrent week (all models)\n45% used\nResets Oct 3 at 8pm (UTC)\n\nCurrent week (Sonnet only)\n99% used";
    let reading = parse_claude_cli_usage(screen, now()).unwrap().unwrap();
    assert_eq!(reading.source, SourceKind::ClaudeCli);
    assert_eq!(reading.windows.len(), 2);
    assert_eq!(reading.windows[0].used.get(), 24.0);
    assert_eq!(
        reading.windows[0].resets_at,
        Some(UnixSeconds(now().0 + 3 * 3600))
    );
    assert_eq!(reading.windows[1].used.get(), 45.0);
    assert!(reading.windows[1].resets_at.is_some());
}

#[test]
fn weekly_only_does_not_fabricate_session_or_reset() {
    let reading =
        parse_claude_cli_usage("Current week (all models)\n80% left\nResets someday", now())
            .unwrap()
            .unwrap();
    assert_eq!(reading.windows.len(), 1);
    assert_eq!(reading.windows[0].kind, WindowKind::Weekly);
    assert_eq!(reading.windows[0].used.get(), 20.0);
    assert_eq!(reading.windows[0].resets_at, None);
}

#[test]
fn loading_and_other_usage_numbers_are_not_quota() {
    for screen in [
        "Usage: 0 input, 0 output",
        "Current week (Sonnet only)\n30% used",
        "Current session\n80% used\nLoading usage data…",
        "Current session\n-2% used",
        "Current session\n101% used",
    ] {
        assert!(
            parse_claude_cli_usage(screen, now()).unwrap().is_none(),
            "{screen}"
        );
    }
}

#[test]
fn detects_provider_errors_before_old_readings_and_redacts_them() {
    let result = parse_claude_cli_usage(
        "Current session\n10% used\nError: Usage endpoint is rate limited. secret",
        now(),
    );
    assert_eq!(result, Err(ClaudeCliError::RateLimited));
    assert!(!result.unwrap_err().to_string().contains("secret"));
    assert_eq!(
        parse_claude_cli_usage("Not logged in · run /login", now()),
        Err(ClaudeCliError::SignInRequired)
    );
}

#[test]
fn reset_zone_and_horizon_are_respected() {
    let reading = parse_claude_cli_usage(
        "Current session\n0% used\nResets 6pm (Australia/Melbourne)",
        now(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        reading.windows[0].resets_at,
        Some(UnixSeconds(now().0 + 2 * 3600))
    );
    let reading = parse_claude_cli_usage("Current session\n0% used\nResets 5am (UTC)", now())
        .unwrap()
        .unwrap();
    assert_eq!(reading.windows[0].resets_at, None);
}

#[test]
fn newer_manual_reading_wins_then_fresh_bridge_can_take_over() {
    use usage_core::{
        ConnectionStatus, Provider, State, derive_provider_usage, ingest_reading, ingest_status,
    };
    let mut state = State::new();
    let manual = parse_claude_cli_usage("Current session\n45% used", now())
        .unwrap()
        .unwrap();
    let mut bridge = manual.clone();
    bridge.source = SourceKind::ClaudeStatusline;
    bridge.observed_at = UnixSeconds(now().0 - 60);
    assert!(ingest_reading(&mut state, bridge.clone()));
    assert!(ingest_reading(&mut state, manual));
    let usage = derive_provider_usage(&state, Provider::Claude, now());
    assert_eq!(usage.authoritative_source, Some(SourceKind::ClaudeCli));
    ingest_status(
        &mut state,
        Provider::Claude,
        SourceKind::ClaudeCli,
        ConnectionStatus::Error {
            message: "Rate limited".into(),
        },
    );
    let retained = derive_provider_usage(&state, Provider::Claude, now());
    assert_eq!(retained.windows[0].observed_at, now());
    bridge.observed_at = UnixSeconds(now().0 + 10);
    assert!(ingest_reading(&mut state, bridge));
    assert_eq!(
        derive_provider_usage(&state, Provider::Claude, UnixSeconds(now().0 + 10))
            .authoritative_source,
        Some(SourceKind::ClaudeStatusline)
    );
}
