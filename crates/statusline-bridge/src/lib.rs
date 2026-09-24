//! Synchronous Claude Code status-line bridge logic.

use std::{
    env,
    ffi::OsString,
    fmt,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use serde_json::Value;

mod chain;
mod storage;

pub use storage::cleanup_stale_temp_files;

/// Maximum accepted Claude status-line input size.
pub const MAX_INPUT_BYTES: usize = 4 * 1_024 * 1_024;

/// Bridge stdout that remains bounded in memory.
#[derive(Debug)]
pub enum BridgeOutput {
    /// Small built-in fallback text.
    Bytes(Vec<u8>),
    /// Chained-command stdout spooled to an anonymous temporary file.
    Spool(File),
}

impl BridgeOutput {
    /// Copies this output to its final destination.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the destination cannot accept all bytes or the
    /// chained-output spool cannot be read.
    pub fn write_to<W: Write>(&mut self, destination: &mut W) -> io::Result<()> {
        match self {
            Self::Bytes(bytes) => destination.write_all(bytes),
            Self::Spool(file) => io::copy(file, destination).map(|_| ()),
        }
    }
}

/// A bridge operation failed.
#[derive(Debug)]
pub enum Error {
    /// Command-line arguments did not match the supported shape.
    Arguments,
    /// The production output path could not be derived.
    MissingHome,
    /// Claude supplied more than the bounded input size.
    InputTooLarge,
    /// Input or configuration JSON was invalid.
    Json(serde_json::Error),
    /// A filesystem or stream operation failed.
    Io(io::Error),
    /// The system clock was earlier than the Unix epoch.
    Clock(SystemTimeError),
    /// An atomic temporary file could not replace the target.
    Persist(tempfile::PersistError),
    /// A bounded child-I/O worker terminated unexpectedly.
    WorkerThread,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments => formatter.write_str("invalid bridge arguments"),
            Self::MissingHome => formatter.write_str("home directory is unavailable"),
            Self::InputTooLarge => formatter.write_str("status-line input exceeds 4 MiB"),
            Self::Json(error) => write!(formatter, "invalid bridge JSON: {error}"),
            Self::Io(error) => write!(formatter, "bridge I/O failed: {error}"),
            Self::Clock(error) => write!(formatter, "system clock is invalid: {error}"),
            Self::Persist(error) => write!(formatter, "atomic bridge write failed: {error}"),
            Self::WorkerThread => formatter.write_str("bridge child I/O worker failed"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Clock(error) => Some(error),
            Self::Persist(error) => Some(error),
            Self::Arguments | Self::MissingHome | Self::InputTooLarge | Self::WorkerThread => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Runs one bridge invocation and returns bytes to write to stdout.
///
/// `args` excludes the executable name. Supplying `--out-dir` is mandatory in
/// tests and keeps all writes inside the caller's temporary directory.
///
/// # Errors
///
/// Returns an error for invalid arguments or input, missing path information,
/// clock failure, or filesystem failure. The binary intentionally suppresses
/// every error so Claude Code always observes a successful bridge process.
pub fn run<I, R>(args: I, input: R) -> Result<Option<BridgeOutput>, Error>
where
    I: IntoIterator<Item = OsString>,
    R: Read,
{
    let out_dir = output_dir(args)?;
    std::fs::create_dir_all(&out_dir)?;
    let now = SystemTime::now();
    cleanup_stale_temp_files(&out_dir, now)?;

    let input = read_bounded(input)?;
    let root: Value = serde_json::from_slice(&input)?;
    let rate_limits = root.get("rate_limits").filter(|value| has_window(value));
    if let Some(rate_limits) = rate_limits {
        let written_at = now
            .duration_since(UNIX_EPOCH)
            .map_err(Error::Clock)?
            .as_secs();
        let session_id = root
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        storage::write_bridge_file(&out_dir, written_at, session_id, rate_limits)?;
    }

    if let Some(command) = chained_command(&out_dir)? {
        return chain::run(&command, &input).map(Some);
    }

    Ok(rate_limits
        .and_then(default_output)
        .map(BridgeOutput::Bytes))
}

fn output_dir<I>(args: I) -> Result<PathBuf, Error>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    match args.next() {
        None => env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support/dev.headroom.app"))
            .ok_or(Error::MissingHome),
        Some(flag) if flag == "--out-dir" => {
            let path = args.next().filter(|value| !value.is_empty());
            if args.next().is_some() {
                return Err(Error::Arguments);
            }
            path.map(PathBuf::from).ok_or(Error::Arguments)
        }
        Some(_) => Err(Error::Arguments),
    }
}

fn read_bounded<R: Read>(input: R) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    input
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error::InputTooLarge);
    }
    Ok(bytes)
}

fn has_window(rate_limits: &Value) -> bool {
    ["five_hour", "seven_day"]
        .into_iter()
        .any(|key| rate_limits.get(key).is_some_and(Value::is_object))
}

fn chained_command(out_dir: &std::path::Path) -> Result<Option<String>, Error> {
    let bytes = match std::fs::read(out_dir.join("bridge.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::Io(error)),
    };
    let config: Value = serde_json::from_slice(&bytes)?;
    Ok(config
        .get("chainedCommand")
        .and_then(Value::as_str)
        .filter(|command| !command.is_empty())
        .map(str::to_owned))
}

fn default_output(rate_limits: &Value) -> Option<Vec<u8>> {
    let parts = [("5h", "five_hour"), ("7d", "seven_day")]
        .into_iter()
        .filter_map(|(label, key)| {
            rate_limits
                .get(key)?
                .get("used_percentage")?
                .as_f64()
                .map(|used| format!("{label} {}%", used.round()))
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return None;
    }

    Some(format!("{}\n", parts.join(" · ")).into_bytes())
}
