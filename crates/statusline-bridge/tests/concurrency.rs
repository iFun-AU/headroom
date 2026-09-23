//! Atomic overlap, interruption, and abandoned-temp cleanup tests.

#![allow(clippy::expect_used)]

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use statusline_bridge::cleanup_stale_temp_files;
use tempfile::tempdir;

const INVOCATIONS: usize = 50;
const KILL_EVERY: usize = 5;

fn fixture() -> &'static [u8] {
    include_bytes!("../../usage-core/tests/fixtures/claude_statusline_input.json")
}

fn temp_files(out_dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(out_dir)
        .expect("tempdir should be readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".tmp"))
        .map(|entry| entry.path())
        .collect()
}

fn varied_delay(index: usize, seed: u64) -> Duration {
    let mut value = seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    Duration::from_millis(value.wrapping_mul(0x2545_F491_4F6C_DD1D) % 11)
}

#[test]
fn overlapping_and_killed_writers_leave_valid_atomic_state() {
    let out_dir = tempdir().expect("tempdir should be created");
    let barrier = Arc::new(Barrier::new(INVOCATIONS));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should follow Unix epoch");
    let seed = now.as_secs() ^ u64::from(now.subsec_nanos());
    let workers = (0..INVOCATIONS)
        .map(|index| {
            let barrier = Arc::clone(&barrier);
            let out_dir = out_dir.path().to_owned();
            thread::spawn(move || invoke_bridge(index, seed, &out_dir, &barrier))
        })
        .collect::<Vec<_>>();

    for worker in workers {
        worker.join().expect("invocation worker should not panic");
    }

    let target = fs::read(out_dir.path().join("claude-rate-limits.json"))
        .expect("at least one complete writer should persist the target");
    let envelope: Value = serde_json::from_slice(&target).expect("target should be complete JSON");
    assert_eq!(envelope["schema"], 1);
    assert_eq!(envelope["rateLimits"]["five_hour"]["used_percentage"], 23.5);

    let abandoned = temp_files(out_dir.path());
    assert!(abandoned.len() <= INVOCATIONS / KILL_EVERY);
    fs::write(out_dir.path().join(".tmp-abandoned-test"), b"partial")
        .expect("synthetic abandoned file should be created");
    let future = SystemTime::now() + Duration::from_hours(2);
    let before_cleanup = temp_files(out_dir.path()).len();
    let removed = cleanup_stale_temp_files(out_dir.path(), future)
        .expect("startup cleanup should remove stale temp files");
    assert_eq!(removed, before_cleanup);
    assert!(temp_files(out_dir.path()).is_empty());
}

fn invoke_bridge(index: usize, seed: u64, out_dir: &Path, barrier: &Barrier) {
    barrier.wait();
    let mut child = Command::new(env!("CARGO_BIN_EXE_howisit-statusline"))
        .args([
            "--out-dir",
            out_dir.to_str().expect("temp path should be UTF-8"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("bridge should spawn");
    let write_result = child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(fixture());

    if index.is_multiple_of(KILL_EVERY) {
        thread::sleep(varied_delay(index, seed));
        let _ = child.kill();
    } else {
        write_result.expect("surviving bridge should receive its fixture");
    }
    child.wait().expect("bridge should be reaped");
}
