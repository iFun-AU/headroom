use crate::WindowKind;

/// Classifies a provider window by duration, using a source hint only when absent.
#[must_use]
pub fn classify(window_minutes: Option<i64>, hint: Option<WindowKind>) -> WindowKind {
    match window_minutes {
        Some(240..=360) => WindowKind::Session,
        Some(9_000..=11_000) => WindowKind::Weekly,
        Some(minutes) => WindowKind::Other {
            minutes: saturating_minutes(minutes),
        },
        None => hint.unwrap_or(WindowKind::Other { minutes: 0 }),
    }
}

fn saturating_minutes(minutes: i64) -> u32 {
    if minutes <= 0 {
        return 0;
    }

    u32::try_from(minutes).unwrap_or(u32::MAX)
}
