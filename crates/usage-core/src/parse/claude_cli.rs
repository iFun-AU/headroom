//! Parse the rendered `/usage` screen, never a transcript or raw ANSI stream.

use thiserror::Error;

use super::claude_cli_reset::reset_time;
use crate::{LimitWindow, Percent, Provider, Reading, SourceKind, UnixSeconds, WindowKind};

/// Safe, fixed diagnostics from Claude's terminal; provider text is never returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ClaudeCliError {
    /// The usage endpoint declined another request.
    #[error("Claude is rate limited. Wait five minutes before refreshing again.")]
    RateLimited,
    /// Claude Code needs the user's normal sign-in or first-run setup.
    #[error("Open Claude Code once and finish signing in, then refresh again.")]
    SignInRequired,
    /// The installed CLI cannot run the isolated probe.
    #[error("Update Claude Code to use background usage refresh.")]
    UnsupportedVersion,
    /// The plan or terminal format did not provide usable subscription limits.
    #[error("Claude Code did not report plan limits. Check /usage in Claude Code.")]
    Unavailable,
}

/// Extracts independent session, all-model weekly, and Fable weekly limits.
/// Missing or malformed windows are omitted, never converted to zero. Unknown
/// reset formats retain the percentage with no inferred reset time.
///
/// # Errors
/// Returns a safe diagnostic for recognized authentication, rate-limit, or CLI errors.
pub fn parse_claude_cli_usage(
    screen: &str,
    observed_at: UnixSeconds,
) -> Result<Option<Reading>, ClaudeCliError> {
    let lower = screen.to_ascii_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.contains("rate_limit_error") || compact.contains("ratelimited") {
        return Err(ClaudeCliError::RateLimited);
    }
    if [
        "notloggedin",
        "pleaselogin",
        "pleasesignin",
        "tokenexpired",
        "run/login",
        "welcometoclaudecode",
        "choosethetextstyle",
        "selectloginmethod",
    ]
    .iter()
    .any(|needle| compact.contains(needle))
    {
        return Err(ClaudeCliError::SignInRequired);
    }
    if compact.contains("unknownoption") || compact.contains("unrecognizedoption") {
        return Err(ClaudeCliError::UnsupportedVersion);
    }
    if compact.contains("error:") || compact.contains("failedtoloadusage") {
        return Err(ClaudeCliError::Unavailable);
    }
    // A cached panel can remain on screen during a fresh request. Never label it fresh.
    if compact.contains("loadingusage") {
        return Ok(None);
    }
    let lines: Vec<_> = screen.lines().map(str::trim).collect();
    let mut windows = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let label = line.to_ascii_lowercase();
        let kind = if label.starts_with("current session") {
            WindowKind::Session
        } else if label == "current week" || label.starts_with("current week (all models)") {
            WindowKind::Weekly
        } else if label == "current week (fable)" || label == "current week (fable only)" {
            WindowKind::Fable
        } else {
            continue;
        };
        let section: Vec<_> = lines[index..]
            .iter()
            .take(6)
            .enumerate()
            .take_while(|(offset, text)| {
                let text = text.to_ascii_lowercase();
                *offset == 0
                    || !(text.starts_with("current ")
                        || text.starts_with("extra usage")
                        || text.starts_with("claude code and cowork credit"))
            })
            .map(|(_, text)| *text)
            .collect();
        // Don't mistake a separately rendered model-specific heading for all models.
        if kind == WindowKind::Weekly
            && section.iter().any(|text| {
                let text = text.to_ascii_lowercase();
                text.contains("only)") || text == "sonnet" || text == "opus"
            })
        {
            continue;
        }
        let Some(used) = section.iter().find_map(|text| percentage(text)) else {
            continue;
        };
        if windows
            .iter()
            .any(|window: &LimitWindow| window.kind == kind)
        {
            return Err(ClaudeCliError::Unavailable);
        }
        let resets_at = section
            .iter()
            .find_map(|text| reset_time(text, observed_at, kind));
        windows.push(LimitWindow {
            kind,
            used,
            resets_at,
            reset_pending: false,
            source: SourceKind::ClaudeCli,
            observed_at,
        });
    }
    if windows.is_empty() {
        return Ok(None);
    }
    Ok(Some(Reading {
        provider: Provider::Claude,
        source: SourceKind::ClaudeCli,
        observed_at,
        plan: None,
        windows,
        partial: false,
        credits: None,
    }))
}

fn percentage(line: &str) -> Option<Percent> {
    let (before, after) = line.split_once('%')?;
    let after = after.trim().to_ascii_lowercase();
    let is_left = after.starts_with("left") || after.starts_with("remaining");
    if !is_left && !after.starts_with("used") {
        return None;
    }
    let digits: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let value: f64 = digits.parse().ok()?;
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return None;
    }
    Percent::new(if is_left { 100.0 - value } else { value }).ok()
}
