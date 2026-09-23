//! Cross-provider malformed-input regression tests.

use usage_core::{
    UnixSeconds,
    parse::{
        CodexRolloutParser, parse_claude_log_line, parse_claude_statusline,
        parse_codex_rate_limits_notification, parse_codex_rate_limits_response,
    },
};

#[test]
fn malformed_json_is_reported_by_every_parser_entry_point() {
    let malformed = "{";
    let observed_at = UnixSeconds(1_790_000_000);
    let mut rollout = CodexRolloutParser::new();

    assert!(parse_codex_rate_limits_response(malformed, observed_at).is_err());
    assert!(parse_codex_rate_limits_notification(malformed, observed_at).is_err());
    assert!(rollout.parse_line(malformed).is_err());
    assert!(parse_claude_statusline(malformed, observed_at).is_err());
    assert!(parse_claude_log_line(malformed).is_err());
}
