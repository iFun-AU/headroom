//! Behavioral Claude bridge effectiveness state and bounded actor.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use usage_core::UnixSeconds;

const COMMAND_CAPACITY: usize = 32;
const OVERRIDE_EVIDENCE_DELAY: Duration = Duration::from_mins(10);

/// Explanation shown when assistant activity continues without bridge writes.
pub const LIKELY_OVERRIDDEN_HINT: &str = "No updates received from Claude Code. A project or organization setting may override your status line, or your plan doesn't report limits.";

/// Behavioral confidence that Claude Code is invoking the installed bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Effectiveness {
    /// Installed, but neither positive nor negative runtime evidence exists.
    Unverified,
    /// A bridge file was written after the current installation.
    Confirmed,
    /// Assistant activity continued for over ten minutes without a bridge write.
    LikelyOverridden,
}

/// Observable effectiveness evidence for Settings and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectivenessSnapshot {
    /// Timestamp of the current installation, or `None` after uninstall.
    pub installed_at: Option<UnixSeconds>,
    /// Current behavioral classification.
    pub effective: Effectiveness,
    /// Latest qualifying bridge-file timestamp.
    pub last_bridge_write: Option<UnixSeconds>,
    /// Latest assistant-usage timestamp from local logs.
    pub last_activity: Option<UnixSeconds>,
}

/// Pure injected-time state machine used by the actor and deterministic tests.
#[derive(Debug, Clone)]
pub struct EffectivenessTracker {
    snapshot: EffectivenessSnapshot,
}

impl EffectivenessTracker {
    /// Starts installed or uninstalled with no runtime evidence.
    #[must_use]
    pub const fn new(installed_at: Option<UnixSeconds>) -> Self {
        Self {
            snapshot: EffectivenessSnapshot {
                installed_at,
                effective: Effectiveness::Unverified,
                last_bridge_write: None,
                last_activity: None,
            },
        }
    }

    /// Returns the current evidence snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> EffectivenessSnapshot {
        self.snapshot
    }

    /// Begins a new installation epoch and clears earlier evidence.
    pub fn install(&mut self, installed_at: UnixSeconds) -> bool {
        let next = EffectivenessSnapshot {
            installed_at: Some(installed_at),
            effective: Effectiveness::Unverified,
            last_bridge_write: None,
            last_activity: None,
        };
        self.replace(next)
    }

    /// Clears installation and runtime evidence.
    pub fn uninstall(&mut self) -> bool {
        self.installation(None)
    }

    /// Records one persisted bridge envelope timestamp.
    pub fn bridge_written(&mut self, at: UnixSeconds) -> bool {
        let Some(installed_at) = self.snapshot.installed_at else {
            return false;
        };
        if at < installed_at || self.snapshot.last_bridge_write.is_some_and(|old| at < old) {
            return false;
        }
        self.snapshot.last_bridge_write = Some(at);
        self.snapshot.effective = Effectiveness::Confirmed;
        true
    }

    /// Records latest assistant usage and evaluates override evidence.
    pub fn assistant_activity(&mut self, at: UnixSeconds) -> bool {
        let Some(installed_at) = self.snapshot.installed_at else {
            return false;
        };
        if at < installed_at || self.snapshot.last_activity.is_some_and(|old| at <= old) {
            return false;
        }
        self.snapshot.last_activity = Some(at);
        let baseline = self.snapshot.last_bridge_write.unwrap_or(installed_at);
        if after_delay(at, baseline, OVERRIDE_EVIDENCE_DELAY) {
            self.snapshot.effective = Effectiveness::LikelyOverridden;
        }
        true
    }

    fn installation(&mut self, installed_at: Option<UnixSeconds>) -> bool {
        let next = EffectivenessSnapshot {
            installed_at,
            effective: Effectiveness::Unverified,
            last_bridge_write: None,
            last_activity: None,
        };
        self.replace(next)
    }

    fn replace(&mut self, next: EffectivenessSnapshot) -> bool {
        if self.snapshot == next {
            return false;
        }
        self.snapshot = next;
        true
    }
}

