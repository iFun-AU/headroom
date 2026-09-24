//! Application actor graph, dynamic source ownership, and graceful shutdown.

use std::{
    env,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use tauri::AppHandle;
use tokio::{sync::watch, task::JoinSet, time::timeout};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::{Alert, log_trunc};
use usage_sources::{
    claude::{
        bridge_install::{BridgeInstallConfig, BridgeInstaller},
        effectiveness::EffectivenessHandle,
    },
    codex::{app_server::AppServerControl, discover::DiscoveryOptions},
    paths::{PathEnvironment, PathOverrides, Paths},
    scheduler::Scheduler,
    store::{StoreClient, UsageStore},
};

use crate::{
    forwarder,
    settings::{Settings, SettingsHandle, SettingsState},
    soak::SoakMode,
    windows::WindowEvents,
};

mod sources;
mod support;

use sources::SourceSupervisor;
pub use support::{bridge_context, display_path, scheduler_config, trigger_visible_refresh};
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
/// Stable platform roots and bundled executable used to derive runtime paths.
#[derive(Debug, Clone)]
pub struct RuntimeRoots {
    home: PathBuf,
    data: PathBuf,
    environment: PathEnvironment,
    bundled_bridge: PathBuf,
}

impl RuntimeRoots {
    /// Captures platform roots and relevant environment overrides once.
    #[must_use]
    pub fn new(home: PathBuf, data: PathBuf, bundled_bridge: PathBuf) -> Self {
        Self {
            home,
            data,
            environment: PathEnvironment {
                codex_home: env::var_os("CODEX_HOME").map(PathBuf::from),
                claude_dir: env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
            },
            bundled_bridge,
        }
    }

    /// Derives every source/shell path from validated settings.
    #[must_use]
    pub fn paths(&self, settings: &Settings) -> Paths {
        Paths::resolve(
            &self.home,
            &self.data,
            PathOverrides {
                codex_home: settings.codex_home.as_deref().map(PathBuf::from),
                claude_dir: settings.claude_dir.as_deref().map(PathBuf::from),
            },
            self.environment.clone(),
        )
    }

    /// Builds documented Codex discovery inputs for current settings.
    #[must_use]
    pub fn discovery(&self, settings: &Settings) -> DiscoveryOptions {
        DiscoveryOptions::standard(
            settings.codex_path.as_deref().map(PathBuf::from),
            &self.home,
        )
    }

    /// Builds the bridge installer for current path settings.
    #[must_use]
    pub fn bridge_installer(&self, settings: &Settings) -> BridgeInstaller {
        let paths = self.paths(settings);
        BridgeInstaller::new(BridgeInstallConfig::from_paths(
            &self.bundled_bridge,
            &paths,
        ))
    }

    /// Returns the canonical application settings path.
    #[must_use]
    pub fn settings_path(&self) -> PathBuf {
        self.paths(&Settings::default())
            .app_support
            .join("settings.json")
    }
}

/// Managed state used by commands, tray actions, and lifecycle handlers.
pub struct RuntimeState {
    /// Usage snapshot/history command client.
    pub store: StoreClient,
    /// Adaptive polling scheduler.
    pub scheduler: Scheduler,
    /// Versioned settings actor handle.
    pub settings: SettingsHandle,
    /// Claude effectiveness actor handle.
    pub effectiveness: EffectivenessHandle,
    /// Current Codex process control, replaced with each source generation.
    pub app_server: watch::Receiver<Option<AppServerControl>>,
    /// Platform roots used for current settings.
    pub roots: RuntimeRoots,
    shutdown: ShutdownCoordinator,
}

impl RuntimeState {
    /// Starts the bounded actor graph and source/forwarder supervisors.
    pub(crate) async fn start(
        app: AppHandle,
        roots: RuntimeRoots,
        initial: SettingsState,
        window_events: WindowEvents,
        soak_mode: Option<SoakMode>,
    ) -> Self {
        let settings_value = initial.settings.clone();
        let (settings, settings_actor) =
            SettingsHandle::channel(roots.settings_path(), initial.clone());
        let installer = roots.bridge_installer(&settings_value);
        let installed_at = match installer.inspect().await {
            Ok(inspection) => {
                if inspection.installed
                    && let Err(error) = installer.refresh_binary().await
                {
                    warn!(error = %log_trunc(&error.to_string()), "could not refresh installed Claude bridge binary");
                }
                inspection.installed_at
            }
            Err(error) => {
                warn!(error = %log_trunc(&error.to_string()), "could not inspect Claude bridge state");
                None
            }
        };
        let (effectiveness, effectiveness_actor) = EffectivenessHandle::channel(installed_at);
        let config = scheduler_config(&settings_value);
        let (scheduler, scheduler_actor) = Scheduler::channel(config);
        let (events, store_handle, store_actor) = UsageStore::channel_with_scheduler(
            settings_value.thresholds.clone(),
            scheduler.clone(),
        );
        let (store, alerts) = store_handle.split();
        if let Err(error) = store
            .update_alert_settings(
                settings_value.thresholds.clone(),
                settings_value.notify_on_reset,
            )
            .await
        {
            warn!(%error, "could not apply initial alert settings");
        }
        let (app_server_tx, app_server) = watch::channel(None);
        let source_supervisor = SourceSupervisor::new(
            roots.clone(),
            settings.subscribe(),
            effectiveness.clone(),
            events,
            scheduler.clone(),
            app_server_tx,
            store.snapshot.clone(),
        );

        let cancel = CancellationToken::new();
        let (finished_tx, finished) = watch::channel(false);
        let shutdown = ShutdownCoordinator::new(cancel.clone(), finished);
        let supervisor = spawn_supervisor(
            app,
            cancel,
            finished_tx,
            settings_actor,
            scheduler_actor,
            store_actor,
            effectiveness_actor,
            source_supervisor,
            store.snapshot.clone(),
            alerts,
            settings.subscribe(),
            effectiveness.subscribe(),
            roots.clone(),
            settings.clone(),
            window_events,
            scheduler.clone(),
            soak_mode,
        );
        shutdown.set_task(supervisor);

        Self {
            store,
            scheduler,
            settings,
            effectiveness,
            app_server,
            roots,
            shutdown,
        }
    }

    /// Marks shutdown as initiated; `true` is returned to the first caller.
    #[must_use]
    pub fn begin_exit(&self) -> bool {
        self.shutdown.begin_exit()
    }

    /// Cancels and joins all owned runtime work within three seconds.
    pub async fn shutdown(&self) {
        self.shutdown.shutdown().await;
    }

    /// Prevents the first native exit request while graceful shutdown runs.
    #[must_use]
    pub fn spawn_exit(&self, app: AppHandle) -> bool {
        if !self.begin_exit() {
            return false;
        }
        let shutdown = self.shutdown.clone();
        tokio::spawn(async move {
            shutdown.shutdown().await;
            app.exit(0);
        });
        true
    }

    /// Cancels actors when the event loop is already exiting.
    pub fn cancel(&self) {
        self.shutdown.cancel.cancel();
    }
}

#[derive(Clone)]
struct ShutdownCoordinator {
    cancel: CancellationToken,
    finished: watch::Receiver<bool>,
    exiting: Arc<AtomicBool>,
    task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl ShutdownCoordinator {
    fn new(cancel: CancellationToken, finished: watch::Receiver<bool>) -> Self {
        Self {
            cancel,
            finished,
            exiting: Arc::new(AtomicBool::new(false)),
            task: Arc::new(Mutex::new(None)),
        }
    }

    fn set_task(&self, task: tokio::task::JoinHandle<()>) {
        if let Ok(mut slot) = self.task.lock() {
            *slot = Some(task);
        } else {
            task.abort();
            warn!("runtime supervisor handle lock was poisoned");
        }
    }

    fn begin_exit(&self) -> bool {
        !self.exiting.swap(true, Ordering::AcqRel)
    }

    async fn shutdown(&self) {
        self.cancel.cancel();
        let mut finished = self.finished.clone();
        if !*finished.borrow_and_update() {
            let _ = timeout(SHUTDOWN_TIMEOUT, finished.changed()).await;
        }
        let task = if let Ok(mut slot) = self.task.lock() {
            slot.take()
        } else {
            warn!("runtime supervisor handle lock was poisoned");
            None
        };
        if let Some(mut task) = task
            && timeout(Duration::from_millis(100), &mut task)
                .await
                .is_err()
        {
            warn!("runtime supervisor join did not complete after shutdown; aborting");
            task.abort();
            let _ = task.await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_supervisor(
    app: AppHandle,
    cancel: CancellationToken,
    finished: watch::Sender<bool>,
    settings_actor: crate::settings::SettingsActor,
    scheduler_actor: usage_sources::scheduler::SchedulerActor,
    store_actor: UsageStore,
    effectiveness_actor: usage_sources::claude::effectiveness::EffectivenessActor,
    sources: SourceSupervisor,
    snapshots: watch::Receiver<Arc<usage_core::UsageSnapshot>>,
    alerts: tokio::sync::mpsc::Receiver<Alert>,
    settings_updates: watch::Receiver<SettingsState>,
    effectiveness_updates: watch::Receiver<
        usage_sources::claude::effectiveness::EffectivenessSnapshot,
    >,
    roots: RuntimeRoots,
    settings: SettingsHandle,
    window_events: WindowEvents,
    scheduler: Scheduler,
    soak_mode: Option<SoakMode>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tasks = JoinSet::new();
        tasks.spawn(settings_actor.run(cancel.child_token()));
        tasks.spawn(scheduler_actor.run(cancel.child_token()));
        tasks.spawn(store_actor.run(cancel.child_token()));
        tasks.spawn(effectiveness_actor.run(cancel.child_token()));
        if let Some(soak_mode) = soak_mode {
            tasks.spawn(soak_mode.run(scheduler, cancel.child_token()));
        }
        tasks.spawn(sources.run(cancel.child_token()));
        tasks.spawn(forwarder::run_usage_events(
            app.clone(),
            snapshots,
            cancel.child_token(),
        ));
        tasks.spawn(forwarder::run_settings_events(
            app.clone(),
            settings_updates,
            cancel.child_token(),
        ));
        tasks.spawn(forwarder::run_alerts(
            app.clone(),
            alerts,
            cancel.child_token(),
        ));
        tasks.spawn(forwarder::run_bridge_events(
            app.clone(),
            effectiveness_updates,
            roots,
            settings.clone(),
            cancel.child_token(),
        ));
        tasks.spawn(crate::windows::run(
            app.clone(),
            settings.subscribe(),
            settings,
            window_events,
            cancel.child_token(),
        ));

        cancel.cancelled().await;
        if timeout(SHUTDOWN_TIMEOUT, drain(&mut tasks)).await.is_err() {
            warn!("runtime tasks did not stop within three seconds; aborting");
            tasks.abort_all();
            drain(&mut tasks).await;
        }
        finished.send_replace(true);
    })
}

async fn drain(tasks: &mut JoinSet<()>) {
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            warn!(error = %log_trunc(&error.to_string()), "runtime task join failed");
        }
    }
}
