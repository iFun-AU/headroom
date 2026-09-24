//! Binary-level tests for the synchronous status-line bridge fast path.

#![allow(clippy::expect_used)]

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use serde_json::Value;
use tempfile::tempdir;

const MAX_INPUT_BYTES: usize = 4 * 1_024 * 1_024;

fn fixture() -> &'static [u8] {
    include_bytes!("../../usage-core/tests/fixtures/claude_statusline_input.json")
}

fn run_bridge(out_dir: &Path, input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_headroom-statusline"))
        .args([
            "--out-dir",
            out_dir.to_str().expect("temp path should be UTF-8"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bridge should spawn");
    let write_result = child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input);
    if let Err(error) = write_result {
        assert!(
            input.len() > MAX_INPUT_BYTES && error.kind() == std::io::ErrorKind::BrokenPipe,
            "bounded reader should accept input or close an oversized pipe: {error}"
        );
    }
    child.wait_with_output().expect("bridge should finish")
}

#[test]
fn documented_input_writes_bridge_file_and_default_text() {
    let out_dir = tempdir().expect("tempdir should be created");
    let output = run_bridge(out_dir.path(), fixture());

    assert!(output.status.success());
    assert_eq!(output.stdout, b"5h 24% \xC2\xB7 7d 41%\n");
    assert!(output.stderr.is_empty());

    let written =
        fs::read(out_dir.path().join("claude-rate-limits.json")).expect("bridge file should exist");
    let envelope: Value = serde_json::from_slice(&written).expect("bridge file should be JSON");
    let input: Value = serde_json::from_slice(fixture()).expect("fixture should be JSON");
    assert_eq!(envelope["schema"], 1);
    assert!(envelope["writtenAt"].is_i64() || envelope["writtenAt"].is_u64());
    assert_eq!(envelope["sessionId"], "abc123");
    assert_eq!(envelope["rateLimits"], input["rate_limits"]);
}

#[test]
fn invalid_or_oversized_input_is_silent_and_successful() {
    let oversized = vec![b' '; MAX_INPUT_BYTES + 1];
    let non_window = br#"{"rate_limits":{"five_hour":true}}"#;
    for input in [&b"garbage"[..], non_window, oversized.as_slice()] {
        let out_dir = tempdir().expect("tempdir should be created");
        let output = run_bridge(out_dir.path(), input);

        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        assert!(!out_dir.path().join("claude-rate-limits.json").exists());
    }
}
