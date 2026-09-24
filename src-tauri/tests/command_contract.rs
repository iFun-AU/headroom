//! Typed Tauri command contract tests.

#![allow(clippy::expect_used)]

use headroom::commands::{
    BridgeStatus, CodexDetection, CommandError, PathKind, PathPurpose, Route, WindowTarget,
};
use serde_json::{Value, json};
use usage_sources::claude::effectiveness::Effectiveness;

#[test]
fn command_enums_serialize_as_closed_documented_variants() {
    for (value, expected) in [
        (
            serde_json::to_value(PathKind::File).expect("path kind should serialize"),
            json!("File"),
        ),
        (
            serde_json::to_value(PathPurpose::CodexHome).expect("path purpose should serialize"),
            json!("CodexHome"),
        ),
        (
            serde_json::to_value(WindowTarget::Popover).expect("window target should serialize"),
            json!("Popover"),
        ),
        (
            serde_json::to_value(Route::Onboarding).expect("route should serialize"),
            json!("Onboarding"),
        ),
    ] {
        assert_eq!(value, expected);
    }
}

#[test]
fn response_types_use_camel_case_without_exposing_internal_errors() {
    let bridge = BridgeStatus {
        installed: true,
        chained: false,
        effective: Effectiveness::Confirmed,
        settings_path: "/tmp/settings.json".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(bridge).expect("bridge status should serialize"),
        json!({
            "installed": true,
            "chained": false,
            "effective": "confirmed",
            "settingsPath": "/tmp/settings.json"
        })
    );

    let detection = CodexDetection {
        path: Some("/tmp/codex".to_owned()),
        version: Some("codex-cli 1.2.3".to_owned()),
    };
    assert_eq!(
        serde_json::to_value(detection).expect("detection should serialize"),
        json!({"path": "/tmp/codex", "version": "codex-cli 1.2.3"})
    );

    let error: Value = serde_json::to_value(CommandError::public("Could not refresh usage"))
        .expect("command error should serialize");
    assert_eq!(error, json!({ "message": "Could not refresh usage" }));
}
