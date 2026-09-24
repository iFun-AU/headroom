//! Weekly projection and alert-transition contract tests.

#![allow(clippy::expect_used, clippy::float_cmp)]

use usage_core::{
    AlertKind, AlertTracker, ConnectionStatus, LimitWindow, Percent, Provider, ProviderUsage,
    SourceKind, UnixSeconds, UsageSnapshot, WindowKind, WindowMinutes, project_weekly,
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

/// Marks every window as observed at `now`, as a fresh source reading would be.
fn observed(mut snapshot: UsageSnapshot, now: i64) -> UsageSnapshot {
    for window in snapshot
        .claude
        .windows
        .iter_mut()
        .chain(snapshot.codex.windows.iter_mut())
    {
        window.observed_at = UnixSeconds(now);
    }
    snapshot.generated_at = UnixSeconds(now);
    snapshot
}

fn claude_session(used: f64, resets_at: i64, source: SourceKind, now: i64) -> UsageSnapshot {
    observed(
        snapshot(
            vec![limit_window(
                WindowKind::Session,
                used,
                Some(resets_at),
                source,
            )],
            Vec::new(),
        ),
        now,
    )
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
    assert_eq!(alerts[0].kind, AlertKind::Threshold);
    assert_eq!(alerts[0].title, "Claude 5-hour limit at 75%");

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
    let source = SourceKind::ClaudeStatusline;

    assert!(
        tracker
            .evaluate(
                &claude_session(74.0, 1_000, source, 100),
                &[75],
                UnixSeconds(100)
            )
            .is_empty()
    );
    assert_eq!(
        tracker
            .evaluate(
                &claude_session(90.0, 1_000, source, 110),
                &[75],
                UnixSeconds(110)
            )
            .len(),
        1
    );
    let reset_alerts = tracker.evaluate(
        &claude_session(0.0, 19_000, source, 1_010),
        &[75],
        UnixSeconds(1_010),
    );
    assert_eq!(reset_alerts.len(), 1);
    assert_eq!(reset_alerts[0].kind, AlertKind::Reset);
    assert_eq!(reset_alerts[0].title, "Claude 5-hour limit reset");
    assert_eq!(
        tracker
            .evaluate(
                &claude_session(76.0, 19_000, source, 1_020),
                &[75],
                UnixSeconds(1_020)
            )
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

#[test]
fn thresholds_are_validated_deduplicated_and_label_custom_codex_windows() {
    let mut tracker = AlertTracker::new();
    let kind = WindowKind::Other { minutes: 30 };
    let below = snapshot(
        Vec::new(),
        vec![limit_window(kind, 70.0, None, SourceKind::CodexAppServer)],
    );
    let crossed = snapshot(
        Vec::new(),
        vec![limit_window(kind, 90.0, None, SourceKind::CodexAppServer)],
    );

    assert!(
        tracker
            .evaluate(&below, &[0, 75, 75, 90, 101], UnixSeconds(100))
            .is_empty()
    );
    let alerts = tracker.evaluate(&crossed, &[0, 75, 75, 90, 101], UnixSeconds(110));
    assert_eq!(alerts.len(), 2);
    assert_eq!(alerts[0].title, "Codex 30-minute limit at 75%");
    assert_eq!(alerts[0].body, "Reset time unavailable");
    assert_eq!(alerts[1].title, "Codex 30-minute limit at 90%");
    assert_eq!(alerts[1].body, "Reset time unavailable");
    assert_eq!(tracker.retained_firings(), 2);

    assert!(
        tracker
            .evaluate(
                &snapshot(Vec::new(), Vec::new()),
                &[75, 90],
                UnixSeconds(120),
            )
            .is_empty()
    );
    assert_eq!(tracker.retained_firings(), 0);
}

#[test]
fn reset_alert_requires_heavy_prior_usage_and_a_later_reset() {
    let mut tracker = AlertTracker::new();
    let codex_weekly = |used: f64, resets_at: i64, now: i64| {
        observed(
            snapshot(
                Vec::new(),
                vec![limit_window(
                    WindowKind::Weekly,
                    used,
                    Some(resets_at),
                    SourceKind::CodexAppServer,
                )],
            ),
            now,
        )
    };

    // 89% is not heavy, so the rollover at 1_000 stays silent.
    assert!(
        tracker
            .evaluate(&codex_weekly(89.0, 1_000, 100), &[], UnixSeconds(100))
            .is_empty()
    );
    assert!(
        tracker
            .evaluate(&codex_weekly(95.0, 5_000, 1_010), &[], UnixSeconds(1_010))
            .is_empty()
    );
    // An unchanged reset time is not a rollover.
    assert!(
        tracker
            .evaluate(&codex_weekly(96.0, 5_000, 1_020), &[], UnixSeconds(1_020))
            .is_empty()
    );
    let alerts = tracker.evaluate(&codex_weekly(1.0, 9_000, 5_010), &[], UnixSeconds(5_010));
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, AlertKind::Reset);
    assert_eq!(alerts[0].title, "Codex weekly limit reset");
    assert!(
        alerts[0].body.starts_with("Usage is back to 1% · Resets "),
        "unexpected body: {}",
        alerts[0].body
    );
}

/// 2026-09-24 14:00:00 AEST, the reset in the owner's report.
const RESET: i64 = 1_790_222_400;
const MINUTE: i64 = 60;

#[test]
fn restart_with_stale_bridge_data_does_not_refire_thresholds() {
    // Owner report: after a restart the stale status-line file (13%, written
    // hours earlier) became the baseline, then the usage API's 100% fired
    // 75/90/100 again.
    let mut tracker = AlertTracker::new();
    let now = RESET - 44 * MINUTE;
    let stale = claude_session(
        13.0,
        RESET,
        SourceKind::ClaudeStatusline,
        RESET - 190 * MINUTE,
    );
    assert!(
        tracker
            .evaluate(&stale, &[75, 90, 100], UnixSeconds(now))
            .is_empty()
    );

    let fresh = claude_session(100.0, RESET - 1, SourceKind::ClaudeOAuth, now + 5);
    assert!(
        tracker
            .evaluate(&fresh, &[75, 90, 100], UnixSeconds(now + 5))
            .is_empty()
    );
}

#[test]
fn one_second_reset_jitter_between_sources_is_not_a_reset() {
    // Owner report: the usage API says 13:59:59 and the status line 14:00:00;
    // switching between them fired "limit reset" 40 minutes early and let the
    // same thresholds fire again under a new key.
    let mut tracker = AlertTracker::new();
    let thresholds = [75, 90, 100];
    let now = RESET - 60 * MINUTE;
    assert!(
        tracker
            .evaluate(
                &claude_session(70.0, RESET - 1, SourceKind::ClaudeOAuth, now),
                &thresholds,
                UnixSeconds(now),
            )
            .is_empty()
    );
    let crossed = tracker.evaluate(
        &claude_session(100.0, RESET - 1, SourceKind::ClaudeOAuth, now + MINUTE),
        &thresholds,
        UnixSeconds(now + MINUTE),
    );
    assert_eq!(crossed.len(), 3);
    for alert in &crossed {
        assert!(
            !alert.body.contains(":59"),
            "reset time should round to the minute: {}",
            alert.body
        );
    }

    let readings = [
        (100.0, RESET, SourceKind::ClaudeStatusline),
        (70.0, RESET - 1, SourceKind::ClaudeOAuth),
        (100.0, RESET, SourceKind::ClaudeStatusline),
    ];
    for (step, (used, resets_at, source)) in (2..).zip(readings) {
        let at = now + step * MINUTE;
        assert!(
            tracker
                .evaluate(
                    &claude_session(used, resets_at, source, at),
                    &thresholds,
                    UnixSeconds(at)
                )
                .is_empty(),
            "step {step} fired an alert"
        );
    }
}

#[test]
fn stale_readings_keep_the_previous_baseline() {
    let mut tracker = AlertTracker::new();
    let now = RESET - 120 * MINUTE;
    let source = SourceKind::ClaudeStatusline;
    assert!(
        tracker
            .evaluate(
                &claude_session(70.0, RESET, source, now),
                &[75],
                UnixSeconds(now)
            )
            .is_empty()
    );
    let later = now + 30 * MINUTE;
    assert!(
        tracker
            .evaluate(
                &claude_session(80.0, RESET, source, now),
                &[75],
                UnixSeconds(later)
            )
            .is_empty()
    );
    assert_eq!(
        tracker
            .evaluate(
                &claude_session(80.0, RESET, source, later),
                &[75],
                UnixSeconds(later)
            )
            .len(),
        1
    );
}

#[test]
fn reset_waits_for_the_previous_reset_time_and_survives_a_pending_reset() {
    let mut tracker = AlertTracker::new();
    let source = SourceKind::ClaudeOAuth;
    let now = RESET - 30 * MINUTE;
    assert!(
        tracker
            .evaluate(
                &claude_session(95.0, RESET, source, now),
                &[],
                UnixSeconds(now)
            )
            .is_empty()
    );
    // A much later reset reported before the window ends is not a rollover.
    let early = now + MINUTE;
    let moved = RESET + 60 * MINUTE;
    assert!(
        tracker
            .evaluate(
                &claude_session(95.0, moved, source, early),
                &[],
                UnixSeconds(early)
            )
            .is_empty()
    );

    // The reset passes without a fresh reading: the store shows 0% pending.
    let after = moved + MINUTE;
    let mut pending = claude_session(0.0, moved, source, now);
    pending.claude.windows[0].reset_pending = true;
    assert!(
        tracker
            .evaluate(&pending, &[], UnixSeconds(after))
            .is_empty()
    );

    let next_window = moved + 300 * MINUTE;
    let alerts = tracker.evaluate(
        &claude_session(2.0, next_window, source, after + MINUTE),
        &[],
        UnixSeconds(after + MINUTE),
    );
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, AlertKind::Reset);
    assert!(alerts[0].body.starts_with("Usage is back to 2% · Resets "));
}
