//! Claude bridge inspection, install, and semantic uninstall commands.

use tauri::State;
use tracing::warn;
use usage_core::log_trunc;
use usage_sources::claude::{
    bridge_install::{BridgeInstaller, UninstallOutcome},
    effectiveness::Effectiveness,
};

use super::{BridgeStatus, CommandError, support::internal};
use crate::runtime::{RuntimeState, bridge_context, display_path};

/// Returns semantic and behavioral Claude bridge status.
///
/// # Errors
///
/// Returns a safe command error when bounded bridge inspection fails.
#[tauri::command]
pub async fn claude_bridge_status(
    state: State<'_, RuntimeState>,
) -> Result<BridgeStatus, CommandError> {
    bridge_status_impl(&state).await
}

/// Installs the bundled Claude bridge after explicit user action.
///
/// # Errors
///
/// Returns a safe command error for read-only settings or installation,
/// effectiveness, persistence, or inspection failures.
#[tauri::command]
pub async fn install_claude_bridge(
    state: State<'_, RuntimeState>,
) -> Result<BridgeStatus, CommandError> {
    let current = state.settings.snapshot();
    if current.read_only {
        return Err(CommandError::public(
            "Reset newer-version settings before enabling the Claude bridge",
        ));
    }
    let (installer, _) = bridge_context(&state);
    let receipt = installer
        .install()
        .await
        .map_err(|error| internal("Could not install the Claude bridge", &error))?;
    if let Err(error) = state.effectiveness.install(receipt.installed_at).await {
        rollback_install(&installer, &state).await;
        return Err(internal("Could not start Claude bridge monitoring", &error));
    }
    let mut settings = current.settings;
    settings.claude_bridge_enabled = true;
    if let Err(error) = state.settings.set(settings).await {
        rollback_install(&installer, &state).await;
        return Err(internal("Could not save Claude bridge settings", &error));
    }
    bridge_status_impl(&state).await
}

/// Semantically restores the prior Claude status-line configuration.
///
/// # Errors
///
/// Returns a safe command error for read-only settings or uninstall,
/// effectiveness, persistence, or inspection failures.
#[tauri::command]
pub async fn uninstall_claude_bridge(
    state: State<'_, RuntimeState>,
) -> Result<BridgeStatus, CommandError> {
    let current = state.settings.snapshot();
    if current.read_only {
        return Err(CommandError::public(
            "Reset newer-version settings before disabling the Claude bridge",
        ));
    }
    let (installer, _) = bridge_context(&state);
    let outcome = installer
        .uninstall()
        .await
        .map_err(|error| internal("Could not uninstall the Claude bridge", &error))?;
    if let Err(error) = state.effectiveness.uninstall().await {
        rollback_uninstall(&installer, &state, outcome).await;
        return Err(internal("Could not stop Claude bridge monitoring", &error));
    }
    let mut settings = current.settings;
    settings.claude_bridge_enabled = false;
    if let Err(error) = state.settings.set(settings).await {
        rollback_uninstall(&installer, &state, outcome).await;
        return Err(internal("Could not save Claude bridge settings", &error));
    }
    bridge_status_impl(&state).await
}

async fn rollback_install(installer: &BridgeInstaller, state: &RuntimeState) {
    if let Err(error) = installer.uninstall().await {
        warn!(error = %log_trunc(&error.to_string()), "could not roll back Claude bridge install");
    }
    if let Err(error) = state.effectiveness.uninstall().await {
        warn!(error = %log_trunc(&error.to_string()), "could not roll back Claude bridge monitoring");
    }
}

async fn rollback_uninstall(
    installer: &BridgeInstaller,
    state: &RuntimeState,
    outcome: UninstallOutcome,
) {
    if outcome != UninstallOutcome::Restored {
        return;
    }
    match installer.install().await {
        Ok(receipt) => {
            if let Err(error) = state.effectiveness.install(receipt.installed_at).await {
                warn!(error = %log_trunc(&error.to_string()), "could not restore Claude bridge monitoring");
            }
        }
        Err(error) => {
            warn!(error = %log_trunc(&error.to_string()), "could not roll back Claude bridge uninstall");
        }
    }
}

async fn bridge_status_impl(state: &RuntimeState) -> Result<BridgeStatus, CommandError> {
    let (installer, settings_path) = bridge_context(state);
    let inspection = installer
        .inspect()
        .await
        .map_err(|error| internal("Could not inspect the Claude bridge", &error))?;
    let effective = if inspection.installed {
        state.effectiveness.snapshot().effective
    } else {
        Effectiveness::Unverified
    };
    Ok(BridgeStatus {
        installed: inspection.installed,
        chained: inspection.chained,
        effective,
        settings_path: display_path(&settings_path),
    })
}
