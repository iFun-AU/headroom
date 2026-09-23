//! JSON-RPC transport value objects and internal commands.

use std::io;

use serde_json::Value;
use thiserror::Error;
use tokio::{sync::oneshot, time::Instant};
use usage_core::log_trunc;

/// One server notification, retaining the complete bounded JSON message.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcNotification {
    /// Notification method.
    pub method: String,
    /// Complete message including method and params.
    pub message: Value,
}

/// Failure reported to a request or notification caller.
#[derive(Debug, Error)]
pub enum RpcError {
    /// The transport actor has stopped.
    #[error("RPC transport is closed")]
    Closed,
    /// The peer closed its stream.
    #[error("RPC peer disconnected")]
    Disconnected,
    /// The request exceeded its configured deadline.
    #[error("RPC request timed out")]
    Timeout,
    /// The fixed pending-request bound was reached.
    #[error("RPC pending request limit reached")]
    Busy,
    /// The peer returned a JSON-RPC error.
    #[error("RPC error {code}: {message}")]
    Remote {
        /// JSON-RPC error code.
        code: i64,
        /// Bounded peer-supplied error message.
        message: String,
    },
}

/// Fatal transport-loop failure.
#[derive(Debug, Error)]
pub enum RpcTransportError {
    /// Reading or writing the child pipes failed.
    #[error("RPC I/O failed: {0}")]
    Io(#[from] io::Error),
    /// The peer closed stdout.
    #[error("RPC peer disconnected")]
    Disconnected,
    /// An outgoing JSON object could not be encoded.
    #[error("RPC JSON encoding failed: {0}")]
    Json(#[from] serde_json::Error),
    /// An outgoing message exceeded the same bound as incoming messages.
    #[error("outgoing RPC message exceeded 1 MiB")]
    OutgoingTooLarge,
}

pub(super) struct Pending {
    pub(super) deadline: Instant,
    pub(super) reply: oneshot::Sender<Result<Value, RpcError>>,
}

pub(super) enum Command {
    Request {
        id: u64,
        method: String,
        params: Value,
        reply: oneshot::Sender<Result<Value, RpcError>>,
    },
    Notify {
        method: String,
        params: Option<Value>,
        reply: oneshot::Sender<Result<(), RpcError>>,
    },
}

pub(super) fn remote_error(value: &Value) -> RpcError {
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
    let message = value.get("message").and_then(Value::as_str).map_or_else(
        || "remote error".to_owned(),
        |text| log_trunc(text).into_owned(),
    );
    RpcError::Remote { code, message }
}
