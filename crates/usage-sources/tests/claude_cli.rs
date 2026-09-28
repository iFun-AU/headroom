//! Real PTY fixtures verify cleanup, cancellation, concurrent clicks and cooldown.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use usage_core::parse::ClaudeCliError;
use usage_sources::claude::cli::{CliError, CliRefresh, ProbeConfig, capture};

fn fixture(body: &str) -> (TempDir, ProbeConfig) {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join("claude");
    fs::write(
        &binary,
        format!("#!/bin/sh\nprintf '%s' $$ > \"$CLAUDE_CONFIG_DIR/pid\"\n{body}\n"),
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let config = ProbeConfig {
        binary,
        claude_dir: Some(temp.path().to_owned()),
        working_directory: temp.path().join("probe"),
        timeout: Duration::from_secs(3),
    };
    (temp, config)
}

fn assert_reaped(config: &ProbeConfig) {
    let pid = fs::read_to_string(config.claude_dir.as_ref().unwrap().join("pid"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    assert!(
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err(),
        "probe process survived"
    );
}

#[test]
fn reconstructs_split_terminal_redraw_and_cleans_process_group() {
    let (_temp, config) = fixture(
        "sleep 60 &\nprintf '%s' $! > \"$CLAUDE_CONFIG_DIR/descendant\"\nprintf '\\033[2J\\033[HCurrent session\\r\\n10%% used\\r\\n'\nsleep 0.1\nprintf '\\033[2;1H45'\nsleep 60",
    );
    let reading = capture(&config, &CancellationToken::new()).unwrap();
    assert_eq!(reading.windows[0].used.get(), 45.0);
    assert_reaped(&config);
    let descendant =
        fs::read_to_string(config.claude_dir.as_ref().unwrap().join("descendant")).unwrap();
    // A killed orphan may briefly remain a zombie until launchd reaps it.
    let output = std::process::Command::new("/bin/ps")
        .args(["-o", "stat=", "-p", descendant.trim()])
        .output()
        .unwrap();
    let state = String::from_utf8_lossy(&output.stdout);
    assert!(
        state.trim().is_empty() || state.trim().starts_with('Z'),
        "descendant survived: {state}"
    );
}

#[test]
fn timeout_and_rate_limit_both_reap_child() {
    let (_temp, mut config) = fixture("sleep 60");
    // Allow the shell fixture to start under parallel test/build load.
    config.timeout = Duration::from_secs(1);
    assert_eq!(
        capture(&config, &CancellationToken::new()),
        Err(CliError::Timeout)
    );
    assert_reaped(&config);
    let (_temp, config) =
        fixture("printf 'Error: Usage endpoint is rate limited.\\r\\n'\nsleep 60");
    assert_eq!(
        capture(&config, &CancellationToken::new()),
        Err(CliError::Provider(ClaudeCliError::RateLimited))
    );
    assert_reaped(&config);
}

#[test]
fn refuses_to_auto_trust_nonempty_directory() {
    let (_temp, config) = fixture("sleep 60");
    fs::create_dir(&config.working_directory).unwrap();
    fs::write(config.working_directory.join("CLAUDE.md"), "unexpected").unwrap();
    assert_eq!(
        capture(&config, &CancellationToken::new()),
        Err(CliError::Setup)
    );
    assert!(!config.claude_dir.as_ref().unwrap().join("pid").exists());
}

#[tokio::test]
async fn concurrent_refresh_is_rejected_and_shutdown_cancels_capture() {
    let (_temp, config) = fixture("sleep 60");
    let (client, actor) = CliRefresh::channel();
    let cancel = CancellationToken::new();
    let worker = tokio::spawn(actor.run(cancel.clone()));
    let other = client.clone();
    let first_config = config.clone();
    let first = tokio::spawn(async move { other.refresh(first_config).await });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !config.claude_dir.as_ref().unwrap().join("pid").exists() {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(client.refresh(config.clone()).await, Err(CliError::Busy));
    cancel.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), first)
            .await
            .unwrap()
            .unwrap(),
        Err(CliError::Cancelled)
    );
    worker.await.unwrap();
    assert_reaped(&config);
}

#[tokio::test]
async fn rate_limit_cooldown_prevents_another_spawn() {
    let (_temp, config) =
        fixture("printf 'Error: Usage endpoint is rate limited.\\r\\n'\nsleep 60");
    let (client, actor) = CliRefresh::channel();
    let cancel = CancellationToken::new();
    let worker = tokio::spawn(actor.run(cancel.clone()));
    assert_eq!(
        client.refresh(config.clone()).await,
        Err(CliError::Provider(ClaudeCliError::RateLimited))
    );
    assert!(matches!(
        client.refresh(config).await,
        Err(CliError::Cooldown(299..=301))
    ));
    cancel.cancel();
    worker.await.unwrap();
}
