//! Existing status-line chaining and timeout tests.

#![allow(clippy::expect_used)]

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use serde_json::Value;
use tempfile::tempdir;

fn fixture() -> &'static [u8] {
    include_bytes!("../../usage-core/tests/fixtures/claude_statusline_input.json")
}

fn configure(out_dir: &Path, command: &str) {
    let config = serde_json::json!({ "chainedCommand": command });
    fs::write(
        out_dir.join("bridge.json"),
        serde_json::to_vec(&config).expect("config should serialize"),
    )
    .expect("config should be written");
}

fn run_bridge(out_dir: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_howisit-statusline"))
        .args([
            "--out-dir",
            out_dir.to_str().expect("temp path should be UTF-8"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bridge should spawn");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(fixture())
        .expect("fixture should be written");
    child.wait_with_output().expect("bridge should finish")
}

#[test]
fn chained_command_receives_stdin_and_replaces_default_output() {
    let out_dir = tempdir().expect("tempdir should be created");
    configure(out_dir.path(), "cat >/dev/null; echo CHAINED");

    let output = run_bridge(out_dir.path());

    assert!(output.status.success());
    assert_eq!(output.stdout, b"CHAINED\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn slow_chain_is_killed_after_bridge_file_is_persisted() {
    let out_dir = tempdir().expect("tempdir should be created");
    configure(out_dir.path(), "sleep 5 & wait");

    let started = Instant::now();
    let output = run_bridge(out_dir.path());
    let elapsed = started.elapsed();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(
        elapsed >= Duration::from_secs(2),
        "returned too early: {elapsed:?}"
    );
    assert!(
        elapsed <= Duration::from_millis(2_300),
        "timeout exceeded budget: {elapsed:?}"
    );
    let bridge_file = fs::read(out_dir.path().join("claude-rate-limits.json"))
        .expect("bridge file should precede chain execution");
    let value: Value = serde_json::from_slice(&bridge_file).expect("bridge file should be valid");
    assert_eq!(value["schema"], 1);
}
