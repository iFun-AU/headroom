//! Semantic Claude status-line bridge installation and removal.

use std::{
    io,
    path::{Path, PathBuf},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
use usage_core::UnixSeconds;

use crate::paths::Paths;

mod storage;

use storage::{
    atomic_write, create_backup, encode_json, ensure_binary, read_object, read_value,
    remove_if_exists,
};

/// All bridge installation paths, explicit for deterministic tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeInstallConfig {
    /// Bridge executable inside the current application bundle.
    pub bundled_binary: PathBuf,
    /// Stable bridge executable in Application Support.
    pub installed_binary: PathBuf,
    /// Claude Code user settings file.
    pub claude_settings: PathBuf,
    /// App-owned semantic uninstall state.
    pub install_state: PathBuf,
    /// App-owned chained-command configuration.
    pub bridge_config: PathBuf,
}

impl BridgeInstallConfig {
    /// Creates a configuration from explicit paths.
    #[must_use]
    pub fn new(
        bundled_binary: impl Into<PathBuf>,
        installed_binary: impl Into<PathBuf>,
        claude_settings: impl Into<PathBuf>,
        install_state: impl Into<PathBuf>,
        bridge_config: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bundled_binary: bundled_binary.into(),
            installed_binary: installed_binary.into(),
            claude_settings: claude_settings.into(),
            install_state: install_state.into(),
            bridge_config: bridge_config.into(),
        }
    }

    /// Uses centralized application paths plus the bundle executable path.
    #[must_use]
    pub fn from_paths(bundled_binary: impl Into<PathBuf>, paths: &Paths) -> Self {
        Self::new(
            bundled_binary,
            &paths.bridge_binary,
            &paths.claude_settings,
            &paths.bridge_install_state,
            &paths.bridge_config,
        )
    }
}

/// Successful installation details used by Settings diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReceipt {
    /// Manual-recovery backup retained beside Claude settings.
    pub backup_path: PathBuf,
    /// Whether an earlier command was configured for chaining.
    pub chained_previous_command: bool,
    /// Whether the stable binary changed during this call.
    pub binary_updated: bool,
    /// Installation timestamp persisted for effectiveness checks.
    pub installed_at: UnixSeconds,
}

/// Read-only semantic installation state for Settings diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeInspection {
    /// Whether Claude settings currently invoke our installed command.
    pub installed: bool,
    /// Whether that active installation chains an earlier user command.
    pub chained: bool,
    /// Installation epoch used by the effectiveness tracker.
    pub installed_at: Option<UnixSeconds>,
}

/// Semantic uninstall result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallOutcome {
    /// The saved prior value was restored, or the formerly absent key removed.
    Restored,
    /// Claude settings no longer point at our command, so no user bytes changed.
    LeftUntouched,
    /// No installer state exists.
    NotInstalled,
}

impl UninstallOutcome {
    /// User-facing explanation for a safely skipped uninstall.
    #[must_use]
    pub const fn message(self) -> Option<&'static str> {
        match self {
            Self::LeftUntouched => {
                Some("Your status line was changed after install; left untouched")
            }
            Self::Restored | Self::NotInstalled => None,
        }
    }
}

