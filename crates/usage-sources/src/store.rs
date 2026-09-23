use std::{sync::Arc, time::Duration};

use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::{
    Alert, AlertKind, AlertTracker, History, HistoryStore, Provider, State, UnixSeconds,
    UsageSnapshot, WindowKind, WindowMinutes, derive_snapshot, ingest_reading, ingest_status,
    project_weekly,
};

use crate::{SourceEvent, scheduler::Scheduler};

/// Capacity of the shared source-to-store event channel.
pub const SOURCE_EVENT_CAPACITY: usize = 256;
/// Capacity of the store-to-notification alert channel.
pub const ALERT_CAPACITY: usize = 32;
const COMMAND_CAPACITY: usize = 32;
const TICK_INTERVAL: Duration = Duration::from_secs(30);
const WEEKLY_WINDOW_MINUTES: WindowMinutes = WindowMinutes(10_080);

/// A store command could not be delivered or answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StoreError {
    /// The store actor has stopped.
    #[error("usage store is unavailable")]
    Closed,
}

/// UI/backend handle for observing snapshots, receiving alerts, and querying history.
pub struct StoreHandle {
    /// Latest derived snapshot.
    pub snapshot: watch::Receiver<Arc<UsageSnapshot>>,
    /// Bounded stream of notification requests.
    pub alerts: mpsc::Receiver<Alert>,
    commands: mpsc::Sender<StoreCommand>,
}

impl StoreHandle {
    /// Separates the cloneable command/snapshot client from the single alert receiver.
    #[must_use]
    pub fn split(self) -> (StoreClient, mpsc::Receiver<Alert>) {
        (
            StoreClient {
                snapshot: self.snapshot,
                commands: self.commands,
            },
            self.alerts,
        )
    }

    /// Requests the latest derived history for one provider.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the actor has stopped.
    pub async fn history(&self, provider: Provider) -> Result<History, StoreError> {
        get_history(&self.commands, provider).await
    }

    /// Requests an immediate snapshot re-derivation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the actor has stopped.
    pub async fn refresh_now(&self) -> Result<(), StoreError> {
        refresh(&self.commands).await
    }

    /// Replaces notification thresholds and reset preference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the actor has stopped.
    pub async fn update_alert_settings(
        &self,
        thresholds: Vec<u8>,
        notify_on_reset: bool,
    ) -> Result<(), StoreError> {
        update_alert_settings(&self.commands, thresholds, notify_on_reset).await
    }
}

/// Cloneable UI/backend client after the alert receiver has one owner.
#[derive(Clone)]
pub struct StoreClient {
    /// Latest derived snapshot.
    pub snapshot: watch::Receiver<Arc<UsageSnapshot>>,
    commands: mpsc::Sender<StoreCommand>,
}

impl StoreClient {
    /// Requests the latest derived history for one provider.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the actor has stopped.
    pub async fn history(&self, provider: Provider) -> Result<History, StoreError> {
        get_history(&self.commands, provider).await
    }

    /// Requests an immediate snapshot re-derivation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the actor has stopped.
    pub async fn refresh_now(&self) -> Result<(), StoreError> {
        refresh(&self.commands).await
    }

    /// Replaces notification thresholds and reset preference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the actor has stopped.
    pub async fn update_alert_settings(
        &self,
        thresholds: Vec<u8>,
        notify_on_reset: bool,
    ) -> Result<(), StoreError> {
        update_alert_settings(&self.commands, thresholds, notify_on_reset).await
    }
}

enum StoreCommand {
    GetHistory(Provider, oneshot::Sender<History>),
    RefreshNow,
    UpdateAlertSettings {
        thresholds: Vec<u8>,
        notify_on_reset: bool,
    },
}

async fn get_history(
    commands: &mpsc::Sender<StoreCommand>,
    provider: Provider,
) -> Result<History, StoreError> {
    let (reply, response) = oneshot::channel();
    commands
        .send(StoreCommand::GetHistory(provider, reply))
        .await
        .map_err(|_| StoreError::Closed)?;
    response.await.map_err(|_| StoreError::Closed)
}

async fn refresh(commands: &mpsc::Sender<StoreCommand>) -> Result<(), StoreError> {
    commands
        .send(StoreCommand::RefreshNow)
        .await
        .map_err(|_| StoreError::Closed)
}

async fn update_alert_settings(
    commands: &mpsc::Sender<StoreCommand>,
    thresholds: Vec<u8>,
    notify_on_reset: bool,
) -> Result<(), StoreError> {
    commands
        .send(StoreCommand::UpdateAlertSettings {
            thresholds,
            notify_on_reset,
        })
        .await
        .map_err(|_| StoreError::Closed)
}

/// Single-owner actor for source state, token history, and alert transitions.
pub struct UsageStore {
    state: State,
    history: HistoryStore,
    alert_tracker: AlertTracker,
    thresholds: Vec<u8>,
    notify_on_reset: bool,
    events: mpsc::Receiver<SourceEvent>,
    commands: mpsc::Receiver<StoreCommand>,
    snapshot: watch::Sender<Arc<UsageSnapshot>>,
    alerts: mpsc::Sender<Alert>,
    scheduler: Option<Scheduler>,
}

impl UsageStore {
    /// Builds all bounded channels and an actor ready to run.
    #[must_use]
    pub fn channel(thresholds: Vec<u8>) -> (mpsc::Sender<SourceEvent>, StoreHandle, Self) {
        Self::build_channels(thresholds, None)
    }