/// Failure to enqueue effectiveness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EffectivenessError {
    /// The actor has stopped.
    #[error("bridge effectiveness actor is unavailable")]
    Closed,
}

/// Cloneable evidence sender and snapshot subscriber.
#[derive(Clone)]
pub struct EffectivenessHandle {
    commands: mpsc::Sender<Command>,
    snapshot: watch::Receiver<EffectivenessSnapshot>,
}

impl EffectivenessHandle {
    /// Creates a bounded actor around an injected installation timestamp.
    #[must_use]
    pub fn channel(installed_at: Option<UnixSeconds>) -> (Self, EffectivenessActor) {
        let tracker = EffectivenessTracker::new(installed_at);
        let (commands, command_rx) = mpsc::channel(COMMAND_CAPACITY);
        let (snapshot_tx, snapshot) = watch::channel(tracker.snapshot());
        (
            Self { commands, snapshot },
            EffectivenessActor {
                tracker,
                commands: command_rx,
                snapshot: snapshot_tx,
            },
        )
    }

    /// Returns the latest evidence snapshot.
    #[must_use]
    pub fn snapshot(&self) -> EffectivenessSnapshot {
        *self.snapshot.borrow()
    }

    /// Subscribes to evidence changes.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<EffectivenessSnapshot> {
        self.snapshot.clone()
    }

    /// Starts a new installation epoch.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivenessError::Closed`] after the actor stops.
    pub async fn install(&self, at: UnixSeconds) -> Result<(), EffectivenessError> {
        self.send(Command::Install(at)).await
    }

    /// Clears the installation epoch.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivenessError::Closed`] after the actor stops.
    pub async fn uninstall(&self) -> Result<(), EffectivenessError> {
        self.send(Command::Uninstall).await
    }

    /// Reports a persisted bridge file.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivenessError::Closed`] after the actor stops.
    pub async fn bridge_written(&self, at: UnixSeconds) -> Result<(), EffectivenessError> {
        self.send(Command::BridgeWritten(at)).await
    }

    /// Reports assistant usage from a local history batch.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivenessError::Closed`] after the actor stops.
    pub async fn assistant_activity(&self, at: UnixSeconds) -> Result<(), EffectivenessError> {
        self.send(Command::AssistantActivity(at)).await
    }

    async fn send(&self, command: Command) -> Result<(), EffectivenessError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| EffectivenessError::Closed)
    }
}

/// Single-owner effectiveness actor.
pub struct EffectivenessActor {
    tracker: EffectivenessTracker,
    commands: mpsc::Receiver<Command>,
    snapshot: watch::Sender<EffectivenessSnapshot>,
}

impl EffectivenessActor {
    /// Applies evidence until cancellation or all handles close.
    pub async fn run(mut self, cancel: CancellationToken) {
        loop {
            tokio::select! {
                () = cancel.cancelled() => return,
                command = self.commands.recv() => {
                    let Some(command) = command else { return; };
                    if self.apply(command) {
                        self.snapshot.send_replace(self.tracker.snapshot());
                    }
                }
            }
        }
    }

    fn apply(&mut self, command: Command) -> bool {
        match command {
            Command::Install(at) => self.tracker.install(at),
            Command::Uninstall => self.tracker.uninstall(),
            Command::BridgeWritten(at) => self.tracker.bridge_written(at),
            Command::AssistantActivity(at) => self.tracker.assistant_activity(at),
        }
    }
}

#[derive(Clone, Copy)]
enum Command {
    Install(UnixSeconds),
    Uninstall,
    BridgeWritten(UnixSeconds),
    AssistantActivity(UnixSeconds),
}

fn after_delay(candidate: UnixSeconds, baseline: UnixSeconds, delay: Duration) -> bool {
    let seconds = i64::try_from(delay.as_secs()).unwrap_or(i64::MAX);
    candidate.0 > baseline.0.saturating_add(seconds)
}
