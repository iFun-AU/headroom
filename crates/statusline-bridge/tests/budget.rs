//! Process-level latency budget for the unchained bridge fast path.

#![allow(clippy::expect_used)]

use std::{
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use tempfile::tempdir;

const RUNS: usize = 100;
const RELEASE_P95_BUDGET: Duration = Duration::from_millis(20);
const DEBUG_P95_BUDGET: Duration = Duration::from_millis(100);

fn fixture() -> &'static [u8] {
    include_bytes!("../../usage-core/tests/fixtures/claude_statusline_input.json")
}

#[test]
fn sequential_unchained_process_p95_meets_budget() {
    let out_dir = tempdir().expect("tempdir should be created");
    let mut durations = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let mut child = Command::new(env!("CARGO_BIN_EXE_howisit-statusline"))
                .args([
                    "--out-dir",
                    out_dir.path().to_str().expect("temp path should be UTF-8"),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("bridge should spawn");
            child
                .stdin
                .take()
                .expect("stdin should be piped")
                .write_all(fixture())
                .expect("fixture should be written");
            assert!(child.wait().expect("bridge should finish").success());
            started.elapsed()
        })
        .collect::<Vec<_>>();
    durations.sort_unstable();

    let p95 = durations[(RUNS * 95).div_ceil(100) - 1];
    let budget = if cfg!(debug_assertions) {
        DEBUG_P95_BUDGET
    } else {
        RELEASE_P95_BUDGET
    };
    assert!(p95 < budget, "p95 {p95:?} exceeded {budget:?}");
}
