use std::collections::BTreeMap;

use chrono::{Days, Local, NaiveDate, TimeZone};

use crate::{
    Bucket, History, HistoryScope, Projection, Provider, Series, SourceKind, TokenCount,
    UnixSeconds,
    history_time::{
        SECONDS_PER_HOUR, first_retained_hour, local_date, local_day_start, local_hour_start,
        utc_day_start, utc_hour_start,
    },
};

const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;
const LOCAL_HOURLY_BUCKETS: i64 = 24;
const DAILY_BUCKETS: u64 = 7;
const DAILY_RECORDS_RETAINED: usize = 8;

/// One normalized token-count event from a local provider source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenEvent {
    /// Provider that consumed the tokens.
    pub provider: Provider,
    /// Source that observed the token count.
    pub source: SourceKind,
    /// Event time in UTC seconds.
    pub at: UnixSeconds,
    /// Tokens attributed to this event.
    pub tokens: TokenCount,
    /// Stable source-specific identity used to suppress duplicate events.
    pub dedupe_key: Option<String>,
}

/// A complete provider-reported set of daily account buckets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyBuckets {
    /// Provider represented by the buckets.
    pub provider: Provider,
    /// Source that supplied the buckets.
    pub source: SourceKind,
    /// Time when the provider data was refreshed.
    pub observed_at: UnixSeconds,
    /// UTC calendar dates and their token totals.
    pub days: Vec<(NaiveDate, TokenCount)>,
}

/// Bounded-storage counters exposed for diagnostics and memory regression tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryMetrics {
    /// Number of retained hourly source buckets.
    pub hourly_buckets: usize,
    /// Number of retained event identities.
    pub dedupe_keys: usize,
}

#[derive(Debug, Default)]
struct HourlyState {
    buckets: BTreeMap<UnixSeconds, u64>,
    observed_at: Option<UnixSeconds>,
}

#[derive(Debug, Default)]
struct DailyState {
    days: BTreeMap<NaiveDate, TokenCount>,
    observed_at: Option<UnixSeconds>,
}

/// In-memory, bounded token-history aggregation keyed by provider and source.
#[derive(Debug, Default)]
pub struct HistoryStore {
    hourly: BTreeMap<(Provider, SourceKind), HourlyState>,
    daily: BTreeMap<(Provider, SourceKind), DailyState>,
    dedupe: BTreeMap<(Provider, SourceKind, String), UnixSeconds>,
    watermark: Option<UnixSeconds>,
}

impl HistoryStore {
    /// Constructs an empty history store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a token event unless it is duplicated or older than the retention window.
    ///
    /// Returns `true` when tokens were added.
    #[must_use]
    pub fn ingest_token(&mut self, event: TokenEvent) -> bool {
        let TokenEvent {
            provider,
            source,
            at,
            tokens,
            dedupe_key,
        } = event;
        let watermark = self.watermark.map_or(at, |stored| stored.max(at));
        self.watermark = Some(watermark);
        self.prune(watermark);

        let earliest_hour = first_retained_hour(watermark);
        let event_hour = utc_hour_start(at);
        if event_hour < earliest_hour {
            return false;
        }

        if let Some(key) = dedupe_key {
            let identity = (provider, source, key);
            if self.dedupe.contains_key(&identity) {
                return false;
            }
            self.dedupe.insert(identity, at);
        }

        let source_state = self.hourly.entry((provider, source)).or_default();
        let bucket = source_state.buckets.entry(event_hour).or_default();
        *bucket = bucket.saturating_add(tokens.0);
        source_state.observed_at =
            Some(source_state.observed_at.map_or(at, |stored| stored.max(at)));
        true
    }

    /// Replaces a source's account-daily data unless the update is older than stored data.
    ///
    /// Returns `true` when the complete update was accepted.
    #[must_use]
    pub fn ingest_daily(&mut self, update: DailyBuckets) -> bool {
        let key = (update.provider, update.source);
        if self
            .daily
            .get(&key)
            .and_then(|state| state.observed_at)
            .is_some_and(|stored| update.observed_at < stored)
        {
            return false;
        }

        let mut days = update.days.into_iter().collect::<BTreeMap<_, _>>();
        while days.len() > DAILY_RECORDS_RETAINED {
            let Some(oldest) = days.first_key_value().map(|(date, _)| *date) else {
                break;
            };
            days.remove(&oldest);
        }
        self.daily.insert(
            key,
            DailyState {
                days,
                observed_at: Some(update.observed_at),
            },
        );
        true
    }

    /// Builds history using the Mac's current local timezone.
    #[must_use]
    pub fn history_at(
        &self,
        provider: Provider,
        now: UnixSeconds,
        projection: Option<Projection>,
    ) -> History {
        self.history_at_in_timezone(provider, now, projection, &Local)
    }

