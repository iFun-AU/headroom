//! Versioned application settings with bounded migration and atomic persistence.

use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::NamedTempFile;
use thiserror::Error;
use ts_rs::TS;

mod actor;

pub use actor::{SettingsActor, SettingsActorError, SettingsHandle};

/// Current on-disk settings schema.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

const NEWER_SETTINGS_NOTICE: &str = "Settings were created by a newer version";
const MAX_CORRUPT_NAME_ATTEMPTS: u8 = 100;

/// Floating-widget layout variants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum WidgetVariant {
    /// Wide horizontal provider summary.
    #[default]
    Pill,
    /// Narrow vertical provider summary.
    Stack,
    /// Two inline bars with no expanded controls.
    Mini,
}

/// Menu-bar presentation of the weekly limits (decision D-026).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TrayStyle {
    /// Text title such as ` C 41% · X 78%` beside the template icon.
    #[default]
    Numbers,
    /// Two stacked colored bars drawn as the icon: Claude on top, Codex below.
    Bars,
}

/// Persisted floating-widget presentation and position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct WidgetSettings {
    /// Whether the widget should be restored on startup.
    pub visible: bool,
    /// Saved physical x coordinate, if the user has positioned the widget.
    pub x: Option<i32>,
    /// Saved physical y coordinate, if the user has positioned the widget.
    pub y: Option<i32>,
    /// Active widget layout.
    pub variant: WidgetVariant,
    /// Window opacity in the inclusive range 0.4 through 1.0.
    pub opacity: f32,
}

impl Default for WidgetSettings {
    fn default() -> Self {
        Self {
            visible: false,
            x: None,
            y: None,
            variant: WidgetVariant::Pill,
            opacity: 1.0,
        }
    }
}

/// Versioned application settings shared with the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
#[allow(clippy::struct_excessive_bools)] // The persisted v1 contract defines these independent toggles.
pub struct Settings {
    /// On-disk schema version.
    #[serde(alias = "schema_version")]
    pub schema_version: u32,
    /// Whether the three-step first-run flow has completed.
    #[serde(alias = "onboarding_completed")]
    pub onboarding_completed: bool,
    /// Optional Codex executable override.
    #[serde(alias = "codex_path")]
    pub codex_path: Option<String>,
    /// Optional Codex data-directory override.
    #[serde(alias = "codex_home")]
    pub codex_home: Option<String>,
    /// Optional Claude configuration-directory override.
    #[serde(alias = "claude_dir")]
    pub claude_dir: Option<String>,
    /// Whether the consent-based Claude status-line bridge is enabled.
    #[serde(alias = "claude_bridge_enabled")]
    pub claude_bridge_enabled: bool,
    /// Reserved post-v1 OAuth toggle; ignored without the feature.
    #[serde(alias = "claude_oauth_enabled")]
    pub claude_oauth_enabled: bool,
    /// Sorted, unique notification thresholds in 1 through 100.
    pub thresholds: Vec<u8>,
    /// Whether a provider reset should produce a notification.
    #[serde(alias = "notify_on_reset")]
    pub notify_on_reset: bool,
    /// Whether the app starts at login.
    #[serde(alias = "launch_at_login")]
    pub launch_at_login: bool,
    /// Whether the app appears in the Dock.
    #[serde(alias = "show_dock_icon")]
    pub show_dock_icon: bool,
    /// Whether the dashboard window floats above other windows.
    #[serde(alias = "main_always_on_top")]
    pub main_always_on_top: bool,
    /// Menu-bar presentation style.
    #[serde(alias = "tray_style")]
    pub tray_style: TrayStyle,
    /// Floating-widget settings.
    pub widget: WidgetSettings,
    /// Active polling interval, with a minimum of 60 seconds.
    #[serde(alias = "poll_active_secs")]
    pub poll_active_secs: u32,
    /// Idle polling interval, with a minimum of 120 seconds.
    #[serde(alias = "poll_idle_secs")]
    pub poll_idle_secs: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            onboarding_completed: false,
            codex_path: None,
            codex_home: None,
            claude_dir: None,
            claude_bridge_enabled: false,
            claude_oauth_enabled: false,
            thresholds: vec![75, 90, 100],
            notify_on_reset: true,
            launch_at_login: false,
            show_dock_icon: false,
            main_always_on_top: false,
            tray_style: TrayStyle::Numbers,
            widget: WidgetSettings::default(),
            poll_active_secs: 120,
            poll_idle_secs: 600,
        }
    }
}

/// Settings plus downgrade-safety metadata exposed to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsState {
    /// Effective in-memory settings.
    pub settings: Settings,
    /// Whether saves are disabled because the disk schema is newer.
    pub read_only: bool,
    /// Human-readable downgrade or recovery notice.
    pub notice: Option<String>,
    /// Whether this build includes the Claude OAuth usage source.
    pub claude_oauth_available: bool,
}

impl SettingsState {
    fn writable(settings: Settings) -> Self {
        Self {
            settings,
            read_only: false,
            notice: None,
            claude_oauth_available: cfg!(feature = "claude-oauth"),
        }
    }

    fn newer_schema() -> Self {
        Self {
            settings: Settings::default(),
            read_only: true,
            notice: Some(NEWER_SETTINGS_NOTICE.to_owned()),
            claude_oauth_available: cfg!(feature = "claude-oauth"),
        }
    }
}

