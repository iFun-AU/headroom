//! Domain value and bounded-logging contract tests.

use std::borrow::Cow;

use usage_core::{Percent, PercentError, SourceKind, log_trunc};

#[test]
fn percent_rejects_non_finite_values() {
    assert!(Percent::new(f64::NAN).is_err());
    assert!(Percent::new(f64::INFINITY).is_err());
    assert!(Percent::new(f64::NEG_INFINITY).is_err());
}

#[test]
fn percent_clamps_finite_values_to_the_valid_range() -> Result<(), PercentError> {
    assert!((Percent::new(120.0)?.get() - 100.0).abs() < f64::EPSILON);
    assert!(Percent::new(-1.0)?.get().abs() < f64::EPSILON);
    assert!((Percent::new(47.25)?.get() - 47.25).abs() < f64::EPSILON);
    Ok(())
}

#[test]
fn log_trunc_borrows_short_text() {
    let input = "brief diagnostic";
    assert_eq!(log_trunc(input), Cow::Borrowed(input));
}

#[test]
fn log_trunc_caps_ascii_at_two_kibibytes() {
    let input = "a".repeat(2_049);
    let truncated = log_trunc(&input);

    assert_eq!(truncated.len(), 2_048);
    assert_eq!(truncated.as_ref(), "a".repeat(2_048));
}

#[test]
fn log_trunc_never_splits_a_utf8_character() {
    let input = format!("{}é", "a".repeat(2_047));
    let truncated = log_trunc(&input);

    assert_eq!(truncated.len(), 2_047);
    assert!(truncated.ends_with('a'));
    assert!(truncated.is_char_boundary(truncated.len()));
}

#[test]
fn source_limit_priority_is_explicit_and_history_only_sources_sort_last() {
    assert_eq!(SourceKind::CodexAppServer.limit_priority(), 0);
    assert_eq!(SourceKind::ClaudeStatusline.limit_priority(), 0);
    assert_eq!(SourceKind::CodexRollout.limit_priority(), 1);
    assert_eq!(SourceKind::ClaudeOAuth.limit_priority(), 1);
    assert_eq!(SourceKind::ClaudeLocalLogs.limit_priority(), u8::MAX);
}
