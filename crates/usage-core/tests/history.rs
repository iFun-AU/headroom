//! Bounded, source-isolated history aggregation contract tests.

#![allow(clippy::expect_used, clippy::too_many_lines, clippy::unwrap_used)]

use chrono::{NaiveDate, TimeZone, Utc};
use chrono_tz::America::New_York;
use usage_core::{
    DailyBuckets, HistoryScope, HistoryStore, Provider, SourceKind, TokenCount, TokenEvent,
    UnixSeconds,
};

fn utc_timestamp(year: i32, month: u32, day: u32, hour: u32) -> UnixSeconds {
    UnixSeconds(
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .expect("test UTC timestamp should exist")
            .timestamp(),
    )
}

fn token_event(
    provider: Provider,
    source: SourceKind,
    at: UnixSeconds,
    tokens: u64,
    dedupe_key: Option<&str>,
) -> TokenEvent {
    TokenEvent {
        provider,
        source,
        at,
        tokens: TokenCount(tokens),
        dedupe_key: dedupe_key.map(str::to_owned),
    }
}

fn token_sum(buckets: &[usage_core::Bucket]) -> u64 {
    buckets.iter().map(|bucket| bucket.tokens.0).sum()
}

#[test]
fn zero_filled_series_have_exact_lengths_sources_scopes_and_labels() {
    let store = HistoryStore::new();
    let now = utc_timestamp(2026, 9, 23, 12);

    let claude = store.history_at(Provider::Claude, now, None);
    assert_eq!(claude.hourly.buckets.len(), 24);
    assert_eq!(claude.daily.buckets.len(), 7);
    assert_eq!(token_sum(&claude.hourly.buckets), 0);
    assert_eq!(token_sum(&claude.daily.buckets), 0);
    assert_eq!(claude.hourly.source, SourceKind::ClaudeLocalLogs);
    assert_eq!(claude.daily.source, SourceKind::ClaudeLocalLogs);
    assert_eq!(claude.hourly.scope, HistoryScope::ThisMac);
    assert_eq!(claude.daily.scope, HistoryScope::ThisMac);
    assert_eq!(claude.hourly.scope_label, "Claude Code on this Mac");
    assert_eq!(claude.daily.scope_label, "Claude Code on this Mac");

    let codex = store.history_at(Provider::Codex, now, None);
    assert_eq!(codex.hourly.buckets.len(), 24);
    assert_eq!(codex.daily.buckets.len(), 7);
    assert_eq!(codex.hourly.source, SourceKind::CodexRollout);
    assert_eq!(codex.daily.source, SourceKind::CodexRollout);
    assert_eq!(codex.hourly.scope, HistoryScope::ThisMac);
    assert_eq!(codex.daily.scope, HistoryScope::ThisMac);
    assert_eq!(codex.hourly.scope_label, "Codex CLI on this Mac");
    assert_eq!(codex.daily.scope_label, "Codex CLI on this Mac");
}

#[test]
fn fresh_codex_account_daily_ends_at_latest_reported_utc_day() {
    let mut store = HistoryStore::new();
    let now = utc_timestamp(2026, 9, 23, 12);
    let days = (16..=22)
        .enumerate()
        .map(|(index, day)| {
            (
                NaiveDate::from_ymd_opt(2026, 9, day).expect("valid fixture date"),
                TokenCount(u64::try_from(index + 1).expect("small fixture count")),
            )
        })
        .collect();
    assert!(store.ingest_daily(DailyBuckets {
        provider: Provider::Codex,
        source: SourceKind::CodexAppServer,
        observed_at: UnixSeconds(now.0 - 60),
        days,
    }));

    let history = store.history_at(Provider::Codex, now, None);
    assert_eq!(history.daily.source, SourceKind::CodexAppServer);
    assert_eq!(history.daily.scope, HistoryScope::Account);
    assert_eq!(history.daily.scope_label, "All Codex usage (account)");
    assert_eq!(history.daily.buckets.len(), 7);
    assert_eq!(
        history.daily.buckets.last().map(|bucket| bucket.start),
        Some(utc_timestamp(2026, 9, 22, 0))
    );
    assert_eq!(
        history.daily.buckets.last().map(|bucket| bucket.tokens),
        Some(TokenCount(7))
    );
    assert!(
        history
            .daily
            .buckets
            .iter()
            .all(|bucket| bucket.start != utc_timestamp(2026, 9, 23, 0))
    );
}

#[test]
fn daily_updates_are_bounded_and_older_snapshots_are_rejected() {
    let mut store = HistoryStore::new();
    let now = utc_timestamp(2026, 9, 23, 12);
    let days = (13..=22)
        .map(|day| {
            (
                NaiveDate::from_ymd_opt(2026, 9, day).expect("valid fixture date"),
                TokenCount(u64::from(day)),
            )
        })
        .collect();
    assert!(store.ingest_daily(DailyBuckets {
        provider: Provider::Codex,
        source: SourceKind::CodexAppServer,
        observed_at: now,
        days,
    }));
    assert!(!store.ingest_daily(DailyBuckets {
        provider: Provider::Codex,
        source: SourceKind::CodexAppServer,
        observed_at: UnixSeconds(now.0 - 1),
        days: Vec::new(),
    }));

    let history = store.history_at(Provider::Codex, now, None);
    assert_eq!(history.daily.buckets.len(), 7);
    assert_eq!(history.daily.buckets[0].tokens, TokenCount(16));
    assert_eq!(history.daily.buckets[6].tokens, TokenCount(22));
}

