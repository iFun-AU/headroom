//! Claude Code integration.

/// Semantic status-line bridge installation and removal.
pub mod bridge_install;
/// Native source for persisted bridge rate limits.
pub mod bridge_source;
/// Behavioral bridge effectiveness evidence.
pub mod effectiveness;
/// Recursive Claude Code local-history source.
pub mod history;
/// Read-only Claude Code OAuth credential access.
#[cfg(feature = "claude-oauth")]
pub mod keychain;
/// Opt-in poller for Claude's unofficial OAuth usage endpoint.
#[cfg(feature = "claude-oauth")]
pub mod oauth;
