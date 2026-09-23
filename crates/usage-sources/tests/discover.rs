//! Codex executable discovery tests using synthetic paths and shell programs.

#![allow(clippy::expect_used)]

use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};

use tempfile::tempdir;
use usage_sources::codex::discover::{DiscoveryError, DiscoveryOptions, discover};

async fn write_executable(path: &Path, contents: &str) {
    tokio::fs::write(path, contents)
        .await
        .expect("fixture should be written");
    let mut permissions = tokio::fs::metadata(path)
        .await
        .expect("fixture metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    tokio::fs::set_permissions(path, permissions)
        .await
        .expect("fixture permissions should update");
}

#[tokio::test]
async fn settings_override_precedes_fixed_candidates_and_invalid_files_are_skipped() {
    let directory = tempdir().expect("tempdir should be created");
    let override_path = directory.path().join("override-codex");
    let first_fixed = directory.path().join("fixed-one");
    let second_fixed = directory.path().join("fixed-two");
    write_executable(&override_path, "#!/bin/sh\nexit 0\n").await;
    write_executable(&first_fixed, "#!/bin/sh\nexit 0\n").await;
    write_executable(&second_fixed, "#!/bin/sh\nexit 0\n").await;
    let options = DiscoveryOptions::testing(
        Some(override_path.clone()),
        vec![first_fixed.clone(), second_fixed],
        Path::new("/usr/bin/false").to_path_buf(),
        Duration::from_secs(1),
    );
    assert_eq!(
        discover(&options).await.expect("discovery should work"),
        Some(override_path)
    );

    let invalid_override = directory.path().join("not-executable");
    tokio::fs::write(&invalid_override, b"not executable")
        .await
        .expect("invalid fixture should be written");
    let options = DiscoveryOptions::testing(
        Some(invalid_override),
        vec![first_fixed.clone()],
        Path::new("/usr/bin/false").to_path_buf(),
        Duration::from_secs(1),
    );
    assert_eq!(
        discover(&options).await.expect("fallback should work"),
        Some(first_fixed)
    );
}

#[tokio::test]
async fn bounded_shell_fallback_returns_an_executable_and_times_out() {
    let directory = tempdir().expect("tempdir should be created");
    let discovered = directory.path().join("shell-codex");
    write_executable(&discovered, "#!/bin/sh\nexit 0\n").await;
    let shell = directory.path().join("shell");
    write_executable(
        &shell,
        &format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", discovered.display()),
    )
    .await;
    let options = DiscoveryOptions::testing(None, Vec::new(), shell, Duration::from_secs(1));
    assert_eq!(
        discover(&options)
            .await
            .expect("shell fallback should work"),
        Some(discovered)
    );

    let slow_shell = directory.path().join("slow-shell");
    write_executable(&slow_shell, "#!/bin/sh\nwhile :; do :; done\n").await;
    let options =
        DiscoveryOptions::testing(None, Vec::new(), slow_shell, Duration::from_millis(50));
    assert!(matches!(
        discover(&options).await,
        Err(DiscoveryError::Timeout)
    ));
}
