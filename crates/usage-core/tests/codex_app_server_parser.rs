//! Codex app-server DTO-to-domain parser contract tests.

#![allow(clippy::expect_used)]

use usage_core::{
    CreditAmount, Provider, SourceKind, UnixSeconds, WindowKind,
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

fn credits_response(credits: &str) -> String {
    format!(
        r#"{{"id":2,"result":{{"rateLimits":{{"limitId":"codex","primary":{{"usedPercent":2,"windowDurationMins":10080,"resetsAt":1790725056}},"credits":{credits}}}}}}}"#
    )
}

#[test]
fn credits_are_parsed_with_exact_decimal_balances() {
    let fixture = parse_codex_rate_limits_response(
        include_str!("fixtures/codex_rate_limits_read.json"),
        OBSERVED_AT,
    )
    .expect("fixture should parse")
    .expect("fixture should produce a reading")
    .credits
    .expect("fixture reports credits");
    assert!(!fixture.enabled);
    assert!(!fixture.unlimited);
    assert_eq!(
        fixture.balance,
        Some(CreditAmount {
            minor: 0,
            exponent: 0,
            currency: None,
        })
    );
    assert_eq!(fixture.source, SourceKind::CodexAppServer);

    let funded = parse_codex_rate_limits_response(
        &credits_response(r#"{"hasCredits":true,"unlimited":false,"balance":"1234.50"}"#),
        OBSERVED_AT,
    )
    .expect("should parse")
    .expect("should produce a reading")
    .credits
    .expect("credits present");
    assert!(funded.enabled);
    assert_eq!(
        funded.balance,
        Some(CreditAmount {
            minor: 123_450,
            exponent: 2,
            currency: None,
        })
    );
}

#[test]
fn unexpected_credit_shapes_drop_only_the_credits_or_balance() {
    for credits in [
        "null",
        r#""unexpected""#,
        r#"{"hasCredits":true}"#,
        r#"{"hasCredits":1,"unlimited":false,"balance":"1"}"#,
    ] {
        let reading = parse_codex_rate_limits_response(&credits_response(credits), OBSERVED_AT)
            .expect("odd credits must not fail the reading")
            .expect("windows still parse");
        assert_eq!(reading.windows.len(), 1, "{credits}");
        assert_eq!(reading.credits, None, "{credits}");
    }
    for balance in ["-5", "1e3", "", ".5", "1.2345678", "abc", "12"] {
        let credits = parse_codex_rate_limits_response(
            &credits_response(&format!(
                r#"{{"hasCredits":true,"unlimited":true,"balance":"{balance}"}}"#
            )),
            OBSERVED_AT,
        )
        .expect("should parse")
        .expect("should produce a reading")
        .credits
        .expect("flags are valid");
        assert!(credits.unlimited);
        assert_eq!(credits.balance.is_some(), balance == "12", "{balance}");
    }
}
