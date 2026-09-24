//! Restartable concrete-source generation owned by the application runtime.

use std::{sync::Arc, time::Duration};

use tokio::{sync::watch, task::JoinSet, time::timeout};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::{UsageSnapshot, log_trunc};
use usage_sources::{
    SourceEvent,
    claude::{
        bridge_source::{BridgeSource, BridgeSourceConfig},
        effectiveness::{EffectivenessHandle, EffectivenessSnapshot},
        history::{HistorySource, HistorySourceConfig},
    },
    codex::{
        app_server::{AppServerConfig, AppServerControl, AppServerSource},
        rollout::{RolloutConfig, RolloutSource},
    },
    scheduler::Scheduler,
};

use crate::{
    runtime::RuntimeRoots,
    settings::{Settings, SettingsState},
};

const GENERATION_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct SourceSupervisor {
    roots: RuntimeRoots,
    settings: watch::Receiver<SettingsState>,
    effectiveness: EffectivenessHandle,
    events: tokio::sync::mpsc::Sender<SourceEvent>,
    scheduler: Scheduler,
    app_server: watch::Sender<Option<AppServerControl>>,
    #[cfg_attr(
        not(feature = "claude-oauth"),
        allow(dead_code, reason = "read only by the optional OAuth source")
    )]
    usage: watch::Receiver<Arc<UsageSnapshot>>,
}

impl SourceSupervisor {
    pub(super) fn new(
        roots: RuntimeRoots,
        settings: watch::Receiver<SettingsState>,
        effectiveness: EffectivenessHandle,
        events: tokio::sync::mpsc::Sender<SourceEvent>,
        scheduler: Scheduler,
        app_server: watch::Sender<Option<AppServerControl>>,
        usage: watch::Receiver<Arc<UsageSnapshot>>,
    ) -> Self {
        Self {
            roots,
            settings,
            effectiveness,
            events,
            scheduler,
            app_server,
            usage,
        }
    }

    pub(super) async fn run(mut self, cancel: CancellationToken) {
        let mut evidence = self.effectiveness.subscribe();
        loop {
            let settings = self.settings.borrow_and_update().settings.clone();
            let installed_at = evidence.borrow_and_update().installed_at;
            let paths = self.roots.paths(&settings);
            let generation_cancel = cancel.child_token();
            let mut tasks = JoinSet::new();

            let (app_server, control) = AppServerSource::new(
                AppServerConfig::new(self.roots.discovery(&settings), env!("CARGO_PKG_VERSION")),
                self.scheduler.clone(),
            );
            self.app_server.send_replace(Some(control));
            spawn_app_server(
                &mut tasks,
                app_server,
                self.events.clone(),
                generation_cancel.child_token(),
            );
            spawn_rollout(
                &mut tasks,
                RolloutSource::new(
                    RolloutConfig::new(&paths.codex_sessions),
                    self.scheduler.clone(),
                ),
                self.events.clone(),
                generation_cancel.child_token(),
            );
            spawn_bridge(
                &mut tasks,
                BridgeSource::new(
                    BridgeSourceConfig::from_paths(&paths, installed_at),
                    self.effectiveness.clone(),
                ),
                self.events.clone(),
                generation_cancel.child_token(),
            );
            spawn_history(
                &mut tasks,
                HistorySource::new(
                    HistorySourceConfig::from_paths(&paths),
                    self.effectiveness.clone(),
                ),
                self.events.clone(),
                generation_cancel.child_token(),
            );
            #[cfg(feature = "claude-oauth")]
            if settings.claude_oauth_enabled {
                spawn_oauth(
                    &mut tasks,
                    &self.scheduler,
                    &self.usage,
                    self.events.clone(),
                    generation_cancel.child_token(),
                );
            }

            let restart = wait_for_restart(
                &mut self.settings,
                &mut evidence,
                &settings,
                installed_at,
                &cancel,
            )
            .await;
            generation_cancel.cancel();
            drain_generation(&mut tasks).await;
            self.app_server.send_replace(None);
            if !restart {
                return;
            }
        }
    }
}

async fn wait_for_restart(
    settings: &mut watch::Receiver<SettingsState>,
    evidence: &mut watch::Receiver<EffectivenessSnapshot>,
    current_settings: &Settings,
    installed_at: Option<usage_core::UnixSeconds>,
    cancel: &CancellationToken,
) -> bool {
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return false,
            changed = settings.changed() => {
                if changed.is_err() {
                    return false;
                }
                let next = &settings.borrow_and_update().settings;
                if source_inputs_changed(current_settings, next) {
                    return true;
                }
            },
            changed = evidence.changed() => {
                if changed.is_err() {
                    return false;
                }
                if evidence.borrow_and_update().installed_at != installed_at {
                    return true;
                }
            }
        }
    }
}

