//! Settings migration and persistence acceptance tests.

#![allow(clippy::expect_used)]

use std::{fs, path::Path};

use headroom::settings::{
    CURRENT_SCHEMA_VERSION, Settings, SettingsActorError, SettingsHandle, WidgetVariant,
    load_settings, migrate, reset_settings, save_settings,
};
use serde_json::{Value, json};
use tempfile::tempdir;
use tokio::time::{Duration, timeout};
use tokio_util::sync::CancellationToken;

fn read_json(path: &Path) -> Value {
    let bytes = fs::read(path).expect("settings file should be readable");
    serde_json::from_slice(&bytes).expect("settings file should contain JSON")
}

#[test]
fn missing_file_creates_current_defaults_with_onboarding_incomplete() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("settings.json");

    let state = load_settings(&path).expect("missing settings should load defaults");

    assert_eq!(state.settings, Settings::default());
    assert!(!state.settings.onboarding_completed);
    assert!(!state.read_only);
    assert_eq!(state.notice, None);
    assert_eq!(read_json(&path)["schemaVersion"], CURRENT_SCHEMA_VERSION);
}

#[test]
fn pure_v0_migration_adds_version_and_preserves_known_values() {
    let migrated = migrate(json!({
        "onboardingCompleted": true,
        "codexPath": "/tmp/codex",
        "widget": { "variant": "Mini", "opacity": 0.75 }
    }))
    .expect("v0 settings should migrate");

    assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
    assert!(migrated.onboarding_completed);
    assert_eq!(migrated.codex_path.as_deref(), Some("/tmp/codex"));
    assert_eq!(migrated.widget.variant, WidgetVariant::Mini);
    assert!((migrated.widget.opacity - 0.75).abs() < f32::EPSILON);
}

#[test]
fn v0_file_is_migrated_and_saved_as_v1() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("settings.json");
    fs::write(
        &path,
        br#"{"onboardingCompleted":true,"notifyOnReset":false}"#,
    )
    .expect("v0 fixture should be written");

    let state = load_settings(&path).expect("v0 settings should load");

    assert!(state.settings.onboarding_completed);
    assert!(!state.settings.notify_on_reset);
    assert!(!state.read_only);
    assert_eq!(read_json(&path)["schemaVersion"], CURRENT_SCHEMA_VERSION);
}

#[test]
fn newer_schema_uses_read_only_defaults_without_changing_file() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("settings.json");
    let original = br#"{"schemaVersion":99,"future":"keep me byte-for-byte"}"#;
    fs::write(&path, original).expect("future fixture should be written");

    let state = load_settings(&path).expect("future settings should degrade safely");

    assert_eq!(state.settings, Settings::default());
    assert!(state.read_only);
    assert_eq!(
        state.notice.as_deref(),
        Some("Settings were created by a newer version")
    );
    assert_eq!(
        fs::read(&path).expect("future file should remain"),
        original
    );
}

#[test]
fn corrupt_json_is_renamed_before_defaults_are_saved() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("settings.json");
    let original = b"{ definitely not JSON";
    fs::write(&path, original).expect("corrupt fixture should be written");

    let state = load_settings(&path).expect("corrupt settings should recover");

    assert_eq!(state.settings, Settings::default());
    assert!(!state.read_only);
    assert_eq!(read_json(&path)["schemaVersion"], 1);
    let corrupt = fs::read_dir(directory.path())
        .expect("settings directory should be readable")
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("settings.json.corrupt-")
        })
        .expect("corrupt settings should be retained");
    assert_eq!(
        fs::read(corrupt.path()).expect("corrupt backup should be readable"),
        original
    );
}

