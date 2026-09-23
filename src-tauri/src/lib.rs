//! Tauri application shell for How Is It.

use tauri::Manager;

/// Builds and runs the desktop application.
///
/// # Errors
///
/// Returns a Tauri error when application setup or the event loop cannot start.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.show()?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
}
