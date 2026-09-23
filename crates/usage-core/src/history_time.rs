use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};

use crate::UnixSeconds;

pub(super) const SECONDS_PER_HOUR: i64 = 3_600;
const HOURS_RETAINED: i64 = 8 * 24;

pub(super) fn utc_hour_start(at: UnixSeconds) -> UnixSeconds {
    UnixSeconds(at.0.div_euclid(SECONDS_PER_HOUR) * SECONDS_PER_HOUR)
}

pub(super) fn first_retained_hour(watermark: UnixSeconds) -> UnixSeconds {
    UnixSeconds(
        utc_hour_start(watermark)
            .0
            .saturating_sub((HOURS_RETAINED - 1) * SECONDS_PER_HOUR),
    )
}

pub(super) fn local_hour_start<Tz>(at: UnixSeconds, timezone: &Tz) -> UnixSeconds
where
    Tz: TimeZone,
{
    local_datetime(at, timezone).map_or_else(
        || utc_hour_start(at),
        |value| {
            let aligned = value
                .with_minute(0)
                .and_then(|date| date.with_second(0))
                .and_then(|date| date.with_nanosecond(0));
            UnixSeconds(aligned.map_or(at.0, |date| date.timestamp()))
        },
    )
}

pub(super) fn local_date<Tz>(at: UnixSeconds, timezone: &Tz) -> NaiveDate
where
    Tz: TimeZone,
{
    local_datetime(at, timezone).map_or(NaiveDate::MIN, |value| value.date_naive())
}

fn local_datetime<Tz>(at: UnixSeconds, timezone: &Tz) -> Option<DateTime<Tz>>
where
    Tz: TimeZone,
{
    DateTime::<Utc>::from_timestamp(at.0, 0).map(|value| value.with_timezone(timezone))
}

pub(super) fn local_day_start<Tz>(date: NaiveDate, timezone: &Tz) -> UnixSeconds
where
    Tz: TimeZone,
{
    let Some(midnight) = date.and_hms_opt(0, 0, 0) else {
        return UnixSeconds(0);
    };
    if let Some(value) = earliest_local(timezone.from_local_datetime(&midnight)) {
        return UnixSeconds(value.timestamp());
    }

    for minute in 1..=(24 * 60) {
        let Some(candidate) = midnight.checked_add_signed(chrono::Duration::minutes(minute)) else {
            break;
        };
        if let Some(value) = earliest_local(timezone.from_local_datetime(&candidate)) {
            return UnixSeconds(value.timestamp());
        }
    }
    UnixSeconds(0)
}

fn earliest_local<Tz>(result: LocalResult<DateTime<Tz>>) -> Option<DateTime<Tz>>
where
    Tz: TimeZone,
{
    match result {
        LocalResult::None => None,
        LocalResult::Single(value) => Some(value),
        LocalResult::Ambiguous(first, second) => {
            if first.timestamp() <= second.timestamp() {
                Some(first)
            } else {
                Some(second)
            }
        }
    }
}

pub(super) fn utc_day_start(date: NaiveDate) -> UnixSeconds {
    UnixSeconds(
        date.and_hms_opt(0, 0, 0)
            .map_or(0, |value: NaiveDateTime| value.and_utc().timestamp()),
    )
}
