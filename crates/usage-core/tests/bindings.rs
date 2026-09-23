//! TypeScript binding export contract tests.

use std::{error::Error, fs, path::Path};

use ts_rs::{Config, TS};
use usage_core::{History, UnixSeconds, UsageSnapshot};

#[test]
fn export_bindings_use_json_number_types() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env();
    UsageSnapshot::export_all(&config)?;
    History::export_all(&config)?;
    UnixSeconds::export_all(&config)?;
    normalize_generated_whitespace(config.out_dir())?;

    let binding = fs::read_to_string(config.out_dir().join("UnixSeconds.ts"))?;
    assert!(binding.contains("number"));
    assert!(!binding.contains("bigint"));
    Ok(())
}

fn normalize_generated_whitespace(directory: &Path) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("ts") {
            continue;
        }

        let source = fs::read_to_string(&path)?;
        let mut normalized = source
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        normalized.push('\n');
        if normalized != source {
            fs::write(path, normalized)?;
        }
    }
    Ok(())
}
