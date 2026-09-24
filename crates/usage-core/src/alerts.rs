use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Local};

use crate::{Percent, Provider, UnixSeconds, UsageSnapshot, WindowKind};

type WindowKey = (Provider, WindowKind);
type FiringKey = (Provider, WindowKind, u8, Option<UnixSeconds>);

/// Readings older than this never move the alert baseline (matches merge freshness).
const FRESH_SECONDS: i64 = 15 * 60;
/// Reset times this close describe the same window. Claude's status line and
/// usage API report the same reset up to a second apart, so exact comparison
/// would mistake a source switch for a new window.
const SAME_RESET_TOLERANCE_SECONDS: i64 = 5 * 60;

#[derive(Debug, Clone, Copy)]
struct PreviousWindow {
    used: Percent,
    /// The first reset time reported for this window, kept while later
    /// readings stay within [`SAME_RESET_TOLERANCE_SECONDS`] of it.
    resets_at: Option<UnixSeconds>,
}

/// A user-facing notification request produced by core transition logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    /// Transition category used by notification preferences.
    pub kind: AlertKind,
    /// Notification title.
    pub title: String,
    /// Short notification body.
    pub body: String,
}

/// Notification transition category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    /// A configured usage threshold was crossed upward.
    Threshold,
    /// A heavily used window rolled over to a later reset.
    Reset,
}

/// Stateful upward-crossing and reset-alert detector.
#[derive(Debug, Default)]
pub struct AlertTracker {
    previous: BTreeMap<WindowKey, PreviousWindow>,
    fired: BTreeSet<FiringKey>,
}

impl AlertTracker {
    /// Constructs an empty alert tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Compares a snapshot with the previous snapshot and returns newly fired alerts.
    ///
    /// Thresholds outside 1 through 100 are ignored, and duplicate thresholds are
    /// evaluated once. The first fresh reading of a window establishes a baseline
    /// without alerting. Stale readings and windows whose reset passed without a
    /// fresh reading keep the previous baseline instead of replacing it, so a
    /// restart that briefly shows old data cannot re-fire crossed thresholds.
    #[must_use]
    pub fn evaluate(
        &mut self,
        snapshot: &UsageSnapshot,
        thresholds: &[u8],
        now: UnixSeconds,
    ) -> Vec<Alert> {
        self.prune_expired(now);
        let thresholds = thresholds
            .iter()
            .copied()
            .filter(|threshold| (1..=100).contains(threshold))
            .collect::<BTreeSet<_>>();
        let mut alerts = Vec::new();
        let mut next = BTreeMap::new();

        for usage in [&snapshot.claude, &snapshot.codex] {
            for window in &usage.windows {
                let key = (usage.provider, window.kind);
                let previous = self.previous.get(&key).copied();
                if window.reset_pending || !is_fresh(window.observed_at, now) {
                    if let Some(previous) = previous {
                        next.insert(key, previous);
                    }
                    continue;
                }
                let current = PreviousWindow {
                    used: window.used,
                    resets_at: canonical_reset(previous, window.resets_at),
                };
                if let Some(previous) = previous {
                    self.detect_reset(key, previous, current, now, &mut alerts);
                    self.detect_thresholds(key, previous, current, &thresholds, &mut alerts);
                }
                next.insert(key, current);
            }
        }

        self.previous = next;
        self.prune_inactive_or_expired(now);
        alerts
    }

    /// Returns the number of retained deduplication entries.
    #[must_use]
    pub fn retained_firings(&self) -> usize {
        self.fired.len()
    }

