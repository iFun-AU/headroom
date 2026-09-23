//! Claude local JSONL parser contract tests.

#![allow(clippy::expect_used)]

use chrono::Utc;
use usage_core::{
    HistoryStore, Provider, SourceKind,
    parse::{ClaudeSession, Error, parse_claude_log_line, parse_claude_log_record},
};

#[test]
fn documented_duplicate_fixture_counts_exactly_once() {
    let events = include_str!("fixtures/claude_log.jsonl")
        .lines()
        .filter_map(|line| parse_claude_log_line(line).expect("fixture line should parse"))
        .collect::<Vec<_>>();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].provider, Provider::Claude);
    assert_eq!(events[0].source, SourceKind::ClaudeLocalLogs);
    assert_eq!(events[0].tokens.0, 92_586);
    assert_eq!(
        events[0].dedupe_key.as_deref(),
        Some("msg_01TEST:req_01TEST")
    );
    assert_eq!(events[0], events[1]);

    let mut history = HistoryStore::new();
    assert!(history.ingest_token(events[0].clone()));
    assert!(!history.ingest_token(events[1].clone()));
    let view = history.history_at_in_timezone(Provider::Claude, events[0].at, None, &Utc);
    assert_eq!(
        view.hourly
            .buckets
            .iter()
            .map(|bucket| bucket.tokens.0)
            .sum::<u64>(),
        92_586
    );
}

#[test]
fn non_assistant_and_assistant_without_usage_are_ignored() {
    for input in [
        r#"{"type":"user","timestamp":"2026-08-26T10:25:40Z"}"#,
        r#"{"type":"assistant","timestamp":"2026-08-26T10:25:40Z","message":{"id":"msg"}}"#,
        r#"{"type":"future","unknown":true}"#,
    ] {
        assert!(
            parse_claude_log_line(input)
                .expect("sparse shape should parse")
                .is_none()
        );
    }
}

#[test]
fn missing_usage_counts_default_to_zero_and_missing_identity_disables_dedupe() {
    let input = r#"{"type":"assistant","timestamp":"2026-08-26T10:25:40Z","message":{"usage":{"output_tokens":11}}}"#;
    let event = parse_claude_log_line(input)
        .expect("partial usage should parse")
        .expect("usage should produce an event");

    assert_eq!(event.tokens.0, 11);
    assert_eq!(event.dedupe_key, None);
}

#[test]
fn overflowing_token_components_are_rejected() {
    let input = r#"{"type":"assistant","timestamp":"2026-08-26T10:25:40Z","message":{"usage":{"input_tokens":18446744073709551615,"output_tokens":1}}}"#;
    assert!(matches!(
        parse_claude_log_line(input),
        Err(Error::TokenOverflow)
    ));
}

#[test]
fn entrypoint_classifies_status_line_capable_sessions() {
    let line = |entrypoint: &str| {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-08-26T10:25:40Z",{entrypoint}"message":{{"usage":{{"output_tokens":3}}}}}}"#
        )
    };
    for (entrypoint, expected) in [
        ("", ClaudeSession::Interactive),
        (r#""entrypoint":"cli","#, ClaudeSession::Interactive),
        (r#""entrypoint":"claude-desktop","#, ClaudeSession::Headless),
        (r#""entrypoint":"claude-vscode","#, ClaudeSession::Headless),
        (r#""entrypoint":"sdk-cli","#, ClaudeSession::Headless),
        (r#""entrypoint":"future-client","#, ClaudeSession::Headless),
    ] {
        let record = parse_claude_log_record(&line(entrypoint))
            .expect("record should parse")
            .expect("usage should produce a record");
        assert_eq!(record.session, expected, "entrypoint: {entrypoint}");
        assert_eq!(record.event.tokens.0, 3);
    }
}
