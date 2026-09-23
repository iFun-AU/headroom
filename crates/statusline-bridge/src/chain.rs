use std::{
    io::{Seek, SeekFrom, Write},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::{BridgeOutput, Error};

const CHAIN_TIMEOUT: Duration = Duration::from_secs(2);
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(5);

pub(crate) fn run(command: &str, input: &[u8]) -> Result<BridgeOutput, Error> {
    let mut process = Command::new("/bin/sh");
    process
        .args(["-c", command])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = process.spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(broken_child_pipe)?;
    let mut stdout = child.stdout.take().ok_or_else(broken_child_pipe)?;
    let owned_input = input.to_vec();
    let input_worker = thread::spawn(move || stdin.write_all(&owned_input));
    let mut spool = tempfile::tempfile()?;
    let output_worker = thread::spawn(move || {
        std::io::copy(&mut stdout, &mut spool)?;
        spool.seek(SeekFrom::Start(0))?;
        Ok::<_, std::io::Error>(spool)
    });

    wait_until_deadline(&mut child)?;
    if let Err(error) = input_worker.join().map_err(|_| Error::WorkerThread)?
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        return Err(Error::Io(error));
    }
    output_worker
        .join()
        .map_err(|_| Error::WorkerThread)?
        .map(BridgeOutput::Spool)
        .map_err(Error::Io)
}

fn wait_until_deadline(child: &mut std::process::Child) -> Result<(), Error> {
    let deadline = Instant::now() + CHAIN_TIMEOUT;
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        let now = Instant::now();
        if now >= deadline {
            kill_and_reap(child)?;
            return Ok(());
        }
        thread::sleep(WAIT_POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
    }
}

fn kill_and_reap(child: &mut std::process::Child) -> Result<(), Error> {
    let group = format!("-{}", child.id());
    let group_killed = Command::new("/bin/kill")
        .args(["-s", "KILL", "--", &group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !group_killed
        && let Err(error) = child.kill()
        && error.kind() != std::io::ErrorKind::InvalidInput
    {
        return Err(Error::Io(error));
    }
    child.wait()?;
    Ok(())
}

fn broken_child_pipe() -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "child stdio pipe was unavailable",
    ))
}
