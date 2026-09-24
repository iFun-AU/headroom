//! Central path-resolution tests using synthetic roots only.

use std::path::PathBuf;

use usage_sources::paths::{PathEnvironment, PathOverrides, Paths};

#[test]
fn defaults_are_derived_from_supplied_roots() {
    let paths = Paths::resolve(
        &PathBuf::from("/test/home"),
        &PathBuf::from("/test/data"),
        PathOverrides::default(),
        PathEnvironment::default(),
    );

    assert_eq!(paths.codex_home, PathBuf::from("/test/home/.codex"));
    assert_eq!(
        paths.codex_sessions,
        PathBuf::from("/test/home/.codex/sessions")
    );
    assert_eq!(paths.claude_dir, PathBuf::from("/test/home/.claude"));
    assert_eq!(
        paths.claude_projects,
        PathBuf::from("/test/home/.claude/projects")
    );
    assert_eq!(
        paths.app_support,
        PathBuf::from("/test/data/dev.headroom.app")
    );
    assert_eq!(
        paths.bridge_rate_limits,
        PathBuf::from("/test/data/dev.headroom.app/claude-rate-limits.json")
    );
    assert_eq!(
        paths.logs,
        PathBuf::from("/test/home/Library/Logs/dev.headroom.app")
    );
}

#[test]
fn settings_override_environment_then_defaults() {
    let environment = PathEnvironment {
        codex_home: Some(PathBuf::from("/env/codex")),
        claude_dir: Some(PathBuf::from("/env/claude")),
    };
    let environment_paths = Paths::resolve(
        &PathBuf::from("/home"),
        &PathBuf::from("/data"),
        PathOverrides::default(),
        environment.clone(),
    );
    assert_eq!(environment_paths.codex_home, PathBuf::from("/env/codex"));
    assert_eq!(environment_paths.claude_dir, PathBuf::from("/env/claude"));

    let settings_paths = Paths::resolve(
        &PathBuf::from("/home"),
        &PathBuf::from("/data"),
        PathOverrides {
            codex_home: Some(PathBuf::from("/settings/codex")),
            claude_dir: Some(PathBuf::from("/settings/claude")),
        },
        environment,
    );
    assert_eq!(settings_paths.codex_home, PathBuf::from("/settings/codex"));
    assert_eq!(settings_paths.claude_dir, PathBuf::from("/settings/claude"));
}
