//! Actor-backed adaptive polling scheduler.

use std::{collections::BTreeMap, time::Duration, time::SystemTime, time::UNIX_EPOCH};

use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{Instant, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;
use usage_core::{Provider, SourceKind};

mod support;

use support::{Command, Jitter, SourceSchedule, active_interval, provider_for};
pub use support::{SchedulerConfig, WakeDetector};

const COMMAND_CAPACITY: usize = 64;

/// A scheduler request could not be fulfilled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SchedulerError {
    /// The scheduler actor has stopped.
    #[error("scheduler is unavailable")]
    Closed,
    /// Its bounded command channel is currently full.
    #[error("scheduler command queue is full")]
    Busy,
    /// Only interval-polled limit sources may request deadlines.
    #[error("source {0:?} is not scheduled")]
    UnsupportedSource(SourceKind),
}

/// Cloneable command and deadline handle for source tasks.
#[derive(Clone)]
pub struct Scheduler {
    commands: mpsc::Sender<Command>,
    codex_due: watch::Receiver<Instant>,
    claude_due: watch::Receiver<Instant>,
}

impl Scheduler {
    /// Creates a handle and its single-owner actor.
    #[must_use]
    pub fn channel(config: SchedulerConfig) -> (Self, SchedulerActor) {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        Self::channel_inner(config, nanos ^ u64::from(std::process::id()), true)
    }

    /// Creates a scheduler with deterministic jitter for repeatable tests.
    #[doc(hidden)]
    #[must_use]
    pub fn channel_with_seed(config: SchedulerConfig, seed: u64) -> (Self, SchedulerActor) {
        Self::channel_inner(config, seed, false)
    }

    fn channel_inner(
        config: SchedulerConfig,
        seed: u64,
        detect_wake: bool,
    ) -> (Self, SchedulerActor) {
        let now = Instant::now();
        let (commands, command_rx) = mpsc::channel(COMMAND_CAPACITY);
        let (codex_tx, codex_due) = watch::channel(now);
        let (claude_tx, claude_due) = watch::channel(now);
        let handle = Self {
            commands,
            codex_due,
            claude_due,
        };
        let actor = SchedulerActor::new(config, seed, detect_wake, command_rx, codex_tx, claude_tx);
        (handle, actor)
    }

    /// Waits until a source is due, adapting if a trigger moves its deadline.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported sources or if the actor stops.
    pub async fn next_due(&self, source: SourceKind) -> Result<Instant, SchedulerError> {
        let mut due = self.due_receiver(source)?;
        loop {
            let deadline = *due.borrow_and_update();
            tokio::select! {
                () = tokio::time::sleep_until(deadline) => {
                    let started_at = Instant::now();
                    if self.try_start(source, started_at).await? {
                        return Ok(started_at);
                    }
                }
                changed = due.changed() => {
                    changed.map_err(|_| SchedulerError::Closed)?;
                }
            }
        }
    }

