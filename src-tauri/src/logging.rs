//! Daily application log initialization with bounded retention.

use std::{fs, io, path::Path};

use thiserror::Error;
use tracing::Level;
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::fmt;

/// Logging setup failure before application actors start.
#[derive(Debug, Error)]
pub enum LoggingError {
    /// The log directory could not be created.
    #[error("could not create application log directory: {0}")]
    Directory(#[from] io::Error),
    /// The daily rolling writer could not be created.
    #[error("could not create daily log writer: {0}")]
    Writer(#[from] tracing_appender::rolling::InitError),
    /// Another global tracing subscriber was already installed.
    #[error("could not install application log subscriber: {0}")]
    Subscriber(String),
}

/// Owns the non-blocking writer guard so buffered records flush through shutdown.
pub struct LogGuard(#[allow(dead_code)] WorkerGuard);

/// Installs an info-level, non-ANSI daily file subscriber retaining seven files.
///
/// # Errors
///
/// Returns an error if the directory/writer cannot be created or tracing is already initialized.
pub fn init(log_directory: &Path) -> Result<LogGuard, LoggingError> {
    fs::create_dir_all(log_directory)?;
    let writer = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("how-is-it")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_directory)?;
    let (writer, guard) = tracing_appender::non_blocking(writer);
    fmt()
        .with_ansi(false)
        .with_max_level(Level::INFO)
        .with_writer(writer)
        .try_init()
        .map_err(|error| LoggingError::Subscriber(error.to_string()))?;
    Ok(LogGuard(guard))
}
