//! Explicit live acceptance probe. Prints only normalized limits or fixed safe errors.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::Duration;
use tokio_util::sync::CancellationToken;
use usage_sources::{
    claude::cli::{ProbeConfig, capture},
    codex::discover::{DiscoveryOptions, discover},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("--live") {
        return Err("Pass --live to run Claude Code /usage against your signed-in account".into());
    }
    let home = dirs::home_dir().ok_or("Home directory unavailable")?;
    let binary = discover(&DiscoveryOptions::claude(&home))
        .await?
        .ok_or("Claude Code not installed")?;
    let working = tempfile::tempdir()?;
    let config = ProbeConfig {
        binary,
        claude_dir: std::env::var_os("CLAUDE_CONFIG_DIR").map(std::path::PathBuf::from),
        working_directory: working.path().join("HeadroomUsageProbe"),
        timeout: Duration::from_secs(35),
    };
    let result =
        tokio::task::spawn_blocking(move || capture(&config, &CancellationToken::new())).await?;
    match result {
        Ok(reading) => {
            for window in reading.windows {
                println!(
                    "{:?}: {}% used; resets_at={:?}",
                    window.kind,
                    window.used.get(),
                    window.resets_at
                );
            }
        }
        Err(error) => {
            eprintln!("{error}");
            return Err("CLI usage probe did not produce limits".into());
        }
    }
    Ok(())
}
