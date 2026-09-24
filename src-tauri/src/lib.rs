//! Tauri application shell for How Is It.

use std::{io, path::PathBuf};

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tracing::warn;
use usage_core::log_trunc;

/// Typed Tauri IPC commands and contract types.
pub mod commands;
/// Throttled backend-to-window event forwarding.
pub mod forwarder;
/// Daily file logging and retention.
pub mod logging;
/// Application actor graph and graceful shutdown ownership.
pub mod runtime;
/// Versioned application settings and persistence.
pub mod settings;
mod soak;
/// Native menu-bar tray and presentation state.
pub mod tray;
/// Native window lifecycle and floating-widget placement.
pub mod windows;

/// Builds and runs the desktop application.
///
/// # Errors
///
/// Returns a Tauri error when application setup or the event loop cannot start.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_liquid_glass::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::get_history,
            commands::refresh_now,
            commands::get_settings,
            commands::set_settings,
            commands::reset_settings,
            commands::complete_onboarding,
            commands::bridge::claude_bridge_status,
            commands::bridge::install_claude_bridge,
            commands::bridge::uninstall_claude_bridge,
            commands::detect_codex,
            commands::pick_path,
            commands::show_window,
            commands::hide_window,
            commands::reveal_logs,
            commands::quit_app,
            commands::ui_visible,
        ])
        .setup(|app| {
            let soak_mode = soak::SoakMode::from_environment()?;
            let (home, data) = if let Some(mode) = &soak_mode {
                mode.platform_roots()
            } else {
                (app.path().home_dir()?, app.path().data_dir()?)
            };
            let executable = std::env::current_exe()?;
            let executable_dir = executable
                .parent()
                .ok_or_else(|| io::Error::other("application executable has no parent"))?;
            let roots = runtime::RuntimeRoots::new(
                home,
                data,
                PathBuf::from(executable_dir).join("howisit-statusline"),
            );
            let default_paths = roots.paths(&settings::Settings::default());
            let log_guard = logging::init(&default_paths.logs)?;
            if !app.manage(log_guard) {
                return Err(io::Error::other("logging guard was already managed").into());
            }
            let initial = settings::load_settings(&roots.settings_path())?;
            if soak_mode.is_none()
                && !initial.read_only
                && let Err(error) = commands::converge_autostart(app.handle(), &initial.settings)
            {
                warn!(error = %log_trunc(&error), "could not synchronize launch-at-login state");
            }
            let window_events = windows::install(app.handle()).map_err(io::Error::other)?;
            let runtime = tauri::async_runtime::block_on(runtime::RuntimeState::start(
                app.handle().clone(),
                roots,
                initial,
                window_events,
                soak_mode,
            ));
            if !app.manage(runtime) {
                return Err(io::Error::other("runtime state was already managed").into());
            }
            tray::build(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())?;
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            if let Some(runtime) = app.try_state::<runtime::RuntimeState>()
                && runtime.spawn_exit(app.clone())
            {
                api.prevent_exit();
            }
        }
        // Bars labels follow the menu-bar appearance; the rendered image is
        // only replaced when it actually changes.
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::ThemeChanged(_),
            ..
        } => tray::refresh(app),
        tauri::RunEvent::Exit => {
            if let Some(runtime) = app.try_state::<runtime::RuntimeState>() {
                runtime.cancel();
            }
        }
        _ => {}
    });
    Ok(())
}