    /// Builds the store with activity events forwarded to the scheduler actor.
    #[must_use]
    pub fn channel_with_scheduler(
        thresholds: Vec<u8>,
        scheduler: Scheduler,
    ) -> (mpsc::Sender<SourceEvent>, StoreHandle, Self) {
        Self::build_channels(thresholds, Some(scheduler))
    }

    fn build_channels(
        thresholds: Vec<u8>,
        scheduler: Option<Scheduler>,
    ) -> (mpsc::Sender<SourceEvent>, StoreHandle, Self) {
        let state = State::new();
        let initial = Arc::new(derive_snapshot(&state, unix_now()));
        let (event_tx, events) = mpsc::channel(SOURCE_EVENT_CAPACITY);
        let (command_tx, commands) = mpsc::channel(COMMAND_CAPACITY);
        let (snapshot, snapshot_rx) = watch::channel(initial);
        let (alerts, alerts_rx) = mpsc::channel(ALERT_CAPACITY);
        let handle = StoreHandle {
            snapshot: snapshot_rx,
            alerts: alerts_rx,
            commands: command_tx,
        };
        let actor = Self {
            state,
            history: HistoryStore::new(),
            alert_tracker: AlertTracker::new(),
            thresholds,
            notify_on_reset: true,
            events,
            commands,
            snapshot,
            alerts,
            scheduler,
        };
        (event_tx, handle, actor)
    }

    /// Runs until cancellation, retaining sole ownership of mutable store state.
    pub async fn run(mut self, cancel: CancellationToken) {
        let mut tick = tokio::time::interval(TICK_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                event = self.events.recv() => {
                    if let Some(event) = event
                        && self.ingest_event(event)
                    {
                        self.publish(unix_now());
                    }
                }
                command = self.commands.recv() => {
                    if let Some(command) = command {
                        self.handle_command(command);
                    }
                }
                _ = tick.tick() => self.publish(unix_now()),
            }
        }
    }

    fn ingest_event(&mut self, event: SourceEvent) -> bool {
        match event {
            SourceEvent::Reading(reading) => ingest_reading(&mut self.state, reading),
            SourceEvent::Tokens(events) => self.ingest_tokens(events),
            SourceEvent::Daily(update) => self.history.ingest_daily(update),
            SourceEvent::Status {
                provider,
                source,
                status,
            } => {
                ingest_status(&mut self.state, provider, source, status);
                true
            }
            SourceEvent::Activity { provider } => {
                if let Some(scheduler) = &self.scheduler
                    && let Err(error) = scheduler.record_activity(provider)
                {
                    warn!(%error, "could not forward source activity to scheduler");
                }
                false
            }
        }
    }

    fn ingest_tokens(&mut self, events: Vec<usage_core::TokenEvent>) -> bool {
        let mut changed = false;
        for event in events {
            changed |= self.history.ingest_token(event);
        }
        changed
    }

    fn handle_command(&mut self, command: StoreCommand) {
        match command {
            StoreCommand::GetHistory(provider, reply) => {
                let _ = reply.send(self.history(provider, unix_now()));
            }
            StoreCommand::RefreshNow => {
                self.trigger_scheduled_refresh();
                self.publish(unix_now());
            }
            StoreCommand::UpdateAlertSettings {
                thresholds,
                notify_on_reset,
            } => {
                self.thresholds = thresholds;
                self.notify_on_reset = notify_on_reset;
            }
        }
    }

    fn trigger_scheduled_refresh(&self) {
        let Some(scheduler) = &self.scheduler else {
            return;
        };
        for source in [
            usage_core::SourceKind::CodexAppServer,
            usage_core::SourceKind::ClaudeOAuth,
        ] {
            if let Err(error) = scheduler.trigger(source) {
                warn!(%error, ?source, "could not request scheduled source refresh");
            }
        }
    }

    fn history(&self, provider: Provider, now: UnixSeconds) -> History {
        let snapshot = self.snapshot.borrow();
        let usage = match provider {
            Provider::Claude => &snapshot.claude,
            Provider::Codex => &snapshot.codex,
        };
        let projection = usage
            .windows
            .iter()
            .find(|window| window.kind == WindowKind::Weekly)
            .and_then(|window| project_weekly(window, WEEKLY_WINDOW_MINUTES, now));
        self.history.history_at(provider, now, projection)
    }

    fn publish(&mut self, now: UnixSeconds) {
        let next = derive_snapshot(&self.state, now);
        let current = self.snapshot.borrow();
        if current.claude == next.claude && current.codex == next.codex {
            return;
        }
        drop(current);

        for alert in self
            .alert_tracker
            .evaluate(&next, &self.thresholds, now)
            .into_iter()
            .filter(|alert| self.notify_on_reset || alert.kind != AlertKind::Reset)
        {
            match self.alerts.try_send(alert) {
                Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
                Err(mpsc::error::TrySendError::Full(alert)) => {
                    warn!(title = %alert.title, "dropping alert because the channel is full");
                }
            }
        }
        drop(self.snapshot.send_replace(Arc::new(next)));
    }
}

fn unix_now() -> UnixSeconds {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    UnixSeconds(i64::try_from(seconds).unwrap_or(i64::MAX))
}
