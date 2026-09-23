//! Codex executable discovery for GUI processes without a reliable shell PATH.

use std::{io, path::PathBuf, process::Stdio, time::Duration};

use thiserror::Error;
use tokio::{io::AsyncRead, io::AsyncReadExt, process::Command};

const SHELL_OUTPUT_CAPACITY: usize = 4 * 1_024;

/// Ordered discovery inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryOptions {
    override_path: Option<PathBuf>,
    candidates: Vec<PathBuf>,
    shell_program: PathBuf,
    shell_timeout: Duration,
}

impl DiscoveryOptions {
    /// Builds the documented production search order.
    #[must_use]
    pub fn standard(override_path: Option<PathBuf>, home: &std::path::Path) -> Self {
        Self {
            override_path,
            candidates: vec![
                PathBuf::from("/opt/homebrew/bin/codex"),
                PathBuf::from("/usr/local/bin/codex"),
                home.join(".local/bin/codex"),
                home.join(".npm-global/bin/codex"),
            ],
            shell_program: PathBuf::from("/bin/zsh"),
            shell_timeout: Duration::from_secs(3),
        }
    }

    /// Builds deterministic inputs for tests without consulting real paths.
    #[doc(hidden)]
    #[must_use]
    pub fn testing(
        override_path: Option<PathBuf>,
        candidates: Vec<PathBuf>,
        shell_program: PathBuf,
        shell_timeout: Duration,
    ) -> Self {
        Self {
            override_path,
            candidates,
            shell_program,
            shell_timeout,
        }
    }
}

/// Executable discovery failure.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// Filesystem inspection or process control failed.
    #[error("Codex discovery {operation} failed: {source}")]
    Io {
        /// Operation being attempted.
        operation: &'static str,
        /// Underlying operating-system error.
        #[source]
        source: io::Error,
    },
    /// The shell lookup exceeded its three-second production bound.
    #[error("Codex shell discovery timed out")]
    Timeout,
    /// The shell process did not expose a stdout pipe.
    #[error("Codex shell discovery stdout was unavailable")]
    MissingStdout,
}

/// Finds the first executable in settings, fixed candidates, then shell lookup.
///
/// # Errors
///
/// Returns a typed error if candidate metadata or the bounded shell lookup fails.
pub async fn discover(options: &DiscoveryOptions) -> Result<Option<PathBuf>, DiscoveryError> {
    if let Some(path) = &options.override_path
        && is_executable(path).await?
    {
        return Ok(Some(path.clone()));
    }
    for path in &options.candidates {
        if is_executable(path).await? {
            return Ok(Some(path.clone()));
        }
    }

    let Some(path) = shell_lookup(options).await? else {
        return Ok(None);
    };
    let executable = is_executable(&path).await?;
    Ok(executable.then_some(path))
}

async fn is_executable(path: &std::path::Path) -> Result<bool, DiscoveryError> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(DiscoveryError::Io {
                operation: "candidate inspection",
                source,
            });
        }
    };
    Ok(metadata.is_file() && executable_permissions(&metadata))
}

async fn shell_lookup(options: &DiscoveryOptions) -> Result<Option<PathBuf>, DiscoveryError> {
    let mut child = Command::new(&options.shell_program)
        .args(["-lc", "command -v codex"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| DiscoveryError::Io {
            operation: "shell spawn",
            source,
        })?;
    let stdout = child.stdout.take().ok_or(DiscoveryError::MissingStdout)?;
    let completed = tokio::time::timeout(options.shell_timeout, async {
        tokio::try_join!(child.wait(), read_bounded(stdout))
    })
    .await;
    let (status, output) = if let Ok(result) = completed {
        result.map_err(|source| DiscoveryError::Io {
            operation: "shell execution",
            source,
        })?
    } else {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(DiscoveryError::Timeout);
    };
    if !status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8_lossy(&output)
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from))
}

async fn read_bounded<R>(mut reader: R) -> io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut retained = Vec::with_capacity(SHELL_OUTPUT_CAPACITY);
    let mut chunk = [0_u8; 512];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(retained);
        }
        let remaining = SHELL_OUTPUT_CAPACITY.saturating_sub(retained.len());
        retained.extend_from_slice(&chunk[..read.min(remaining)]);
    }
}

#[cfg(unix)]
fn executable_permissions(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable_permissions(_metadata: &std::fs::Metadata) -> bool {
    true
}
