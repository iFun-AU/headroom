//! PTY ownership. Nonblocking descriptors keep every read and shutdown bounded.

use nix::{
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use usage_core::Reading;

use super::{CliError, ProbeConfig, screen::CaptureScreen};

struct OwnedChild(Box<dyn Child + Send + Sync>);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        // portable-pty establishes a new session whose process group is the child
        // PID. Signal only this owned group, before wait can release its PID.
        if let Some(pid) = self.0.process_id().and_then(|pid| i32::try_from(pid).ok()) {
            let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts a fresh Claude `/usage` session, captures a stable panel and reaps it.
/// Intended for `spawn_blocking`; cancellation is checked at least every 25 ms.
///
/// # Errors
/// Returns fixed diagnostics only. No terminal text is logged or persisted.
pub fn capture(config: &ProbeConfig, cancel: &CancellationToken) -> Result<Reading, CliError> {
    if cancel.is_cancelled() {
        return Err(CliError::Cancelled);
    }
    prepare_directory(&config.working_directory)?;
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 60,
            cols: 160,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|_| CliError::Process)?;
    let fd = pair.master.as_raw_fd().ok_or(CliError::Process)?;
    let flags = fcntl(fd, FcntlArg::F_GETFL).map_err(|_| CliError::Process)?;
    fcntl(
        fd,
        FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
    )
    .map_err(|_| CliError::Process)?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|_| CliError::Process)?;
    let mut writer = pair.master.take_writer().map_err(|_| CliError::Process)?;
    let _child = OwnedChild(
        pair.slave
            .spawn_command(command(config))
            .map_err(|_| CliError::Process)?,
    );
    drop(pair.slave);
    read_panel(&mut reader, &mut writer, config.timeout, cancel)
}

fn command(config: &ProbeConfig) -> CommandBuilder {
    let mut command = CommandBuilder::new(&config.binary);
    command.args([
        "--safe-mode",
        "--tools",
        "",
        "--strict-mcp-config",
        "--setting-sources",
        "user",
        "--settings",
        r#"{"disableAllHooks":true,"remoteControlAtStartup":false}"#,
        "/usage",
    ]);
    command.cwd(&config.working_directory);
    if let Some(directory) = &config.claude_dir {
        command.env("CLAUDE_CONFIG_DIR", directory);
    } else {
        command.env_remove("CLAUDE_CONFIG_DIR");
    }
    command.env("TERM", "xterm-256color");
    command.env("TZ", "UTC");
    command.env("LC_ALL", "en_US.UTF-8");
    command.env("DISABLE_AUTOUPDATER", "1");
    command
}

fn prepare_directory(directory: &Path) -> Result<(), CliError> {
    std::fs::create_dir_all(directory).map_err(|_| CliError::Process)?;
    // Never auto-trust arbitrary project content at a path that has been replaced.
    let metadata = std::fs::symlink_metadata(directory).map_err(|_| CliError::Process)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || std::fs::read_dir(directory)
            .map_err(|_| CliError::Process)?
            .next()
            .is_some()
    {
        return Err(CliError::Setup);
    }
    Ok(())
}

fn read_panel(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<Reading, CliError> {
    let mut screen = CaptureScreen::new();
    let deadline = Instant::now() + timeout;
    let mut last_change = Instant::now();
    let mut bytes = [0_u8; 8192];
    loop {
        if cancel.is_cancelled() {
            return Err(CliError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(CliError::Timeout);
        }
        match reader.read(&mut bytes) {
            Ok(0) => return screen.reading()?.ok_or(CliError::Process),
            Ok(count) => {
                if screen.feed(&bytes[..count], writer)? {
                    last_change = Instant::now();
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if last_change.elapsed() >= Duration::from_millis(150) {
                    screen.respond_to_trust(writer)?;
                }
                if last_change.elapsed() >= Duration::from_millis(750)
                    && let Some(reading) = screen.reading()?
                {
                    return Ok(reading);
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            // PTYs may report EIO instead of EOF when their slave exits.
            Err(_) => return screen.reading()?.ok_or(CliError::Process),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn default_auth_profile_is_not_replaced_by_an_explicit_directory() {
        let mut config = ProbeConfig {
            binary: PathBuf::from("/example/claude"),
            claude_dir: None,
            working_directory: PathBuf::from("/example/empty"),
            timeout: Duration::from_secs(35),
        };
        assert!(command(&config).get_env("CLAUDE_CONFIG_DIR").is_none());
        config.claude_dir = Some(PathBuf::from("/example/other-profile"));
        assert_eq!(
            command(&config).get_env("CLAUDE_CONFIG_DIR"),
            Some(std::ffi::OsStr::new("/example/other-profile"))
        );
    }
}
