use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Local};

use crate::{Percent, Provider, UnixSeconds, UsageSnapshot, WindowKind};

type WindowKey = (Provider, WindowKind);
type FiringKey = (Provider, WindowKind, u8, Option<UnixSeconds>);

#[derive(Debug, Clone, Copy)]
struct PreviousWindow {
    used: Percent,
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
    /// evaluated once. The first snapshot establishes a baseline without alerting.
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
                let current = PreviousWindow {
                    used: window.used,
                    resets_at: window.resets_at,
                };
                if let Some(previous) = self.previous.get(&key).copied() {
                    self.detect_reset(key, previous, current, &mut alerts);
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

    fn detect_reset(
        &mut self,
        key: WindowKey,
        previous: PreviousWindow,
        current: PreviousWindow,
        alerts: &mut Vec<Alert>,
    ) {
        let reset_increased = matches!(
            (previous.resets_at, current.resets_at),
            (Some(before), Some(after)) if after > before
        );
        if !reset_increased || previous.used.get() < 90.0 {
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
                body: "A new usage window has started".to_owned(),
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

fn provider_label(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "Claude",
        Provider::Codex => "Codex",
    }
}

fn window_label(kind: WindowKind) -> String {
    match kind {
        WindowKind::Session => "session".to_owned(),
        WindowKind::Weekly => "weekly".to_owned(),
        WindowKind::Other { minutes } => format!("{minutes}-minute"),
    }
}

fn reset_body(resets_at: Option<UnixSeconds>) -> String {
    resets_at
        .and_then(|at| DateTime::from_timestamp(at.0, 0))
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
