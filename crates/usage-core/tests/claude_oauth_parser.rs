//! Claude OAuth usage parser contract tests (D-024 verified shape).

#![allow(clippy::expect_used)]

use usage_core::{Provider, SourceKind, UnixSeconds, WindowKind, parse::parse_claude_oauth_usage};

const OBSERVED: UnixSeconds = UnixSeconds(1_790_000_000);

fn kinds(reading: &usage_core::Reading) -> Vec<WindowKind> {
    reading.windows.iter().map(|window| window.kind).collect()
}

#[test]
fn verified_fixture_maps_only_session_and_weekly() {
    let reading = parse_claude_oauth_usage(
        include_str!("fixtures/claude_oauth_usage.json"),
        OBSERVED,
        Some("max"),
    )
    .expect("fixture should parse")
    .expect("fixture should contain windows");

    assert_eq!(reading.provider, Provider::Claude);
    assert_eq!(reading.source, SourceKind::ClaudeOAuth);
    assert_eq!(reading.observed_at, OBSERVED);
    assert_eq!(reading.plan.as_deref(), Some("Max"));
    assert!(!reading.partial);
    assert_eq!(kinds(&reading), [WindowKind::Session, WindowKind::Weekly]);
    assert!((reading.windows[0].used.get() - 23.0).abs() < f64::EPSILON);
    assert!((reading.windows[1].used.get() - 41.5).abs() < f64::EPSILON);
    // 2026-09-23T15:00:00Z
    assert_eq!(
        reading.windows[0].resets_at,
        Some(UnixSeconds(1_790_175_600))
    );
    assert!(
        reading.windows.iter().all(
            |window| window.source == SourceKind::ClaudeOAuth && window.observed_at == OBSERVED
        )
    );
}

#[test]
fn resets_at_accepts_iso_seconds_milliseconds_and_null() {
    for (resets_at, expected) in [
        (
            r#""2026-09-23T15:00:00Z""#,
            Some(UnixSeconds(1_790_175_600)),
        ),
        ("1790175600", Some(UnixSeconds(1_790_175_600))),
        ("1790175600123", Some(UnixSeconds(1_790_175_600))),
        ("null", None),
        (r#""not a date""#, None),
    ] {
        let input = format!(r#"{{"five_hour":{{"utilization":10,"resets_at":{resets_at}}}}}"#);
        let reading = parse_claude_oauth_usage(&input, OBSERVED, None)
            .expect("lenient reset should parse")
            .expect("window should exist");
        assert_eq!(
            reading.windows[0].resets_at, expected,
            "resets_at: {resets_at}"
        );
        assert_eq!(reading.plan, None);
    }
}

#[test]
fn missing_windows_or_utilization_produce_no_reading() {
    for input in [
        "{}",
        r#"{"five_hour":null,"seven_day":null,"future_key":{"utilization":3}}"#,
        r#"{"five_hour":{"resets_at":null}}"#,
    ] {
        assert!(
            parse_claude_oauth_usage(input, OBSERVED, Some("pro"))
                .expect("sparse response should parse")
                .is_none(),
            "input: {input}"
        );
    }
}

#[test]
fn single_window_and_out_of_range_values_are_clamped() {
    let reading = parse_claude_oauth_usage(
        r#"{"seven_day":{"utilization":140.0,"resets_at":null}}"#,
        OBSERVED,
        None,
    )
    .expect("single window should parse")
    .expect("single window should produce a reading");
    assert_eq!(kinds(&reading), [WindowKind::Weekly]);
    assert!((reading.windows[0].used.get() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn invalid_json_and_wrongly_typed_known_fields_are_errors() {
    for input in ["not json", r#"{"five_hour":{"utilization":"high"}}"#] {
        assert!(
            parse_claude_oauth_usage(input, OBSERVED, None).is_err(),
            "input: {input}"
        );
    }
}
