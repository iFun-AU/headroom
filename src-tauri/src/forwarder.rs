//! Throttled backend-to-window event forwarding.

use std::{sync::Arc, time::Duration};

use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use usage_core::{Alert, UsageSnapshot, log_trunc};
use usage_sources::claude::effectiveness::EffectivenessSnapshot;

use crate::{
    commands::BridgeStatus,
    runtime::{RuntimeRoots, display_path},
    settings::{SettingsHandle, SettingsState},
    tray,
};

const SNAPSHOT_THROTTLE: Duration = Duration::from_millis(250);

/// Coalesces snapshot bursts and emits the latest trailing value every 250 ms.
pub async fn forward_snapshots<F>(
    mut snapshots: watch::Receiver<Arc<UsageSnapshot>>,
    cancel: CancellationToken,
    mut emit: F,
) where
    F: FnMut(Arc<UsageSnapshot>) + Send + 'static,
{
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            changed = snapshots.changed() => {
                if changed.is_err() {
                    return;
                }
            }
        }
        let mut latest = Arc::clone(&snapshots.borrow_and_update());
        let throttle = tokio::time::sleep(SNAPSHOT_THROTTLE);
        tokio::pin!(throttle);
        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                changed = snapshots.changed() => {
                    if changed.is_err() {
                        emit(latest);
                        return;
                    }
                    latest = Arc::clone(&snapshots.borrow_and_update());
                }
                () = &mut throttle => {
                    emit(latest);
                    break;
                }
            }
        }
    }
}

/// Emits throttled `usage-updated` events to all windows.
pub async fn run_usage_events(
    app: AppHandle,
    snapshots: watch::Receiver<Arc<UsageSnapshot>>,
    cancel: CancellationToken,
) {
    forward_snapshots(snapshots, cancel, move |snapshot| {
        tray::update(&app, &snapshot);
        if let Err(error) = app.emit("usage-updated", &*snapshot) {
            warn!(error = %log_trunc(&error.to_string()), "could not emit usage update");
        }
    })
    .await;
}

/// Emits every persisted `settings-changed` state transition.
pub async fn run_settings_events(
    app: AppHandle,
    mut settings: watch::Receiver<SettingsState>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            changed = settings.changed() => {
                if changed.is_err() {
                    return;
                }
                let state = settings.borrow_and_update().clone();
                if let Err(error) = app.emit("settings-changed", state) {
                    warn!(error = %log_trunc(&error.to_string()), "could not emit settings update");
                }
            }
        }
    }
}

/// Delivers bounded core alerts through the native notification plugin.
pub async fn run_alerts(
    app: AppHandle,
    mut alerts: mpsc::Receiver<Alert>,
    cancel: CancellationToken,
) {
    loop {
        let alert = tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            alert = alerts.recv() => alert,
        };
        let Some(alert) = alert else {
            return;
        };
        if let Err(error) = app
            .notification()
            .builder()
            .title(alert.title)
            .body(alert.body)
            .show()
        {
            warn!(error = %log_trunc(&error.to_string()), "could not show usage notification");
        }
    }
}

/// Emits semantic and behavioral Claude bridge status changes.
pub async fn run_bridge_events(
    app: AppHandle,
    mut effectiveness: watch::Receiver<EffectivenessSnapshot>,
    roots: RuntimeRoots,
    settings: SettingsHandle,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return,
            changed = effectiveness.changed() => {
                if changed.is_err() {
                    return;
                }
            }
        }
        let effective = effectiveness.borrow_and_update().effective;
        let current = settings.snapshot().settings;
        let paths = roots.paths(&current);
        let inspection = match roots.bridge_installer(&current).inspect().await {
            Ok(inspection) => inspection,
            Err(error) => {
                warn!(error = %log_trunc(&error.to_string()), "could not inspect bridge for status event");
                continue;
            }
        };
        let status = BridgeStatus {
            installed: inspection.installed,
            chained: inspection.chained,
            effective,
            settings_path: display_path(&paths.claude_settings),
        };
        if let Err(error) = app.emit("bridge-status-changed", status) {
            warn!(error = %log_trunc(&error.to_string()), "could not emit bridge status");
        }
    }
}
