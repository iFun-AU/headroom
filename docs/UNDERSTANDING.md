# Implementation understanding

## Product in five lines

1. How Is It is a macOS 26+ menu-bar app that shows Claude and Codex plan-limit usage.
2. Session and weekly percentages use real-time push where possible, with Codex polling and file events as fallback.
3. Token charts report observed activity, labelled honestly as either this Mac or the Codex account; they never imply a token quota.
4. The product has a main dashboard, a tray popover, and an optional always-on-top widget, all backed by one Rust store.
5. v1 is a local universal build with notifications, settings, onboarding and a consent-based Claude status-line bridge; Claude OAuth is not built.

## Data sources

| Source | Provides | Update mode | Limit priority |
|---|---|---|---|
| Codex app-server | Codex limit windows, plan, account-wide completed UTC-day totals | JSON-RPC push plus scheduled/on-demand reads | 0 — primary |
| Codex rollout files | Codex limit windows and local token deltas | FSEvents-triggered file tail; also triggers an app-server read | 1 — fallback |
| Claude status-line bridge file | Claude limit windows | Claude invokes bridge; FSEvents pushes the written snapshot into the app | 0 — primary |
| Claude local logs | Local Claude Code token history and activity evidence | Recursive file watch/tail | History only |
| Claude OAuth | Claude limit windows | Poll | Post-v1 only; not compiled in v1 |

Source priority is explicit through `SourceKind::limit_priority`; lower is more authoritative. History-series selection is separate from limit priority. (`DEVELOPMENT.md` §§2.1–2.6, 7.1, 7.5.)

## One Codex update, end to end

1. `usage-sources::codex::app_server` owns a `codex app-server` child with piped stdio, `kill_on_drop`, and a stderr drain.
2. `codex::rpc` reads a newline-delimited JSON-RPC notification or response, correlates requests, and forwards supported notifications.
3. The lenient app-server DTO parser applies the `limitId == "codex"` filter, classifies windows by duration, and creates a strict `Reading`.
4. The source sends a bounded `SourceEvent::Reading` to the `UsageStore` actor.
5. The store ingests that source independently, applies full/partial and ordering rules, derives a `UsageSnapshot`, and updates its watch channel only when the snapshot changes.
6. The Tauri forwarder coalesces updates to at most one per 250 ms, emits `usage-updated`, and updates the tray text/icon only when needed.
7. `ui/src/ipc.ts` owns the typed Tauri listener; `useSnapshot` updates React state and the dashboard, popover and widget re-render from the generated Rust bindings.

A rollout-file event follows the same parser → bounded `SourceEvent` → store path and additionally triggers an app-server refresh so independent Codex activity is reconciled. (`DEVELOPMENT.md` §§2.1, 3.1–3.3, 8.1–8.2, 9.1, 10.3–10.4.)

## Windows and routes

| Window | Size | Behavior/routes |
|---|---:|---|
| `main` | 720×520; minimum 560×420 | `#/` Overview, `#/claude`, `#/codex`, `#/settings`, `#/onboarding`; Settings and Onboarding are routes, not extra windows |
| `popover` | 340×420 | Tray-left-click popover; fixed size; hides when focus is lost |
| `widget` Pill | 280×72 | Always on top, draggable, edge snapping; hover controls remain inside the same bounds |
| `widget` Stack | 160×180 | Same window resized for this variant |
| `widget` Mini | 200×24 | Two thin bars; hover values render inline |

There are exactly three native windows. (`DEVELOPMENT.md` §§10.1–10.2, 11.4.)

## Settings fields and defaults

| Field | Default / rule |
|---|---|
| `schema_version` | `1` |
| `onboarding_completed` | `false` |
| `codex_path`, `codex_home`, `claude_dir` | `None` (automatic discovery/default paths) |
| `claude_bridge_enabled` | `false` |
| `claude_oauth_enabled` | `false`; ignored because v1 does not compile the feature |
| `thresholds` | `[75, 90, 100]`, sorted/deduplicated and clamped to 1…100 |
| `notify_on_reset` | `true` |
| `launch_at_login` | `false` |
| `show_dock_icon` | `false` |
| `widget.visible` | `false` |
| `widget.x`, `widget.y` | `None` until a position is saved |
| `widget.variant` | `Pill` |
| `widget.opacity` | `1.0`, validated to 0.4…1.0 |
| `poll_active_secs` | `120`, minimum 60 |
| `poll_idle_secs` | `600`, minimum 120 |

The specification does not state defaults or concrete coordinate types for the nested widget settings. The non-intrusive defaults and nullable integer coordinates are recorded in D-001. All other defaults are from `DEVELOPMENT.md` §9.3.

## Required answers

