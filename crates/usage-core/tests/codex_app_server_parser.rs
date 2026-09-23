//! Codex app-server DTO-to-domain parser contract tests.

#![allow(clippy::expect_used)]

use usage_core::{
    Provider, SourceKind, UnixSeconds, WindowKind,
    parse::{parse_codex_rate_limits_notification, parse_codex_rate_limits_response},
};

const OBSERVED_AT: UnixSeconds = UnixSeconds(1_790_000_000);

#[test]
fn documented_response_yields_one_full_weekly_reading() {
    let reading = parse_codex_rate_limits_response(
        include_str!("fixtures/codex_rate_limits_read.json"),
        OBSERVED_AT,
    )
    .expect("fixture should parse")
    .expect("codex snapshot should produce a reading");

    assert_eq!(reading.provider, Provider::Codex);
    assert_eq!(reading.source, SourceKind::CodexAppServer);
    assert_eq!(reading.observed_at, OBSERVED_AT);
    assert_eq!(reading.plan.as_deref(), Some("Pro"));
    assert!(!reading.partial);
    assert_eq!(reading.windows.len(), 1);
    assert_eq!(reading.windows[0].kind, WindowKind::Weekly);
    assert!((reading.windows[0].used.get() - 2.0).abs() < f64::EPSILON);
    assert_eq!(
        reading.windows[0].resets_at,
        Some(UnixSeconds(1_790_725_056))
    );
}

#[test]
fn sparse_notification_yields_only_secondary_as_partial() {
    let reading = parse_codex_rate_limits_notification(
        include_str!("fixtures/codex_rate_limits_updated.json"),
        OBSERVED_AT,
    )
    .expect("fixture should parse")
    .expect("codex notification should produce a reading");

    assert!(reading.partial);
    assert_eq!(reading.windows.len(), 1);
    assert_eq!(reading.windows[0].kind, WindowKind::Session);
    assert!((reading.windows[0].used.get() - 37.0).abs() < f64::EPSILON);
}

#[test]
fn premium_map_entry_never_contributes_windows() {
    let input = r#"{
      "result": {
        "rateLimitsByLimitId": {
          "codex": {
            "limitId": "codex",
            "primary": {"usedPercent": 4, "windowDurationMins": 10080}
          },
          "premium": {
            "limitId": "premium",
            "primary": {"usedPercent": 99, "windowDurationMins": 300}
          }
        }
      }
    }"#;

    let reading = parse_codex_rate_limits_response(input, OBSERVED_AT)
        .expect("shape should parse")
        .expect("codex map entry should produce a reading");
    assert_eq!(reading.windows.len(), 1);
    assert_eq!(reading.windows[0].kind, WindowKind::Weekly);
    assert!((reading.windows[0].used.get() - 4.0).abs() < f64::EPSILON);
}

#[test]
fn premium_only_and_unknown_notifications_are_ignored() {
    let premium = r#"{"result":{"rateLimits":{"limitId":"premium","primary":{"usedPercent":99,"windowDurationMins":300}}}}"#;
    assert!(
        parse_codex_rate_limits_response(premium, OBSERVED_AT)
            .expect("shape should parse")
            .is_none()
    );

    let unrelated = r#"{"method":"remoteControl/status/changed","params":{"newField":true}}"#;
    assert!(
        parse_codex_rate_limits_notification(unrelated, OBSERVED_AT)
            .expect("unknown methods should remain parseable")
            .is_none()
    );
}
