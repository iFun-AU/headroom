//! Bounded newline-delimited JSON-RPC transport for `codex app-server`.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, oneshot, watch},
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use usage_core::log_trunc;

mod support;

use support::{Command, Pending, remote_error};
pub use support::{RpcError, RpcNotification, RpcTransportError};

const COMMAND_CAPACITY: usize = 64;
const NOTIFICATION_CAPACITY: usize = 32;
const MAX_PENDING_REQUESTS: usize = 64;
const MAX_RPC_LINE_BYTES: usize = 1024 * 1024;
const READ_CHUNK_BYTES: usize = 16 * 1024;

/// Cloneable request handle. The driver owns all mutable protocol state.
#[derive(Clone)]
pub struct RpcClient {
    commands: mpsc::Sender<Command>,
    next_id: Arc<AtomicU64>,
    pending: watch::Receiver<usize>,
}

impl RpcClient {
    /// Builds a client, bounded notification receiver, and unspawned driver.
    #[must_use]
    pub fn new<R, W>(
        reader: R,
        writer: W,
        request_timeout: Duration,
    ) -> (Self, mpsc::Receiver<RpcNotification>, RpcDriver<R, W>)
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let (commands, command_rx) = mpsc::channel(COMMAND_CAPACITY);
        let (notifications, notification_rx) = mpsc::channel(NOTIFICATION_CAPACITY);
        let (pending_tx, pending) = watch::channel(0);
        let client = Self {
            commands,
            next_id: Arc::new(AtomicU64::new(1)),
            pending,
        };
        let driver = RpcDriver {
            reader,
            writer,
            commands: command_rx,
            notifications,
            pending: HashMap::with_capacity(MAX_PENDING_REQUESTS),
            pending_count: pending_tx,
            request_timeout,
            line: Vec::new(),
            discarding_line: false,
        };
        (client, notification_rx, driver)
    }

    /// Sends one request and waits for its correlated response.
    ///
    /// # Errors
    ///
    /// Returns a typed transport, timeout, capacity, or remote error.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Request {
                id,
                method: method.to_owned(),
                params,
                reply,
            })
            .await
            .map_err(|_| RpcError::Closed)?;
        response.await.map_err(|_| RpcError::Closed)?
    }

    /// Sends a notification after the driver has written it to the pipe.
    ///
    /// # Errors
    ///
    /// Returns [`RpcError::Closed`] when the transport stops before the write.
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), RpcError> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Notify {
                method: method.to_owned(),
                params,
                reply,
            })
            .await
            .map_err(|_| RpcError::Closed)?;
        response.await.map_err(|_| RpcError::Closed)?
    }

    /// Returns the driver's current bounded pending-map size.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        *self.pending.borrow()
    }
}

/// Owns the reader, writer, pending map, and line buffer.
pub struct RpcDriver<R, W> {
    reader: R,
    writer: W,
    commands: mpsc::Receiver<Command>,
    notifications: mpsc::Sender<RpcNotification>,
    pending: HashMap<u64, Pending>,
    pending_count: watch::Sender<usize>,
    request_timeout: Duration,
    line: Vec<u8>,
    discarding_line: bool,
}

