//! Explicit, temp-only instrumentation for the automated release soak.

#![cfg_attr(test, allow(clippy::expect_used))]

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use usage_sources::scheduler::Scheduler;

use crate::runtime::trigger_visible_refresh;

const ROOT_VARIABLE: &str = "HEADROOM_SOAK_ROOT";
const UI_INTERVAL_VARIABLE: &str = "HEADROOM_SOAK_UI_INTERVAL_MS";
const DEFAULT_UI_INTERVAL: Duration = Duration::from_secs(30);
const MIN_UI_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub(crate) struct SoakMode {
    root: PathBuf,
    ui_interval: Duration,
}

impl SoakMode {
    pub(crate) fn from_environment() -> io::Result<Option<Self>> {
        Self::from_values(
            env::var_os(ROOT_VARIABLE).map(PathBuf::from),
            env::var(UI_INTERVAL_VARIABLE).ok().as_deref(),
            &env::temp_dir(),
        )
    }

    fn from_values(
        root: Option<PathBuf>,
        interval_ms: Option<&str>,
        temporary_directory: &Path,
    ) -> io::Result<Option<Self>> {
        let Some(root) = root else {
            return Ok(None);
        };
        if !root.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{ROOT_VARIABLE} must be absolute"),
            ));
        }
        let root = fs::canonicalize(root)?;
        let temporary_directory = fs::canonicalize(temporary_directory)?;
        if root == temporary_directory || !root.starts_with(&temporary_directory) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{ROOT_VARIABLE} must be a child of the system temp directory"),
            ));
        }
        let ui_interval = interval_ms.map_or(Ok(DEFAULT_UI_INTERVAL), |value| {
            value
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{UI_INTERVAL_VARIABLE} must be milliseconds"),
                    )
                })
        })?;
        if ui_interval < MIN_UI_INTERVAL {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{UI_INTERVAL_VARIABLE} must be at least 100"),
            ));
        }
        Ok(Some(Self { root, ui_interval }))
    }

    pub(crate) fn platform_roots(&self) -> (PathBuf, PathBuf) {
        (self.root.join("home"), self.root.join("data"))
    }

    pub(crate) fn active_path(&self) -> PathBuf {
        self.root.join("active")
    }

    pub(crate) const fn ui_interval(&self) -> Duration {
        self.ui_interval
    }

    pub(crate) async fn run(self, scheduler: Scheduler, cancel: CancellationToken) {
        let active_path = self.active_path();
        let mut interval = tokio::time::interval(self.ui_interval());
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => return,
                _ = interval.tick() => {
                    match tokio::fs::metadata(&active_path).await {
                        Ok(_) => {
                            if let Err(error) = trigger_visible_refresh(&scheduler) {
                                warn!(%error, "automated soak ui_visible trigger failed");
                            } else {
                                info!("automated soak triggered ui_visible");
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => {
                            warn!(error = %usage_core::log_trunc(&error.to_string()), "automated soak active marker could not be read");
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io, path::Path};

    use tempfile::tempdir;

    use super::SoakMode;

    #[test]
    fn accepts_only_an_existing_absolute_root_below_the_temp_directory() {
        let temporary = tempdir().expect("temporary directory should exist");
        let root = temporary.path().join("soak");
        std::fs::create_dir(&root).expect("soak root should be created");

        let mode = SoakMode::from_values(Some(root.clone()), None, temporary.path())
            .expect("temp root should be valid")
            .expect("soak should be enabled");
        let root = std::fs::canonicalize(root).expect("root should canonicalize");
        assert_eq!(
            mode.platform_roots(),
            (root.join("home"), root.join("data"))
        );

        let outside =
            SoakMode::from_values(Some(Path::new("/").to_path_buf()), None, temporary.path());
        assert_eq!(
            outside.expect_err("outside root must fail").kind(),
            io::ErrorKind::InvalidInput
        );
        let relative = SoakMode::from_values(
            Some(Path::new("relative").to_path_buf()),
            None,
            temporary.path(),
        );
        assert_eq!(
            relative.expect_err("relative root must fail").kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn constrains_the_optional_smoke_interval() {
        let temporary = tempdir().expect("temporary directory should exist");
        let root = temporary.path().join("soak");
        std::fs::create_dir(&root).expect("soak root should be created");

        let mode = SoakMode::from_values(Some(root.clone()), Some("250"), temporary.path())
            .expect("interval should be valid")
            .expect("soak should be enabled");
        assert_eq!(mode.ui_interval(), std::time::Duration::from_millis(250));

        assert!(SoakMode::from_values(Some(root.clone()), Some("99"), temporary.path()).is_err());
        assert!(SoakMode::from_values(Some(root), Some("nope"), temporary.path()).is_err());
    }
}
