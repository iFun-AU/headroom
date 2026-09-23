//! Single-owner settings mutation and persistence actor.

use std::path::PathBuf;

use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

use super::{Settings, SettingsError, SettingsState, reset_settings, save_settings};

const COMMAND_CAPACITY: usize = 32;

/// Settings actor command or persistence failure.
#[derive(Debug, Error)]
pub enum SettingsActorError {
    /// The settings actor is no longer running.
    #[error("settings service is unavailable")]
    Closed,
    /// A newer on-disk schema disabled ordinary saves.
    #[error("settings are read-only until they are explicitly reset")]
    ReadOnly,
    /// Atomic settings persistence failed.
    #[error(transparent)]
    Persistence(#[from] SettingsError),
    /// A bounded blocking persistence worker could not be joined.
    #[error("settings persistence worker stopped unexpectedly: {0}")]
    Task(#[from] tokio::task::JoinError),
}

/// Cloneable read/mutation handle for application settings.
#[derive(Clone)]
pub struct SettingsHandle {
    commands: mpsc::Sender<Command>,
    state: watch::Receiver<SettingsState>,
}

impl SettingsHandle {
    /// Creates the bounded handle and single-owner actor.
    #[must_use]
    pub fn channel(path: PathBuf, initial: SettingsState) -> (Self, SettingsActor) {
        let (commands, command_rx) = mpsc::channel(COMMAND_CAPACITY);
        let (state_tx, state) = watch::channel(initial.clone());
        (
            Self { commands, state },
            SettingsActor {
                path,
                state: initial,
                commands: command_rx,
                state_tx,
            },
        )
    }

    /// Returns the current in-memory state without blocking.
    #[must_use]
    pub fn snapshot(&self) -> SettingsState {
        self.state.borrow().clone()
    }

    /// Subscribes to persisted settings changes.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<SettingsState> {
        self.state.clone()
    }

    /// Validates and persists a complete settings replacement.
    ///
    /// # Errors
    ///
    /// Returns read-only, closed-service, worker, or persistence errors.
    pub async fn set(&self, settings: Settings) -> Result<SettingsState, SettingsActorError> {
        self.request(CommandKind::Set(settings)).await
    }

    /// Explicitly replaces even a newer schema with current defaults.
    ///
    /// # Errors
    ///
    /// Returns closed-service, worker, or persistence errors.
    pub async fn reset(&self) -> Result<SettingsState, SettingsActorError> {
        self.request(CommandKind::Reset).await
    }

    /// Marks first-run onboarding complete and persists the change.
    ///
    /// # Errors
    ///
    /// Returns read-only, closed-service, worker, or persistence errors.
    pub async fn complete_onboarding(&self) -> Result<SettingsState, SettingsActorError> {
        self.request(CommandKind::CompleteOnboarding).await
    }

    /// Persists the latest settled floating-widget position without replacing other settings.
    ///
    /// # Errors
    ///
    /// Returns read-only, closed-service, worker, or persistence errors.
    pub async fn update_widget_position(
        &self,
        x: i32,
        y: i32,
    ) -> Result<SettingsState, SettingsActorError> {
        self.request(CommandKind::UpdateWidgetPosition { x, y })
            .await
    }

    /// Persists floating-widget visibility without replacing other settings.
    ///
    /// # Errors
    ///
    /// Returns read-only, closed-service, worker, or persistence errors.
    pub async fn update_widget_visibility(
        &self,
        visible: bool,
    ) -> Result<SettingsState, SettingsActorError> {
        self.request(CommandKind::UpdateWidgetVisibility(visible))
            .await
    }

    async fn request(&self, kind: CommandKind) -> Result<SettingsState, SettingsActorError> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command { kind, reply })
            .await
            .map_err(|_| SettingsActorError::Closed)?;
        response.await.map_err(|_| SettingsActorError::Closed)?
    }
}

/// Sole owner of settings state and serialized disk mutations.
pub struct SettingsActor {
    path: PathBuf,
    state: SettingsState,
    commands: mpsc::Receiver<Command>,
    state_tx: watch::Sender<SettingsState>,
}

impl SettingsActor {
    /// Applies commands until cancelled or all handles close.
    pub async fn run(mut self, cancel: CancellationToken) {
        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                command = self.commands.recv() => {
                    let Some(command) = command else { return; };
                    let result = self.apply(command.kind).await;
                    if let Ok(state) = &result
                        && *state != self.state
                    {
                        self.state = state.clone();
                        self.state_tx.send_replace(state.clone());
                    }
                    let _ = command.reply.send(result);
                }
            }
        }
    }

    async fn apply(&self, command: CommandKind) -> Result<SettingsState, SettingsActorError> {
        match command {
            CommandKind::Reset => {
                let path = self.path.clone();
                Ok(tokio::task::spawn_blocking(move || reset_settings(&path)).await??)
            }
            CommandKind::Set(settings) => {
                self.ensure_writable()?;
                self.persist(settings).await
            }
            CommandKind::CompleteOnboarding => {
                self.ensure_writable()?;
                if self.state.settings.onboarding_completed {
                    return Ok(self.state.clone());
                }
                let mut settings = self.state.settings.clone();
                settings.onboarding_completed = true;
                self.persist(settings).await
            }
            CommandKind::UpdateWidgetPosition { x, y } => {
                self.ensure_writable()?;
                if self.state.settings.widget.x == Some(x)
                    && self.state.settings.widget.y == Some(y)
                {
                    return Ok(self.state.clone());
                }
                let mut settings = self.state.settings.clone();
                settings.widget.x = Some(x);
                settings.widget.y = Some(y);
                self.persist(settings).await
            }
            CommandKind::UpdateWidgetVisibility(visible) => {
                self.ensure_writable()?;
                if self.state.settings.widget.visible == visible {
                    return Ok(self.state.clone());
                }
                let mut settings = self.state.settings.clone();
                settings.widget.visible = visible;
                self.persist(settings).await
            }
        }
    }

    fn ensure_writable(&self) -> Result<(), SettingsActorError> {
        if self.state.read_only {
            return Err(SettingsActorError::ReadOnly);
        }
        Ok(())
    }

    async fn persist(&self, settings: Settings) -> Result<SettingsState, SettingsActorError> {
        let path = self.path.clone();
        let settings =
            tokio::task::spawn_blocking(move || save_settings(&path, settings)).await??;
        Ok(SettingsState::writable(settings))
    }
}

struct Command {
    kind: CommandKind,
    reply: oneshot::Sender<Result<SettingsState, SettingsActorError>>,
}

enum CommandKind {
    Set(Settings),
    Reset,
    CompleteOnboarding,
    UpdateWidgetPosition { x: i32, y: i32 },
    UpdateWidgetVisibility(bool),
}