#[test]
fn stale_codex_account_daily_falls_back_to_rollout() {
    let mut store = HistoryStore::new();
    let now = utc_timestamp(2026, 9, 23, 12);
    assert!(store.ingest_daily(DailyBuckets {
        provider: Provider::Codex,
        source: SourceKind::CodexAppServer,
        observed_at: UnixSeconds(now.0 - 86_400),
        days: vec![(
            NaiveDate::from_ymd_opt(2026, 9, 22).expect("valid fixture date"),
            TokenCount(9_999),
        )],
    }));
    assert!(store.ingest_token(token_event(
        Provider::Codex,
        SourceKind::CodexRollout,
        UnixSeconds(now.0 - 60),
        42,
        Some("rollout"),
    )));

    let history = store.history_at(Provider::Codex, now, None);
    assert_eq!(history.daily.source, SourceKind::CodexRollout);
    assert_eq!(history.daily.scope, HistoryScope::ThisMac);
    assert_eq!(history.daily.scope_label, "Codex CLI on this Mac");
    assert_eq!(token_sum(&history.daily.buckets), 42);
    assert_ne!(token_sum(&history.daily.buckets), 9_999);
}

#[test]
fn codex_hourly_and_daily_series_never_mix_sources() {
    let mut store = HistoryStore::new();
    let now = utc_timestamp(2026, 9, 23, 12);
    assert!(store.ingest_token(token_event(
        Provider::Codex,
        SourceKind::CodexRollout,
        UnixSeconds(now.0 - 60),
        50,
        Some("local"),
    )));
    assert!(store.ingest_token(token_event(
        Provider::Codex,
        SourceKind::CodexAppServer,
        UnixSeconds(now.0 - 60),
        5_000,
        Some("must-not-mix"),
    )));
    assert!(store.ingest_daily(DailyBuckets {
        provider: Provider::Codex,
        source: SourceKind::CodexAppServer,
        observed_at: now,
        days: vec![(
            NaiveDate::from_ymd_opt(2026, 9, 22).expect("valid fixture date"),
            TokenCount(7),
        )],
    }));

    let history = store.history_at(Provider::Codex, now, None);
    assert_eq!(history.hourly.source, SourceKind::CodexRollout);
    assert_eq!(token_sum(&history.hourly.buckets), 50);
    assert_eq!(history.daily.source, SourceKind::CodexAppServer);
    assert_eq!(token_sum(&history.daily.buckets), 7);
}

#[test]
fn token_events_are_deduplicated_per_provider_and_source() {
    let mut store = HistoryStore::new();
    let now = utc_timestamp(2026, 9, 23, 12);
    let first = token_event(
        Provider::Claude,
        SourceKind::ClaudeLocalLogs,
        UnixSeconds(now.0 - 60),
        100,
        Some("message:request"),
    );
    assert!(store.ingest_token(first.clone()));
    assert!(!store.ingest_token(first));
    assert!(store.ingest_token(token_event(
        Provider::Claude,
        SourceKind::ClaudeLocalLogs,
        UnixSeconds(now.0 - 30),
        20,
        None,
    )));

    let history = store.history_at(Provider::Claude, now, None);
    assert_eq!(token_sum(&history.hourly.buckets), 120);
    assert_eq!(store.metrics().dedupe_keys, 1);
}

#[test]
fn hourly_buckets_and_dedupe_keys_are_pruned_after_eight_days() {
    let mut store = HistoryStore::new();
    let now = utc_timestamp(2026, 9, 23, 12);
    let old = UnixSeconds(now.0 - (8 * 86_400) - 3_600);
    assert!(store.ingest_token(token_event(
        Provider::Codex,
        SourceKind::CodexRollout,
        old,
        10,
        Some("old"),
    )));
    assert!(store.ingest_token(token_event(
        Provider::Codex,
        SourceKind::CodexRollout,
        now,
        20,
        Some("new"),
    )));

    let metrics = store.metrics();
    assert_eq!(metrics.hourly_buckets, 1);
    assert_eq!(metrics.dedupe_keys, 1);
    assert!(!store.ingest_token(token_event(
        Provider::Codex,
        SourceKind::CodexRollout,
        old,
        10,
        Some("old"),
    )));
}

#[test]
fn fall_back_transition_day_aggregates_all_twenty_five_local_hours() {
    let mut store = HistoryStore::new();
    let start = New_York
        .with_ymd_and_hms(2026, 11, 1, 0, 0, 0)
        .single()
        .expect("local midnight should exist");
    let end = New_York
        .with_ymd_and_hms(2026, 11, 2, 0, 0, 0)
        .single()
        .expect("next local midnight should exist");
    assert_eq!(end.timestamp() - start.timestamp(), 25 * 3_600);

    let mut at = start.timestamp();
    while at < end.timestamp() {
        assert!(store.ingest_token(token_event(
            Provider::Claude,
            SourceKind::ClaudeLocalLogs,
            UnixSeconds(at),
            1,
            Some(&format!("hour-{at}")),
        )));
        at += 3_600;
    }

    let now = UnixSeconds(end.timestamp() - 1);
    let history = store.history_at_in_timezone(Provider::Claude, now, None, &New_York);
    assert_eq!(history.hourly.buckets.len(), 24);
    assert_eq!(history.daily.buckets.len(), 7);
    assert_eq!(
        history.daily.buckets.last().map(|bucket| bucket.start),
        Some(UnixSeconds(start.timestamp()))
    );
    assert_eq!(
        history.daily.buckets.last().map(|bucket| bucket.tokens),
        Some(TokenCount(25))
    );
}
