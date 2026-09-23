//! One fully owned Codex app-server process attempt.

use std::{io, path::Path, process::Stdio, time::Duration};

use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::{mpsc, oneshot, watch},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};
use usage_core::log_trunc;

use crate::{SourceEvent, scheduler::Scheduler};

use super::super::{super::rpc::RpcClient, AppServerError, AppServerSession};

const STDERR_LINE_CAPACITY: usize = 2 * 1_024;
const STDERR_CHUNK_CAPACITY: usize = 4 * 1_024;
const TASK_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub(super) enum AttemptResult {
    Cancelled,
    Unsupported(String),
    Stopped { became_ready: bool },
}

#[derive(Debug, Error)]
pub(super) enum AttemptError {
    #[error("could not spawn Codex app-server: {0}")]
    Spawn(#[source] io::Error),
    #[error("Codex app-server {0} pipe was unavailable")]
    MissingPipe(&'static str),
    #[error("Codex app-server PID was unavailable")]
    MissingPid,
    #[error("usage event channel is closed")]
    EventChannelClosed,
}

pub(super) async fn run_attempt(
    binary: &Path,
    events: mpsc::Sender<SourceEvent>,
    cancel: CancellationToken,
    scheduler: Scheduler,
    app_version: &str,
    request_timeout: Duration,
    pid_state: &watch::Sender<Option<u32>>,
) -> Result<AttemptResult, AttemptError> {
    let mut child = spawn_child(binary)?;
    let Some(stdin) = child.stdin.take() else {
        reap_child(&mut child, false).await;
        return Err(AttemptError::MissingPipe("stdin"));
    };
    let Some(stdout) = child.stdout.take() else {
        reap_child(&mut child, false).await;
        return Err(AttemptError::MissingPipe("stdout"));
    };
    let Some(stderr) = child.stderr.take() else {
        reap_child(&mut child, false).await;
        return Err(AttemptError::MissingPipe("stderr"));
    };
    let Some(pid) = child.id() else {
        reap_child(&mut child, false).await;
        return Err(AttemptError::MissingPid);
    };
    pid_state.send_replace(Some(pid));
    info!(pid, "codex app-server started");

    let task_cancel = cancel.child_token();
    let (client, notifications, driver) = RpcClient::new(stdout, stdin, request_timeout);
    let mut tasks = JoinSet::new();
    let driver_cancel = task_cancel.child_token();
    tasks.spawn(async move {
        if let Err(error) = driver.run(driver_cancel).await {
            debug!(error = %log_trunc(&error.to_string()), "Codex RPC driver stopped");
        }
    });
    let stderr_cancel = task_cancel.child_token();
    tasks.spawn(async move {
        if let Err(error) = drain_stderr(stderr, stderr_cancel).await {
            debug!(error = %log_trunc(&error.to_string()), "Codex stderr drain stopped");
        }
    });

    let (ready_tx, ready_rx) = oneshot::channel();
    let session =
        AppServerSession::new(client, notifications, scheduler, app_version).with_ready(ready_tx);
    let (result, child_reaped) = run_session(&mut child, session, events, ready_rx, &cancel).await;

    task_cancel.cancel();
    reap_child(&mut child, child_reaped).await;
    pid_state.send_replace(None);
    finish_tasks(&mut tasks).await;
    result
}

fn spawn_child(binary: &Path) -> Result<Child, AttemptError> {
    Command::new(binary)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(AttemptError::Spawn)
}

async fn run_session(
    child: &mut Child,
    session: AppServerSession,
    events: mpsc::Sender<SourceEvent>,
    ready: oneshot::Receiver<()>,
    cancel: &CancellationToken,
) -> (Result<AttemptResult, AttemptError>, bool) {
    let mut became_ready = false;
    let mut ready_done = false;
    let session = session.run(events, cancel.child_token());
    tokio::pin!(session);
    tokio::pin!(ready);
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return (Ok(AttemptResult::Cancelled), false),
            ready_result = &mut ready, if !ready_done => {
                became_ready = ready_result.is_ok();
                ready_done = true;
            }
            result = &mut session => {
                let mapped = match result {
                    Err(AppServerError::Unsupported { reason }) => {
                        Ok(AttemptResult::Unsupported(reason))
                    }
                    Err(AppServerError::EventChannelClosed) => {
                        Err(AttemptError::EventChannelClosed)
                    }
                    Ok(()) | Err(_) => Ok(AttemptResult::Stopped { became_ready }),
                };
                return (mapped, false);
            }
            result = child.wait() => {
                if let Err(error) = result {
                    debug!(error = %log_trunc(&error.to_string()), "waiting for Codex app-server failed");
                }
                return (Ok(AttemptResult::Stopped { became_ready }), true);
            }
        }
    }
}

async fn reap_child(child: &mut Child, already_reaped: bool) {
    if already_reaped {
        return;
    }
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.start_kill();
    }
    let _ = child.wait().await;
}

async fn finish_tasks(tasks: &mut JoinSet<()>) {
    let joined = tokio::time::timeout(TASK_SHUTDOWN_TIMEOUT, async {
        while tasks.join_next().await.is_some() {}
    })
    .await;
    if joined.is_err() {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
}

async fn drain_stderr<R>(mut stderr: R, cancel: CancellationToken) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut chunk = [0_u8; STDERR_CHUNK_CAPACITY];
    let mut line = Vec::with_capacity(STDERR_LINE_CAPACITY);
    loop {
        let read = tokio::select! {
            biased;
            () = cancel.cancelled() => return Ok(()),
            result = stderr.read(&mut chunk) => result?,
        };
        if read == 0 {
            log_stderr_line(&line);
            return Ok(());
        }
        for &byte in &chunk[..read] {
            if byte == b'\n' {
                log_stderr_line(&line);
                line.clear();
            } else if line.len() < STDERR_LINE_CAPACITY {
                line.push(byte);
            }
        }
    }
}

fn log_stderr_line(line: &[u8]) {
    if !line.is_empty() {
        debug!(stderr = %log_trunc(&String::from_utf8_lossy(line)), "Codex app-server stderr");
    }
}