1. **Why duration classification?** Codex's `primary` and `secondary` positions are not semantic: a verified Pro response put the 10,080-minute weekly window in `primary` and had no `secondary`. Classify 240…360 minutes as Session and 9,000…11,000 as Weekly, using a provider hint only when duration is absent. (`DEVELOPMENT.md` §§2.1, 7.2.)
2. **Stale app-server versus fresh rollout:** the fresh rollout becomes authoritative because the 20-minute-old app-server data exceeds the 15-minute freshness limit. The provider shows `Degraded`, retaining data while signalling that the higher-priority source is not healthy enough to lead. (`DEVELOPMENT.md` §7.4 D1 and D4; required test e.)
3. **Full versus partial:** a full reading replaces that source's complete window map, so absent windows disappear. A partial reading upserts only present windows, preserves absent windows and plan, and rejects pre-rollover values with an earlier reset. (`DEVELOPMENT.md` §§7.3–7.4 I1–I2.)
4. **Bridge output and atomicity:** with at least one rate-limit window, it writes schema 1, `writtenAt`, `sessionId`, and the verbatim `rateLimits` object to `<out-dir>/claude-rate-limits.json` (production app support). Each overlapping invocation creates a unique same-directory `NamedTempFile`, writes and `sync_all`s it, then atomically `persist`s it; a reader therefore sees an old or complete new file, never a partial one. (`DEVELOPMENT.md` §8.3.)
5. **Uninstall after user edits:** re-read current `settings.json`; if the current `statusLine.command` is no longer ours, leave it untouched and report that. Otherwise restore exactly the saved prior `statusLine` (or remove it if originally absent) while retaining every other current key. The backup is manual recovery only. (`DEVELOPMENT.md` §8.3 installer step 7.)
6. **Fork-safe Codex counting:** per rollout file, add the first non-null event's `last_token_usage.total_tokens`; later add `total - previous` when increasing, `last_token_usage.total_tokens` on a counter decrease, and zero for an equal repeat. Appendix A.6 totals **3,000 tokens**. (`DEVELOPMENT.md` §2.2 and Appendix A.6.)
7. **Why the account chart ends yesterday:** `account/usage/read` reports completed UTC calendar days and omits the current UTC day; absence is not zero. Keep those dates in UTC and label the final column by date such as `Sep 22`, never `Today`. (`DEVELOPMENT.md` §§2.1, 7.5.)
8. **Threshold visuals:** 0–74% uses the provider accent and no severity icon; 75–89% uses amber `--warn` plus a warning triangle; 90–99% uses magenta-red `--crit` plus the critical octagon/exclamation; 100% keeps `--crit`, adds the hourglass/limit-reached signal, static halo or 1.8-second pulse, and reset countdown. The numeric percentage and wording remain visible so colour is never the only signal. (`DEVELOPMENT.md` §11.2; design prompt §§5.3, 7; `States*` and `Accessibility.png`.)
9. **Why animate width:** `scaleX` deforms capsule ends, the bright core, and the specular head. With at most eight periodically changing bars, animating layout width is an acceptable cost and preserves the approved geometry. (`DEVELOPMENT.md` §11.2.)
10. **Widget hover boundary:** Pill and Stack replace or overlay content with close/opacity/expand controls; Mini shows values inline. Nothing may increase or draw outside the exact variant window because transparent-window content is clipped and the native window must retain its fixed hit area and snap geometry. (`DEVELOPMENT.md` §§10.2, 11.4; design D7.)
11. **`claude-oauth`:** it gates the undocumented Anthropic usage poller and Keychain credential access described for M9. M9 requires owner sign-off and is explicitly excluded from v1, so neither it nor its `reqwest`/`security-framework` dependencies are built. (`DEVELOPMENT.md` §§1, 2.4, 8.4, 13 M9, 15.)
12. **Logging:** never log, print, persist or commit OAuth/access tokens, `auth.json` contents, account IDs, passwords, credentials, bearer headers or other secrets. Child stderr, file content and HTTP bodies must pass through UTF-8-safe `log_trunc`, capped at 2 KiB, before logging. (`DEVELOPMENT.md` §§0, 6.3, 6.5.)
13. **Handover:** every §14.4 human acceptance item; the real bridge enable/use/override/uninstall checks; live app-server behavior that cannot be exercised without real account data; the 8-hour soak and Safari WebView memory cycle; WKWebView Reduce Transparency verification; native glass/tray/widget comparison; the real log-secret grep and `/Applications` install/smoke check because both touch files outside repo/temp; and optional T8.4 signing/notarization. (`DEVELOPMENT.md` §§0.1, 14.3–14.4, 15; design prompt §7.)

## Open verification items

| Item | Task | Rule and planned outcome |
|---|---|---|
| Codex app-server is experimental and may reject `initialize`/methods | T5.1/T5.6 | Regenerate its schema without reading account data; implement documented DTOs plus fake-RPC tests. A live account check touches real user data, so list it in handover under §0.1. |
| Older Codex uses `resets_in_seconds` | T2.2 | Implement the documented fallback and a fixture test; an old live binary is unavailable, so retain the compatibility path and record the unrun live check. |
| Claude OAuth response shape | T9.1 | M9 is excluded. Do not access Keychain or the endpoint; carry the gated verification into the post-v1 handover notes. |
| WKWebView honors `prefers-reduced-transparency` on macOS 26 | T7.5 | Implement the media rule and `.no-glass` fallback, test the deterministic class path, and put the native manual check on handover. Do not add a workaround dependency if the query is unsupported. |

## Toolchain preflight

Recorded on 2026-09-23 in `the repository root`:

```text
$ sw_vers -productVersion
27.0

$ xcode-select -p
/Applications/Xcode.app/Contents/Developer

$ rustc --version && cargo --version
rustc 1.95.0 (59807616e 2026-04-14)
cargo 1.95.0 (f2d3ce0bd 2026-03-21)

$ rustup target list --installed
aarch64-apple-darwin
```

The Intel standard library was missing, so the permitted no-sudo install was run:

```text
$ rustup target add x86_64-apple-darwin
info: downloading component rust-std

$ rustup target list --installed
aarch64-apple-darwin
x86_64-apple-darwin
```

```text
$ node --version && npm --version
v25.9.0
11.12.1

$ codex --version
WARNING: proceeding, even though we could not create PATH aliases: Operation not permitted (os error 1)
codex-cli 0.154.0

$ claude --version
2.1.273 (Claude Code)
```

The OS, Xcode tools, Rust and Node meet the stated minimums. Both provider CLIs are present. Their presence does not authorize reading real account/user data; automated work still uses fixtures, fakes and temp directories.
