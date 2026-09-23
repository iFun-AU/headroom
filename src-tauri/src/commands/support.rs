//! Shared command error, process, plugin, and window helpers.

use std::{path::PathBuf, process::Stdio, time::Duration};

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
use tracing::{error, warn};
use usage_core::log_trunc;

use super::{CommandError, WindowTarget};

const VERSION_TIMEOUT: Duration = Duration::from_secs(3);
const VERSION_OUTPUT_LIMIT: u64 = 4 * 1_024;

pub(super) fn sync_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().map_err(|error| error.to_string())? == enabled {
        return Ok(());
    }
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|error| error.to_string())
}

pub(super) fn window(
    app: &AppHandle,
    target: WindowTarget,
) -> Result<tauri::WebviewWindow, CommandError> {
    let label = match target {
        WindowTarget::Main => "main",
        WindowTarget::Popover => "popover",
        WindowTarget::Widget => "widget",
    };
    app.get_webview_window(label)
        .ok_or_else(|| CommandError::public("The requested window is unavailable"))
}

pub(super) async fn codex_version(path: &PathBuf) -> Option<String> {
    let mut child = match Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            error!(error = %log_trunc(&error.to_string()), "could not start Codex version probe");
            return None;
        }
    };
    let stdout = child.stdout.take()?;
    let completed = tokio::time::timeout(VERSION_TIMEOUT, async {
        tokio::try_join!(child.wait(), read_bounded(stdout))
    })
    .await;
    let (status, output) = match completed {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            error!(error = %log_trunc(&error.to_string()), "Codex version probe failed");
            return None;
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            warn!("Codex version probe timed out");
            return None;
        }
    };
    if !status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output).trim().to_owned();
    (!version.is_empty()).then_some(version)
}

async fn read_bounded<R>(reader: R) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    reader
        .take(VERSION_OUTPUT_LIMIT)
        .read_to_end(&mut bytes)
        .await?;
    Ok(bytes)
}

pub(super) fn internal(public: &'static str, error: &impl std::fmt::Display) -> CommandError {
    error!(error = %log_trunc(&error.to_string()), message = public, "command failed");
    CommandError::public(public)
}