    /// Returns the currently computed deadline without waiting for it.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported sources or if the actor stops.
    pub async fn scheduled_for(&self, source: SourceKind) -> Result<Instant, SchedulerError> {
        ensure_supported(source)?;
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Query { source, reply })
            .await
            .map_err(|_| SchedulerError::Closed)?;
        response.await.map_err(|_| SchedulerError::Closed)
    }

    /// Requests the earliest read allowed by the minimum gap and backoff.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported sources, a full queue, or a stopped actor.
    pub fn trigger(&self, source: SourceKind) -> Result<(), SchedulerError> {
        ensure_supported(source)?;
        self.try_command(Command::Trigger {
            source,
            at: Instant::now(),
        })
    }

    /// Records provider activity for active/idle interval selection.
    ///
    /// # Errors
    ///
    /// Returns an error if the bounded queue is full or the actor has stopped.
    pub fn record_activity(&self, provider: Provider) -> Result<(), SchedulerError> {
        self.try_command(Command::Activity {
            provider,
            at: Instant::now(),
        })
    }

    /// Replaces interval policy after validated settings change.
    ///
    /// # Errors
    ///
    /// Returns an error if the bounded queue is full or the actor has stopped.
    pub async fn update_config(&self, config: SchedulerConfig) -> Result<(), SchedulerError> {
        self.commands
            .send(Command::UpdateConfig(config))
            .await
            .map_err(|_| SchedulerError::Closed)
    }

    /// Clears failure backoff and marks this source's data fresh.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported sources, a full queue, or a stopped actor.
    pub fn record_success(&self, source: SourceKind) -> Result<(), SchedulerError> {
        ensure_supported(source)?;
        self.try_command(Command::Success {
            source,
            at: Instant::now(),
        })
    }

    /// Advances exponential backoff and optionally honors Retry-After.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported sources, a full queue, or a stopped actor.
    pub fn record_failure(
        &self,
        source: SourceKind,
        retry_after: Option<Duration>,
    ) -> Result<(), SchedulerError> {
        ensure_supported(source)?;
        self.try_command(Command::Failure {
            source,
            retry_after,
            at: Instant::now(),
        })
    }

    /// Triggers a read when the source's known data is older than 30 seconds.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported sources, a full queue, or a stopped actor.
    pub fn ui_visible(&self, source: SourceKind) -> Result<(), SchedulerError> {
        ensure_supported(source)?;
        self.try_command(Command::UiVisible {
            source,
            at: Instant::now(),
        })
    }

    fn due_receiver(&self, source: SourceKind) -> Result<watch::Receiver<Instant>, SchedulerError> {
        match source {
            SourceKind::CodexAppServer => Ok(self.codex_due.clone()),
            SourceKind::ClaudeOAuth => Ok(self.claude_due.clone()),
            _ => Err(SchedulerError::UnsupportedSource(source)),
        }
    }

    async fn try_start(&self, source: SourceKind, at: Instant) -> Result<bool, SchedulerError> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Started { source, at, reply })
            .await
            .map_err(|_| SchedulerError::Closed)?;
        response.await.map_err(|_| SchedulerError::Closed)
    }

    fn try_command(&self, command: Command) -> Result<(), SchedulerError> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => SchedulerError::Busy,
                mpsc::error::TrySendError::Closed(_) => SchedulerError::Closed,
            })
    }
}

/// Single-owner scheduler state machine.
pub struct SchedulerActor {
    config: SchedulerConfig,
    commands: mpsc::Receiver<Command>,
    schedules: BTreeMap<SourceKind, SourceSchedule>,
    activity: BTreeMap<Provider, Instant>,
    due: BTreeMap<SourceKind, watch::Sender<Instant>>,
    jitter: Jitter,
    wake: Option<WakeDetector>,
}

impl SchedulerActor {
    fn new(
        config: SchedulerConfig,
        seed: u64,
        detect_wake: bool,
        commands: mpsc::Receiver<Command>,
        codex_due: watch::Sender<Instant>,
        claude_due: watch::Sender<Instant>,
    ) -> Self {
        Self {
            config,
            commands,
            schedules: [SourceKind::CodexAppServer, SourceKind::ClaudeOAuth]
                .into_iter()
                .map(|source| (source, SourceSchedule::default()))
                .collect(),
            activity: BTreeMap::new(),
            due: [
                (SourceKind::CodexAppServer, codex_due),
                (SourceKind::ClaudeOAuth, claude_due),
            ]
            .into_iter()
            .collect(),
            jitter: Jitter::new(seed),
            wake: detect_wake.then(|| WakeDetector::new(SystemTime::now(), Instant::now())),
        }
    }

