//! Semantic Claude bridge installer tests in isolated directories.

#![allow(clippy::expect_used)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use serde_json::{Value, json};
use tempfile::TempDir;
use usage_core::UnixSeconds;
use usage_sources::claude::bridge_install::{
    BridgeInstallConfig, BridgeInstallError, BridgeInstaller, UninstallOutcome,
};

const INSTALLED_AT: UnixSeconds = UnixSeconds(1_790_132_400);

struct Fixture {
    _directory: TempDir,
    config: BridgeInstallConfig,
    installer: BridgeInstaller,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("tempdir should be created");
        let bundled = directory.path().join("bundle/howisit-statusline");
        fs::create_dir_all(bundled.parent().expect("bundle path should have a parent"))
            .expect("bundle directory should be created");
        fs::write(&bundled, b"bridge-v1").expect("bundled bridge should be written");
        let app_support = directory.path().join("app-support");
        let config = BridgeInstallConfig::new(
            bundled,
            app_support.join("bin/howisit-statusline"),
            directory.path().join("claude/settings.json"),
            app_support.join("bridge-install.json"),
            app_support.join("bridge.json"),
        );
        let installer = BridgeInstaller::new(config.clone());
        Self {
            _directory: directory,
            config,
            installer,
        }
    }

    fn write_settings(&self, bytes: &[u8]) {
        fs::create_dir_all(
            self.config
                .claude_settings
                .parent()
                .expect("settings path should have a parent"),
        )
        .expect("settings directory should be created");
        fs::write(&self.config.claude_settings, bytes).expect("settings should be written");
    }

    fn read_settings(&self) -> Value {
        read_json(&self.config.claude_settings)
    }

    fn command(&self) -> String {
        format!("'{}'", self.config.installed_binary.display())
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("JSON file should be readable"))
        .expect("JSON file should be valid")
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .expect("file metadata should exist")
        .permissions()
        .mode()
        & 0o777
}

#[tokio::test]
async fn install_into_missing_settings_creates_command_state_backup_and_executable() {
    let fixture = Fixture::new();
    let receipt = fixture
        .installer
        .install_at(INSTALLED_AT)
        .await
        .expect("installation should succeed");

    assert_eq!(
        fixture.read_settings(),
        json!({ "statusLine": { "type": "command", "command": fixture.command() } })
    );
    assert_eq!(
        fs::read(&fixture.config.installed_binary).expect("binary should exist"),
        b"bridge-v1"
    );
    assert_eq!(mode(&fixture.config.installed_binary), 0o755);
    assert_eq!(
        read_json(&fixture.config.install_state),
        json!({
            "previousStatusLine": null,
            "previousStatusLinePresent": false,
            "installedAt": INSTALLED_AT.0
        })
    );
    assert_eq!(read_json(&receipt.backup_path), json!({}));
    assert_eq!(mode(&fixture.config.install_state), 0o600);
    assert_eq!(mode(&fixture.config.bridge_config), 0o600);
    assert!(receipt.binary_updated);
    assert!(!receipt.chained_previous_command);

    assert!(
        !fixture
            .installer
            .refresh_binary()
            .await
            .expect("refresh should work")
    );
    fs::write(&fixture.config.bundled_binary, b"bridge-v2").expect("bundle should be updated");
    assert!(
        fixture
            .installer
            .refresh_binary()
            .await
            .expect("refresh should work")
    );
    assert_eq!(
        fs::read(&fixture.config.installed_binary).expect("binary should exist"),
        b"bridge-v2"
    );
}

#[tokio::test]
async fn existing_status_object_preserves_future_keys_and_chains_previous_command() {
    let fixture = Fixture::new();
    let original = json!({
        "statusLine": {
            "type": "command",
            "command": "x.sh",
            "padding": 2,
            "refreshInterval": 5,
            "hideVimModeIndicator": true,
            "futureKey": { "a": 1 }
        }
    });
    fixture.write_settings(&serde_json::to_vec_pretty(&original).expect("JSON should encode"));
    fs::set_permissions(
        &fixture.config.claude_settings,
        fs::Permissions::from_mode(0o640),
    )
    .expect("settings permissions should update");

    let receipt = fixture
        .installer
        .install_at(INSTALLED_AT)
        .await
        .expect("installation should succeed");
    let installed = fixture.read_settings();
    let status = installed["statusLine"]
        .as_object()
        .expect("statusLine should be an object");
    assert_eq!(status["type"], "command");
    assert_eq!(status["command"], fixture.command());
    for key in [
        "padding",
        "refreshInterval",
        "hideVimModeIndicator",
        "futureKey",
    ] {
        assert_eq!(status[key], original["statusLine"][key]);
    }
    assert_eq!(
        read_json(&fixture.config.bridge_config),
        json!({ "chainedCommand": "x.sh" })
    );
    assert!(receipt.chained_previous_command);
    assert_eq!(mode(&fixture.config.claude_settings), 0o640);
    assert_eq!(mode(&receipt.backup_path), 0o640);

    let state_before = fs::read(&fixture.config.install_state).expect("state should exist");
    let second = fixture
        .installer
        .install_at(UnixSeconds(INSTALLED_AT.0 + 1))
        .await
        .expect("repeated installation should be idempotent");
    assert_eq!(second.installed_at, INSTALLED_AT);
    assert_eq!(
        fs::read(&fixture.config.install_state).expect("state should exist"),
        state_before
    );
    assert_eq!(
        fixture
            .installer
            .uninstall()
            .await
            .expect("uninstall should work"),
        UninstallOutcome::Restored
    );
    assert_eq!(fixture.read_settings(), original);
}

