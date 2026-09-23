//! Scheduler timing value objects and internal messages.

use std::time::{Duration, SystemTime};

use tokio::{sync::oneshot, time::Instant};
use usage_core::{Provider, SourceKind};

const WAKE_DRIFT: Duration = Duration::from_mins(1);

/// Scheduler timing policy. Settings may replace the active and idle values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerConfig {
    /// Codex polling interval while its provider is active.
    pub codex_active: Duration,
    /// Claude OAuth polling interval while its provider is active.
    pub claude_active: Duration,
    /// Polling interval after provider activity expires.
    pub idle: Duration,
    /// Time since the last event for which a provider remains active.
    pub active_for: Duration,
    /// Hard minimum between reads of one source.
    pub minimum_gap: Duration,
    /// Maximum data age before showing UI triggers a read.
    pub visible_max_age: Duration,
    /// First retry delay after a failure.
    pub initial_backoff: Duration,
    /// Maximum scheduler backoff, before a longer Retry-After value.
    pub maximum_backoff: Duration,
    /// Wake-detection and deadline reevaluation interval.
    pub tick_interval: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            codex_active: Duration::from_mins(2),
            claude_active: Duration::from_mins(3),
            idle: Duration::from_mins(10),
            active_for: Duration::from_mins(10),
            minimum_gap: Duration::from_secs(15),
            visible_max_age: Duration::from_secs(30),
            initial_backoff: Duration::from_secs(30),
            maximum_backoff: Duration::from_mins(30),
            tick_interval: Duration::from_secs(1),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct SourceSchedule {
    pub(super) last_read: Option<Instant>,
    pub(super) last_data: Option<Instant>,
    pub(super) triggered_at: Option<Instant>,
    pub(super) backoff_until: Option<Instant>,
    pub(super) failures: u32,
}

impl SourceSchedule {
    pub(super) fn deadline(
        &self,
        now: Instant,
        interval: Duration,
        minimum_gap: Duration,
    ) -> Instant {
        let interval_due = self.last_read.map_or(now, |last| last + interval);
        let mut due = self
            .triggered_at
            .map_or(interval_due, |trigger| trigger.min(interval_due));
        if let Some(last_read) = self.last_read {
            due = due.max(last_read + minimum_gap);
        }
        if let Some(backoff) = self.backoff_until {
            due = due.max(backoff);
        }
        due
    }
}

pub(super) enum Command {
    UpdateConfig(SchedulerConfig),
    Query {
        source: SourceKind,
        reply: oneshot::Sender<Instant>,
    },
    Started {
        source: SourceKind,
        at: Instant,
        reply: oneshot::Sender<bool>,
    },
    Trigger {
        source: SourceKind,
        at: Instant,
    },
    Activity {
        provider: Provider,
        at: Instant,
    },
    Success {
        source: SourceKind,
        at: Instant,
    },
    Failure {
        source: SourceKind,
        retry_after: Option<Duration>,
        at: Instant,
    },
    UiVisible {
        source: SourceKind,
        at: Instant,
    },
}

pub(super) struct Jitter(u64);

impl Jitter {
    pub(super) fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    pub(super) fn apply(&mut self, base: Duration, maximum: Duration) -> Duration {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        let basis_points = 8_000_u128 + u128::from(self.0 % 4_001);
        let milliseconds = base.as_millis().saturating_mul(basis_points) / 10_000;
        Duration::from_millis(u64::try_from(milliseconds).unwrap_or(u64::MAX)).min(maximum)
    }
}

/// Detects wall-clock versus monotonic drift caused by system sleep or clock jumps.
pub struct WakeDetector {
    wall: SystemTime,
    monotonic: Instant,
}

impl WakeDetector {
    /// Starts a detector from an explicit clock pair.
    #[must_use]
    pub const fn new(wall: SystemTime, monotonic: Instant) -> Self {
        Self { wall, monotonic }
    }

    /// Updates the clock pair and reports drift strictly greater than 60 seconds.
    pub fn sample(&mut self, wall: SystemTime, monotonic: Instant) -> bool {
        let wall_delta = wall
            .duration_since(self.wall)
            .or_else(|_| self.wall.duration_since(wall))
            .unwrap_or_default();
        let monotonic_delta = monotonic.saturating_duration_since(self.monotonic);
        self.wall = wall;
        self.monotonic = monotonic;
        wall_delta.abs_diff(monotonic_delta) > WAKE_DRIFT
    }
}

pub(super) const fn provider_for(source: SourceKind) -> Option<Provider> {
    match source {
        SourceKind::CodexAppServer => Some(Provider::Codex),
        SourceKind::ClaudeOAuth => Some(Provider::Claude),
        _ => None,
    }
}

pub(super) const fn active_interval(config: SchedulerConfig, source: SourceKind) -> Duration {
    match source {
        SourceKind::ClaudeOAuth => config.claude_active,
        _ => config.codex_active,
    }
}
