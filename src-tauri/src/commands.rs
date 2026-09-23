//! Typed Tauri command surface and shared tray action implementations.

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::oneshot;
use tracing::warn;
use usage_core::{History, Provider, UsageSnapshot, log_trunc};
use usage_sources::codex::discover::discover;

use crate::{
    runtime::{RuntimeState, display_path, scheduler_config, trigger_visible_refresh},
    settings::{Settings, SettingsState},
};

pub(crate) mod bridge;
mod support;
mod types;

pub use bridge::{claude_bridge_status, install_claude_bridge, uninstall_claude_bridge};
use support::{codex_version, internal, sync_autostart, window};
pub use types::{
    BridgeStatus, CodexDetection, CommandError, PathKind, PathPurpose, Route, WindowTarget,
};

/// Converges the native `LaunchAgent` with persisted settings during startup.
///
/// # Errors
///
/// Returns a plugin error if the current state cannot be read or changed.
pub(crate) fn converge_autostart(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    sync_autostart(app, settings.launch_at_login)
}

/// Returns the latest immutable usage snapshot.
///
/// # Errors
///
/// This command currently has no fallible operation.
#[tauri::command]
pub async fn get_snapshot(state: State<'_, RuntimeState>) -> Result<UsageSnapshot, CommandError> {
    Ok(state.store.snapshot.borrow().as_ref().clone())
}

/// Returns bounded history for one provider.
///
/// # Errors
///
/// Returns a safe error when the store actor is unavailable.
#[tauri::command]
pub async fn get_history(
    provider: Provider,
    state: State<'_, RuntimeState>,
) -> Result<History, CommandError> {
    state
        .store
        .history(provider)
        .await
        .map_err(|error| internal("Could not load usage history", &error))
}

/// Requests immediate refresh of every compiled polling source.
///
/// # Errors
///
/// Returns a safe error when the store actor is unavailable.
#[tauri::command]
pub async fn refresh_now(state: State<'_, RuntimeState>) -> Result<(), CommandError> {
    refresh_now_impl(&state).await
}

/// Returns current settings plus downgrade-safety metadata.
///
/// # Errors
///
/// This command currently has no fallible operation.
#[tauri::command]
pub async fn get_settings(state: State<'_, RuntimeState>) -> Result<SettingsState, CommandError> {
    Ok(state.settings.snapshot())
}

/// Validates, persists, and applies a complete settings replacement.
///
/// # Errors
///
/// Returns a safe error when autostart, persistence, or runtime application fails.
#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    mut settings: Settings,
    state: State<'_, RuntimeState>,
) -> Result<SettingsState, CommandError> {
    let prior = state.settings.snapshot().settings;
    settings.claude_bridge_enabled = prior.claude_bridge_enabled;
    let autostart_changed = settings.launch_at_login != prior.launch_at_login;
    if autostart_changed {
        sync_autostart(&app, settings.launch_at_login)
            .map_err(|error| internal("Could not update launch at login", &error))?;
    }
    let saved = match state.settings.set(settings).await {
        Ok(saved) => saved,
        Err(error) => {
            if autostart_changed && let Err(rollback) = sync_autostart(&app, prior.launch_at_login)
            {
                warn!(error = %log_trunc(&rollback), "could not roll back launch-at-login state");
            }
            return Err(internal("Could not save settings", &error));
        }
    };
    apply_runtime_settings(&state, &saved.settings).await?;
    Ok(saved)
}