    /// Builds history using an explicit timezone, primarily for deterministic DST tests.
    #[must_use]
    pub fn history_at_in_timezone<Tz>(
        &self,
        provider: Provider,
        now: UnixSeconds,
        projection: Option<Projection>,
        timezone: &Tz,
    ) -> History
    where
        Tz: TimeZone,
    {
        let (local_source, local_label) = local_series_metadata(provider);
        let hourly = self.local_hourly_series(provider, local_source, local_label, now, timezone);
        let daily = if provider == Provider::Codex {
            self.fresh_codex_account_daily(now).map_or_else(
                || self.local_daily_series(provider, local_source, local_label, now, timezone),
                account_daily_series,
            )
        } else {
            self.local_daily_series(provider, local_source, local_label, now, timezone)
        };

        History {
            provider,
            hourly,
            daily,
            projection,
        }
    }

    /// Returns bounded-storage counters for diagnostics.
    #[must_use]
    pub fn metrics(&self) -> HistoryMetrics {
        HistoryMetrics {
            hourly_buckets: self.hourly.values().map(|state| state.buckets.len()).sum(),
            dedupe_keys: self.dedupe.len(),
        }
    }

    fn prune(&mut self, watermark: UnixSeconds) {
        let earliest_hour = first_retained_hour(watermark);
        for state in self.hourly.values_mut() {
            state.buckets.retain(|start, _| *start >= earliest_hour);
        }
        self.hourly.retain(|_, state| !state.buckets.is_empty());

        let dedupe_cutoff = UnixSeconds(watermark.0.saturating_sub(8 * SECONDS_PER_DAY));
        self.dedupe.retain(|_, at| *at > dedupe_cutoff);
    }

    fn local_hourly_series<Tz>(
        &self,
        provider: Provider,
        source: SourceKind,
        label: &str,
        now: UnixSeconds,
        timezone: &Tz,
    ) -> Series
    where
        Tz: TimeZone,
    {
        let state = self.hourly.get(&(provider, source));
        let current_hour = local_hour_start(now, timezone);
        let buckets = (0..LOCAL_HOURLY_BUCKETS)
            .rev()
            .map(|hours_ago| {
                let start = UnixSeconds(
                    current_hour
                        .0
                        .saturating_sub(hours_ago.saturating_mul(SECONDS_PER_HOUR)),
                );
                Bucket {
                    start,
                    tokens: TokenCount(
                        state
                            .and_then(|value| value.buckets.get(&start))
                            .copied()
                            .unwrap_or_default(),
                    ),
                }
            })
            .collect();

        Series {
            source,
            scope: HistoryScope::ThisMac,
            scope_label: label.to_owned(),
            observed_at: state.and_then(|value| value.observed_at),
            buckets,
        }
    }

    fn local_daily_series<Tz>(
        &self,
        provider: Provider,
        source: SourceKind,
        label: &str,
        now: UnixSeconds,
        timezone: &Tz,
    ) -> Series
    where
        Tz: TimeZone,
    {
        let state = self.hourly.get(&(provider, source));
        let current_date = local_date(now, timezone);
        let buckets = (0..DAILY_BUCKETS)
            .rev()
            .map(|days_ago| {
                let date = current_date
                    .checked_sub_days(Days::new(days_ago))
                    .unwrap_or(current_date);
                let tokens = state.map_or(0, |value| {
                    value
                        .buckets
                        .iter()
                        .filter(|(start, _)| local_date(**start, timezone) == date)
                        .fold(0_u64, |sum, (_, count)| sum.saturating_add(*count))
                });
                Bucket {
                    start: local_day_start(date, timezone),
                    tokens: TokenCount(tokens),
                }
            })
            .collect();

        Series {
            source,
            scope: HistoryScope::ThisMac,
            scope_label: label.to_owned(),
            observed_at: state.and_then(|value| value.observed_at),
            buckets,
        }
    }

    fn fresh_codex_account_daily(&self, now: UnixSeconds) -> Option<&DailyState> {
        self.daily
            .get(&(Provider::Codex, SourceKind::CodexAppServer))
            .filter(|state| !state.days.is_empty())
            .filter(|state| {
                state
                    .observed_at
                    .is_some_and(|at| now.0.saturating_sub(at.0) < SECONDS_PER_DAY)
            })
    }
}

fn local_series_metadata(provider: Provider) -> (SourceKind, &'static str) {
    match provider {
        Provider::Claude => (SourceKind::ClaudeLocalLogs, "Claude Code on this Mac"),
        Provider::Codex => (SourceKind::CodexRollout, "Codex CLI on this Mac"),
    }
}

fn account_daily_series(state: &DailyState) -> Series {
    let latest_date = state
        .days
        .last_key_value()
        .map_or(NaiveDate::MAX, |(date, _)| *date);
    let buckets = (0..DAILY_BUCKETS)
        .rev()
        .map(|days_ago| {
            let date = latest_date
                .checked_sub_days(Days::new(days_ago))
                .unwrap_or(latest_date);
            Bucket {
                start: utc_day_start(date),
                tokens: state.days.get(&date).copied().unwrap_or(TokenCount(0)),
            }
        })
        .collect();

    Series {
        source: SourceKind::CodexAppServer,
        scope: HistoryScope::Account,
        scope_label: "All Codex usage (account)".to_owned(),
        observed_at: state.observed_at,
        buckets,
    }
}
