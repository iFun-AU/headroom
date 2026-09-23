use std::{
    fs,
    io::Write,
    path::Path,
    time::{Duration, SystemTime},
};

use serde_json::{Map, Number, Value};
use tempfile::NamedTempFile;

use crate::Error;

const STALE_TEMP_AGE: Duration = Duration::from_hours(1);
const TARGET_FILE: &str = "claude-rate-limits.json";

/// Removes abandoned bridge temporary files at least one hour old.
///
/// Only regular files whose names start with `.tmp` are eligible. The supplied
/// time makes startup cleanup deterministic in overlap and crash tests.
///
/// # Errors
///
/// Returns an error when the directory cannot be listed, metadata cannot be
/// read, or an eligible file cannot be removed.
pub fn cleanup_stale_temp_files(out_dir: &Path, now: SystemTime) -> Result<usize, Error> {
    let mut removed = 0;
    for entry in fs::read_dir(out_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() || !entry.file_name().to_string_lossy().starts_with(".tmp")
        {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        if now
            .duration_since(modified)
            .is_ok_and(|age| age >= STALE_TEMP_AGE)
        {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

pub(crate) fn write_bridge_file(
    out_dir: &Path,
    written_at: u64,
    session_id: &str,
    rate_limits: &Value,
) -> Result<(), Error> {
    let mut envelope = Map::new();
    envelope.insert("schema".to_owned(), Value::Number(Number::from(1)));
    envelope.insert(
        "writtenAt".to_owned(),
        Value::Number(Number::from(written_at)),
    );
    envelope.insert("sessionId".to_owned(), Value::String(session_id.to_owned()));
    envelope.insert("rateLimits".to_owned(), rate_limits.clone());

    let mut temporary = NamedTempFile::new_in(out_dir)?;
    serde_json::to_writer(temporary.as_file_mut(), &Value::Object(envelope))?;
    temporary.as_file_mut().write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(out_dir.join(TARGET_FILE))
        .map_err(Error::Persist)?;
    Ok(())
}
