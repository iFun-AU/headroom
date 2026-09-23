//! Limit-window classification contract tests.

use usage_core::{WindowKind, classify};

#[test]
fn classifies_session_boundaries_by_duration() {
    assert_eq!(classify(Some(300), None), WindowKind::Session);
    assert_eq!(classify(Some(240), None), WindowKind::Session);
    assert_eq!(classify(Some(360), None), WindowKind::Session);
}

#[test]
fn classifies_weekly_boundaries_by_duration() {
    assert_eq!(classify(Some(10_080), None), WindowKind::Weekly);
    assert_eq!(classify(Some(9_000), None), WindowKind::Weekly);
    assert_eq!(classify(Some(11_000), None), WindowKind::Weekly);
}

#[test]
fn preserves_other_positive_durations() {
    assert_eq!(classify(Some(60), None), WindowKind::Other { minutes: 60 });
}

#[test]
fn saturates_out_of_range_durations_without_integer_wrapping() {
    assert_eq!(
        classify(Some(i64::MAX), None),
        WindowKind::Other { minutes: u32::MAX }
    );
    assert_eq!(classify(Some(-1), None), WindowKind::Other { minutes: 0 });
}

#[test]
fn uses_hint_only_when_duration_is_absent() {
    assert_eq!(
        classify(None, Some(WindowKind::Session)),
        WindowKind::Session
    );
    assert_eq!(classify(None, Some(WindowKind::Weekly)), WindowKind::Weekly);
    assert_eq!(classify(None, None), WindowKind::Other { minutes: 0 });
}
