//! Temporary all-source fixture construction for the diagnostic probe.

use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{SecondsFormat, Utc};
use tempfile::{TempDir, tempdir};
use usage_core::UnixSeconds;
use usage_sources::{
    codex::discover::DiscoveryOptions,
    paths::{PathEnvironment, PathOverrides, Paths},
};

pub(super) struct Fixture {
    pub(super) root: TempDir,
    pub(super) paths: Paths,
    pub(super) discovery: DiscoveryOptions,
    pub(super) installed_at: UnixSeconds,
}

pub(super) fn fixture() -> io::Result<Fixture> {
    let root = tempdir()?;
    let home = root.path().join("home");
    let data = root.path().join("data");
    fs::create_dir_all(&home)?;
    fs::create_dir_all(&data)?;
    let paths = Paths::resolve(
        &home,
        &data,
        PathOverrides::default(),
        PathEnvironment::default(),
    );
    create_sources(&paths)?;
    let binary = create_fake_codex(root.path())?;
    let discovery = DiscoveryOptions::testing(
        Some(binary),
        Vec::new(),
        PathBuf::from("/usr/bin/false"),
        Duration::from_secs(1),
    );
    Ok(Fixture {
        root,
        paths,
        discovery,
        installed_at: UnixSeconds(unix_now().0 - 1),
    })
}

fn create_sources(paths: &Paths) -> io::Result<()> {
    let now = unix_now();
    let rollout_dir = paths.codex_sessions.join("2026/09/23");
    let history_dir = paths.claude_projects.join("fixture-project");
    fs::create_dir_all(&rollout_dir)?;
    fs::create_dir_all(&history_dir)?;
    fs::create_dir_all(&paths.app_support)?;

    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let rollout = format!(
        r#"{{"timestamp":"{timestamp}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"total_tokens":211554}},"last_token_usage":{{"total_tokens":211554}}}},"rate_limits":{{"limit_id":"codex","primary":{{"used_percent":1,"window_minutes":10080,"resets_at":{}}},"plan_type":"pro"}}}}}}"#,
        now.0 + 604_800,
    );
    fs::write(
        rollout_dir.join("rollout-fixture.jsonl"),
        format!("{rollout}\n"),
    )?;

    let history = format!(
        r#"{{"type":"assistant","timestamp":"{timestamp}","requestId":"req-probe","message":{{"id":"msg-probe","usage":{{"input_tokens":7,"output_tokens":5}}}}}}"#,
    );
    fs::write(history_dir.join("session.jsonl"), format!("{history}\n"))?;

    let bridge = format!(
        r#"{{"schema":1,"writtenAt":{},"sessionId":"probe","rateLimits":{{"five_hour":{{"used_percentage":23.5,"resets_at":{}}},"seven_day":{{"used_percentage":41.2,"resets_at":{}}}}}}}"#,
        now.0,
        now.0 + 18_000,
        now.0 + 604_800,
    );
    fs::write(&paths.bridge_rate_limits, bridge)
}

fn create_fake_codex(root: &Path) -> io::Result<PathBuf> {
    let now = unix_now();
    let binary = root.join("codex-fixture");
    let limits = format!(
        r#"{{"id":2,"result":{{"rateLimits":{{"limitId":"codex","primary":{{"usedPercent":2,"windowDurationMins":10080,"resetsAt":{}}},"planType":"pro"}}}}}}"#,
        now.0 + 604_800,
    );
    let date = Utc::now().date_naive();
    let script = format!(
        "#!/bin/sh\nIFS= read -r initialize || exit 1\nprintf '%s\\n' '{{\"id\":1,\"result\":{{}}}}'\nIFS= read -r initialized || exit 1\nIFS= read -r limits || exit 1\nprintf '%s\\n' '{limits}'\nIFS= read -r usage || exit 1\nprintf '%s\\n' '{{\"id\":3,\"result\":{{\"dailyUsageBuckets\":[{{\"startDate\":\"{date}\",\"tokens\":345678}}]}}}}'\nwhile IFS= read -r line; do :; done\n",
    );
    fs::write(&binary, script)?;
    let mut permissions = fs::metadata(&binary)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions)?;
    Ok(binary)
}

fn unix_now() -> UnixSeconds {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    UnixSeconds(i64::try_from(seconds).unwrap_or(i64::MAX))
}
