//! Codex rollout JSONL parser contract tests.

#![allow(clippy::expect_used)]

use usage_core::{Provider, SourceKind, UnixSeconds, WindowKind, parse::CodexRolloutParser};

#[test]
fn documented_fixture_yields_limits_tokens_and_legacy_reset() {
    let mut parser = CodexRolloutParser::new();
    let lines = include_str!("fixtures/codex_rollout.jsonl")
        .lines()
        .map(|line| parser.parse_line(line).expect("fixture line should parse"))
        .collect::<Vec<_>>();

    let first = &lines[0];
    let reading = first
        .reading
        .as_ref()
        .expect("codex limits should produce a reading");
    assert_eq!(reading.provider, Provider::Codex);
    assert_eq!(reading.source, SourceKind::CodexRollout);
    assert_eq!(reading.observed_at, UnixSeconds(1_790_125_989));
    assert_eq!(reading.plan.as_deref(), Some("Pro"));
    assert!(!reading.partial);
    assert_eq!(reading.windows.len(), 1);
    assert_eq!(reading.windows[0].kind, WindowKind::Weekly);
    assert!((reading.windows[0].used.get() - 1.0).abs() < f64::EPSILON);
    assert_eq!(
        first.token.as_ref().map(|event| event.tokens.0),
        Some(211_554)
    );

    assert!(lines[1].reading.is_none());
    assert!(lines[1].token.is_none());
    assert_eq!(lines[1].filtered_limit_id.as_deref(), Some("premium"));

    let legacy = lines[2]
        .reading
        .as_ref()
        .expect("legacy codex limits should produce a reading");
    assert_eq!(legacy.windows[0].kind, WindowKind::Session);
    assert_eq!(
        legacy.windows[0].resets_at,
        Some(UnixSeconds(1_790_132_400))
    );
}

#[test]
fn forked_session_counts_only_new_tokens() {
    let mut parser = CodexRolloutParser::new();
    let deltas = include_str!("fixtures/codex_rollout_fork.jsonl")
        .lines()
        .map(|line| {
            parser
                .parse_line(line)
                .expect("fixture line should parse")
                .token
                .expect("each info record should yield a token event")
                .tokens
                .0
        })
        .collect::<Vec<_>>();

    assert_eq!(deltas, vec![1_000, 0, 2_000]);
    assert_eq!(deltas.into_iter().sum::<u64>(), 3_000);
}

#[test]
fn counter_reset_uses_last_usage_and_premium_still_emits_tokens() {
    let mut parser = CodexRolloutParser::new();
    let first = r#"{"timestamp":"2026-09-23T03:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":100},"last_token_usage":{"total_tokens":7}},"rate_limits":{"limit_id":"premium","primary":{"used_percent":99,"window_minutes":300}}}}"#;
    let reset = r#"{"timestamp":"2026-09-23T03:01:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":20},"last_token_usage":{"total_tokens":5}},"rate_limits":{"limit_id":"premium"}}}"#;

    let first_event = parser.parse_line(first).expect("line should parse");
    assert!(first_event.reading.is_none());
    assert_eq!(
        first_event
            .token
            .expect("token should survive filter")
            .tokens
            .0,
        7
    );

    let reset_event = parser.parse_line(reset).expect("line should parse");
    assert!(reset_event.reading.is_none());
    assert_eq!(
        reset_event
            .token
            .expect("reset should yield last usage")
            .tokens
            .0,
        5
    );
}

#[test]
fn unrelated_lines_are_ignored_leniently() {
    let mut parser = CodexRolloutParser::new();
    let parsed = parser
        .parse_line(r#"{"type":"response_item","future":{"field":true}}"#)
        .expect("unknown line should remain parseable");

    assert!(parsed.reading.is_none());
    assert!(parsed.token.is_none());
}
