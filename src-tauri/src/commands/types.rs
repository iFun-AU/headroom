//! Serializable command arguments, responses, and public errors.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use usage_sources::claude::effectiveness::Effectiveness;

/// Public, deliberately bounded error returned by every Tauri command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CommandError {
    /// Safe user-facing failure summary.
    pub message: String,
}

impl CommandError {
    /// Creates a public error without embedding provider output or credentials.
    #[must_use]
    pub fn public(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Native path-picker mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PathKind {
    /// Select one file.
    File,
    /// Select one directory.
    Directory,
}

/// Settings field for which a native path is being selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PathPurpose {
    /// Codex executable override.
    CodexBinary,
    /// Codex data root override.
    CodexHome,
    /// Claude configuration root override.
    ClaudeDir,
}

/// One of the three native windows addressable by commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum WindowTarget {
    /// Dashboard/settings/onboarding window.
    Main,
    /// Menu-bar popover window.
    Popover,
    /// Floating widget window.
    Widget,
}

/// Hash routes hosted by the main window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Route {
    /// Two-provider overview.
    Overview,
    /// Claude detail dashboard.
    Claude,
    /// Codex detail dashboard.
    Codex,
    /// Application settings.
    Settings,
    /// First-run onboarding.
    Onboarding,
}

/// Claude bridge installation and behavioral status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BridgeStatus {
    /// Whether Claude settings currently invoke our bridge.
    pub installed: bool,
    /// Whether the bridge invokes an earlier configured command.
    pub chained: bool,
    /// Behavioral confidence that Claude is invoking the bridge.
    pub effective: Effectiveness,
    /// Claude settings path shown in diagnostics.
    pub settings_path: String,
}

/// Result of bounded Codex executable detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CodexDetection {
    /// Resolved executable path, if found.
    pub path: Option<String>,
    /// Bounded `codex --version` output, if executable probing succeeded.
    pub version: Option<String>,
}
