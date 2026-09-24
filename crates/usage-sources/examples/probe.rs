//! Diagnostic source runner with an automated, real-data-free fixture mode.

use std::{
    error::Error,
    io::{self, Write},
    path::Path,
    time::Duration,
};

use serde_json::Value;
use tempfile::TempDir;
use tokio::{io::AsyncReadExt, task::JoinSet, time::timeout};
use tokio_util::sync::CancellationToken;
use usage_core::{History, SourceKind, UnixSeconds, UsageSnapshot};
use usage_sources::{
    claude::{
        bridge_source::{BridgeSource, BridgeSourceConfig},
        effectiveness::{Effectiveness, EffectivenessHandle},
        history::{HistorySource, HistorySourceConfig},
    },
    codex::{
        app_server::{AppServerConfig, AppServerSource},
        discover::DiscoveryOptions,
        rollout::{RolloutConfig, RolloutSource},
    },
    paths::{PathOverrides, Paths},
    scheduler::{Scheduler, SchedulerConfig},
    store::{StoreHandle, UsageStore},
};

#[path = "probe/claude_oauth.rs"]
mod claude_oauth;
#[path = "probe/fixture.rs"]
mod fixture;

const DEFAULT_LIVE_SECONDS: u64 = 30;
const FIXTURE_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_STATE_BYTES: u64 = 4 * 1_024 * 1_024;

struct ProbeConfig {
    paths: Paths,
    discovery: DiscoveryOptions,
    installed_at: Option<UnixSeconds>,
    stop: Stop,
    _temporary_root: Option<TempDir>,
}

#[derive(Clone, Copy)]
enum Stop {
    Fixture,
    After(Duration),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let arguments = Arguments::parse(std::env::args().skip(1))?;
    if arguments.claude_oauth_dump_keys {
        return claude_oauth::dump_keys().await;
    }
    let config = if arguments.fixture {
        let fixture = fixture::fixture()?;
        ProbeConfig {
            paths: fixture.paths,
            discovery: fixture.discovery,
            installed_at: Some(fixture.installed_at),
            stop: Stop::Fixture,
            _temporary_root: Some(fixture.root),
        }
    } else {
        live_config(arguments.seconds).await?
    };
    run(config).await
}

async fn live_config(seconds: u64) -> Result<ProbeConfig, Box<dyn Error>> {
    let home = dirs::home_dir().ok_or_else(|| io::Error::other("home directory unavailable"))?;
    let paths = Paths::discover(PathOverrides::default())?;
    let installed_at = read_installed_at(&paths.bridge_install_state).await?;
    Ok(ProbeConfig {
        paths,
        discovery: DiscoveryOptions::standard(None, &home),
        installed_at,
        stop: Stop::After(Duration::from_secs(seconds)),
        _temporary_root: None,
    })
}

async fn run(config: ProbeConfig) -> Result<(), Box<dyn Error>> {
    let cancel = CancellationToken::new();
    let (scheduler, scheduler_actor) = Scheduler::channel(SchedulerConfig::default());
    let (events, mut store, store_actor) =
        UsageStore::channel_with_scheduler(vec![75, 90, 100], scheduler.clone());
    let (effectiveness, effectiveness_actor) = EffectivenessHandle::channel(config.installed_at);
    let mut tasks = JoinSet::new();
    tasks.spawn(scheduler_actor.run(cancel.child_token()));
    tasks.spawn(store_actor.run(cancel.child_token()));
    tasks.spawn(effectiveness_actor.run(cancel.child_token()));

    let (app_server, _control) = AppServerSource::new(
        AppServerConfig::new(config.discovery, env!("CARGO_PKG_VERSION")),
        scheduler.clone(),
    );
    let source_events = events.clone();
    let source_cancel = cancel.child_token();
    tasks.spawn(async move {
        match app_server.run(source_events, source_cancel).await {
            Ok(()) | Err(_) => {}
        }
    });

    let rollout = RolloutSource::new(RolloutConfig::new(&config.paths.codex_sessions), scheduler);
    let source_events = events.clone();
    let source_cancel = cancel.child_token();
    tasks.spawn(async move {
        match rollout.run(source_events, source_cancel).await {
            Ok(()) | Err(_) => {}
        }
    });

    let bridge = BridgeSource::new(
        BridgeSourceConfig::from_paths(&config.paths, config.installed_at),
        effectiveness.clone(),
    );
    let source_events = events.clone();
    let source_cancel = cancel.child_token();
    tasks.spawn(async move {
        match bridge.run(source_events, source_cancel).await {
            Ok(()) | Err(_) => {}
        }
    });

    let history = HistorySource::new(
        HistorySourceConfig::from_paths(&config.paths),
        effectiveness.clone(),
    );
    let source_cancel = cancel.child_token();
    tasks.spawn(async move {
        match history.run(events, source_cancel).await {
            Ok(()) | Err(_) => {}
        }
    });

    write_snapshot(&store.snapshot.borrow())?;
    let result = observe(&mut store, &effectiveness, config.stop).await;
    cancel.cancel();
    shutdown(&mut tasks).await?;
    result
}