/// Settings load, migration, or persistence failure.
#[derive(Debug, Error)]
pub enum SettingsError {
    /// Filesystem operation failed.
    #[error("settings filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    /// JSON could not be decoded or encoded.
    #[error("settings JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// Parsed JSON does not have a valid settings shape.
    #[error("settings document is invalid: {0}")]
    Invalid(&'static str),
    /// The settings were created by a newer application schema.
    #[error("settings schema {found} is newer than {CURRENT_SCHEMA_VERSION}")]
    NewerSchema {
        /// Version observed on disk.
        found: u64,
    },
}

/// Loads, migrates, validates, and rewrites settings at the current schema.
///
/// Missing or corrupt files become writable defaults. A newer schema remains
/// byte-for-byte untouched and returns read-only defaults until explicit reset.
///
/// # Errors
///
/// Returns an error when filesystem recovery or atomic persistence fails.
pub fn load_settings(path: &Path) -> Result<SettingsState, SettingsError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let settings = save_settings(path, Settings::default())?;
            return Ok(SettingsState::writable(settings));
        }
        Err(error) => return Err(error.into()),
    };

    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return recover_corrupt(path);
    };
    let settings = match migrate(value) {
        Ok(settings) => settings,
        Err(SettingsError::NewerSchema { .. }) => return Ok(SettingsState::newer_schema()),
        Err(_) => return recover_corrupt(path),
    };
    let settings = save_settings(path, settings)?;
    Ok(SettingsState::writable(settings))
}

/// Migrates a JSON settings value through every schema and validates v1.
///
/// # Errors
///
/// Returns [`SettingsError::NewerSchema`] for downgrade-safe handling, or a
/// typed validation/JSON error for malformed settings.
pub fn migrate(mut value: Value) -> Result<Settings, SettingsError> {
    let mut version = schema_version(&value)?;
    if version > u64::from(CURRENT_SCHEMA_VERSION) {
        return Err(SettingsError::NewerSchema { found: version });
    }
    while version < u64::from(CURRENT_SCHEMA_VERSION) {
        value = match version {
            0 => v0_to_v1(value)?,
            _ => return Err(SettingsError::Invalid("missing migration step")),
        };
        version += 1;
    }
    let settings = serde_json::from_value(value)?;
    Ok(validate(settings))
}

/// Validates and atomically writes settings at the current schema.
///
/// # Errors
///
/// Returns an error when the parent directory, serialization, sync, or atomic
/// replacement fails.
pub fn save_settings(path: &Path, settings: Settings) -> Result<Settings, SettingsError> {
    let settings = validate(settings);
    let parent = path
        .parent()
        .ok_or(SettingsError::Invalid("settings path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), &settings)?;
    temporary.write_all(b"\n")?;
    temporary.as_file_mut().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    File::open(parent)?.sync_all()?;
    Ok(settings)
}

/// Explicitly replaces any settings schema with current defaults.
///
/// # Errors
///
/// Returns an error when the atomic save fails.
pub fn reset_settings(path: &Path) -> Result<SettingsState, SettingsError> {
    let settings = save_settings(path, Settings::default())?;
    Ok(SettingsState::writable(settings))
}

fn schema_version(value: &Value) -> Result<u64, SettingsError> {
    let object = value
        .as_object()
        .ok_or(SettingsError::Invalid("top-level value must be an object"))?;
    let Some(version) = object
        .get("schemaVersion")
        .or_else(|| object.get("schema_version"))
    else {
        return Ok(0);
    };
    version
        .as_u64()
        .ok_or(SettingsError::Invalid("schema version must be unsigned"))
}

fn v0_to_v1(mut value: Value) -> Result<Value, SettingsError> {
    let object = value
        .as_object_mut()
        .ok_or(SettingsError::Invalid("top-level value must be an object"))?;
    object.remove("schema_version");
    object.insert(
        "schemaVersion".to_owned(),
        Value::from(CURRENT_SCHEMA_VERSION),
    );
    Ok(value)
}

fn validate(mut settings: Settings) -> Settings {
    settings.schema_version = CURRENT_SCHEMA_VERSION;
    for threshold in &mut settings.thresholds {
        *threshold = (*threshold).clamp(1, 100);
    }
    settings.thresholds.sort_unstable();
    settings.thresholds.dedup();
    settings.widget.opacity = if settings.widget.opacity.is_finite() {
        settings.widget.opacity.clamp(0.4, 1.0)
    } else {
        WidgetSettings::default().opacity
    };
    settings.poll_active_secs = settings.poll_active_secs.max(60);
    settings.poll_idle_secs = settings.poll_idle_secs.max(120);
    settings
}

fn recover_corrupt(path: &Path) -> Result<SettingsState, SettingsError> {
    let corrupt = next_corrupt_path(path)?;
    fs::rename(path, corrupt)?;
    let settings = save_settings(path, Settings::default())?;
    Ok(SettingsState::writable(settings))
}

fn next_corrupt_path(path: &Path) -> Result<PathBuf, SettingsError> {
    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    for suffix in 0..MAX_CORRUPT_NAME_ATTEMPTS {
        let extra = if suffix == 0 {
            String::new()
        } else {
            format!("-{suffix}")
        };
        let mut candidate: OsString = path.as_os_str().to_owned();
        candidate.push(format!(".corrupt-{unix}{extra}"));
        let candidate = PathBuf::from(candidate);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(SettingsError::Invalid(
        "too many colliding corrupt settings backups",
    ))
}
