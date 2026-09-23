use num_traits::ToPrimitive;

use crate::{LimitWindow, Projection, UnixSeconds, WindowKind, WindowMinutes};

const MINIMUM_PROJECTION_AGE_SECONDS: i64 = 6 * 60 * 60;

/// Projects weekly percentage usage linearly to the provider reset.
///
/// Session and other windows, windows without a reset, zero-duration windows,
/// and weekly windows open for less than six hours do not produce a projection.
#[must_use]
pub fn project_weekly(
    window: &LimitWindow,
    duration: WindowMinutes,
    now: UnixSeconds,
) -> Option<Projection> {
    if window.kind != WindowKind::Weekly {
        return None;
    }

    let resets_at = window.resets_at?;
    let duration_seconds = i64::from(duration.0).checked_mul(60)?;
    if duration_seconds == 0 {
        return None;
    }

    let window_start = resets_at.0.saturating_sub(duration_seconds);
    let elapsed_seconds = now.0.saturating_sub(window_start);
    if elapsed_seconds < MINIMUM_PROJECTION_AGE_SECONDS {
        return None;
    }

    let elapsed_fraction =
        (elapsed_seconds.to_f64()? / duration_seconds.to_f64()?).clamp(0.01, 1.0);
    let projected_percent_at_reset = window.used.get() / elapsed_fraction;
    let hits_limit_at = if projected_percent_at_reset > 100.0 {
        let hit_offset = (duration_seconds.to_f64()? * (100.0 / projected_percent_at_reset))
            .round()
            .to_i64()?;
        let candidate = UnixSeconds(window_start.saturating_add(hit_offset));
        (candidate < resets_at).then_some(candidate)
    } else {
        None
    };

    Some(Projection {
        projected_percent_at_reset,
        hits_limit_at,
    })
}