/// Bridge installer failure.
#[derive(Debug, Error)]
pub enum BridgeInstallError {
    /// A bounded settings/configuration file exceeded 4 MiB.
    #[error("JSON file is too large: {path}")]
    FileTooLarge {
        /// File that exceeded the bound.
        path: PathBuf,
    },
    /// A settings/configuration file was invalid JSON.
    #[error("invalid JSON in {path}: {source}")]
    InvalidJson {
        /// File that could not be parsed.
        path: PathBuf,
        /// Parser failure.
        #[source]
        source: serde_json::Error,
    },
    /// A settings/configuration top level was not an object.
    #[error("JSON top level must be an object: {path}")]
    NonObject {
        /// Invalid file.
        path: PathBuf,
    },
    /// A required path had no parent or was not a regular file.
    #[error("invalid bridge path: {path}")]
    InvalidPath {
        /// Invalid path.
        path: PathBuf,
    },
    /// A bridge command path was not representable as UTF-8.
    #[error("bridge command path is not valid UTF-8: {path}")]
    NonUtf8Path {
        /// Invalid command path.
        path: PathBuf,
    },
    /// Installer state was missing required fields or used invalid types.
    #[error("bridge installer state is invalid")]
    InvalidState,
    /// A filesystem operation failed.
    #[error("could not {operation} at {path}: {source}")]
    Io {
        /// Fixed operation name.
        operation: &'static str,
        /// Affected path.
        path: PathBuf,
        /// Operating-system failure.
        #[source]
        source: io::Error,
    },
    /// JSON encoding failed.
    #[error("could not encode bridge state: {0}")]
    Json(#[from] serde_json::Error),
    /// The platform clock was earlier than the Unix epoch.
    #[error("system clock is invalid: {0}")]
    Clock(#[from] SystemTimeError),
    /// A blocking installer worker could not be joined.
    #[error("bridge installer worker stopped unexpectedly: {0}")]
    Task(#[from] tokio::task::JoinError),
}

/// Performs bridge filesystem mutations on bounded blocking workers.
#[derive(Debug, Clone)]
pub struct BridgeInstaller {
    config: BridgeInstallConfig,
}

impl BridgeInstaller {
    /// Creates an installer without touching the filesystem.
    #[must_use]
    pub const fn new(config: BridgeInstallConfig) -> Self {
        Self { config }
    }

    /// Installs using the current wall-clock timestamp.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, filesystem, clock, or worker error.
    pub async fn install(&self) -> Result<InstallReceipt, BridgeInstallError> {
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let installed_at = UnixSeconds(i64::try_from(seconds).unwrap_or(i64::MAX));
        self.install_at(installed_at).await
    }

    /// Installs with an injected timestamp for deterministic tests.
    #[doc(hidden)]
    ///
    /// # Errors
    ///
    /// Returns a typed validation, filesystem, or worker error.
    pub async fn install_at(
        &self,
        installed_at: UnixSeconds,
    ) -> Result<InstallReceipt, BridgeInstallError> {
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || install_sync(&config, installed_at)).await?
    }

    /// Re-copies a changed bundled bridge without editing Claude settings.
    ///
    /// # Errors
    ///
    /// Returns a filesystem or worker error.
    pub async fn refresh_binary(&self) -> Result<bool, BridgeInstallError> {
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || {
            ensure_binary(&config.bundled_binary, &config.installed_binary)
        })
        .await?
    }

    /// Inspects installer state and Claude settings without modifying either.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, filesystem, or worker error.
    pub async fn inspect(&self) -> Result<BridgeInspection, BridgeInstallError> {
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || inspect_sync(&config)).await?
    }

