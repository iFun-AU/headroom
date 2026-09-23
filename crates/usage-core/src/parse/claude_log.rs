use serde::Deserialize;

use super::{Error, Result, parse_timestamp};
use crate::{Provider, SourceKind, TokenCount, TokenEvent};

const ASSISTANT_TYPE: &str = "assistant";
/// Entrypoint Claude Code records for its interactive terminal UI.
const INTERACTIVE_ENTRYPOINT: &str = "cli";

/// Kind of Claude Code client that wrote a conversation-log record.
///
/// Claude Code runs a configured status-line command only in its interactive
/// terminal UI, so only [`Self::Interactive`] sessions can invoke the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeSession {
    /// Interactive terminal session (`entrypoint` is `cli`), or a log written
    /// before Claude Code recorded an entrypoint.
    Interactive,
    /// Client without a status line: the Claude desktop app, IDE extensions,
    /// `claude -p`, and SDK or other embedded entrypoints.
    Headless,
}

/// One Claude token event together with the client that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeLogRecord {
    /// Token usage for history aggregation.
    pub event: TokenEvent,
    /// Whether the producing session could have run the status line.
    pub session: ClaudeSession,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LogLineDto {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    entrypoint: Option<String>,
    message: Option<MessageDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MessageDto {
    id: Option<String>,
    usage: Option<UsageDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UsageDto {
    #[serde(rename = "input_tokens")]
    input: Option<u64>,
    #[serde(rename = "output_tokens")]
    output: Option<u64>,
    #[serde(rename = "cache_creation_input_tokens")]
    cache_creation: Option<u64>,
    #[serde(rename = "cache_read_input_tokens")]
    cache_read: Option<u64>,
}

/// Parses one Claude Code conversation-log JSONL record into a token event.
///
/// Only assistant records with a `message.usage` object produce token events.
/// Unknown fields and unrelated record types are ignored.
///
/// # Errors
///
/// Returns an error for invalid JSON, a missing or invalid timestamp on a
/// relevant assistant record, or an overflowing token-component sum.
pub fn parse_claude_log_line(input: &str) -> Result<Option<TokenEvent>> {
    parse_claude_log_record(input).map(|record| record.map(|record| record.event))
}

/// Parses one Claude Code conversation-log JSONL record, keeping the session
/// kind needed by the status-line bridge effectiveness check.
///
/// # Errors
///
/// Same as [`parse_claude_log_line`].
pub fn parse_claude_log_record(input: &str) -> Result<Option<ClaudeLogRecord>> {
    let dto: LogLineDto = serde_json::from_str(input)?;
    if dto.kind.as_deref() != Some(ASSISTANT_TYPE) {
        return Ok(None);
    }

    let Some(message) = dto.message else {
        return Ok(None);
    };
    let Some(usage) = message.usage else {
        return Ok(None);
    };
    let timestamp = dto
        .timestamp
        .as_deref()
        .ok_or(Error::MissingField("timestamp"))?;
    let tokens = usage.total()?;
    let dedupe_key = message
        .id
        .zip(dto.request_id)
        .map(|(message_id, request_id)| format!("{message_id}:{request_id}"));

    let session = match dto.entrypoint.as_deref() {
        None | Some(INTERACTIVE_ENTRYPOINT) => ClaudeSession::Interactive,
        Some(_) => ClaudeSession::Headless,
    };

    Ok(Some(ClaudeLogRecord {
        event: TokenEvent {
            provider: Provider::Claude,
            source: SourceKind::ClaudeLocalLogs,
            at: parse_timestamp(timestamp)?,
            tokens: TokenCount(tokens),
            dedupe_key,
        },
        session,
    }))
}

impl UsageDto {
    fn total(&self) -> Result<u64> {
        [
            self.input,
            self.output,
            self.cache_creation,
            self.cache_read,
        ]
        .into_iter()
        .flatten()
        .try_fold(0_u64, u64::checked_add)
        .ok_or(Error::TokenOverflow)
    }
}
