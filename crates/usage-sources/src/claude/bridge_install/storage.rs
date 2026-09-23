//! Bounded JSON reads, atomic writes, and bridge-binary installation.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
};

use serde_json::{Map, Value};
use tempfile::NamedTempFile;

use super::BridgeInstallError;

const MAX_JSON_BYTES: u64 = 4 * 1_024 * 1_024;
const COPY_BUFFER_BYTES: usize = 64 * 1_024;

pub(super) struct JsonObjectFile {
    pub(super) value: Map<String, Value>,
    pub(super) original: Option<Vec<u8>>,
    pub(super) mode: Option<u32>,
}

pub(super) fn read_object(
    path: &Path,
    missing_is_empty: bool,
) -> Result<Option<JsonObjectFile>, BridgeInstallError> {
    let bytes = match read_bounded(path)? {
        Some(bytes) => bytes,
        None if missing_is_empty => {
            return Ok(Some(JsonObjectFile {
                value: Map::new(),
                original: None,
                mode: None,
            }));
        }
        None => return Ok(None),
    };
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|source| BridgeInstallError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })?;
    let Value::Object(value) = value else {
        return Err(BridgeInstallError::NonObject {
            path: path.to_path_buf(),
        });
    };
    let mode = fs::metadata(path)
        .map_err(|source| io_error("inspect permissions", path, source))?
        .permissions()
        .mode();
    Ok(Some(JsonObjectFile {
        value,
        original: Some(bytes),
        mode: Some(mode),
    }))
}

pub(super) fn read_value(path: &Path) -> Result<Option<Value>, BridgeInstallError> {
    let Some(bytes) = read_bounded(path)? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|source| BridgeInstallError::InvalidJson {
            path: path.to_path_buf(),
            source,
        })
}

pub(super) fn encode_json(value: &Value) -> Result<Vec<u8>, BridgeInstallError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn atomic_write(
    path: &Path,
    bytes: &[u8],
    mode: Option<u32>,
) -> Result<(), BridgeInstallError> {
    let parent = path
        .parent()
        .ok_or_else(|| BridgeInstallError::InvalidPath {
            path: path.to_path_buf(),
        })?;
    fs::create_dir_all(parent).map_err(|source| io_error("create directory", parent, source))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|source| io_error("create temporary file", parent, source))?;
    temporary
        .as_file_mut()
        .write_all(bytes)
        .map_err(|source| io_error("write temporary file", path, source))?;
    if let Some(mode) = mode {
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|source| io_error("set temporary permissions", path, source))?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| io_error("sync temporary file", path, source))?;
    temporary
        .persist(path)
        .map_err(|error| io_error("persist temporary file", path, error.error))?;
    sync_directory(parent)
}

pub(super) fn ensure_binary(source: &Path, target: &Path) -> Result<bool, BridgeInstallError> {
    let source_metadata =
        fs::metadata(source).map_err(|error| io_error("inspect bundled bridge", source, error))?;
    if !source_metadata.is_file() {
        return Err(BridgeInstallError::InvalidPath {
            path: source.to_path_buf(),
        });
    }
    if files_equal(source, target)? {
        set_executable(target)?;
        return Ok(false);
    }

    let parent = target
        .parent()
        .ok_or_else(|| BridgeInstallError::InvalidPath {
            path: target.to_path_buf(),
        })?;
    fs::create_dir_all(parent)
        .map_err(|error| io_error("create bridge bin directory", parent, error))?;
    let mut input = BufReader::with_capacity(
        COPY_BUFFER_BYTES,
        File::open(source).map_err(|error| io_error("open bundled bridge", source, error))?,
    );
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| io_error("create bridge temporary file", parent, error))?;
    {
        let mut output = BufWriter::with_capacity(COPY_BUFFER_BYTES, temporary.as_file_mut());
        io::copy(&mut input, &mut output)
            .map_err(|error| io_error("copy bridge binary", target, error))?;
        output
            .flush()
            .map_err(|error| io_error("flush bridge binary", target, error))?;
    }
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o755))
        .map_err(|error| io_error("set bridge permissions", target, error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io_error("sync bridge binary", target, error))?;
    temporary
        .persist(target)
        .map_err(|error| io_error("persist bridge binary", target, error.error))?;
    sync_directory(parent)?;
    Ok(true)
}

pub(super) fn remove_if_exists(path: &Path) -> Result<(), BridgeInstallError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("remove file", path, error)),
    }
}

fn read_bounded(path: &Path) -> Result<Option<Vec<u8>>, BridgeInstallError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("open JSON file", path, error)),
    };
    let size = file
        .metadata()
        .map_err(|error| io_error("inspect JSON file", path, error))?
        .len();
    if size > MAX_JSON_BYTES {
        return Err(BridgeInstallError::FileTooLarge {
            path: path.to_path_buf(),
        });
    }
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    file.take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error("read JSON file", path, error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_JSON_BYTES {
        return Err(BridgeInstallError::FileTooLarge {
            path: path.to_path_buf(),
        });
    }
    Ok(Some(bytes))
}

fn files_equal(left_path: &Path, right_path: &Path) -> Result<bool, BridgeInstallError> {
    let right_metadata = match fs::metadata(right_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error("inspect installed bridge", right_path, error)),
    };
    let left_metadata = fs::metadata(left_path)
        .map_err(|error| io_error("inspect bundled bridge", left_path, error))?;
    if !right_metadata.is_file() || left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }
    let mut left_reader = BufReader::with_capacity(
        COPY_BUFFER_BYTES,
        File::open(left_path).map_err(|error| io_error("open bundled bridge", left_path, error))?,
    );
    let mut right_reader = BufReader::with_capacity(
        COPY_BUFFER_BYTES,
        File::open(right_path)
            .map_err(|error| io_error("open installed bridge", right_path, error))?,
    );
    let mut left_buffer = vec![0_u8; COPY_BUFFER_BYTES].into_boxed_slice();
    let mut right_buffer = vec![0_u8; COPY_BUFFER_BYTES].into_boxed_slice();
    loop {
        let left_read = left_reader
            .read(&mut left_buffer)
            .map_err(|error| io_error("compare bundled bridge", left_path, error))?;
        let right_read = right_reader
            .read(&mut right_buffer)
            .map_err(|error| io_error("compare installed bridge", right_path, error))?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn set_executable(path: &Path) -> Result<(), BridgeInstallError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|error| io_error("set bridge permissions", path, error))
}

fn sync_directory(path: &Path) -> Result<(), BridgeInstallError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error("sync directory", path, error))
}

pub(super) fn create_backup(
    path: &Path,
    bytes: &[u8],
    mode: Option<u32>,
) -> Result<(), BridgeInstallError> {
    let parent = path
        .parent()
        .ok_or_else(|| BridgeInstallError::InvalidPath {
            path: path.to_path_buf(),
        })?;
    fs::create_dir_all(parent)
        .map_err(|error| io_error("create backup directory", parent, error))?;
    if path.exists() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error("create settings backup", path, error))?;
    file.write_all(bytes)
        .map_err(|error| io_error("write settings backup", path, error))?;
    if let Some(mode) = mode {
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| io_error("set backup permissions", path, error))?;
    }
    file.sync_all()
        .map_err(|error| io_error("sync settings backup", path, error))?;
    sync_directory(parent)
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> BridgeInstallError {
    BridgeInstallError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