    /// Runs until cancellation with skipped missed ticks after system sleep.
    pub async fn run(mut self, cancel: CancellationToken) {
        let start = Instant::now() + self.config.tick_interval;
        let mut tick = tokio::time::interval_at(start, self.config.tick_interval);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        self.publish_all(Instant::now());

        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                command = self.commands.recv() => {
                    let Some(command) = command else { break; };
                    self.handle(command, Instant::now());
                }
                _ = tick.tick() => {
                    let now = Instant::now();
                    if self.wake.as_mut().is_some_and(|wake| wake.sample(SystemTime::now(), now)) {
                        self.trigger_all(now);
                    }
                    self.publish_all(now);
                }
            }
        }
    }

    fn handle(&mut self, command: Command, now: Instant) {
        match command {
            Command::UpdateConfig(config) => {
                self.config = config;
            }
            Command::Query { source, reply } => {
                let _ = reply.send(self.deadline(source, now));
            }
            Command::Started { source, at, reply } => {
                let accepted = self.deadline(source, now) <= now;
                if accepted && let Some(schedule) = self.schedules.get_mut(&source) {
                    schedule.last_read = Some(at);
                    schedule.triggered_at = None;
                }
                let _ = reply.send(accepted);
            }
            Command::Trigger { source, at } => self.trigger(source, at),
            Command::Activity { provider, at } => {
                self.activity.insert(provider, at);
            }
            Command::Success { source, at } => {
                if let Some(schedule) = self.schedules.get_mut(&source) {
                    schedule.failures = 0;
                    schedule.backoff_until = None;
                    schedule.last_data = Some(at);
                }
            }
            Command::Failure {
                source,
                retry_after,
                at,
            } => {
                self.fail(source, retry_after, at);
            }
            Command::UiVisible { source, at } => {
                let stale = self.schedules.get(&source).is_some_and(|schedule| {
                    schedule.last_data.is_none_or(|last| {
                        at.saturating_duration_since(last) > self.config.visible_max_age
                    })
                });
                if stale {
                    self.trigger(source, at);
                }
            }
        }
        self.publish_all(now);
    }

    fn fail(&mut self, source: SourceKind, retry_after: Option<Duration>, now: Instant) {
        let failures = self
            .schedules
            .get(&source)
            .map_or(1, |schedule| schedule.failures.saturating_add(1));
        let base = self
            .config
            .initial_backoff
            .saturating_mul(
                1_u32
                    .checked_shl(failures.saturating_sub(1))
                    .unwrap_or(u32::MAX),
            )
            .min(self.config.maximum_backoff);
        let delay = self
            .jitter
            .apply(base, self.config.maximum_backoff)
            .max(retry_after.unwrap_or_default());
        if let Some(schedule) = self.schedules.get_mut(&source) {
            schedule.failures = failures;
            schedule.backoff_until = Some(now + delay);
            schedule.triggered_at = Some(now);
        }
    }

    fn trigger(&mut self, source: SourceKind, now: Instant) {
        if let Some(schedule) = self.schedules.get_mut(&source) {
            schedule.triggered_at = Some(now);
        }
    }

    fn trigger_all(&mut self, now: Instant) {
        for schedule in self.schedules.values_mut() {
            schedule.triggered_at = Some(now);
        }
    }

    fn deadline(&self, source: SourceKind, now: Instant) -> Instant {
        let Some(schedule) = self.schedules.get(&source) else {
            return now;
        };
        let provider = provider_for(source).unwrap_or(Provider::Codex);
        let interval = if self.is_active(provider, now) {
            active_interval(self.config, source)
        } else {
            self.config.idle
        };
        schedule.deadline(now, interval, self.config.minimum_gap)
    }

    fn is_active(&self, provider: Provider, now: Instant) -> bool {
        self.activity
            .get(&provider)
            .is_some_and(|last| now.saturating_duration_since(*last) <= self.config.active_for)
    }

    fn publish_all(&mut self, now: Instant) {
        for source in [SourceKind::CodexAppServer, SourceKind::ClaudeOAuth] {
            let deadline = self.deadline(source, now);
            if let Some(sender) = self.due.get_mut(&source)
                && *sender.borrow() != deadline
            {
                sender.send_replace(deadline);
            }
        }
    }
}

fn ensure_supported(source: SourceKind) -> Result<(), SchedulerError> {
    provider_for(source)
        .map(|_| ())
        .ok_or(SchedulerError::UnsupportedSource(source))
}