async fn observe(
    store: &mut StoreHandle,
    effectiveness: &EffectivenessHandle,
    stop: Stop,
) -> Result<(), Box<dyn Error>> {
    let duration = match stop {
        Stop::Fixture => FIXTURE_TIMEOUT,
        Stop::After(duration) => duration,
    };
    let deadline = tokio::time::sleep(duration);
    tokio::pin!(deadline);
    let mut fixture_check = tokio::time::interval(Duration::from_millis(25));
    loop {
        tokio::select! {
            changed = store.snapshot.changed() => {
                changed.map_err(|_| io::Error::other("usage store stopped"))?;
                write_snapshot(&store.snapshot.borrow())?;
            }
            _ = fixture_check.tick(), if matches!(stop, Stop::Fixture) => {
                if fixture_ready(store, effectiveness).await? {
                    return Ok(());
                }
            }
            () = &mut deadline => {
                return match stop {
                    Stop::Fixture => Err(io::Error::other("fixture probe timed out").into()),
                    Stop::After(_) => Ok(()),
                };
            }
        }
    }
}

async fn fixture_ready(
    store: &StoreHandle,
    effectiveness: &EffectivenessHandle,
) -> Result<bool, usage_sources::store::StoreError> {
    let snapshot = store.snapshot.borrow().clone();
    if snapshot.claude.windows.is_empty()
        || snapshot.codex.windows.is_empty()
        || snapshot.codex.authoritative_source != Some(SourceKind::CodexAppServer)
    {
        return Ok(false);
    }
    let claude = store.history(usage_core::Provider::Claude).await?;
    let codex = store.history(usage_core::Provider::Codex).await?;
    Ok(history_tokens(&claude) > 0
        && history_tokens(&codex) > 0
        && effectiveness.snapshot().effective == Effectiveness::Confirmed)
}

fn history_tokens(history: &History) -> u64 {
    history
        .hourly
        .buckets
        .iter()
        .map(|bucket| bucket.tokens.0)
        .sum()
}

fn write_snapshot(snapshot: &UsageSnapshot) -> Result<(), Box<dyn Error>> {
    let mut bytes = serde_json::to_vec_pretty(snapshot)?;
    bytes.push(b'\n');
    io::stdout().write_all(&bytes)?;
    Ok(())
}

async fn shutdown(tasks: &mut JoinSet<()>) -> Result<(), Box<dyn Error>> {
    let joined = timeout(SHUTDOWN_TIMEOUT, async {
        while let Some(result) = tasks.join_next().await {
            result.map_err(|_| io::Error::other("probe task panicked"))?;
        }
        Ok::<(), io::Error>(())
    })
    .await;
    if let Ok(result) = joined {
        result.map_err(Into::into)
    } else {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        Err(io::Error::other("probe tasks did not stop within three seconds").into())
    }
}

async fn read_installed_at(path: &Path) -> Result<Option<UnixSeconds>, Box<dyn Error>> {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata().await?;
    if metadata.len() > MAX_STATE_BYTES {
        return Err(io::Error::other("bridge state exceeds 4 MiB").into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.take(MAX_STATE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_STATE_BYTES {
        return Err(io::Error::other("bridge state exceeds 4 MiB").into());
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    let installed_at = value
        .get("installedAt")
        .and_then(Value::as_i64)
        .ok_or_else(|| io::Error::other("bridge state has no valid installedAt"))?;
    Ok(Some(UnixSeconds(installed_at)))
}

struct Arguments {
    fixture: bool,
    seconds: u64,
    claude_oauth_dump_keys: bool,
}

impl Arguments {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, io::Error> {
        let mut fixture = false;
        let mut claude_oauth_dump_keys = false;
        let mut seconds = DEFAULT_LIVE_SECONDS;
        let mut arguments = arguments.peekable();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--fixture" => fixture = true,
                "--claude-oauth-dump-keys" => claude_oauth_dump_keys = true,
                "--seconds" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| io::Error::other("--seconds requires a value"))?;
                    seconds = value
                        .parse()
                        .map_err(|_| io::Error::other("--seconds must be an integer"))?;
                    if seconds == 0 {
                        return Err(io::Error::other("--seconds must be positive"));
                    }
                }
                _ => {
                    return Err(io::Error::other(
                        "usage: probe [--fixture] [--seconds N] | --claude-oauth-dump-keys",
                    ));
                }
            }
        }
        Ok(Self {
            fixture,
            seconds,
            claude_oauth_dump_keys,
        })
    }
}