impl<R, W> RpcDriver<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    /// Runs the protocol loop until cancellation, disconnect, or fatal I/O.
    ///
    /// # Errors
    ///
    /// Returns a fatal pipe, encoding, size, or disconnect error after draining
    /// every pending response.
    pub async fn run(mut self, cancel: CancellationToken) -> Result<(), RpcTransportError> {
        let mut chunk = vec![0_u8; READ_CHUNK_BYTES].into_boxed_slice();
        loop {
            let has_deadline = !self.pending.is_empty();
            let deadline = self.next_deadline().unwrap_or_else(Instant::now);
            tokio::select! {
                biased;
                () = cancel.cancelled() => {
                    self.drain_pending(false);
                    return Ok(());
                }
                command = self.commands.recv() => {
                    let Some(command) = command else {
                        self.drain_pending(false);
                        return Ok(());
                    };
                    match self.handle_command(command, &cancel).await {
                        Ok(true) => {}
                        Ok(false) => {
                            self.drain_pending(false);
                            return Ok(());
                        }
                        Err(error) => {
                            self.drain_pending(true);
                            return Err(error);
                        }
                    }
                }
                read = self.reader.read(&mut chunk) => {
                    let read = match read {
                        Ok(read) => read,
                        Err(error) => {
                            self.drain_pending(true);
                            return Err(RpcTransportError::Io(error));
                        }
                    };
                    if read == 0 {
                        self.drain_pending(true);
                        return Err(RpcTransportError::Disconnected);
                    }
                    match self.consume(&chunk[..read], &cancel).await {
                        Ok(true) => {}
                        Ok(false) => {
                            self.drain_pending(false);
                            return Ok(());
                        }
                        Err(error) => {
                            self.drain_pending(true);
                            return Err(error);
                        }
                    }
                }
                () = tokio::time::sleep_until(deadline), if has_deadline => {
                    self.expire(Instant::now());
                }
            }
        }
    }

    async fn handle_command(
        &mut self,
        command: Command,
        cancel: &CancellationToken,
    ) -> Result<bool, RpcTransportError> {
        match command {
            Command::Request {
                id,
                method,
                params,
                reply,
            } => {
                if self.pending.len() == MAX_PENDING_REQUESTS {
                    let _ = reply.send(Err(RpcError::Busy));
                    return Ok(true);
                }
                let message = json!({ "id": id, "method": method, "params": params });
                if !self.write_message(&message, cancel).await? {
                    let _ = reply.send(Err(RpcError::Closed));
                    return Ok(false);
                }
                self.pending.insert(
                    id,
                    Pending {
                        deadline: Instant::now() + self.request_timeout,
                        reply,
                    },
                );
                self.update_pending_count();
            }
            Command::Notify {
                method,
                params,
                reply,
            } => {
                let mut message = json!({ "method": method });
                if let Some(params) = params {
                    message["params"] = params;
                }
                let written = self.write_message(&message, cancel).await?;
                let _ = reply.send(if written {
                    Ok(())
                } else {
                    Err(RpcError::Closed)
                });
                return Ok(written);
            }
        }
        Ok(true)
    }

    async fn consume(
        &mut self,
        bytes: &[u8],
        cancel: &CancellationToken,
    ) -> Result<bool, RpcTransportError> {
        for &byte in bytes {
            if byte == b'\n' {
                if self.discarding_line {
                    self.discarding_line = false;
                } else {
                    let line = std::mem::take(&mut self.line);
                    if !self.handle_line(&line, cancel).await? {
                        return Ok(false);
                    }
                }
            } else if !self.discarding_line {
                if self.line.len() == MAX_RPC_LINE_BYTES {
                    self.line.clear();
                    self.discarding_line = true;
                    warn!("discarding oversized RPC line");
                } else {
                    self.line.push(byte);
                }
            }
        }
        Ok(true)
    }

    async fn handle_line(
        &mut self,
        line: &[u8],
        cancel: &CancellationToken,
    ) -> Result<bool, RpcTransportError> {
        let message: Value = match serde_json::from_slice(line) {
            Ok(message) => message,
            Err(error) => {
                warn!(error = %log_trunc(&error.to_string()), "ignoring malformed RPC line");
                return Ok(true);
            }
        };
        if message.get("result").is_some() || message.get("error").is_some() {
            self.complete_response(message);
            return Ok(true);
        }
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let (Some(method), Some(id)) = (method.as_deref(), message.get("id")) {
            debug!(method = %log_trunc(method), "rejecting unsupported server RPC request");
            let reply = json!({
                "id": id,
                "error": { "code": -32601, "message": "not supported" }
            });
            return self.write_message(&reply, cancel).await;
        }
        if let Some(method) = method {
            let notification = RpcNotification { method, message };
            if let Err(mpsc::error::TrySendError::Full(notification)) =
                self.notifications.try_send(notification)
            {
                warn!(method = %log_trunc(&notification.method), "RPC notification queue is full; dropping message");
            }
        }
        Ok(true)
    }

    fn complete_response(&mut self, message: Value) {
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return;
        };
        let Some(pending) = self.pending.remove(&id) else {
            debug!(id, "ignoring response for unknown RPC request");
            return;
        };
        let response = message
            .get("error")
            .map(remote_error)
            .map_or(Ok(message), Err);
        let _ = pending.reply.send(response);
        self.update_pending_count();
    }

    async fn write_message(
        &mut self,
        message: &Value,
        cancel: &CancellationToken,
    ) -> Result<bool, RpcTransportError> {
        let mut encoded = serde_json::to_vec(message)?;
        if encoded.len() >= MAX_RPC_LINE_BYTES {
            return Err(RpcTransportError::OutgoingTooLarge);
        }
        encoded.push(b'\n');
        tokio::select! {
            () = cancel.cancelled() => Ok(false),
            result = async {
                self.writer.write_all(&encoded).await?;
                self.writer.flush().await
            } => result.map(|()| true).map_err(RpcTransportError::Io),
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.pending.values().map(|pending| pending.deadline).min()
    }

    fn expire(&mut self, now: Instant) {
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            if let Some(pending) = self.pending.remove(&id) {
                let _ = pending.reply.send(Err(RpcError::Timeout));
            }
        }
        self.update_pending_count();
    }

    fn drain_pending(&mut self, disconnected: bool) {
        for (_, pending) in self.pending.drain() {
            let error = if disconnected {
                RpcError::Disconnected
            } else {
                RpcError::Closed
            };
            let _ = pending.reply.send(Err(error));
        }
        self.update_pending_count();
    }

    fn update_pending_count(&mut self) {
        self.pending_count.send_replace(self.pending.len());
    }
}