/// Explicitly resets even a newer settings schema to current defaults.
///
/// # Errors
///
/// Returns a safe error when autostart, persistence, or runtime application fails.
#[tauri::command]
pub async fn reset_settings(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<SettingsState, CommandError> {
    let prior = state.settings.snapshot().settings;
    if prior.launch_at_login {
        sync_autostart(&app, false)
            .map_err(|error| internal("Could not disable launch at login", &error))?;
    }
    let saved = match state.settings.reset().await {
        Ok(saved) => saved,
        Err(error) => {
            if prior.launch_at_login
                && let Err(rollback) = sync_autostart(&app, true)
            {
                warn!(error = %log_trunc(&rollback), "could not roll back launch-at-login state");
            }
            return Err(internal("Could not reset settings", &error));
        }
    };
    apply_runtime_settings(&state, &saved.settings).await?;
    Ok(saved)
}

/// Completes first-run onboarding and persists it.
///
/// # Errors
///
/// Returns a safe error when settings are read-only or persistence fails.
#[tauri::command]
pub async fn complete_onboarding(
    state: State<'_, RuntimeState>,
) -> Result<SettingsState, CommandError> {
    state
        .settings
        .complete_onboarding()
        .await
        .map_err(|error| internal("Could not complete onboarding", &error))
}

/// Discovers Codex and returns bounded version output.
///
/// # Errors
///
/// Returns a safe error when executable discovery fails.
#[tauri::command]
pub async fn detect_codex(state: State<'_, RuntimeState>) -> Result<CodexDetection, CommandError> {
    let settings = state.settings.snapshot().settings;
    let path = discover(&state.roots.discovery(&settings))
        .await
        .map_err(|error| internal("Could not detect Codex", &error))?;
    let Some(path) = path else {
        return Ok(CodexDetection {
            path: None,
            version: None,
        });
    };
    let version = codex_version(&path).await;
    Ok(CodexDetection {
        path: Some(display_path(&path)),
        version,
    })
}

/// Opens a non-blocking native path picker without saving its result.
///
/// # Errors
///
/// Returns a safe error if the dialog callback channel closes unexpectedly.
#[tauri::command]
pub async fn pick_path(
    app: AppHandle,
    kind: PathKind,
    purpose: PathPurpose,
) -> Result<Option<String>, CommandError> {
    let title = match purpose {
        PathPurpose::CodexBinary => "Choose the Codex executable",
        PathPurpose::CodexHome => "Choose the Codex data folder",
        PathPurpose::ClaudeDir => "Choose the Claude configuration folder",
    };
    let (reply, response) = oneshot::channel();
    let picker = app.dialog().file().set_title(title);
    let callback = move |selected: Option<tauri_plugin_dialog::FilePath>| {
        let path = selected.and_then(|selected| selected.into_path().ok());
        let _ = reply.send(path);
    };
    match kind {
        PathKind::File => picker.pick_file(callback),
        PathKind::Directory => picker.pick_folder(callback),
    }
    response
        .await
        .map(|path| path.map(|path| display_path(&path)))
        .map_err(|error| internal("The path picker closed unexpectedly", &error))
}

/// Shows and focuses a documented native window, optionally navigating main.
///
/// # Errors
///
/// Returns a safe error for an invalid target/route or native window failure.
#[tauri::command]
pub async fn show_window(
    app: AppHandle,
    which: WindowTarget,
    route: Option<Route>,
    state: State<'_, RuntimeState>,
) -> Result<(), CommandError> {
    show_window_impl(&app, &state, which, route).await
}

/// Hides the popover or widget window.
///
/// # Errors
///
/// Returns a safe error for the main target or a native window failure.
#[tauri::command]
pub async fn hide_window(
    app: AppHandle,
    which: WindowTarget,
    state: State<'_, RuntimeState>,
) -> Result<(), CommandError> {
    if which == WindowTarget::Main {
        return Err(CommandError::public(
            "The main window is hidden by closing it",
        ));
    }
    let window = window(&app, which)?;
    window
        .hide()
        .map_err(|error| internal("Could not hide the window", &error))?;
    if which == WindowTarget::Widget
        && !state.settings.snapshot().read_only
        && let Err(error) = state.settings.update_widget_visibility(false).await
    {
        if let Err(show_error) = window.show() {
            warn!(error = %log_trunc(&show_error.to_string()), "could not roll back widget hide");
        }
        return Err(internal("Could not save widget visibility", &error));
    }
    Ok(())
}

/// Reveals the daily-rotated application log directory in Finder.
///
/// # Errors
///
/// Returns a safe error when the directory cannot be created or revealed.
#[tauri::command]
pub async fn reveal_logs(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<(), CommandError> {
    let settings = state.settings.snapshot().settings;
    let logs = state.roots.paths(&settings).logs;
    tokio::fs::create_dir_all(&logs)
        .await
        .map_err(|error| internal("Could not create the log directory", &error))?;
    app.opener()
        .reveal_item_in_dir(logs)
        .map_err(|error| internal("Could not reveal logs in Finder", &error))
}

/// Gracefully stops owned work and exits the application.
///
/// # Errors
///
/// This command currently has no fallible operation.
#[tauri::command]
pub async fn quit_app(app: AppHandle, state: State<'_, RuntimeState>) -> Result<(), CommandError> {
    quit_app_impl(&app, &state).await;
    Ok(())
}

/// Marks a UI window visible and refreshes data older than thirty seconds.
///
/// # Errors
///
/// Returns a safe error when the scheduler is unavailable.
#[tauri::command]
pub async fn ui_visible(label: String, state: State<'_, RuntimeState>) -> Result<(), CommandError> {
    drop(label);
    trigger_visible_refresh(&state.scheduler)
        .map_err(|error| internal("Could not refresh visible usage", &error))
}

pub(crate) async fn refresh_now_impl(state: &RuntimeState) -> Result<(), CommandError> {
    state
        .store
        .refresh_now()
        .await
        .map_err(|error| internal("Could not refresh usage", &error))?;
    if let Some(control) = state.app_server.borrow().clone()
        && control.pid().is_none()
        && let Err(error) = control.retry()
    {
        warn!(%error, "could not queue Codex app-server retry");
    }
    Ok(())
}

pub(crate) async fn show_window_impl(
    app: &AppHandle,
    state: &RuntimeState,
    which: WindowTarget,
    route: Option<Route>,
) -> Result<(), CommandError> {
    if which == WindowTarget::Popover {
        return Err(CommandError::public(
            "The popover is controlled by the menu bar icon",
        ));
    }
    if route.is_some() && which != WindowTarget::Main {
        return Err(CommandError::public(
            "Routes can only be opened in the main window",
        ));
    }
    let window = window(app, which)?;
    if let Some(route) = route {
        window
            .emit("navigate", route)
            .map_err(|error| internal("Could not navigate the main window", &error))?;
    }
    window
        .show()
        .and_then(|()| window.set_focus())
        .map_err(|error| internal("Could not show the window", &error))?;
    if which == WindowTarget::Widget
        && !state.settings.snapshot().read_only
        && let Err(error) = state.settings.update_widget_visibility(true).await
    {
        if let Err(hide_error) = window.hide() {
            warn!(error = %log_trunc(&hide_error.to_string()), "could not roll back widget show");
        }
        return Err(internal("Could not save widget visibility", &error));
    }
    Ok(())
}

pub(crate) async fn quit_app_impl(app: &AppHandle, state: &RuntimeState) {
    let _ = state.begin_exit();
    state.shutdown().await;
    app.exit(0);
}

async fn apply_runtime_settings(
    state: &RuntimeState,
    settings: &Settings,
) -> Result<(), CommandError> {
    state
        .scheduler
        .update_config(scheduler_config(settings))
        .await
        .map_err(|error| internal("Could not apply refresh intervals", &error))?;
    state
        .store
        .update_alert_settings(settings.thresholds.clone(), settings.notify_on_reset)
        .await
        .map_err(|error| internal("Could not apply alert settings", &error))
}