#[tokio::test]
async fn unrelated_top_level_settings_are_semantically_unchanged() {
    let fixture = Fixture::new();
    let original = json!({
        "permissions": { "allow": ["Read", "WebSearch"] },
        "theme": "dark",
        "futureTopLevel": [1, 2, 3]
    });
    fixture.write_settings(&serde_json::to_vec(&original).expect("JSON should encode"));

    fixture
        .installer
        .install_at(INSTALLED_AT)
        .await
        .expect("installation should succeed");
    let mut installed = fixture.read_settings();
    installed
        .as_object_mut()
        .expect("settings should be an object")
        .remove("statusLine");
    assert_eq!(installed, original);
}

#[tokio::test]
async fn invalid_json_and_non_object_top_level_are_never_overwritten() {
    for bytes in [b"{not-json".as_slice(), b"[1,2,3]".as_slice()] {
        let fixture = Fixture::new();
        fixture.write_settings(bytes);

        let error = fixture
            .installer
            .install_at(INSTALLED_AT)
            .await
            .expect_err("invalid settings must fail");
        assert!(matches!(
            error,
            BridgeInstallError::InvalidJson { .. } | BridgeInstallError::NonObject { .. }
        ));
        assert_eq!(
            fs::read(&fixture.config.claude_settings).expect("settings should remain"),
            bytes
        );
    }
}

#[tokio::test]
async fn uninstall_restores_exact_previous_value_and_keeps_later_top_level_edits() {
    let fixture = Fixture::new();
    let previous = json!({
        "type": "command",
        "command": "x.sh",
        "padding": 2,
        "futureKey": { "a": 1 }
    });
    fixture.write_settings(
        &serde_json::to_vec(&json!({ "statusLine": previous })).expect("settings should encode"),
    );
    fixture
        .installer
        .install_at(INSTALLED_AT)
        .await
        .expect("installation should succeed");
    let mut current = fixture.read_settings();
    current["theme"] = Value::String("dark".to_owned());
    fixture.write_settings(&serde_json::to_vec(&current).expect("settings should encode"));

    assert_eq!(
        fixture
            .installer
            .uninstall()
            .await
            .expect("uninstall should work"),
        UninstallOutcome::Restored
    );
    let restored = fixture.read_settings();
    assert_eq!(restored["statusLine"], previous);
    assert_eq!(restored["theme"], "dark");
    assert!(!fixture.config.install_state.exists());
    assert_eq!(
        read_json(&fixture.config.bridge_config),
        json!({ "chainedCommand": null })
    );
}

#[tokio::test]
async fn uninstall_leaves_user_changed_status_line_byte_for_byte() {
    let fixture = Fixture::new();
    fixture.write_settings(br#"{"statusLine":{"type":"command","command":"x.sh"}}"#);
    fixture
        .installer
        .install_at(INSTALLED_AT)
        .await
        .expect("installation should succeed");
    let changed = b"{\n  \"statusLine\": {\"type\":\"command\",\"command\":\"user-new.sh\"},\n  \"theme\": \"dark\"\n}\n";
    fixture.write_settings(changed);

    let outcome = fixture
        .installer
        .uninstall()
        .await
        .expect("uninstall should work");
    assert_eq!(outcome, UninstallOutcome::LeftUntouched);
    assert_eq!(
        outcome.message(),
        Some("Your status line was changed after install; left untouched")
    );
    assert_eq!(
        fs::read(&fixture.config.claude_settings).expect("settings should remain"),
        changed
    );
    assert!(fixture.config.install_state.exists());
    assert_eq!(
        read_json(&fixture.config.bridge_config),
        json!({ "chainedCommand": "x.sh" })
    );
}

#[tokio::test]
async fn uninstall_distinguishes_explicit_null_from_an_absent_key() {
    let fixture = Fixture::new();
    fixture.write_settings(br#"{"statusLine":null,"theme":"dark"}"#);
    fixture
        .installer
        .install_at(INSTALLED_AT)
        .await
        .expect("installation should succeed");

    assert_eq!(
        fixture
            .installer
            .uninstall()
            .await
            .expect("uninstall should work"),
        UninstallOutcome::Restored
    );
    assert_eq!(
        fixture.read_settings(),
        json!({ "statusLine": null, "theme": "dark" })
    );
}
