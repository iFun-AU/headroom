use serde::Deserialize;

use super::{Error, Result, parse_timestamp};
use crate::{Provider, SourceKind, TokenCount, TokenEvent};

const ASSISTANT_TYPE: &str = "assistant";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LogLineDto {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
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

/// Parses one Claude Code conversation-log JSONL record.
///
/// Only assistant records with a `message.usage` object produce token events.
/// Unknown fields and unrelated record types are ignored.
///
/// # Errors
///
/// Returns an error for invalid JSON, a missing or invalid timestamp on a
/// relevant assistant record, or an overflowing token-component sum.
pub fn parse_claude_log_line(input: &str) -> Result<Option<TokenEvent>> {
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

    Ok(Some(TokenEvent {
        provider: Provider::Claude,
        source: SourceKind::ClaudeLocalLogs,
        at: parse_timestamp(timestamp)?,
        tokens: TokenCount(tokens),
        dedupe_key,
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
