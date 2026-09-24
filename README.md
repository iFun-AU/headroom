# How Is It

How Is It is a macOS menu-bar app that shows Claude and Codex session limits, weekly limits, and recent token activity. It provides a compact popover, a full dashboard, an optional floating widget, and threshold/reset notifications.

> **Local build only.** Version 1 is ad-hoc signed, not Developer ID signed or notarized, and is not a distributable release. Build and use it on the same Mac. The app requires macOS 26 or later.

## What it reads

- **Codex:** starts one owned `codex app-server` process for account limits and completed daily totals, and reads rollout JSONL files for near-real-time local activity and fallback limits. It never reads `.codex/auth.json`.
- **Claude:** reads local Claude Code conversation logs for token activity. Plan-limit updates are available only when you explicitly enable the status-line bridge described below.
- **Claude usage API (optional, unofficial):** only in builds with the `claude-oauth` feature, and only after you turn on **Settings → Accounts → Claude usage API**. It then reads Claude Code's sign-in from the Keychain (read-only) and polls Anthropic's undocumented usage endpoint every few minutes. Default builds never read the credential or call the endpoint.

Provider data, settings, and logs stay on the Mac. Codex authentication remains owned by Codex.

## Requirements

- macOS 26+
- Xcode Command Line Tools
- Rust 1.95+ with both Apple targets
- Node.js and npm
- Codex CLI and/or Claude Code for the provider you want to monitor

The verified development baseline is macOS 27.0, Codex CLI 0.154.0, and Claude Code 2.1.273. `codex app-server` is experimental; if it becomes incompatible, How Is It keeps rollout-file data where possible and marks Codex as degraded.

## Build the universal app

From the repository root, one command builds, ad-hoc signs and verifies the app (add `--install` to replace `/Applications/How Is It.app` and launch it):

```bash
scripts/build-app.sh
```

Options: `--no-oauth` builds without the Claude usage API, `--icons` regenerates the bundle icons from `src-tauri/icons/app-icon.svg` (needs `brew install librsvg`), and `--clean` removes build caches and temp files for a full rebuild. The script checks prerequisites and fails with the fix if one is missing.

The equivalent manual steps:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
scripts/build-sidecar.sh universal-apple-darwin
cargo tauri build --target universal-apple-darwin --features claude-oauth
codesign --force --deep -s - \
  "target/universal-apple-darwin/release/bundle/macos/How Is It.app"
codesign --verify --deep --strict --verbose=2 \
  "target/universal-apple-darwin/release/bundle/macos/How Is It.app"
```

The local app is at:

```text
target/universal-apple-darwin/release/bundle/macos/How Is It.app
```

Both `how-is-it` and its bundled `howisit-statusline` sidecar should report `x86_64 arm64` with `lipo -archs`.

## Install and first run

After building and signing, copy the app to `/Applications`:

```bash
ditto \
  "target/universal-apple-darwin/release/bundle/macos/How Is It.app" \
  "/Applications/How Is It.app"
```

Open **How Is It** from Finder and complete onboarding:

1. Confirm the detected Codex CLI. GUI apps do not always inherit an interactive shell's `PATH`; use **Browse…** if the automatic path is wrong.
2. Optionally enable real-time Claude limits. This is the only onboarding step that changes Claude settings.
3. Choose **Allow Notifications** if you want threshold and reset alerts. If permission was denied, restore it in **System Settings → Notifications → How Is It**. Alerts also respect Focus settings.

Because this build is not notarized, macOS may require the local-build open flow. Do not redistribute the bundle as a production installer.

## Claude status-line bridge

Enabling **Settings → Accounts → Claude → Real-time updates** performs a consented, local mutation:

- copies the bundled bridge to `~/Library/Application Support/dev.howisit.app/bin/howisit-statusline`;
- backs up `~/.claude/settings.json` beside the original as `settings.json.howisit-backup-<timestamp>`;
- adds or updates the top-level `statusLine` command while preserving every unrelated setting;
- preserves and chains an existing status-line command; and
- writes bridge state and rate-limit snapshots under How Is It's Application Support directory.

Claude Code hides most footer keyboard hints while any custom status line is configured. Claude supplies plan limits only for eligible plans and only after the first API response in a session.

Claude Code runs status-line commands only in its interactive terminal UI (`claude` in a terminal). Sessions in the Claude desktop app, IDE extensions, `claude -p`, and the Agent SDK never invoke the bridge, so plan limits update only while you use Claude Code in a terminal. Limits are account-wide, so one terminal response also reflects usage from those other clients. When only such clients have been active, the app reports **Terminal Only** instead of **Likely Overridden**.

To uninstall the bridge, choose **Disable** in the same Accounts pane before removing the app. How Is It re-reads the current settings, restores the exact prior `statusLine` value, and retains unrelated edits made after installation. If another tool or person has replaced the command, How Is It leaves the file untouched instead of overwriting that newer choice.

Project-local, organization, managed, or server settings can take precedence over the user-level `~/.claude/settings.json`. In that case the bridge can be installed but not invoked; after continued Claude activity without bridge writes, the app reports **Likely Overridden** and shows a hint. Resolve the higher-precedence setting rather than repeatedly reinstalling the bridge.

## Claude usage API

Use this when you run Claude Code in the Claude desktop app or an IDE, where the status-line bridge never runs. It works only in builds made with `--features claude-oauth` (the build command above includes it); omit the flag to build without any Keychain or network access for Claude.

- The endpoint is unofficial and may change or disappear without notice; the app then shows **Unsupported** for Claude and stops polling until you toggle the setting or restart.
- The first poll shows a macOS prompt to allow access to `Claude Code-credentials`. Choose **Always Allow**. If you deny it, polling stops until you turn the setting off and on.
- The token is only read, never refreshed, stored, or logged. If it has expired, the app shows **Sign-in expired**; open Claude Code to refresh it.
- Polls skip while the status-line bridge delivered data in the last three minutes, and back off on errors and rate limits. Opening the popover never forces an extra poll.

## Everyday use

- Click the menu-bar item for the compact popover.
- Open the dashboard for 24-hour and seven-day activity views.
- Configure thresholds, reset alerts, launch-at-login, Dock visibility, source paths, and the floating widget in Settings.
- Use **Settings → Diagnostics → Reveal Logs** for local diagnostics. Logs are stored in `~/Library/Logs/dev.howisit.app` and retained for seven days.

When a source is unavailable, the app shows **Not Configured** or **Degraded** rather than inventing usage. Claude limit bars become stale when Claude Code has not produced a bridge update for 15 minutes; this is expected in v1.

## Development checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix ui run typecheck
npm --prefix ui run lint
npm --prefix ui run build
```

Implementation details, verified data shapes, and acceptance criteria are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). Recorded deviations and evidence are in [docs/DECISIONS.md](docs/DECISIONS.md). Human-only installation, live-provider, native-UI, eight-hour soak, and optional distribution checks are in [docs/HANDOVER.md](docs/HANDOVER.md).