    /// A reset needs a later window (beyond the tolerance, which
    /// [`canonical_reset`] already applied) whose predecessor's reset time has
    /// actually arrived, after that predecessor reached 90% or more.
    fn detect_reset(
        &mut self,
        key: WindowKey,
        previous: PreviousWindow,
        current: PreviousWindow,
        now: UnixSeconds,
        alerts: &mut Vec<Alert>,
    ) {
        let rolled_over = matches!(
            (previous.resets_at, current.resets_at),
            (Some(before), Some(after))
                if after > before
                    && now.0 >= before.0.saturating_sub(SAME_RESET_TOLERANCE_SECONDS)
        );
        if !rolled_over || previous.used.get() < 90.0 {
            return;
        }

        let firing = (key.0, key.1, 0, current.resets_at);
        if self.fired.insert(firing) {
            alerts.push(Alert {
                kind: AlertKind::Reset,
                title: format!(
                    "{} {} limit reset",
                    provider_label(key.0),
                    window_label(key.1)
                ),
                body: format!(
                    "Usage is back to {:.0}% · {}",
                    current.used.get(),
                    reset_body(current.resets_at)
                ),
            });
        }
    }

    fn detect_thresholds(
        &mut self,
        key: WindowKey,
        previous: PreviousWindow,
        current: PreviousWindow,
        thresholds: &BTreeSet<u8>,
        alerts: &mut Vec<Alert>,
    ) {
        for threshold in thresholds {
            let threshold_percent = f64::from(*threshold);
            if previous.used.get() >= threshold_percent || current.used.get() < threshold_percent {
                continue;
            }

            let firing = (key.0, key.1, *threshold, current.resets_at);
            if self.fired.insert(firing) {
                alerts.push(Alert {
                    kind: AlertKind::Threshold,
                    title: format!(
                        "{} {} limit at {}%",
                        provider_label(key.0),
                        window_label(key.1),
                        threshold
                    ),
                    body: reset_body(current.resets_at),
                });
            }
        }
    }

    fn prune_expired(&mut self, now: UnixSeconds) {
        self.fired
            .retain(|(_, _, _, reset)| reset.is_none_or(|at| at > now));
    }

    fn prune_inactive_or_expired(&mut self, now: UnixSeconds) {
        self.fired.retain(|(provider, kind, _, reset)| {
            reset.is_some_and(|at| at > now)
                || self
                    .previous
                    .get(&(*provider, *kind))
                    .is_some_and(|window| window.resets_at == *reset)
        });
    }
}

fn is_fresh(observed_at: UnixSeconds, now: UnixSeconds) -> bool {
    now.0.saturating_sub(observed_at.0) < FRESH_SECONDS
}

/// Keeps the window's established reset time while a new reading reports one
/// within [`SAME_RESET_TOLERANCE_SECONDS`], so source jitter neither looks like a
/// reset nor creates a new deduplication key.
fn canonical_reset(
    previous: Option<PreviousWindow>,
    reported: Option<UnixSeconds>,
) -> Option<UnixSeconds> {
    match (previous.and_then(|window| window.resets_at), reported) {
        (Some(established), Some(reported))
            if (reported.0 - established.0).abs() <= SAME_RESET_TOLERANCE_SECONDS =>
        {
            Some(established)
        }
        _ => reported,
    }
}

fn provider_label(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "Claude",
        Provider::Codex => "Codex",
    }
}

fn window_label(kind: WindowKind) -> String {
    match kind {
        WindowKind::Session => "5-hour".to_owned(),
        WindowKind::Weekly => "weekly".to_owned(),
        WindowKind::Other { minutes } => format!("{minutes}-minute"),
    }
}

/// Formats the reset rounded to the nearest minute: the usage API reports
/// resets such as 13:59:59.6, which would otherwise read as "1:59 PM".
fn reset_body(resets_at: Option<UnixSeconds>) -> String {
    resets_at
        .and_then(|at| DateTime::from_timestamp(at.0.saturating_add(30), 0))
        .map_or_else(
            || "Reset time unavailable".to_owned(),
            |utc| {
                format!(
                    "Resets {}",
                    utc.with_timezone(&Local).format("%a %-I:%M %p")
                )
            },
        )
}
