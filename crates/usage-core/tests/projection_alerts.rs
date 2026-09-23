//! Weekly projection and alert-transition contract tests.

#![allow(clippy::expect_used, clippy::float_cmp)]

use usage_core::{
    AlertTracker, ConnectionStatus, LimitWindow, Percent, Provider, ProviderUsage, SourceKind,
    UnixSeconds, UsageSnapshot, WindowKind, WindowMinutes, project_weekly,
};

fn percent(value: f64) -> Percent {
    Percent::new(value).expect("test percentages are finite")
}

fn limit_window(
    kind: WindowKind,
    used: f64,
    resets_at: Option<i64>,
    source: SourceKind,
) -> LimitWindow {
    LimitWindow {
        kind,
        used: percent(used),
        resets_at: resets_at.map(UnixSeconds),
        reset_pending: false,
        source,
        observed_at: UnixSeconds(100),
    }
}

fn provider_usage(provider: Provider, windows: Vec<LimitWindow>) -> ProviderUsage {
    ProviderUsage {
        provider,
        plan: None,
        windows,
        status: ConnectionStatus::Connected,
        authoritative_source: None,
        last_updated: None,
        sources: Vec::new(),
    }
}

fn snapshot(claude_windows: Vec<LimitWindow>, codex_windows: Vec<LimitWindow>) -> UsageSnapshot {
    UsageSnapshot {
        claude: provider_usage(Provider::Claude, claude_windows),
        codex: provider_usage(Provider::Codex, codex_windows),
        generated_at: UnixSeconds(100),
    }
}

#[test]
fn projection_uses_only_weekly_percentage_and_timing() {
    let session = limit_window(
        WindowKind::Session,
        75.0,
        Some(604_800),
        SourceKind::ClaudeStatusline,
    );
    assert_eq!(
        project_weekly(&session, WindowMinutes(10_080), UnixSeconds(302_400)),
        None
    );

    let weekly = limit_window(
        WindowKind::Weekly,
        75.0,
        Some(604_800),
        SourceKind::ClaudeStatusline,
    );
    let projection = project_weekly(&weekly, WindowMinutes(10_080), UnixSeconds(302_400))
        .expect("a half-open weekly window should project");
    assert!((projection.projected_percent_at_reset - 150.0).abs() < f64::EPSILON);
    assert_eq!(projection.hits_limit_at, Some(UnixSeconds(403_200)));
}

#[test]
fn projection_is_suppressed_during_the_first_six_hours() {
    let weekly = limit_window(
        WindowKind::Weekly,
        10.0,
        Some(604_800),
        SourceKind::CodexAppServer,
    );
    assert_eq!(
        project_weekly(&weekly, WindowMinutes(10_080), UnixSeconds((6 * 3_600) - 1)),
        None
    );
    assert!(project_weekly(&weekly, WindowMinutes(10_080), UnixSeconds(6 * 3_600)).is_some());
}

#[test]
fn projection_handles_missing_zero_duration_and_non_exhausting_windows() {
    let missing_reset = limit_window(WindowKind::Weekly, 50.0, None, SourceKind::CodexAppServer);
    assert_eq!(
        project_weekly(&missing_reset, WindowMinutes(10_080), UnixSeconds(604_800)),
        None
    );

    let weekly = limit_window(
        WindowKind::Weekly,
        50.0,
        Some(604_800),
        SourceKind::CodexAppServer,
    );
    assert_eq!(
        project_weekly(&weekly, WindowMinutes(0), UnixSeconds(604_800)),
        None
    );
    let projection = project_weekly(&weekly, WindowMinutes(10_080), UnixSeconds(604_800))
        .expect("a complete non-exhausting window should project");
    assert!((projection.projected_percent_at_reset - 50.0).abs() < f64::EPSILON);
    assert_eq!(projection.hits_limit_at, None);
}

