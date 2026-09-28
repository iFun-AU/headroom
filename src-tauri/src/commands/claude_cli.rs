//! Explicit Claude refresh, independent of status-line installation and OAuth access.

use super::CommandError;
use crate::runtime::RuntimeState;
use crate::settings::Settings;
use std::time::Duration;
use tauri::State;
use usage_core::{ConnectionStatus, Provider, SourceKind};
use usage_sources::{
    SourceEvent,
    claude::cli::{CliError, ProbeConfig},
    codex::discover::discover,
};

/// Runs a temporary Claude Code `/usage` session and publishes its validated limits.
///
/// # Errors
/// Returns a fixed safe diagnostic if discovery, sign-in, capture, or cooldown fails.
#[tauri::command]
pub async fn refresh_claude_cli(state: State<'_, RuntimeState>) -> Result<(), CommandError> {
    refresh(&state).await
}

pub(super) async fn refresh(state: &RuntimeState) -> Result<(), CommandError> {
    let settings = state.settings.snapshot().settings;
    let paths = state.roots.paths(&settings);
    let result = async {
        let binary = discover(&state.roots.claude_discovery())
            .await
            .map_err(|_| CliError::Process)?
            .ok_or(CliError::NotInstalled)?;
        state
            .claude_cli
            .refresh(ProbeConfig {
                binary,
                claude_dir: state.roots.claude_config_override(&settings),
                working_directory: paths.app_support.join("ClaudeUsageProbe"),
                timeout: Duration::from_secs(35),
            })
            .await
    }
    .await;
    // A settings change while the probe was running must not publish into a new profile.
    if state
        .roots
        .claude_config_override(&state.settings.snapshot().settings)
        != state.roots.claude_config_override(&settings)
    {
        return Err(CommandError::public(CliError::Cancelled.to_string()));
    }
    let event = match &result {
        Ok(reading) => SourceEvent::Reading(reading.clone()),
        // Busy/cooldown are request outcomes, not fresh evidence that the source failed.
        Err(CliError::Busy | CliError::Cooldown(_) | CliError::Cancelled) => {
            return result
                .map(|_| ())
                .map_err(|error| CommandError::public(error.to_string()));
        }
        Err(error) => SourceEvent::Status {
            provider: Provider::Claude,
            source: SourceKind::ClaudeCli,
            status: ConnectionStatus::Error {
                message: error.to_string(),
            },
        },
    };
    state
        .source_events
        .send(event)
        .await
        .map_err(|_| CommandError::public("Usage updates are unavailable"))?;
    result
        .map(|_| ())
        .map_err(|error| CommandError::public(error.to_string()))
}

pub(super) async fn clear_changed_profile(
    state: &RuntimeState,
    prior: &Settings,
    next: &Settings,
) -> Result<(), CommandError> {
    if state.roots.claude_config_override(prior) != state.roots.claude_config_override(next) {
        state
            .source_events
            .send(SourceEvent::ClearSource {
                provider: Provider::Claude,
                source: SourceKind::ClaudeCli,
            })
            .await
            .map_err(|_| CommandError::public("Could not clear the previous Claude profile"))?;
    }
    Ok(())
}