    /// Semantically restores the saved prior status line.
    ///
    /// # Errors
    ///
    /// Returns a typed validation, filesystem, or worker error.
    pub async fn uninstall(&self) -> Result<UninstallOutcome, BridgeInstallError> {
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || uninstall_sync(&config)).await?
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallState {
    previous_status_line: Value,
    previous_status_line_present: bool,
    installed_at: i64,
}

fn install_sync(
    config: &BridgeInstallConfig,
    installed_at: UnixSeconds,
) -> Result<InstallReceipt, BridgeInstallError> {
    let binary_updated = ensure_binary(&config.bundled_binary, &config.installed_binary)?;
    let settings =
        read_object(&config.claude_settings, true)?.ok_or(BridgeInstallError::InvalidState)?;
    let ours = bridge_command(&config.installed_binary)?;
    if current_status_command(&settings.value) == Some(ours.as_str())
        && let Some(state_value) = read_value(&config.install_state)?
    {
        let state: InstallState =
            serde_json::from_value(state_value).map_err(|_| BridgeInstallError::InvalidState)?;
        let bridge_config =
            read_object(&config.bridge_config, true)?.ok_or(BridgeInstallError::InvalidState)?;
        let installed_at = UnixSeconds(state.installed_at);
        return Ok(InstallReceipt {
            backup_path: backup_path(&config.claude_settings, installed_at)?,
            chained_previous_command: bridge_config
                .value
                .get("chainedCommand")
                .and_then(Value::as_str)
                .is_some(),
            binary_updated,
            installed_at,
        });
    }

    let mut root = settings.value;
    let previous_present = root.contains_key("statusLine");
    let previous = root.get("statusLine").cloned().unwrap_or(Value::Null);
    let chained = previous
        .as_object()
        .and_then(|status| status.get("command"))
        .and_then(Value::as_str)
        .filter(|command| *command != ours)
        .map(str::to_owned);

    let backup_path = backup_path(&config.claude_settings, installed_at)?;
    let backup_bytes = settings.original.as_deref().unwrap_or(b"{}\n");
    create_backup(&backup_path, backup_bytes, settings.mode)?;

    let state = InstallState {
        previous_status_line: previous.clone(),
        previous_status_line_present: previous_present,
        installed_at: installed_at.0,
    };
    let bridge_config = updated_bridge_config(&config.bridge_config, chained.as_deref())?;
    atomic_write(
        &config.install_state,
        &encode_json(&serde_json::to_value(state)?)?,
        Some(0o600),
    )?;
    atomic_write(
        &config.bridge_config,
        &encode_json(&bridge_config)?,
        Some(0o600),
    )?;

    let mut status = previous.as_object().cloned().unwrap_or_default();
    status.insert("type".to_owned(), Value::String("command".to_owned()));
    status.insert("command".to_owned(), Value::String(ours));
    root.insert("statusLine".to_owned(), Value::Object(status));
    atomic_write(
        &config.claude_settings,
        &encode_json(&Value::Object(root))?,
        settings.mode,
    )?;

    Ok(InstallReceipt {
        backup_path,
        chained_previous_command: chained.is_some(),
        binary_updated,
        installed_at,
    })
}

fn inspect_sync(config: &BridgeInstallConfig) -> Result<BridgeInspection, BridgeInstallError> {
    let Some(state_value) = read_value(&config.install_state)? else {
        return Ok(BridgeInspection::default());
    };
    let state: InstallState =
        serde_json::from_value(state_value).map_err(|_| BridgeInstallError::InvalidState)?;
    let Some(settings) = read_object(&config.claude_settings, false)? else {
        return Ok(BridgeInspection::default());
    };
    let ours = bridge_command(&config.installed_binary)?;
    if current_status_command(&settings.value) != Some(ours.as_str()) {
        return Ok(BridgeInspection::default());
    }
    let chained = read_object(&config.bridge_config, false)?
        .and_then(|config| {
            config
                .value
                .get("chainedCommand")
                .and_then(Value::as_str)
                .map(|_| true)
        })
        .unwrap_or(false);
    Ok(BridgeInspection {
        installed: true,
        chained,
        installed_at: Some(UnixSeconds(state.installed_at)),
    })
}

fn uninstall_sync(config: &BridgeInstallConfig) -> Result<UninstallOutcome, BridgeInstallError> {
    let Some(state_value) = read_value(&config.install_state)? else {
        return Ok(UninstallOutcome::NotInstalled);
    };
    let state: InstallState =
        serde_json::from_value(state_value).map_err(|_| BridgeInstallError::InvalidState)?;
    let settings =
        read_object(&config.claude_settings, true)?.ok_or(BridgeInstallError::InvalidState)?;
    let mut root = settings.value;
    let ours = bridge_command(&config.installed_binary)?;
    let current_command = root
        .get("statusLine")
        .and_then(Value::as_object)
        .and_then(|status| status.get("command"))
        .and_then(Value::as_str);
    if current_command != Some(ours.as_str()) {
        return Ok(UninstallOutcome::LeftUntouched);
    }

    let cleared_config = updated_bridge_config(&config.bridge_config, None)?;
    if state.previous_status_line_present {
        root.insert("statusLine".to_owned(), state.previous_status_line);
    } else {
        root.remove("statusLine");
    }
    atomic_write(
        &config.bridge_config,
        &encode_json(&cleared_config)?,
        Some(0o600),
    )?;
    atomic_write(
        &config.claude_settings,
        &encode_json(&Value::Object(root))?,
        settings.mode,
    )?;
    remove_if_exists(&config.install_state)?;
    Ok(UninstallOutcome::Restored)
}

fn updated_bridge_config(path: &Path, command: Option<&str>) -> Result<Value, BridgeInstallError> {
    let mut config = match read_object(path, true)? {
        Some(config) => config.value,
        None => Map::new(),
    };
    config.insert(
        "chainedCommand".to_owned(),
        command.map_or(Value::Null, |value| Value::String(value.to_owned())),
    );
    Ok(Value::Object(config))
}

fn current_status_command(settings: &Map<String, Value>) -> Option<&str> {
    settings
        .get("statusLine")
        .and_then(Value::as_object)
        .and_then(|status| status.get("command"))
        .and_then(Value::as_str)
}

fn bridge_command(path: &Path) -> Result<String, BridgeInstallError> {
    let value = path
        .to_str()
        .ok_or_else(|| BridgeInstallError::NonUtf8Path {
            path: path.to_path_buf(),
        })?;
    Ok(format!("'{}'", value.replace('\'', "'\"'\"'")))
}

fn backup_path(settings: &Path, installed_at: UnixSeconds) -> Result<PathBuf, BridgeInstallError> {
    let file_name = settings
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BridgeInstallError::InvalidPath {
            path: settings.to_path_buf(),
        })?;
    Ok(settings.with_file_name(format!("{file_name}.howisit-backup-{}", installed_at.0)))
}