#[test]
fn upward_threshold_crossing_fires_once() {
    let mut tracker = AlertTracker::new();
    let initial = snapshot(
        vec![limit_window(
            WindowKind::Session,
            74.0,
            Some(1_000),
            SourceKind::ClaudeStatusline,
        )],
        Vec::new(),
    );
    assert!(
        tracker
            .evaluate(&initial, &[75], UnixSeconds(100))
            .is_empty()
    );

    let crossed = snapshot(
        vec![limit_window(
            WindowKind::Session,
            76.0,
            Some(1_000),
            SourceKind::ClaudeStatusline,
        )],
        Vec::new(),
    );
    let alerts = tracker.evaluate(&crossed, &[75], UnixSeconds(110));
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].title, "Claude session limit at 75%");

    let higher = snapshot(
        vec![limit_window(
            WindowKind::Session,
            80.0,
            Some(1_000),
            SourceKind::ClaudeStatusline,
        )],
        Vec::new(),
    );
    assert!(
        tracker
            .evaluate(&higher, &[75], UnixSeconds(120))
            .is_empty()
    );
}

#[test]
fn same_threshold_can_fire_again_after_a_reset() {
    let mut tracker = AlertTracker::new();
    let below = snapshot(
        vec![limit_window(
            WindowKind::Session,
            74.0,
            Some(1_000),
            SourceKind::ClaudeStatusline,
        )],
        Vec::new(),
    );
    let first_crossing = snapshot(
        vec![limit_window(
            WindowKind::Session,
            90.0,
            Some(1_000),
            SourceKind::ClaudeStatusline,
        )],
        Vec::new(),
    );
    let after_reset = snapshot(
        vec![limit_window(
            WindowKind::Session,
            0.0,
            Some(2_000),
            SourceKind::ClaudeStatusline,
        )],
        Vec::new(),
    );
    let second_crossing = snapshot(
        vec![limit_window(
            WindowKind::Session,
            76.0,
            Some(2_000),
            SourceKind::ClaudeStatusline,
        )],
        Vec::new(),
    );

    assert!(tracker.evaluate(&below, &[75], UnixSeconds(100)).is_empty());
    assert_eq!(
        tracker
            .evaluate(&first_crossing, &[75], UnixSeconds(110))
            .len(),
        1
    );
    let reset_alerts = tracker.evaluate(&after_reset, &[75], UnixSeconds(120));
    assert_eq!(reset_alerts.len(), 1);
    assert_eq!(reset_alerts[0].title, "Claude session limit reset");
    assert_eq!(
        tracker
            .evaluate(&second_crossing, &[75], UnixSeconds(130))
            .len(),
        1
    );
}

#[test]
fn downward_change_does_not_fire_and_expired_dedupe_state_is_pruned() {
    let mut tracker = AlertTracker::new();
    let high = snapshot(
        vec![limit_window(
            WindowKind::Weekly,
            90.0,
            Some(1_000),
            SourceKind::CodexAppServer,
        )],
        Vec::new(),
    );
    let low = snapshot(
        vec![limit_window(
            WindowKind::Weekly,
            70.0,
            Some(1_000),
            SourceKind::CodexAppServer,
        )],
        Vec::new(),
    );
    assert!(tracker.evaluate(&high, &[75], UnixSeconds(100)).is_empty());
    assert!(tracker.evaluate(&low, &[75], UnixSeconds(110)).is_empty());

    let crossing = snapshot(
        vec![limit_window(
            WindowKind::Weekly,
            80.0,
            Some(1_000),
            SourceKind::CodexAppServer,
        )],
        Vec::new(),
    );
    assert_eq!(
        tracker.evaluate(&crossing, &[75], UnixSeconds(120)).len(),
        1
    );
    assert_eq!(tracker.retained_firings(), 1);
    assert!(
        tracker
            .evaluate(&snapshot(Vec::new(), Vec::new()), &[75], UnixSeconds(1_001))
            .is_empty()
    );
    assert_eq!(tracker.retained_firings(), 0);
}
