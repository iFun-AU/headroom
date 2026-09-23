//! Stateless runtime helpers shared with commands.

use std::{path::Path, time::Duration};

use usage_core::SourceKind;
use usage_sources::{
    claude::bridge_install::BridgeInstaller,
    scheduler::{Scheduler, SchedulerConfig},
};

use super::RuntimeState;
use crate::settings::Settings;

/// Builds scheduler timing from already-clamped settings.
#[must_use]
pub fn scheduler_config(settings: &Settings) -> SchedulerConfig {
    SchedulerConfig {
        codex_active: Duration::from_secs(u64::from(settings.poll_active_secs)),
        claude_active: Duration::from_secs(u64::from(settings.poll_active_secs)),
        idle: Duration::from_secs(u64::from(settings.poll_idle_secs)),
        ..SchedulerConfig::default()
    }
}

/// Produces a bridge installer and Claude settings path for current state.
#[must_use]
pub fn bridge_context(state: &RuntimeState) -> (BridgeInstaller, std::path::PathBuf) {
    let settings = state.settings.snapshot().settings;
    let paths = state.roots.paths(&settings);
    (
        state.roots.bridge_installer(&settings),
        paths.claude_settings,
    )
}

/// Lossily represents a local path for the string-only IPC contract.
#[must_use]
pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Triggers the polling source when a UI surface becomes visible.
///
/// # Errors
///
/// Returns an error string when the scheduler is busy or unavailable.
pub fn trigger_visible_refresh(scheduler: &Scheduler) -> Result<(), String> {
    scheduler
        .ui_visible(SourceKind::CodexAppServer)
        .map_err(|error| error.to_string())
}