#[test]
fn validation_clamps_and_normalizes_every_bounded_field() {
    let settings = migrate(json!({
        "schemaVersion": 1,
        "thresholds": [101, 0, 90, 75, 75],
        "widget": {
            "visible": true,
            "x": -1440,
            "y": 80,
            "variant": "Stack",
            "opacity": 0.1
        },
        "pollActiveSecs": 1,
        "pollIdleSecs": 119
    }))
    .expect("v1 settings should validate");

    assert_eq!(settings.thresholds, vec![1, 75, 90, 100]);
    assert!((settings.widget.opacity - 0.4).abs() < f32::EPSILON);
    assert_eq!(settings.widget.x, Some(-1440));
    assert_eq!(settings.widget.y, Some(80));
    assert_eq!(settings.poll_active_secs, 60);
    assert_eq!(settings.poll_idle_secs, 120);
}

#[test]
fn save_clamps_and_reset_overwrites_a_future_schema() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("settings.json");
    let mut settings = Settings {
        schema_version: 99,
        thresholds: vec![100, 0, 100],
        poll_active_secs: 0,
        ..Settings::default()
    };
    settings.widget.opacity = 2.0;

    let saved = save_settings(&path, settings).expect("settings should save atomically");
    assert_eq!(saved.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(saved.thresholds, vec![1, 100]);
    assert_eq!(saved.poll_active_secs, 60);
    assert!((saved.widget.opacity - 1.0).abs() < f32::EPSILON);

    fs::write(&path, br#"{"schemaVersion":99}"#).expect("future fixture should replace settings");
    let reset = reset_settings(&path).expect("explicit reset should replace future settings");
    assert_eq!(reset.settings, Settings::default());
    assert!(!reset.read_only);
    assert_eq!(reset.notice, None);
    assert_eq!(read_json(&path)["schemaVersion"], 1);
}

#[tokio::test]
async fn settings_actor_serializes_writes_and_allows_explicit_read_only_reset() {
    let directory = tempdir().expect("tempdir should be created");
    let path = directory.path().join("settings.json");
    fs::write(&path, br#"{"schemaVersion":99,"future":true}"#)
        .expect("future fixture should be written");
    let initial = load_settings(&path).expect("future settings should load read-only");
    let (handle, actor) = SettingsHandle::channel(path.clone(), initial);
    let cancel = CancellationToken::new();
    let task = tokio::spawn(actor.run(cancel.child_token()));

    assert!(matches!(
        handle.set(Settings::default()).await,
        Err(SettingsActorError::ReadOnly)
    ));
    let reset = handle.reset().await.expect("explicit reset should succeed");
    assert!(!reset.read_only);
    assert_eq!(read_json(&path)["schemaVersion"], 1);

    let mut changed = reset.settings;
    changed.poll_idle_secs = 1;
    let changed = handle
        .set(changed)
        .await
        .expect("writable set should succeed");
    assert_eq!(changed.settings.poll_idle_secs, 120);
    let completed = handle
        .complete_onboarding()
        .await
        .expect("onboarding should save");
    assert!(completed.settings.onboarding_completed);
    assert_eq!(handle.snapshot(), completed);

    let positioned = handle
        .update_widget_position(-120, 64)
        .await
        .expect("widget position should save");
    assert_eq!(positioned.settings.widget.x, Some(-120));
    assert_eq!(positioned.settings.widget.y, Some(64));
    let visible = handle
        .update_widget_visibility(true)
        .await
        .expect("widget visibility should save");
    assert!(visible.settings.widget.visible);
    assert!(visible.settings.onboarding_completed);

    let mut updates = handle.subscribe();
    updates.borrow_and_update();
    handle
        .update_widget_position(-120, 64)
        .await
        .expect("unchanged widget position should be a no-op");
    assert!(
        timeout(Duration::from_millis(10), updates.changed())
            .await
            .is_err(),
        "an unchanged native move must not cause a settings feedback loop"
    );
    assert_eq!(read_json(&path)["widget"]["x"], -120);
    assert_eq!(read_json(&path)["widget"]["visible"], true);

    cancel.cancel();
    timeout(Duration::from_millis(100), task)
        .await
        .expect("settings actor should cancel promptly")
        .expect("settings actor should not panic");
}