fn source_inputs_changed(current: &Settings, next: &Settings) -> bool {
    current.codex_path != next.codex_path
        || current.codex_home != next.codex_home
        || current.claude_dir != next.claude_dir
        || current.claude_oauth_enabled != next.claude_oauth_enabled
}

async fn drain_generation(tasks: &mut JoinSet<()>) {
    if timeout(GENERATION_SHUTDOWN_TIMEOUT, drain(tasks))
        .await
        .is_err()
    {
        warn!("source generation did not stop within three seconds; aborting");
        tasks.abort_all();
        drain(tasks).await;
    }
}

async fn drain(tasks: &mut JoinSet<()>) {
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            warn!(error = %log_trunc(&error.to_string()), "source task join failed");
        }
    }
}

#[cfg(feature = "claude-oauth")]
fn spawn_oauth(
    tasks: &mut JoinSet<()>,
    scheduler: &Scheduler,
    usage: &watch::Receiver<Arc<UsageSnapshot>>,
    events: tokio::sync::mpsc::Sender<SourceEvent>,
    cancel: CancellationToken,
) {
    use usage_sources::claude::{
        keychain::KeychainCredentials,
        oauth::{OAuthSource, ReqwestUsageHttp},
    };

    let http = match ReqwestUsageHttp::new(env!("CARGO_PKG_VERSION")) {
        Ok(http) => http,
        Err(error) => {
            warn!(error = %log_trunc(&error.to_string()), "Claude OAuth source could not start");
            return;
        }
    };
    let source = OAuthSource::new(
        http,
        KeychainCredentials::new(),
        scheduler.clone(),
        usage.clone(),
    );
    tasks.spawn(async move {
        if let Err(error) = source.run(events, cancel).await {
            warn!(error = %log_trunc(&error.to_string()), "Claude OAuth source stopped");
        }
    });
}

fn spawn_app_server(
    tasks: &mut JoinSet<()>,
    source: AppServerSource,
    events: tokio::sync::mpsc::Sender<SourceEvent>,
    cancel: CancellationToken,
) {
    tasks.spawn(async move {
        if let Err(error) = source.run(events, cancel).await {
            warn!(error = %log_trunc(&error.to_string()), "Codex app-server source stopped");
        }
    });
}

fn spawn_rollout(
    tasks: &mut JoinSet<()>,
    source: RolloutSource,
    events: tokio::sync::mpsc::Sender<SourceEvent>,
    cancel: CancellationToken,
) {
    tasks.spawn(async move {
        if let Err(error) = source.run(events, cancel).await {
            warn!(error = %log_trunc(&error.to_string()), "Codex rollout source stopped");
        }
    });
}

fn spawn_bridge(
    tasks: &mut JoinSet<()>,
    source: BridgeSource,
    events: tokio::sync::mpsc::Sender<SourceEvent>,
    cancel: CancellationToken,
) {
    tasks.spawn(async move {
        if let Err(error) = source.run(events, cancel).await {
            warn!(error = %log_trunc(&error.to_string()), "Claude bridge source stopped");
        }
    });
}

fn spawn_history(
    tasks: &mut JoinSet<()>,
    source: HistorySource,
    events: tokio::sync::mpsc::Sender<SourceEvent>,
    cancel: CancellationToken,
) {
    tasks.spawn(async move {
        if let Err(error) = source.run(events, cancel).await {
            warn!(error = %log_trunc(&error.to_string()), "Claude history source stopped");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::source_inputs_changed;
    use crate::settings::Settings;

    #[test]
    fn only_source_inputs_restart_the_generation() {
        let current = Settings::default();
        let mut display_change = current.clone();
        display_change.widget.visible = true;
        display_change.thresholds = vec![50, 80];
        assert!(!source_inputs_changed(&current, &display_change));

        let mut path_change = current.clone();
        path_change.codex_path = Some("/tmp/codex".to_owned());
        assert!(source_inputs_changed(&current, &path_change));

        let mut oauth_change = current.clone();
        oauth_change.claude_oauth_enabled = true;
        assert!(source_inputs_changed(&current, &oauth_change));
    }
}
