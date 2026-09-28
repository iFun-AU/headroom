//! Conservative reset timestamps for Claude's human-readable terminal labels.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::{UnixSeconds, WindowKind};

pub(super) fn reset_time(text: &str, now: UnixSeconds, kind: WindowKind) -> Option<UnixSeconds> {
    let raw = text
        .trim()
        .strip_prefix("Resets ")
        .or_else(|| text.trim().strip_prefix("Reset "))?;
    if let Ok(date) = DateTime::parse_from_rfc3339(raw) {
        return Some(UnixSeconds(date.timestamp()));
    }
    // The probe explicitly sets TZ=UTC. An explicit IANA zone takes precedence.
    let (raw, zone) = if let Some((label, zone)) = raw.rsplit_once(" (") {
        (label, zone.strip_suffix(')')?.parse::<Tz>().ok()?)
    } else {
        (raw, chrono_tz::UTC)
    };
    let now = DateTime::<Utc>::from_timestamp(now.0, 0)?.with_timezone(&zone);
    let horizon = match kind {
        WindowKind::Session => 5 * 3600,
        _ => 7 * 86400,
    };
    let raw = raw.trim().to_ascii_lowercase();
    let (date, clock) = if let Some((date, clock)) = raw.split_once(" at ") {
        (Some(date), clock)
    } else {
        (None, raw.as_str())
    };
    let mut clock = clock.replace(' ', "");
    if !clock.contains(':') && (clock.ends_with("am") || clock.ends_with("pm")) {
        clock.insert_str(clock.len() - 2, ":00");
    }
    let time = ["%I:%M%P", "%H:%M"]
        .iter()
        .find_map(|format| NaiveTime::parse_from_str(&clock, format).ok())?;
    for offset in 0..=7 {
        let candidate = now
            .date_naive()
            .checked_add_signed(Duration::days(offset))?;
        if let Some(date) = date {
            let parsed = ["%b %d %Y", "%b %d, %Y", "%B %d %Y"]
                .iter()
                .find_map(|format| NaiveDate::parse_from_str(date, format).ok())
                .or_else(|| {
                    NaiveDate::parse_from_str(&format!("{date} {}", candidate.year()), "%b %d %Y")
                        .ok()
                });
            if parsed != Some(candidate) {
                continue;
            }
        }
        // During a DST fold prefer the earliest future occurrence. Never invent a
        // reset in a nonexistent local time or beyond the reported window length.
        let local = zone.from_local_datetime(&candidate.and_time(time));
        for at in [local.earliest(), local.latest()].into_iter().flatten() {
            let seconds = at.timestamp() - now.timestamp();
            if seconds > 0 && seconds <= horizon {
                return Some(UnixSeconds(at.timestamp()));
            }
        }
    }
    None
}
