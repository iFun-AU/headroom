use std::{
    env,
    path::{Path, PathBuf},
};

use thiserror::Error;

const APP_IDENTIFIER: &str = "dev.howisit.app";

/// Settings-controlled path overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathOverrides {
    /// Explicit Codex home directory.
    pub codex_home: Option<PathBuf>,
    /// Explicit Claude configuration directory.
    pub claude_dir: Option<PathBuf>,
}

/// Process-environment path overrides, separated for deterministic tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathEnvironment {
    /// `CODEX_HOME`, when inherited by the process.
    pub codex_home: Option<PathBuf>,
    /// `CLAUDE_CONFIG_DIR`, when inherited by the process.
    pub claude_dir: Option<PathBuf>,
}

/// Every filesystem location used by sources and shell integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Effective Codex configuration root.
    pub codex_home: PathBuf,
    /// Codex rollout-session tree.
    pub codex_sessions: PathBuf,
    /// Effective Claude configuration root.
    pub claude_dir: PathBuf,
    /// Claude local-conversation tree.
    pub claude_projects: PathBuf,
    /// Claude user settings file.
    pub claude_settings: PathBuf,
    /// Application Support root.
    pub app_support: PathBuf,
    /// Claude bridge rate-limit file.
    pub bridge_rate_limits: PathBuf,
    /// Claude bridge chaining configuration.
    pub bridge_config: PathBuf,
    /// Claude bridge installer state.
    pub bridge_install_state: PathBuf,
    /// Stable installed bridge executable.
    pub bridge_binary: PathBuf,
    /// Application log directory.
    pub logs: PathBuf,
}

/// A required platform directory could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Error {
    /// The user's home directory is unavailable.
    #[error("home directory is unavailable")]
    MissingHome,
    /// The platform Application Support directory is unavailable.
    #[error("application data directory is unavailable")]
    MissingData,
}

impl Paths {
    /// Resolves paths from settings, inherited environment, and platform roots.
    ///
    /// Settings override environment values, which override default dotfolders.
    ///
    /// # Errors
    ///
    /// Returns an error if the home or platform data directory is unavailable.
    pub fn discover(overrides: PathOverrides) -> Result<Self, Error> {
        let home = dirs::home_dir().ok_or(Error::MissingHome)?;
        let data = dirs::data_dir().ok_or(Error::MissingData)?;
        let environment = PathEnvironment {
            codex_home: env::var_os("CODEX_HOME").map(PathBuf::from),
            claude_dir: env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
        };
        Ok(Self::resolve(&home, &data, overrides, environment))
    }

    /// Resolves paths from explicit roots without consulting the real machine.
    #[must_use]
    pub fn resolve(
        home: &Path,
        data: &Path,
        overrides: PathOverrides,
        environment: PathEnvironment,
    ) -> Self {
        let codex_home = overrides
            .codex_home
            .or(environment.codex_home)
            .unwrap_or_else(|| home.join(".codex"));
        let claude_dir = overrides
            .claude_dir
            .or(environment.claude_dir)
            .unwrap_or_else(|| home.join(".claude"));
        let app_support = data.join(APP_IDENTIFIER);

        Self {
            codex_sessions: codex_home.join("sessions"),
            claude_projects: claude_dir.join("projects"),
            claude_settings: claude_dir.join("settings.json"),
            bridge_rate_limits: app_support.join("claude-rate-limits.json"),
            bridge_config: app_support.join("bridge.json"),
            bridge_install_state: app_support.join("bridge-install.json"),
            bridge_binary: app_support.join("bin/howisit-statusline"),
            logs: home.join("Library/Logs").join(APP_IDENTIFIER),
            codex_home,
            claude_dir,
            app_support,
        }
    }
}
