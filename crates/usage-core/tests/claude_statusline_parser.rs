//! Claude status-line and bridge-envelope parser contract tests.

#![allow(clippy::expect_used)]

use usage_core::{Provider, SourceKind, UnixSeconds, WindowKind, parse::parse_claude_statusline};

const FALLBACK_OBSERVED_AT: UnixSeconds = UnixSeconds(1_790_000_000);

#[test]
fn documented_input_yields_session_and_weekly_windows() {
    let reading = parse_claude_statusline(
        include_str!("fixtures/claude_statusline_input.json"),
        FALLBACK_OBSERVED_AT,
    )
    .expect("fixture should parse")
    .expect("rate limits should produce a reading");

    assert_eq!(reading.provider, Provider::Claude);
    assert_eq!(reading.source, SourceKind::ClaudeStatusline);
    assert_eq!(reading.observed_at, FALLBACK_OBSERVED_AT);
    assert_eq!(reading.plan, None);
    assert!(!reading.partial);
    assert_eq!(reading.windows.len(), 2);
    assert_eq!(reading.windows[0].kind, WindowKind::Session);
    assert!((reading.windows[0].used.get() - 23.5).abs() < f64::EPSILON);
    assert_eq!(
        reading.windows[0].resets_at,
        Some(UnixSeconds(1_738_425_600))
    );
    assert_eq!(reading.windows[1].kind, WindowKind::Weekly);
    assert!((reading.windows[1].used.get() - 41.2).abs() < f64::EPSILON);
}

#[test]
fn missing_five_hour_yields_only_weekly() {
    let input = r#"{"rate_limits":{"seven_day":{"used_percentage":52,"resets_at":1738857600}},"future":true}"#;
    let reading = parse_claude_statusline(input, FALLBACK_OBSERVED_AT)
        .expect("shape should parse")
        .expect("weekly window should produce a reading");

    assert_eq!(reading.windows.len(), 1);
    assert_eq!(reading.windows[0].kind, WindowKind::Weekly);
}

#[test]
fn bridge_envelope_uses_written_at_and_camel_case_field() {
    let input = r#"{"schema":1,"writtenAt":1790128800,"sessionId":"abc123","rateLimits":{"five_hour":{"used_percentage":9,"resets_at":1790132400}}}"#;
    let reading = parse_claude_statusline(input, FALLBACK_OBSERVED_AT)
        .expect("bridge file should parse")
        .expect("session window should produce a reading");

    assert_eq!(reading.observed_at, UnixSeconds(1_790_128_800));
    assert_eq!(reading.windows.len(), 1);
    assert_eq!(reading.windows[0].observed_at, reading.observed_at);
}

#[test]
fn absent_rate_limits_or_windows_yields_no_reading() {
    for input in [r#"{"session_id":"abc123"}"#, r#"{"rate_limits":{}}"#] {
        assert!(
            parse_claude_statusline(input, FALLBACK_OBSERVED_AT)
                .expect("sparse shape should parse")
                .is_none()
        );
    }
}
