# Headroom — Development Guide

> **Audience:** an AI coding agent (or junior engineer) that will implement this app step by step.
> **Author:** AI Assistant · **Created:** 2026-09-23 · **Modified:** 2026-09-23 (v2.2, final; see §17)
> **Status:** **Final (v2.2). Approved for autonomous implementation of M0–M8 end to end**, with no further sign-off gates. Excluded: M9 (Claude OAuth) and T8.4 (distribution signing), which need the owner. Decision rules are in §0.1.
> **Target:** macOS **26.0 minimum** (Liquid Glass; no older-macOS support), **universal binary** (Apple Silicon + Intel), Rust 1.95+, Tauri 2.11+

---

## 0. How to use this document (read first)

You are implementing a macOS menu bar app that shows Claude and Codex plan usage. Follow these rules **exactly**:

1. **Work milestone by milestone (§13), task by task.** Do not start a task until the previous task's *Verify* commands pass.
2. **Never invent API fields, endpoints, or file formats.** Every external format you need is specified in §2 and in the fixtures in Appendix A. If reality differs from this document, **don't guess and don't stop**: follow the "Reality differs" rule in §0.1.
3. **Items marked `⚠ VERIFY` are not yet confirmed.** Run the probe step described next to them before you write code that depends on them, then apply the "⚠ VERIFY" rule in §0.1.
4. **Keep each change small.** No file over ~400 lines, and no function over ~60 lines. Put pure logic in `usage-core` and keep I/O out of it.
5. **After every task, run:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
   After touching `ui/`:
   ```bash
   npm --prefix ui run typecheck && npm --prefix ui run lint
   ```
6. **Do not add dependencies** beyond §4 without writing down why in `docs/DECISIONS.md`.
7. **Never log, print, or persist secrets** (OAuth tokens, `auth.json` contents, account IDs).
8. **Every public Rust item gets a `///` doc comment**, and every TS module gets a header comment (see the project's global doc standard).

### 0.1 Autonomy and decision rules (so M0–M8 can finish without waiting)

The implementing agent is expected to work through M0–M8 to the Definition of Done (§15) **without pausing for approval**. When something isn't covered, use the rule below, write a `docs/DECISIONS.md` entry (context, choice, reason, evidence), and **continue**.

| Situation | Rule |
|---|---|
| **Reality differs from §2** (a field is renamed, missing or extra; a different shape) | Keep the documented *behavior*. Make the DTO lenient enough for both the documented and the observed shape, add the observed shape as a new fixture next to the documented one, and log it. Never read secrets to investigate. Print key paths and types only. |
| **⚠ VERIFY item** | Run the probe or check. If it matches, note "verified" in `DECISIONS.md`. If it differs, apply the row above. If it can't run in this environment, implement the documented behavior, add a test with the documented fixture, and put the item on the handover checklist (§15). |
| **A source is unavailable on this machine** (e.g. no `codex` binary, no Claude Code) | Build and test against fixtures and fakes (tempdirs, `tokio::io::duplex`). The app must show `NotConfigured` for that source. Put the live check on the handover checklist. |
| **Spec is silent on a detail** | Choose the option that is simplest, keeps the documented UI and behavior, and needs no new setting, IPC type or dependency. Log it. |
| **A dependency seems necessary** that §4 doesn't list | First try `std` or an existing dependency. A **dev-dependency** for tests only (e.g. a property-testing crate) may be added with a log entry. A new **runtime** dependency is allowed only if it's widely used, maintained, needs no network or crypto, and adds no `unsafe` to our code. Log it. Otherwise implement it yourself. |
| **An IPC or settings type seems to need a change** | Don't change the public contract (§7.1, §9.3, §10.3). Implement inside the existing types and log the limitation. The only allowed changes are additive, backward-compatible fields with defaults, logged with the reason. |
| **Design and spec disagree** | Data and behavior follow this document; visuals follow `docs/design/`. Log it. |
| **A check fails and the cause is unclear** | Make at most three focused attempts. If it's still failing, isolate it with a smaller reproduction test, fix the root cause, and log what you learned. Never weaken a test or lint to pass. |
| **The step needs a human** (passwords, `sudo`, Apple credentials, GUI permission prompts, using Claude Code or Codex interactively, sleeping the Mac, an 8-hour soak) | Do everything that can be automated. Record the manual step with exact instructions on the **handover checklist** (§15), then move on. |

**Hard stops** (the only reasons to halt and ask the owner):
1. The next step would read, modify or delete **real user data**: the real `~/.claude/settings.json`, `~/.codex/auth.json`, Keychain items, or files outside the repo and temp dirs.
2. It would need a **secret, password or credential**.
3. The required toolchain can't be installed without `sudo` or a GUI prompt (Xcode, the Command Line Tools, or macOS < 26).

Before halting, record the blocker in `docs/PROGRESS.md`.

**Resuming:** work may span many sessions. `docs/PROGRESS.md` is the source of truth for where work stopped. Always resume from the first unchecked task.

---

## 1. Product summary

**Headroom** shows, at a glance, how much of the user's **Claude** (Claude Code / claude.ai plan) and **Codex** (ChatGPT plan) rate limits have been used:

| Metric | Meaning | Shown as |
|---|---|---|
| Session limit | Rolling **5-hour** window, % used + reset time | Neon progress bar + countdown |
| Weekly limit | Rolling **7-day** window, % used + reset time | Neon progress bar + countdown |
| Hourly activity | Tokens per hour, last 24 h (this Mac) | Bar chart |
| Daily activity | Tokens per day, last 7 days | Bar chart |

**Surfaces:** main dashboard window, menu bar icon + popover, and an always-on-top floating widget.
**Update model:** **real-time push wherever the tools support it**, with **interval polling as a fallback** (§3.3).

### In scope (v1)
- macOS 26+ only. One Claude account and one Codex account (whichever is signed in locally).
- Liquid Glass windows and neon progress bars (see the design brief in Appendix B).
- Threshold notifications (75 / 90 / 100 %) and reset notifications.
- Launch at login and a Dock icon toggle.
- A local universal `.app` build (ad-hoc signed). Distribution signing is optional (T8.4).

### Post-v1 (specified, but NOT built in v1)
- **Claude OAuth poller** (§2.4, §8.4, milestone M9). It uses an undocumented endpoint. It is compiled only with the cargo feature `claude-oauth` (off by default) and only after the owner explicitly accepts its policy, response shape, and failure semantics in `DECISIONS.md`.

### Out of scope (do NOT build)
- macOS < 26, Windows or Linux builds, multiple accounts per provider, API-key (Console) billing, claude.ai web chat history, the App Store (the glass plugin uses a private API), and auto-update.
- Non-`codex` Codex limit IDs (e.g. `"premium"`). They are filtered out in v1 and logged at `debug` (§2.1).

---

## 2. Verified facts about the data sources

These were verified on 2026-09-23 against **Claude Code 2.1.273**, **codex-cli 0.154.0**, and **macOS 27.0**. Treat this section as the contract.

### 2.1 Codex: `codex app-server` (PUSH + on-demand read), primary source

- **Transport:** spawn `codex app-server` as a child process. Protocol is **newline-delimited JSON-RPC over stdio**, one JSON object per line. The `"jsonrpc":"2.0"` field is **not required** (verified working without it).
- **Handshake (verified):**
  1. Send request: `{"id":1,"method":"initialize","params":{"clientInfo":{"name":"headroom","title":"Headroom","version":"<app version>"}}}`
  2. Wait for the response with `id:1`. Its `result` has keys `userAgent`, `codexHome`, `platformFamily`, and `platformOs`.
  3. Send notification: `{"method":"initialized"}`
- **Read rate limits (verified):** request `account/rateLimits/read` with params `{"excludeResetCreditDetails":true}`. The response is in Appendix A.2. Important fields:
  - `result.rateLimits`: `RateLimitSnapshot` (backward-compatible single bucket)
  - `result.rateLimitsByLimitId`: map keyed by `limitId` (e.g. `"codex"`)
  - `RateLimitSnapshot.primary` / `.secondary`: `RateLimitWindow | null`
  - `RateLimitWindow = { usedPercent: int, windowDurationMins: int|null, resetsAt: int|null (unix seconds) }`
  - `planType`: string enum (`"plus"`, `"pro"`, `"team"`, …, `"unknown"`)
- **Push (verified in schema):** notification `account/rateLimits/updated` with `params.rateLimits: RateLimitSnapshot`. It is **sparse**: merge the fields that are present into the last snapshot, and a `null`/missing field does **not** clear a previous value.
- **Daily token history (verified):** request `account/usage/read` with params `null`. It returns `dailyUsageBuckets: [{startDate:"YYYY-MM-DD", tokens:int}]` (262 days returned in the probe) plus a `summary`.
  - `startDate` is a **UTC calendar day**, not a local day. The **current UTC day is not included**: on 2026-09-23 the latest bucket was 2026-09-22. Never treat a missing today as zero (see §7.5).
  - Cross-check (2026-09-16…22): account totals were 84–99 % (92 % over 7 days) of this Mac's rollout totals counted with the §2.2 rule. The rest is usage from other devices or clients.
- **Other notifications** arrive unprompted (e.g. `remoteControl/status/changed`). **Ignore unknown methods.**
- **Server→client requests** (a message with both `id` and `method`) must get an error reply: `{"id":<same>,"error":{"code":-32601,"message":"not supported"}}`.
- **Limit ID filter (v1):** only the snapshot with `limitId == "codex"` feeds the Codex bars. Take it from `rateLimitsByLimitId["codex"]`, falling back to `rateLimits` only if its `limitId` is `"codex"` or `null`. Drop every other `limitId` (e.g. `"premium"`), with a `debug!` naming the id. The same filter applies to rollout lines (§2.2) and sparse notifications.
- ⚠ **CRITICAL:** `primary` is **not** always the 5-hour window. On the tested Pro account, `primary` was the **weekly** window (`windowDurationMins: 10080`) and `secondary` was `null`. **Always classify windows by `windowDurationMins`**, never by position (see §7.2).
- ⚠ The subcommand is labeled `[experimental]`. If `initialize` fails or a method returns error `-32601`, fall back to §2.2 and set the status to `Degraded`.
- **Regenerate the schema any time:**
  ```bash
  codex app-server generate-json-schema --out /tmp/codex-schema
  ```
  Relevant files: `v2/GetAccountRateLimitsResponse.json`, `v2/AccountRateLimitsUpdatedNotification.json`, `v2/GetAccountTokenUsageResponse.json`.

> **Caveat:** our own app-server process only emits `account/rateLimits/updated` for activity it sees. Usage from the user's *separate* Codex CLI or app is detected through §2.2 file events, which then trigger an `account/rateLimits/read`.

### 2.2 Codex: session rollout files (near-real-time via file events), secondary source

- **Location:** `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<timestamp>-<uuid>.jsonl`. The default `CODEX_HOME` is `~/.codex`.
- **Relevant lines:** `type == "event_msg"` and `payload.type == "token_count"` (Appendix A.1). Fields are **snake_case** here, unlike the app-server's camelCase:
  - `timestamp`: RFC 3339 UTC string
  - `payload.rate_limits.primary | secondary`: `{ used_percent: float, window_minutes: int, resets_at: int (unix s) } | null`
  - `payload.rate_limits.limit_id`: e.g. `"codex"`. Lines with other ids such as `"premium"` may have all-null windows, so skip null windows.
  - `payload.rate_limits.plan_type`: string or null
  - `payload.info`: may be `null`. When present, `info.total_token_usage.total_tokens` is **cumulative for the session file**.
- **Hourly tokens (fork-safe, verified 2026-09-23):** resumed or forked sessions start a **new** file whose first `total_token_usage` already contains the parent conversation. Counting that first total double-counts history (it inflated two days to 152 % and 258 % of the account totals). Per file, for each line with non-null `info`:
  1. First such line in the file → add `info.last_token_usage.total_tokens` only.
  2. `total > prev_total` → add `total - prev_total`.
  3. `total < prev_total` (counter reset) → add `info.last_token_usage.total_tokens`.
  4. `total == prev_total` (repeated event) → add 0.

  Bucket the added amount by the line's `timestamp` hour in **local time**. Fixture: Appendix A.6.
- ⚠ Older Codex builds wrote `resets_in_seconds` instead of `resets_at`. If `resets_at` is missing and `resets_in_seconds` is present, compute `resets_at = timestamp + resets_in_seconds`.

### 2.3 Claude: status line bridge (PUSH), primary source

Claude Code runs a user-configured **status line command** and sends it JSON on stdin. The official docs are at <https://code.claude.com/docs/en/statusline> (fetched 2026-09-23).

- **Fields we use** (Appendix A.4):
  - `rate_limits.five_hour.used_percentage` (0–100, float), `rate_limits.five_hour.resets_at` (unix seconds)
  - `rate_limits.seven_day.used_percentage`, `rate_limits.seven_day.resets_at`
  - `session_id`, `model.display_name`
- **Presence rules (from the docs):** `rate_limits` appears only for Pro/Max subscribers and **only after the first API response of a session**. Each window may be absent independently, and Claude Code **drops a window once its `resets_at` passes**.
- **When it runs (from the docs):** on session start, after each assistant message and other events (debounced **300 ms**), on the optional `refreshInterval` timer, and when a window's `resets_at` is reached. An in-flight script is **cancelled** when a new update triggers.
- **Configured in** `~/.claude/settings.json` → `"statusLine": {"type":"command","command":"<path>", "refreshInterval": <seconds, optional>}`. The tested machine had **no** existing `statusLine`, but the installer must chain any existing one (§8.3).
- **Side effect to disclose to the user:** with a custom status line, Claude Code hides most footer keyboard hints.

### 2.4 Claude: OAuth usage endpoint (POLL, **post-v1**, opt-in, unofficial), fallback source

> **v1 does not build this.** It is implemented in M9 behind the cargo feature `claude-oauth`, and only after the owner records acceptance in `DECISIONS.md`. The spec is kept here so M9 is ready.

- ⚠ **Undocumented.** It is used internally by Claude Code: the strings `api/oauth/usage`, `oauth-2025-04-20`, `five_hour`, `seven_day`, `seven_day_opus`, and `seven_day_sonnet` appear in the 2.1.273 binary. It may change without notice. **Off by default.** The user enables it in Settings after reading a warning.
- **Request:** `GET https://api.anthropic.com/api/oauth/usage`
  Headers: `Authorization: Bearer <accessToken>`, `anthropic-beta: oauth-2025-04-20`, `Accept: application/json`, `User-Agent: headroom/<version>`
- **Token source:** macOS Keychain generic password, service **`Claude Code-credentials`** (item presence verified). The value is JSON containing `claudeAiOauth` with `accessToken`, `expiresAt` (epoch **milliseconds**), and `subscriptionType` (these key names appear in the binary).
- ⚠ **VERIFY the response shape** in T9.1 with the probe (§13, M9) before writing the parser. Expected (community-documented) shape, Appendix A.5:
  `{ "five_hour": {"utilization": float 0-100, "resets_at": "ISO-8601" | null} | null, "seven_day": {...} | null, ... }`
  Parse **leniently**: `utilization` is a number, and `resets_at` may be an ISO string, a number, or null.
- **Hard rules:**
  - **Read-only.** Never write, refresh, or rotate the token. Refreshing could invalidate Claude Code's own session.
  - **Only** a local `expiresAt < now` or an HTTP **401** maps to `AuthExpired` (*"Open Claude Code to refresh sign-in"*). Re-check the Keychain on the next scheduled tick.
  - HTTP **403 / 404** map to `Unsupported { reason }`. A 403 can mean the account lacks permission or the endpoint is unavailable, and it is not an expiry. Stop polling until the app restarts or the setting is toggled.
  - The first Keychain read shows a macOS prompt: *"Headroom wants to access 'Claude Code-credentials'"*. The onboarding copy must explain this and tell the user to choose "Always Allow".

### 2.5 Claude: local conversation logs (history charts)

- **Location:** `~/.claude/projects/**/*.jsonl` (recursive, including `…/<session>/subagents/agent-*.jsonl`). `CLAUDE_CONFIG_DIR` overrides `~/.claude`, and a GUI app does **not** inherit shell env, so offer a Settings override.
- **Relevant lines:** `type == "assistant"` with `message.usage` (Appendix A.3).
- **Tokens per line:** `input_tokens + output_tokens + cache_creation_input_tokens + cache_read_input_tokens` (missing counts as 0).
- **Deduplicate** by `(message.id, requestId)`. The same response can be logged multiple times.
- **Bucket** by `timestamp` (RFC 3339 UTC) → local hour.
- **Label honestly in the UI:** "Claude Code on this Mac". This does **not** include claude.ai web or desktop chat.

### 2.6 Summary matrix

| Provider | Limit bars (% + reset) | Real-time trigger | Poll fallback | History charts |
|---|---|---|---|---|
| Codex | app-server `account/rateLimits/read` + `updated` notification; rollout `rate_limits` (limit ID `codex` only) | app-server notification; FSEvents on `sessions/` | `account/rateLimits/read` every 2–10 min | Hourly: rollout deltas (this Mac) · Daily: `account/usage/read` (account-wide; fallback: rollout, this Mac) |
| Claude | status line bridge file · (post-v1: OAuth endpoint) | FSEvents on bridge file (written by Claude Code within ~300 ms of each response) | **v1: none.** Bars show "Updated Xm ago" / Stale while Claude Code is idle. Post-v1: OAuth every 3–10 min | Hourly + daily: local JSONL (this Mac) |

---

## 3. Architecture

### 3.1 Process and component diagram

```
┌───────────────────────────── Headroom.app (one Rust process) ──────────────────────────────┐
│                                                                                            │
│  usage-sources (tokio tasks, each owns a CancellationToken child)                          │
│  ┌──────────────────────┐ ┌─────────────────────┐ ┌──────────────────────┐ ┌─────────────┐ │
│  │ CodexAppServerSource │ │ CodexRolloutSource  │ │ ClaudeBridgeSource   │ │ClaudeOAuth  │ │
│  │ child: codex         │ │ FSEvents + tail     │ │ FSEvents on json     │ │(post-v1, M9)│ │
│  │ app-server (stdio)   │ │ sessions/**.jsonl   │ │ written by bridge    │ │reqwest poll │ │
│  └─────────┬────────────┘ └─────────┬───────────┘ └─────────┬────────────┘ └──────┬──────┘ │
│            │  ClaudeHistorySource (FSEvents + tail ~/.claude/projects/**.jsonl)     │       │
│            │                        │                       │                     │       │
│            ▼                        ▼                       ▼                     ▼       │
│       mpsc::Sender<SourceEvent>  (bounded, capacity 256)                                   │
│            │                                                                               │
│            ▼                                                                               │
│  ┌────────────────────────────── UsageStore actor (single owner of state) ──────────────┐  │
│  │ per-source state → derive provider view by priority · history series · alerts · Tick│  │
│  └───────────────┬───────────────────────────────────────────────────────────┬─────────┘  │
│                  │ watch::Sender<Arc<UsageSnapshot>>                          │ Alerts     │
│                  ▼                                                            ▼            │
│  src-tauri: forwarder (throttle 250 ms) → emit "usage-updated" · tray title/icon · notif.  │
│                  │                                                                         │
│   ┌──────────────┴──────────────┬───────────────────────────┐                              │
│   ▼                             ▼                           ▼                              │
│  WebView "main"            WebView "popover"          WebView "widget"                     │
│  (React + TS, one Vite app, route chosen by window label)                                  │
└────────────────────────────────────────────────────────────────────────────────────────────┘

 Separate tiny binary:  headroom-statusline  (invoked by Claude Code; synchronous, deps serde_json + tempfile, no tokio)
   stdin JSON ─► writes ~/Library/Application Support/dev.headroom.app/claude-rate-limits.json (atomic)
             └─► optionally pipes stdin to the user's previous statusLine command and prints its stdout
```

### 3.2 Why this shape
- **State per source, view derived.** The store keeps each `(provider, source)` state separately and **derives** what the UI shows using an explicit source priority (§7.4). A fallback source can never corrupt or override a healthy primary source.
- **Actor, not shared mutable state.** Only the `UsageStore` task mutates state. There are no `Arc<Mutex<…>>` graphs, so there are no lock-order bugs and no reference cycles, which are the main source of Rust memory leaks.
- **Bounded channels everywhere.** A stuck consumer causes backpressure, not unbounded memory growth.
- **`usage-core` is pure** (no I/O, no tokio, `#![forbid(unsafe_code)]`), so all parsing and merging is unit-testable with fixtures.
- **WebView UI** gets the neon and glass visuals from CSS plus the Liquid Glass plugin. All data logic stays in Rust, and the UI only renders `UsageSnapshot`.

### 3.3 Update strategy: real-time first, interval fallback

Priority per provider, from most to least real-time:

| # | Mechanism | Latency | Condition |
|---|---|---|---|
| 1 | **Push**: Codex `account/rateLimits/updated`; Claude bridge file change | < 1 s | app-server running / bridge installed and Claude Code active |
| 2 | **Event-triggered read**: an FSEvents change in Codex `sessions/` triggers `account/rateLimits/read` | 3–15 s | Codex CLI or app activity on this Mac |
| 3 | **Adaptive interval poll** | see below | always, as a safety net |

**Adaptive interval (per source, implemented in `Scheduler`, §9.4):**

| State | Codex `rateLimits/read` | Claude OAuth (post-v1, M9 only) |
|---|---|---|
| **Active**: any push or file event for this provider in the last 10 min | every **120 s** | every **180 s**, only if bridge data is older than 3 min |
| **Idle** | every **600 s** | every **600 s** |
| **UI visible**: a popover, main window, or widget was just shown | immediate read if data is older than 30 s | same |
| **Error / 429** | exponential backoff 30 s → 30 min with ±20 % jitter; honor `Retry-After` | same |
| **System just woke** (wall-clock jump > 2× interval detected) | immediate read | immediate read |

Minimum gap between two reads of the same source: **15 s** (debounce bursts of file events).

**Countdown timers** ("Resets in 2h 14m") tick **in the UI** from `resetsAt`, once per second. This is not a backend event.

---

## 4. Tech stack and versions

Use `cargo add` / `npm install` to pick exact patch versions. The versions below were current on 2026-09-23.

### 4.1 Rust (edition 2024, rustc ≥ 1.95)

| Crate | Version | Used in | Purpose |
|---|---|---|---|
| `tauri` | 2.11 | src-tauri | app shell; features `tray-icon`, `macos-private-api`, `image-png` |
| `tauri-build` | 2.x | src-tauri | build script |
| `tauri-plugin-liquid-glass` | 0.1.6 | src-tauri + npm `tauri-plugin-liquid-glass-api` | Liquid Glass (NSGlassEffectView, macOS 26+) |
| `tauri-plugin-positioner` | 2.3 (feature `tray-icon`) | src-tauri | position popover under the tray icon |
| `tauri-plugin-notification` | 2.4 | src-tauri | threshold alerts |
| `tauri-plugin-autostart` | 2.5 | src-tauri | launch at login |
| `tauri-plugin-dialog` | 2.x | src-tauri | native file/folder picker for path overrides (called from Rust) |
| `tauri-plugin-opener` | 2.x | src-tauri | "Reveal logs" in Finder (`reveal_item_in_dir`, called from Rust) |
| `tempfile` | 3 | statusline-bridge, sources | unique same-directory temp files for atomic writes (`NamedTempFile::new_in` + `persist`) |
| `tokio` | 1.53 | sources, src-tauri | features `rt-multi-thread, macros, sync, time, process, io-util, fs` |
| `tokio-util` | 0.7 | sources | `CancellationToken` |
| `notify` | 8.2 | sources | FSEvents file watching |
| `reqwest` | 0.13 | sources, **only under feature `claude-oauth` (M9)** | HTTPS (feature `json`, rustls TLS) |
| `security-framework` | 3.7 | sources, **only under feature `claude-oauth` (M9)** | read Keychain (read-only) |
| `serde` / `serde_json` | 1 | all | (de)serialization; `serde_json` feature `preserve_order` in the installer |
| `chrono` | 0.4 | core, sources | time zones and local-hour bucketing |
| `thiserror` | 2 | all libs | typed errors |
| `tracing`, `tracing-subscriber`, `tracing-appender` | 0.2.5+ | src-tauri | logs. `tracing-appender` rotates **by time only**: use `RollingFileAppender::builder().rotation(Rotation::DAILY).max_log_files(7)`. There is no size cap, so volume is bounded by §6.3 instead. |
| `ts-rs` | 12 | core | **generate TS types from Rust types** (type-safe IPC) |
| `dirs` | 7 | sources, bridge | home and app-support paths |

> **Why `ts-rs` and not `tauri-specta`?** `tauri-specta` is still `2.0.0-rc.*`. `ts-rs` is stable (12.x) and simpler. Command wrappers are hand-written in one typed file (§10.3).

### 4.2 Frontend
- Vite + **React 19** + **TypeScript 5.x**, `"strict": true`, `"noUncheckedIndexedAccess": true`, `"exactOptionalPropertyTypes": true`
- `@tauri-apps/api` v2, `tauri-plugin-liquid-glass-api`, `@tauri-apps/plugin-notification`
- ESLint with `typescript-eslint` `strictTypeChecked`
- **No UI or chart library.** Charts are hand-written SVG (< 150 lines each). Styling uses plain CSS with custom properties (tokens), so there's no Tailwind.

---

## 5. Repository layout

```
headroom/
├── Cargo.toml                    # [workspace] members + [workspace.lints] (§6.1)
├── rust-toolchain.toml           # channel = "1.95"
├── docs/
│   ├── DEVELOPMENT.md            # this file
│   ├── DECISIONS.md              # append-only log of deviations and choices (§0.1)
│   ├── PROGRESS.md               # task/phase log; source of truth for resuming (§0.1)
│   ├── UNDERSTANDING.md          # the agent's phase-0 notes
│   ├── HANDOVER.md               # human-only checks left at the end (§15)
│   ├── AGENT_KICKOFF_PROMPT.md   # how the implementing agent works phase by phase
│   └── design/                   # approved UI design: screenshots, previews, design-tokens.css, gen.py (§11)
├── crates/
│   ├── usage-core/               # PURE: domain types, parsers (DTO→domain), merge, aggregation, alerts
│   │   ├── src/{lib.rs, domain.rs, window.rs, merge.rs, history.rs, alerts.rs, projection.rs}
│   │   ├── src/parse/{mod.rs, codex_app_server.rs, codex_rollout.rs, claude_statusline.rs, claude_oauth.rs, claude_log.rs}
│   │   └── tests/fixtures/*.json[l]   # Appendix A, copied verbatim
│   ├── usage-sources/            # ASYNC I/O: each source is a task that sends SourceEvent
│   │   ├── src/{lib.rs, event.rs, scheduler.rs, tail.rs, watch.rs, paths.rs}
│   │   ├── src/codex/{app_server.rs, rpc.rs, rollout.rs, discover.rs}
│   │   ├── src/claude/{bridge_source.rs, bridge_install.rs, oauth.rs, keychain.rs, history.rs}
│   │   ├── src/store.rs          # UsageStore actor
│   │   └── examples/probe.rs     # CLI: runs all sources and prints snapshots (no UI)
│   └── statusline-bridge/        # bin "headroom-statusline", synchronous (serde_json + tempfile)
│       └── src/main.rs
│       └── tests/concurrency.rs  # T3.3
├── scripts/
│   ├── build-sidecar.sh          # builds statusline-bridge for a triple; lipo for universal-apple-darwin (§10.1, T8.2)
│   └── soak.sh                   # automated 60-min accelerated soak (§14.3, T8.1)
├── src-tauri/
│   ├── Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json, icons/
│   └── src/{main.rs, lib.rs, commands.rs, windows.rs, tray.rs, forwarder.rs, notify.rs, settings.rs}
└── ui/
    ├── package.json, tsconfig.json, vite.config.ts, eslint.config.js, index.html
    └── src/
        ├── main.tsx              # picks <Dashboard/> | <Popover/> | <Widget/> by window label
        ├── bindings/             # GENERATED by ts-rs. Do not edit.
        ├── ipc.ts                # typed invoke/listen wrappers (the only file that calls Tauri)
        ├── hooks/{useSnapshot.ts, useNow.ts, useSettings.ts}
        ├── styles/{tokens.css, glass.css, neon.css}
        ├── components/{NeonBar.tsx, GlassCard.tsx, ProviderCard.tsx, HourlyChart.tsx, DailyChart.tsx, StatTile.tsx, Countdown.tsx, StatusBadge.tsx}
        └── screens/{Dashboard.tsx, Popover.tsx, Widget.tsx, Settings.tsx, Onboarding.tsx}
```

---

## 6. Engineering rules (type safety and no memory leaks)

### 6.1 Workspace lints (put in the root `Cargo.toml`)

```toml
[workspace.lints.rust]
unsafe_code = "forbid"          # src-tauri overrides to "deny" (see note)
missing_docs = "warn"
unused_must_use = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
dbg_macro = "deny"
print_stdout = "deny"           # statusline-bridge allows this at its one print site
mem_forget = "deny"
rc_buffer = "deny"
await_holding_lock = "deny"
large_futures = "warn"
```
Each crate: `[lints] workspace = true`. In **tests** you may use `unwrap`/`expect`; add `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` in `lib.rs`.
> **Note:** if a Tauri macro (`generate_context!`, `generate_handler!`) trips `unsafe_code`, set `unsafe_code = "deny"` in `src-tauri` only and add `#[allow(unsafe_code)]` on that single item with a comment. Never write `unsafe` yourself.

### 6.2 Type safety rules
1. **DTO → domain split.** External JSON is parsed into lenient `Dto` structs (all fields `Option`, with `#[serde(default)]` and no `deny_unknown_fields`, because the APIs evolve). Then `TryFrom<Dto>` converts them into **strict domain types** (§7). UI and store only ever see domain types.
2. **Newtypes for units:** `Percent`, `UnixSeconds`, `TokenCount`, `WindowMinutes`. Never pass a raw `f64`/`i64` across module boundaries.
3. **Enums, not strings,** for `Provider`, `WindowKind`, `SourceKind`, and `ConnectionStatus`.
4. **`ts-rs` for every type crossing IPC.** `i64` and `u64` become `bigint` in ts-rs by default, but JSON sends numbers, so annotate every such field with `#[ts(type = "number")]`. There's a unit test for this in M1.
5. **TS:** the strict flags from §4.2. No `any`, no `as` casts except in `ipc.ts` at the IPC boundary, and all generated types are imported from `bindings/`.

### 6.3 Memory and resource rules (checklist for code review)

| Risk | Rule |
|---|---|
| Unbounded queues | Only `mpsc::channel(N)` (bounded). **Never** `unbounded_channel`. |
| Reference cycles | No `Rc`/`Arc` cycles. Only the store owns state. Tasks own their own data. |
| Leaked tasks | Every `tokio::spawn` goes into a `JoinSet` owned by its parent and gets a `CancellationToken` child. On shutdown: `cancel()`, then `join_all` with a 3 s timeout. |
| Leaked child process | `tokio::process::Command::kill_on_drop(true)`. Always **drain stderr** (a full pipe blocks the child). |
| RPC pending map | `HashMap<RequestId, oneshot::Sender>`. Remove entries on response **and** on a 15 s timeout, and drain the map on disconnect. |
| Growing history | `BTreeMap<HourStart, TokenCount>` pruned to **8 days** (≤ 192 keys per provider). Dedupe set pruned to 8 days. |
| File tail buffers | Per-file partial line buffer capped at **16 MB**. Skip longer lines with a `warn!`. Forget tail state for files not modified in 8 days. |
| Watchers | `notify` watcher handles are owned by their source task and dropped on cancel. |
| Alert memory | "Already fired" set is keyed by `(provider, window, threshold, resets_at)` and pruned when `resets_at` has passed. |
| Logs | Daily rotation, `max_log_files(7)` (§4.1). There is no built-in line truncation, so **any external text** (child stderr, file content, HTTP bodies) must be passed through `usage_core::log_trunc(&str) -> Cow<str>` (cap 2 KB, UTF-8 safe) before logging. Default level `info`. Never log per-line file events at `info`. |
| Forbidden | `Box::leak`, `mem::forget`, `static mut`, `lazy_static` holding growing collections. |
| Frontend | Every `listen()` returns an unlisten function, and it must be called in the `useEffect` cleanup. One `setInterval` per window (`useNow`). Charts re-render SVG, never append. |

### 6.4 Error handling
- Each crate has an `Error` enum (`thiserror`). There's no `anyhow` in libraries. `src-tauri` may map errors to a `CommandError { code, message }` (serializable, ts-rs) for the UI.
- A source error **never crashes the app**. It becomes `ConnectionStatus::Error { message }` for that provider, and the source retries with backoff.
- User-facing messages are short and actionable ("Codex CLI not found — set its path in Settings"). Technical details go to the log.

### 6.5 Security
- Tokens are held in a `struct AccessToken(String)` with a **manual `Debug` that prints `AccessToken(***)`** and no `Display` or `Serialize`.
- Never read `~/.codex/auth.json`. The app-server handles Codex auth.
- HTTP: 10 s timeout, HTTPS only, and no redirects to other hosts (`reqwest::redirect::Policy::none()`).
- The settings file is not a secret store and must never contain tokens.

---

## 7. Domain model (`usage-core`)

### 7.1 Types (copy this, then add doc comments)

```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Provider { Claude, Codex }

/// Classified limit window. Classify by duration, never by "primary/secondary" position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum WindowKind {
    /// ≈ 5 hours (240..=360 minutes).
    Session,
    /// ≈ 7 days (9_000..=11_000 minutes).
    Weekly,
    /// Anything else, kept so nothing is silently dropped.
    Other { minutes: u32 },
}

/// 0.0..=100.0, finite. Constructed only via `Percent::new`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, TS)]
#[serde(transparent)]
#[ts(export, type = "number")]
pub struct Percent(f64);

/// Seconds since the Unix epoch (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export, type = "number")]
pub struct UnixSeconds(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(transparent)]
#[ts(export, type = "number")]
pub struct TokenCount(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceKind { CodexAppServer, CodexRollout, ClaudeStatusline, ClaudeOAuth, ClaudeLocalLogs }

impl SourceKind {
    /// Explicit priority for limit readings; lower = more authoritative. Used by §7.4.
    /// ClaudeLocalLogs never produces limit readings (history only).
    pub const fn limit_priority(self) -> u8 {
        match self {
            Self::CodexAppServer | Self::ClaudeStatusline => 0,
            Self::CodexRollout | Self::ClaudeOAuth => 1,
            Self::ClaudeLocalLogs => u8::MAX,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LimitWindow {
    pub kind: WindowKind,
    pub used: Percent,
    pub resets_at: Option<UnixSeconds>,
    /// True when `resets_at` has passed and no fresh reading has arrived yet (UI shows 0 % + "Reset").
    pub reset_pending: bool,
    pub source: SourceKind,
    pub observed_at: UnixSeconds,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum ConnectionStatus {
    Connected,
    /// Newest reading older than 15 min.
    Stale,
    /// Source works but with reduced capability (e.g. app-server unavailable, rollout only).
    Degraded { reason: String },
    NotConfigured { hint: String },
    AuthExpired { hint: String },
    /// Source reachable but refuses or does not offer the data (e.g. HTTP 403/404, RPC -32601).
    Unsupported { reason: String },
    Error { message: String },
}

/// Health of one source, shown in Settings → Diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceHealth {
    pub source: SourceKind,
    pub status: ConnectionStatus,
    pub last_success: Option<UnixSeconds>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderUsage {
    pub provider: Provider,
    /// Display name, e.g. "Pro", "Max". None if unknown.
    pub plan: Option<String>,
    /// Derived view (§7.4). Sorted: Session, Weekly, Other(ascending minutes). May be empty.
    pub windows: Vec<LimitWindow>,
    /// Derived provider status (§7.4 rule D4). Never taken from a lower-priority source while a higher one is healthy.
    pub status: ConnectionStatus,
    /// Which source the displayed windows came from, if any.
    pub authoritative_source: Option<SourceKind>,
    pub last_updated: Option<UnixSeconds>,
    /// One entry per source configured for this provider.
    pub sources: Vec<SourceHealth>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageSnapshot {
    pub claude: ProviderUsage,
    pub codex: ProviderUsage,
    pub generated_at: UnixSeconds,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Bucket { pub start: UnixSeconds, pub tokens: TokenCount }

/// Where a history series' numbers come from. Drives the label under each chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HistoryScope {
    /// Only tool activity recorded in local files on this Mac.
    ThisMac,
    /// Account-wide, as reported by the provider (all devices and clients).
    Account,
}

/// One chart's data. Hourly and daily are separate series because they can come from
/// different sources with different scopes (e.g. Codex hourly = this Mac, daily = account).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Series {
    pub source: SourceKind,
    pub scope: HistoryScope,
    /// Human label, e.g. "Claude Code on this Mac", "All Codex usage (account)".
    pub scope_label: String,
    /// When this series' underlying data was last refreshed. None = no data yet.
    pub observed_at: Option<UnixSeconds>,
    pub buckets: Vec<Bucket>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct History {
    pub provider: Provider,
    /// Exactly 24 buckets, oldest first, zero-filled, local-hour aligned, last = current hour.
    pub hourly: Series,
    /// Exactly 7 buckets, oldest first, zero-filled, local-midnight aligned, last = today.
    /// Exception: the Codex `Account` series holds the 7 latest *reported UTC days* (last = yesterday UTC), see §7.5.
    pub daily: Series,
    /// Percentage-based (from the Weekly limit window), NOT derived from token counts.
    pub projection: Option<Projection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Projection {
    /// Projected % at weekly reset, linear from window start. May exceed 100.
    #[ts(type = "number")]
    pub projected_percent_at_reset: f64,
    /// When 100 % is projected to be reached, if before reset.
    pub hits_limit_at: Option<UnixSeconds>,
}
```

### 7.2 Window classification (pure fn, unit-tested)

```rust
pub fn classify(window_minutes: Option<i64>, hint: Option<WindowKind>) -> WindowKind
```
- `240..=360` → `Session`; `9_000..=11_000` → `Weekly`; any other `Some(m)` → `Other { minutes: m as u32 }` (saturating/`try_from`, no `as` truncation surprises).
- `None` → use `hint`. Claude's `five_hour` gives the hint `Session` and `seven_day` gives `Weekly`. With no hint → `Other { minutes: 0 }`.

### 7.3 Reading (source → store message)

```rust
pub struct Reading {
    pub provider: Provider,
    pub source: SourceKind,
    pub observed_at: UnixSeconds,
    pub plan: Option<String>,
    /// Only the windows this reading contains. Parsers have already applied the limit-ID filter (§2.1).
    pub windows: Vec<LimitWindow>,
    /// false = a complete snapshot from this source (replaces the source's windows).
    /// true  = sparse update (Codex `account/rateLimits/updated`); upserts only the windows present.
    pub partial: bool,
}
```
Which readings are full vs partial: app-server `rateLimits/read` → **full**; app-server `updated` notification → **partial**; each rollout `token_count` line → **full** (it carries the complete primary + secondary pair); bridge file → **full** (Claude Code drops windows after reset, and full replacement mirrors that).

### 7.4 State model and merge rules (`merge.rs`, the most important logic, 100 % unit-tested)

**State:** `BTreeMap<(Provider, SourceKind), SourceState>` where
```rust
pub struct SourceState {
    pub status: ConnectionStatus,          // this source's own status
    pub observed_at: Option<UnixSeconds>,  // time of the last successful reading
    pub plan: Option<String>,
    pub windows: BTreeMap<WindowKind, LimitWindow>,
}
```

**Ingest (per source, never touching other sources):**
- **I1. Full reading** → replace `windows` entirely with the reading's windows. Windows missing from an authoritative full read therefore disappear. Set `observed_at`, set `status = Connected`, and set `plan` if `Some`.
- **I2. Partial reading** → upsert only the windows present, and apply the rollover guard per window: ignore an incoming window whose `resets_at` is **earlier** than the stored one (it's pre-rollover data). Set `plan` only if `Some`. Absent or `null` fields never clear anything.
- **I3. Status event** → set that source's `status` only. Keep its `windows` and `observed_at`: an error does not erase the last good data, and it ages into `Stale` via D3.
- **I4. Out-of-order guard** → ignore a full reading whose `observed_at` is older than the stored `observed_at` for the same source.

**Derive (build `ProviderUsage` for the snapshot; pure function of state + `now`):**
- **D1. Authoritative source** = among the provider's sources that have data (`observed_at.is_some()`), pick the one with the lowest `limit_priority()` whose data is **fresh** (`now - observed_at < 15 min`). If none are fresh, pick the one with the newest `observed_at`. None with data → no windows.
- **D2. Window set** = exactly the authoritative source's windows. **Value refinement:** for each of those window kinds, if another source has the same kind with `resets_at >=` the authoritative one's **and** a newer `observed_at`, use that source's `used`/`resets_at` (e.g. a rollout line written 3 s after the last app-server read). A lower-priority source can **never add** a window the authoritative source doesn't have.
- **D3. Reset handling** on every derive: if `resets_at <= now` → display `used = 0`, `reset_pending = true`. Drop a window from the view if `now - resets_at > 1 day`.
- **D4. Provider status** (evaluate in order; the first match wins):
  1. No source has data → the status of the **highest-priority configured source** (e.g. `NotConfigured { hint: "Enable real-time updates" }`).
  2. The authoritative source is not fresh → `Stale`.
  3. The authoritative source is fresh **and** is the highest-priority source → `Connected`.
  4. The authoritative source is fresh but a higher-priority source is failing → `Degraded { reason }`, with the reason taken from that higher-priority source (e.g. "Codex app-server stopped; using session files").

  Errors from **lower**-priority sources never change the provider status. They appear only in `sources[]` (Settings → Diagnostics).
- **D5. `plan`** = the authoritative source's plan, else any other source's plan (highest priority first).

**Required tests (one per rule, plus these races):** (a) app-server full read without `secondary` removes the secondary window that an older full read had; (b) a rollout error while the app-server is healthy leaves the provider `Connected`; (c) the same events delivered in two different orders give an identical snapshot (property test over permutations of a fixed event set, excluding I4-rejected ones); (d) a sparse update with only `secondary` keeps `primary` and `plan`; (e) a stale app-server (20 min) plus a fresh rollout → the rollout becomes authoritative; (f) a `"premium"` limit ID never reaches the store (parser test).

### 7.5 History aggregation (`history.rs`)
- Input: `TokenEvent { provider, source, at: UnixSeconds, tokens: TokenCount, dedupe_key: Option<String> }` and `DailyBuckets { provider, source, observed_at, days: Vec<(NaiveDate, TokenCount)> }`.
- Store hourly `BTreeMap<UnixSeconds /*UTC hour start*/, u64>` per `(provider, source)`, plus the last `observed_at`. Build the local-time views (24 h, 7 days) on demand using `chrono::Local`. This handles DST: a local day can have 23 or 25 hours.
- **Series selection (each series carries its own `source`, `scope`, `scope_label`, and `observed_at`):**

| Provider | `hourly` series | `daily` series |
|---|---|---|
| Claude | `ClaudeLocalLogs`, `ThisMac`, "Claude Code on this Mac" | `ClaudeLocalLogs`, `ThisMac`, "Claude Code on this Mac" |
| Codex | `CodexRollout`, `ThisMac`, "Codex CLI on this Mac" | `CodexAppServer`, `Account`, "All Codex usage (account)" if its daily data is < 1 day old; **else** `CodexRollout`, `ThisMac`, "Codex CLI on this Mac" |

- Never mix sources inside one series. Never sum app-server daily with rollout daily.
- **Codex `Account` daily series:** it holds the 7 most recent buckets the app-server reported. These are UTC days ending **yesterday (UTC)**, because the current day is never reported (§2.1). Keep them as UTC dates and don't re-align them to local midnight. The UI labels the last column by its date (e.g. "Sep 22"), not "Today". Its `scope_label` is "All Codex usage (account)", and the chart note says "tokens per UTC day".

### 7.6 Projection (`projection.rs`)
- `window_start = resets_at - duration`. `elapsed_frac = (now - window_start) / duration`, clamped to `[0.01, 1]`.
- `projected = used / elapsed_frac`. `hits_limit_at = window_start + duration * (100 / projected)` if `projected > 100`, else `None`.
- Only for `Weekly`. Return `None` if the window has been open for less than 6 h (too noisy).

### 7.7 Alerts (`alerts.rs`)
- Thresholds come from settings (default `[75, 90, 100]`). Fire when `used` crosses a threshold **upward** since the previous snapshot. Deduplicate by `(provider, kind, threshold, resets_at)`.
- "Reset" alert: when the window's `resets_at` increases and the previous `used >= 90`. The message is "Claude session limit reset".
- Output: `Vec<Alert { title, body }>`. The Tauri layer sends notifications, so core does no I/O.

---

## 8. Source specifications (`usage-sources`)

Every source has this shape:

```rust
pub async fn run(cfg: XConfig, tx: mpsc::Sender<SourceEvent>, cancel: CancellationToken) -> Result<(), SourceError>
```
```rust
pub enum SourceEvent {
    Reading(Reading),
    Tokens(Vec<TokenEvent>),                   // each TokenEvent carries its SourceKind
    Daily(DailyBuckets),                       // carries provider, source, observed_at (§7.5)
    Status { provider: Provider, source: SourceKind, status: ConnectionStatus },
    Activity { provider: Provider },          // feeds the Scheduler's active/idle state
}
```
Loop with `tokio::select! { _ = cancel.cancelled() => break, … }`. Never block the runtime: file reads use `tokio::fs` or `spawn_blocking`.

### 8.1 Codex app-server source (`codex/app_server.rs`, `codex/rpc.rs`)

1. **Discover the binary** (`discover.rs`), in this order: settings override → `/opt/homebrew/bin/codex` → `/usr/local/bin/codex` → `~/.local/bin/codex` → `~/.npm-global/bin/codex` → output of `/bin/zsh -lc 'command -v codex'` (3 s timeout). This is needed because **GUI apps do not inherit the shell PATH**. If none is found → `NotConfigured { hint: "Install Codex CLI or set its path in Settings" }`.
2. **Spawn:** `Command::new(path).arg("app-server").stdin(piped).stdout(piped).stderr(piped).kill_on_drop(true)`.
3. **RPC client** (`rpc.rs`) is generic over `AsyncRead + AsyncWrite` so tests can use `tokio::io::duplex`:
   - The writer task serializes one JSON object plus `\n` per message.
   - The reader task uses `BufReader::lines()`. For each line: if it has `id` and (`result` or `error`) → complete the pending oneshot. If it has `method` and `id` → reply `-32601`. If it has `method` only → forward as a notification.
   - Request timeout is 15 s. On timeout, remove from the pending map and return `RpcError::Timeout`.
   - The stderr task reads lines → `tracing::debug!` (truncated to 2 KB). This keeps the pipe drained.
4. **Handshake** (§2.1) → `account/rateLimits/read` → **one** full `Reading` built from the `"codex"` snapshot only (limit-ID filter, §2.1). Other IDs are dropped with a `debug!`. Then `account/usage/read` → `SourceEvent::Daily(DailyBuckets { source: CodexAppServer, observed_at: now, … })`.
5. **Notifications:** `account/rateLimits/updated` → if `limitId` is `"codex"` or absent → a `Reading { partial: true, … }` built from the present fields only. Otherwise drop it.
6. **Method not supported** (`-32601` on `account/rateLimits/read`) → status `Unsupported { reason }` for `CodexAppServer`. Stop this source; the rollout source keeps working (§7.4 D4 shows `Degraded`).
7. **Poll loop:** wait for the `Scheduler` signal (§9.4) → re-read `rateLimits`. Re-read `account/usage/read` at most every 15 min.
8. **Record the child PID** in the source state and log it at `info` (`codex app-server started pid=…`). The soak test uses it (§14.3).
9. **Child exit or broken pipe:** status `Degraded { reason: "Codex app-server stopped; using session files" }`, then restart with backoff (5 s → 5 min). After 10 consecutive failures, stop until settings change or the user clicks "Retry".

### 8.2 Codex rollout source (`codex/rollout.rs`, `tail.rs`, `watch.rs`)

1. **Root:** `<codex_home>/sessions`. If it's missing → `NotConfigured` (but keep the app-server source running).
2. **Initial scan:** only files with mtime within the last **8 days**. Parse the whole file once and record `offset = len`.
3. **Watch:** `notify::recommended_watcher` recursive on the root. On create or modify of `*.jsonl` → send `Activity`, then `tail.read_new(path)`:
   - If `len < offset` (truncated or rotated) → reset `offset = 0`.
   - Read `[offset, len)`, split on `\n`, and keep the trailing partial line in the buffer (cap 16 MB).
   - Cheap prefilter before JSON parsing: the line contains `"token_count"`.
4. **Per line:** parse the DTO (Appendix A.1). If `rate_limits.limit_id` is `"codex"` (or absent) and at least one window is non-null → emit a **full** `Reading` (source `CodexRollout`, `observed_at` = line timestamp). Always emit `TokenEvent`s (source `CodexRollout`) from the cumulative delta (§2.2), whatever the limit ID.
5. **Debounce:** coalesce events per file for 500 ms. After processing, ask the Scheduler for a "triggered read" of the app-server (min gap 15 s).

### 8.3 Claude status line bridge (`crates/statusline-bridge` + `claude/bridge_install.rs` + `claude/bridge_source.rs`)

**Bridge binary (`headroom-statusline`): synchronous (no async runtime, no tokio). Dependencies are `serde_json` and `tempfile` only. It must never fail Claude Code.**

*Performance budgets:*
- **Unchained fast path** (no `chainedCommand`): p95 **< 20 ms** wall time, measured in T3.3.
- **Chained path:** the bridge's own work stays inside the same 20 ms budget. The chained command gets a **hard 2 s timeout**, after which it is killed. Total worst case is about 2.02 s. Claude Code may cancel the bridge earlier (§2.3), and that is fine because the rate-limit file is written **before** the chained command runs.

*Arguments:* `--out-dir <path>` (optional; **tests only**) overrides the output/config directory. The installer never passes it, so in production the directory is always `~/Library/Application Support/dev.headroom.app/`.

*Steps:*
1. Read all of stdin (cap 4 MB).
2. Parse leniently. If `rate_limits` is present with at least one window, write
   `{"schema":1,"writtenAt":<unix s>,"sessionId":"…","rateLimits":<the rate_limits object verbatim>}`
   to `<out-dir>/claude-rate-limits.json` **atomically, safe under overlapping runs** (Claude Code can start a new invocation while cancelling the old one):
   - `tempfile::NamedTempFile::new_in(<out-dir>)` gives a **unique** name in the same directory (same filesystem, so rename is atomic). Then write, `sync_all()`, and `persist(<target>)` (an atomic `rename`).
   - Never use a fixed temp name. Last writer wins, which is correct because all writers carry the same account's data. A reader always sees a complete file.
   - If the process is killed before `persist`, the `NamedTempFile` drop removes the temp file. The app also deletes `.tmp*` files older than 1 h in that directory at startup, which covers SIGKILL.
3. Read `<out-dir>/bridge.json` → `{"chainedCommand": "<string>" | null}`.
   - If `chainedCommand` is set → run `/bin/sh -c <chainedCommand>`, write the **original stdin bytes** to it, wait at most 2 s, print its stdout verbatim, and kill it on timeout.
   - Else print a compact default such as `5h 24% · 7d 41%` (only the windows present; percentages rounded with `f64::round`, so 23.5 → 24), or nothing.
4. **Always `exit(0)`**, even on errors. On error, print nothing.

**Installer (`bridge_install.rs`).** It only runs when the user clicks "Enable real-time Claude updates" in onboarding or Settings, which is an explicit consent step:
1. Copy the bridge from the app bundle (`Contents/MacOS/headroom-statusline`, shipped via Tauri `bundle.externalBin`) to `~/Library/Application Support/dev.headroom.app/bin/headroom-statusline` and `chmod 755`. Re-copy on each app launch if the version differs, so the path in `settings.json` stays stable even if the app moves.
2. Read `<claude_dir>/settings.json` (missing → `{}`) into `serde_json::Value` with `preserve_order`. **If it fails to parse, or the top level isn't an object → abort and show an error. Never overwrite a file you can't parse.**
3. Write a backup to `settings.json.headroom-backup-<unix>`. It is kept for **manual** recovery only (see step 7).
4. Save the **entire** previous `statusLine` value (any JSON, or "absent") into app state `<app_support>/bridge-install.json` → `{"previousStatusLine": <value> | null, "installedAt": <unix>}`. If the previous value is an object with a `command` that is not ours → also write `bridge.json.chainedCommand = <that command>`.
5. **Build the new value by cloning the previous object** (or `{}` if it was absent or not an object) and overwriting **only** `type = "command"` and `command = "'<bridge path>'"`. Every other key is preserved as-is: `padding`, `refreshInterval`, `hideVimModeIndicator`, and any future keys. Leave all other top-level keys of `settings.json` untouched.
6. Write atomically (`NamedTempFile::new_in(<claude_dir>)` + `persist`), preserving the original file's permission bits.
7. **Uninstall is semantic, not a byte restore.** It keeps any unrelated edits the user made to `settings.json` after install:
   - Re-read `settings.json` (same parse rules as step 2).
   - If `statusLine.command` is **not** ours → change nothing and tell the user ("Your status line was changed after install; left untouched").
   - Else set `statusLine` back to `previousStatusLine` **exactly**, or remove the key if it was absent. Every other key stays as it is **now**. Write atomically, then delete `bridge-install.json` and clear `chainedCommand`.
   - The backup from step 3 is **not** used automatically.

**Effectiveness check (settings precedence).** Claude Code settings precedence is: managed settings (file, MDM, or server-managed) > `--settings` CLI > project `.claude/settings.local.json` > project `.claude/settings.json` > user `~/.claude/settings.json`. We only edit the **user** file, so any higher scope that sets `statusLine` silently wins. This can't be detected completely by reading files (MDM and server-managed settings aren't on disk), so it is checked **behaviorally**:
- `BridgeStatus.effective: Effectiveness` = `Unverified` (installed, no evidence yet) → `Confirmed` (the bridge file was written after `installedAt`) → `LikelyOverridden` (the Claude local logs show assistant activity more than 10 min after `installedAt`, but the bridge file hasn't been written since `installedAt`).
- A `rate_limits` absence (non-Pro/Max plan) also produces no write. The `LikelyOverridden` hint text must therefore mention both causes: *"No updates received from Claude Code. A project or organization setting may override your status line, or your plan doesn't report limits."*
- `Confirmed` is sticky until uninstall. It may flip to `LikelyOverridden` later, e.g. when the user opens a project with its own `statusLine`.

**Bridge source (`bridge_source.rs`):** watch the app-support directory for `claude-rate-limits.json`. On change → read, parse (Appendix A.4 `rate_limits`), and emit a **full** `Reading { source: ClaudeStatusline, observed_at: writtenAt }` plus `Activity`. If the bridge isn't installed → `NotConfigured { hint: "Enable real-time updates" }`.

### 8.4 Claude OAuth poller (`claude/oauth.rs`, `claude/keychain.rs`): **post-v1, M9, cargo feature `claude-oauth`**
0. All code in this section is behind `#[cfg(feature = "claude-oauth")]`. The v1 build does not compile it.
1. Run only if `settings.claude_oauth_enabled == true`.
2. Scheduler tick → if the latest Claude reading from `ClaudeStatusline` is < 3 min old → skip (the push data is fresher).
3. Keychain read with `security_framework::passwords::get_generic_password("Claude Code-credentials", <account>)`. The account is usually the macOS username. If that fails, fall back to `ItemSearchOptions` with only `service` set. Parse the JSON → `claudeAiOauth.accessToken` (wrap it in `AccessToken`), `expiresAt` (ms), and `subscriptionType`.
4. Expired locally → `AuthExpired`. Otherwise do the GET (§2.4). Status handling: 200 → parse (§2.4, lenient) → full `Reading { source: ClaudeOAuth }`. **401 → `AuthExpired`. 403/404 → `Unsupported { reason }`, and stop until restart or toggle.** 429 → backoff with `Retry-After`. 5xx or network error → `Error`, then backoff.
5. `plan` comes from `subscriptionType` (capitalize: `"max"` → `"Max"`).

### 8.5 Claude history source (`claude/history.rs`)
Same tail and watch machinery as §8.2, rooted at `<claude_dir>/projects`, recursive. Prefilter: the line contains `"usage"` and `"assistant"`. Emit `TokenEvent { source: ClaudeLocalLogs, dedupe_key: Some(format!("{msg_id}:{request_id}")) }` and `Activity`. The last activity time also feeds the bridge effectiveness check (§8.3). The store deduplicates (§6.3 pruning).

### 8.6 Paths (`paths.rs`)
One module computes every path from `Settings` overrides + `dirs::home_dir()`. **No other module may build paths itself.** App support directory: `dirs::data_dir()/dev.headroom.app`.

---

## 9. Store, scheduler and wiring

### 9.1 `UsageStore` actor (`store.rs`)
```rust
pub struct StoreHandle { pub snapshot: watch::Receiver<Arc<UsageSnapshot>>, pub alerts: mpsc::Receiver<Alert>, cmd: mpsc::Sender<StoreCmd> }
enum StoreCmd { GetHistory(Provider, oneshot::Sender<History>), RefreshNow }
```
- Owns the per-source state map (§7.4) and the history maps (§7.5). The `merge.rs` ingest and derive functions are pure. The actor only routes events into them.
- `select!` over: the `SourceEvent` receiver, the `StoreCmd` receiver, a 30 s `interval` (Tick: re-derive for D3 resets and D4 staleness), and `cancel`.
- After each state change or Tick: **derive** a new `UsageSnapshot` and send it only if it differs (`send_if_modified`).
- Alerts go out through a bounded channel (capacity 32). If the channel is full → drop the alert with a `warn!`.

### 9.2 Startup order (`src-tauri/src/lib.rs`)
1. Init tracing → load `Settings` → build `Paths`.
2. Create the root `CancellationToken` and a `JoinSet`. Spawn the store, the scheduler, and the enabled sources.
3. Build the tray, create the windows (hidden), and start the forwarder (§10.4).
4. `RunEvent::ExitRequested` → `cancel.cancel()` → `timeout(3s, joinset.join_all())`.

### 9.3 Settings (`src-tauri/src/settings.rs`)
A Rust struct (serde + ts-rs), stored as JSON at `<app_support>/settings.json` with atomic writes. **Versioned from day one.**

Fields (v1):
`schema_version: u32 (= 1), onboarding_completed: bool (default false), codex_path: Option<String>, codex_home: Option<String>, claude_dir: Option<String>, claude_bridge_enabled: bool, claude_oauth_enabled: bool (default false; ignored unless built with feature claude-oauth), thresholds: Vec<u8> (default [75,90,100]), notify_on_reset: bool (default true), launch_at_login: bool (default false), show_dock_icon: bool (default false), widget: WidgetSettings { visible, x, y, variant: Pill|Stack|Mini, opacity: f32 (0.4..=1.0) }, poll_active_secs: u32 (120, min 60), poll_idle_secs: u32 (600, min 120)`.

**Load and migrate** (`settings.rs`, pure fn `migrate(serde_json::Value) -> Result<Settings, SettingsError>`, unit-tested):
1. Missing file → defaults (`onboarding_completed = false`, so onboarding runs).
2. Read `schema_version` (missing → treat as `0`, the pre-release format). Apply migrations **in sequence**: `0 → 1`, then `1 → 2` in the future. Each step is one function `fn v0_to_v1(Value) -> Value` with its own test.
3. `schema_version` greater than the app knows (a downgrade) → **do not overwrite the file**. Run with in-memory defaults, show a banner "Settings were created by a newer version", and disable saving until the user clicks "Reset settings".
4. Unparseable JSON → rename it to `settings.json.corrupt-<unix>`, then use defaults.
5. After loading, validate and clamp (minimums, opacity range, thresholds deduped, sorted, and within 1..=100).
6. Always save with the current `schema_version`.

### 9.4 Scheduler (`scheduler.rs`)
- Tracks `last_activity[provider]`, `last_read[source]`, and `backoff[source]`.
- Exposes `async fn next_due(&self, source) -> Instant` and `fn trigger(source)` (from file events and UI visibility).
- Implements the table in §3.3. Uses `tokio::time::interval` with `MissedTickBehavior::Skip`. Sleep and wake detection: compare `SystemTime` deltas against `Instant` deltas each tick, and if they differ by more than 60 s → trigger all.

---

## 10. Tauri shell (`src-tauri`)

### 10.1 `tauri.conf.json` essentials
```jsonc
{
  "productName": "Headroom",
  "identifier": "dev.headroom.app",
  "app": {
    "macOSPrivateApi": true,
    "windows": [
      { "label": "main", "title": "Headroom", "width": 720, "height": 520, "minWidth": 560, "minHeight": 420,
        "transparent": true, "titleBarStyle": "Overlay", "hiddenTitle": true, "visible": false },
      { "label": "popover", "width": 340, "height": 420, "decorations": false, "transparent": true,
        "alwaysOnTop": true, "skipTaskbar": true, "resizable": false, "visible": false, "focus": true },
      { "label": "widget", "width": 280, "height": 72, "decorations": false, "transparent": true,
        "alwaysOnTop": true, "skipTaskbar": true, "resizable": false, "visible": false,
        "visibleOnAllWorkspaces": true, "shadow": false }
    ],
    "security": { "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:" }
  },
  "bundle": { "externalBin": ["binaries/headroom-statusline"], "macOS": { "minimumSystemVersion": "26.0" } }
}
```
- **Settings and Onboarding are routes inside the `main` window** (`#/settings`, `#/onboarding`), not extra windows. There are exactly three windows: `main`, `popover`, and `widget`.
- **`externalBin` naming:** Tauri looks for `binaries/headroom-statusline-<target-triple>` matching the **bundle target**:
  - Dev or native build: `headroom-statusline-aarch64-apple-darwin` (or `-x86_64-apple-darwin` on Intel).
  - Universal build: `headroom-statusline-universal-apple-darwin`, which **you must create yourself** with `lipo` (Tauri lipos the main app but not sidecars). See T8.2.
  - The script `scripts/build-sidecar.sh <triple>` builds `statusline-bridge` and copies or lipos it into `src-tauri/binaries/`. `beforeBuildCommand` / `beforeDevCommand` call it.
- **Capabilities** (`capabilities/default.json`): `core:default`, **`core:window:allow-start-dragging`** (needed by `data-tauri-drag-region`), `liquid-glass:default`, `notification:default`, `autostart:default`, `positioner:default`, and our own commands. The dialog and opener plugins are invoked **from Rust commands only**, so their JS permissions are not granted.

### 10.2 Windows and tray behavior (`windows.rs`, `tray.rs`)
- **Dock icon:** `app.set_activation_policy(ActivationPolicy::Accessory)` unless `show_dock_icon`.
- **Main:** `CloseRequested` → `api.prevent_close()` + `hide()`. This is "minimize to menu bar".
  - **Drag region:** `titleBarStyle: "Overlay"` removes the native drag area. The toolbar root element must have `data-tauri-drag-region`, and interactive children (buttons, the segmented control) must **not** have it. Leave about 78 px of left padding for the traffic-light buttons.
  - On first launch (`onboarding_completed == false`): show `main` on the `#/onboarding` route. Otherwise start hidden (menu-bar only).
- **Tray:** `TrayIconBuilder` with a monochrome template icon (`icon_as_template(true)`) and `show_menu_on_left_click(false)`.
  - Left click → toggle **popover**, positioned with `tauri_plugin_positioner::Position::TrayBottomCenter`. Call `tauri_plugin_positioner::on_tray_event` inside `on_tray_icon_event` first.
  - Right click → native menu: Open Dashboard · Show Floating Widget · Refresh Now · Settings… · Quit.
  - **Title** next to the icon: the highest `used` among visible windows, e.g. `" 72%"`, via `tray.set_title()`. It updates on every snapshot, but only when the text changes.
  - Icon state: swap between 3 bundled template PNGs (normal / warning ≥ 75 / critical ≥ 90). Don't render icons at runtime.
- **Popover:** `WindowEvent::Focused(false)` → `hide()`.
- **Widget:** restore its position from settings, save it on `Moved` (debounced 500 ms), and drag with `data-tauri-drag-region`. The "collapse to widget" button hides main and shows the widget.
  - **The window is exactly the variant's size:** Pill 280×72, Stack 160×180, Mini 200×24. When `widget.variant` changes, call `set_size` and re-apply the glass corner radius.
  - **Hover UI stays inside the window bounds** (anything outside is clipped). Pill and Stack: close / opacity / expand controls replace or overlay the content. Mini: the two values show inline. See `docs/design/screenshots/WidgetDark.png`.
  - **Edge snap:** when a drag ends within 24 pt of a screen edge of the current monitor's visible frame, move the widget to a 12 pt inset from that edge, then save the position.
- **Liquid Glass:** in each window's UI bootstrap, call `isGlassSupported()` then `setLiquidGlassEffect({ cornerRadius })` (main 0 because the native frame is used, popover 18, widget by variant: Pill 36, Stack 24, Mini 12). Plain CSS `backdrop-filter` does **not** blur the desktop through a transparent window, so the plugin is required. If `isGlassSupported()` returns false or the call throws, add `class="no-glass"` on `<html>`. That class renders opaque tinted panels, the same style used for Reduce Transparency.

### 10.3 IPC contract (the UI's only contact with Rust)

**Commands** (`commands.rs`, all `async`, all return `Result<T, CommandError>`):

| Command | Args | Returns |
|---|---|---|
| `get_snapshot` | – | `UsageSnapshot` |
| `get_history` | `provider: Provider` | `History` |
| `refresh_now` | – | `()` |
| `get_settings` | – | `SettingsState { settings: Settings, readOnly: bool, notice: Option<String> }` |
| `set_settings` | `settings: Settings` | `SettingsState` (validated and clamped; error if `readOnly`) |
| `reset_settings` | – | `SettingsState` |
| `complete_onboarding` | – | `SettingsState` (sets `onboarding_completed = true`) |
| `claude_bridge_status` | – | `BridgeStatus { installed, chained: bool, effective: Effectiveness, settingsPath }` |
| `install_claude_bridge` / `uninstall_claude_bridge` | – | `BridgeStatus` |
| `detect_codex` | – | `CodexDetection { path: Option<String>, version: Option<String> }` (runs discovery §8.1 step 1 and `codex --version`) |
| `pick_path` | `kind: PathKind` (`File \| Directory`), `purpose: PathPurpose` (`CodexBinary \| CodexHome \| ClaudeDir`) | `Option<String>` (native dialog from **Rust** via `tauri-plugin-dialog`, non-blocking callback API bridged through a `oneshot`; `None` = cancelled). It does **not** save. The UI then calls `set_settings`. |
| `show_window` | `which: WindowTarget` (`Main \| Widget`), `route: Option<Route>` (`Overview \| Claude \| Codex \| Settings \| Onboarding`) | `()` (shows and focuses the window; if `route` is set, emits `navigate` to it) |
| `hide_window` | `which: WindowTarget` (`Popover \| Widget`) | `()` |
| `reveal_logs` | – | `()` (`tauri-plugin-opener` `reveal_item_in_dir(<log dir>)`) |
| `quit_app` | – | `()` (runs the §9.2 shutdown sequence, then `app.exit(0)`) |
| `ui_visible` | `label: String` | `()` (triggers a scheduler refresh when data is older than 30 s) |

All enum arguments (`PathKind`, `PathPurpose`, `WindowTarget`, `Route`, `Effectiveness`) are Rust enums exported with ts-rs. **There are no free-form strings.**

**Events:** `usage-updated` → `UsageSnapshot`; `settings-changed` → `SettingsState`; `bridge-status-changed` → `BridgeStatus`; `navigate` → `Route` (main window only).

**Tray menu → commands mapping** (Rust side calls the same functions): Open Dashboard → `show_window(Main, Overview)` · Show Floating Widget → `show_window(Widget)` · Refresh Now → `refresh_now` · Settings… → `show_window(Main, Settings)` · Quit → `quit_app`.

**`ui/src/ipc.ts`** is the only file that imports `@tauri-apps/api`:
```ts
/**
 * @file ipc.ts
 * @description Typed wrappers around Tauri invoke/listen. The ONLY module allowed to call Tauri APIs.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { UsageSnapshot } from "./bindings/UsageSnapshot";
import type { History } from "./bindings/History";
import type { Provider } from "./bindings/Provider";

export const getSnapshot = (): Promise<UsageSnapshot> => invoke<UsageSnapshot>("get_snapshot");
export const getHistory = (provider: Provider): Promise<History> => invoke<History>("get_history", { provider });
export const onUsageUpdated = (cb: (s: UsageSnapshot) => void): Promise<UnlistenFn> =>
  listen<UsageSnapshot>("usage-updated", (e) => cb(e.payload));
// …one wrapper per command in the table above
```

**Binding generation:** `cargo test -p usage-core export_bindings` writes `ui/src/bindings/*.ts` (configure `TS_RS_EXPORT_DIR` in `.cargo/config.toml` → `[env] TS_RS_EXPORT_DIR = { value = "ui/src/bindings", relative = true }`). CI check: regenerate, then `git diff --exit-code ui/src/bindings`.

### 10.4 Forwarder (`forwarder.rs`)
Loop: `snapshot_rx.changed().await` → throttle (at most one emit per 250 ms, trailing edge guaranteed) → `app.emit("usage-updated", &*snapshot)` → update the tray title and icon. A separate loop reads `alerts` → `tauri_plugin_notification`.

---

## 11. UI implementation (`ui/`)

The visual target is the approved design in **`docs/design/`**: 29 artboards covering every screen, route and state in dark and light, plus tokens and component math. Read `docs/design/README.md` first, and use `docs/design/DESIGN_IMPLEMENTATION_PROMPT.md` as the step-by-step UI guide. Appendix B is only a summary. Implementation rules:

### 11.1 Tokens (`styles/tokens.css`)
Copy **`docs/design/design-tokens.css`** and split it into `tokens.css` (the `:root` variables + the light-mode media query), `glass.css` (`.glass/.card/.tile/.ctl/.tipbox`), and `neon.css` (`.num/.neon`, keyframes, reduced-motion and reduced-transparency rules). Use its variable names everywhere (`--label`, `--label2`, `--claude`, `--codex`, `--warn`, `--crit`, `--claude-text`, …). There is no second token set. `--gk` is the glow multiplier: 1 in dark, 0.38 in light. Window roots keep `html, body { background: transparent; margin: 0; }`.

### 11.2 `NeonBar` (the hero component)
Props: `{ percent: number; accent: "claude" | "codex"; label: string; resetsAt: number | null; resetPending: boolean }`.
- Color: `percent >= 90` → `--crit`, `>= 75` → `--warn`, else the accent.
- Layers and numbers (track, reflection, fill, core, head, bloom, 100 % halo) follow `bar()` in `docs/design/source/gen.py` and the anatomy on `ComponentsDark.png`. Bloom is `0 0 (3+0.07p)·gk px` + `0 0 (8+0.2p)·gk px`, ×1.2 / ×1.25 for warn and crit.
- Fill: animate **`width: p%`** with `transition: width 600ms cubic-bezier(.3,1.5,.5,1)` (spring overshoot on the first render). Don't use `transform: scaleX`: it squashes the capsule ends, the specular head and the core line. At most 8 bars update every few seconds, so the layout cost is negligible.
- At 100 %: a slow pulse (brightness 1 ↔ 1.55 plus a halo, 1.8 s). Disabled under `prefers-reduced-motion: reduce`, which leaves a static halo.
- Always render the percent text (tabular numerals) and, at ≥ 75 %, a warning glyph. Color is never the only signal.
- Accessibility: `role="progressbar" aria-valuenow aria-valuemin=0 aria-valuemax=100 aria-label`.

### 11.3 Hooks
- `useSnapshot()`: calls `getSnapshot()` on mount and subscribes with `onUsageUpdated`. **Cleanup calls unlisten.** Calls `uiVisible(label)` on `visibilitychange`/focus.
- `useNow(1000)`: a single interval per window that drives every `Countdown`.
- `useHistory(provider)`: fetches on mount and when the snapshot's `lastUpdated` changes, at most once per 60 s.

### 11.4 Screens
- **Routing:** the `main` window uses a hash router with routes `#/` (Overview), `#/claude`, `#/codex`, `#/settings`, and `#/onboarding`, driven by the `navigate` event and in-app links. There is no router library: `window.location.hash` plus a `switch` over the `Route` type.
- **Dashboard:** segmented control Overview · Claude · Codex. Overview = two `ProviderCard`s. Detail = `HourlyChart` (24 bars, current hour highlighted), `DailyChart` (7 plain bars, **no limit or pace line**), and a stat row (peak hour, 7-day average, projection text).
  - **No "limit pace" line on token charts.** Token totals have no reliable quota denominator (providers weight usage differently), so any line there would be misleading.
  - **Projection text** comes only from `History.projection`, which is computed from the **Weekly limit percentage** (§7.6), never from token counts. Example: "At this pace: ~130% by reset (limit reached ≈ Fri 14:00)". Hide it when `projection` is `None`.
  - **Scope labels:** show `series.scopeLabel` under **each** chart separately (hourly and daily can differ, e.g. Codex). Show an "as of" time from `series.observedAt` when it's older than 5 min.
- **Popover:** a compact `ProviderCard` for each provider, plus a footer: Open Dashboard → `showWindow(Main, Overview)` · Float Widget → `showWindow(Widget)` · Settings → `showWindow(Main, Settings)` · Quit → `quitApp()`.
- **Settings route** sections: Accounts (Codex path with a **Browse…** button → `pickPath` → `setSettings`; a Detect button → `detectCodex`; Claude bridge install/uninstall with the `effective` state and hint), Display, Alerts, Refresh intervals, and Diagnostics (per-source `SourceHealth` list + a **Reveal Logs** button → `revealLogs`). When `readOnly` → show the notice banner + a "Reset settings" button, and disable inputs.
- **Widget:** variants `Pill` (280×72), `Stack` (160×180), and `Mini` (200×24, two thin bars only). A hover reveals the controls.
- **Onboarding route** (shown while `onboarding_completed == false`) has 3 steps: (1) Codex: `detectCodex` result, or a Browse… path picker; (2) enable real-time Claude updates: explains the `~/.claude/settings.json` change, the footer-hint side effect, and that project or organization settings can override it. Enable / Skip. (3) Notifications permission request. Finish → `completeOnboarding()` → navigate to Overview. **The Claude OAuth step is not part of v1** (it is added in M9 only if built with `claude-oauth`).
- **Empty and edge states** (all required): loading skeleton · `NotConfigured` with its hint + action button · `AuthExpired` · `Unsupported` · `Degraded` (small badge; the data is still shown) · `Stale` ("Updated 14m ago", dimmed bars) · a provider with only one window ("No 5-hour limit on this plan") · `resetPending` ("Reset — waiting for new data") · bridge `LikelyOverridden` (warning card with the §8.3 hint text) · settings `readOnly` banner.

---

## 12. Notifications, launch at login, logging
- Notifications: ask for permission in onboarding step 3, not at launch. Title examples: "Claude weekly limit at 90%", with body "Resets Mon 9:00 AM".
- Autostart: `tauri-plugin-autostart` (LaunchAgent), toggled from Settings.
- Logs: `~/Library/Logs/dev.headroom.app/`, rotated daily and keeping 7 files (§4.1). External text goes through `log_trunc` (§6.3). The "Reveal Logs" button in Settings → Diagnostics calls `reveal_logs`.

---

## 13. Milestones and tasks

Each task: **Goal · Files · Steps · Acceptance · Verify**. The standard verify commands from §0 rule 5 always apply as well.

### M0: Scaffold
- **T0.1 Workspace.** Create the root `Cargo.toml` (members `crates/*`, `src-tauri`), `rust-toolchain.toml` (with `targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]`), workspace lints (§6.1), and empty crates `usage-core` (lib), `usage-sources` (lib, with feature `claude-oauth = []` off by default), `statusline-bridge` (bin).
  *Acceptance:* `cargo build --workspace` succeeds with zero warnings.
- **T0.2 Tauri + UI.** Run `npm create tauri-app@latest` (React + TS). Move the frontend to `ui/`, set `frontendDist`/`devUrl` accordingly, and apply the TS strict flags and ESLint config (§4.2). Add npm scripts `typecheck` (`tsc --noEmit`) and `lint` (`eslint . --max-warnings 0`). Add `scripts/build-sidecar.sh` (§10.1) and wire it into `beforeDevCommand`.
  *Verify:* `npm --prefix ui run typecheck && npm --prefix ui run lint && cargo tauri dev` opens a window.
- **T0.3 Docs.** Create `docs/DECISIONS.md` with the first entries: "ts-rs chosen over tauri-specta (rc)", "macOS 26.0 minimum", and "Claude OAuth deferred to M9 behind feature `claude-oauth`".

### M1: Core domain (`usage-core`, no I/O)
- **T1.1** Types from §7.1 with constructors (`Percent::new` rejects NaN/∞ and clamps to 0..=100) and `log_trunc`. *Tests:* NaN rejected, 120 → 100, −1 → 0. `log_trunc` never splits a UTF-8 character.
- **T1.2** `classify()` (§7.2). *Tests:* 300 → Session, 10080 → Weekly, 60 → Other(60), None + hint.
- **T1.3** ts-rs export test `export_bindings`. *Test:* the generated `UnixSeconds.ts` contains `number`, not `bigint`.
- **T1.4** `merge.rs` (§7.4). *Tests:* **one test per rule I1–I4 and D1–D5**, plus the required race tests (a)–(e) listed in §7.4. The permutation test (c) must cover all orderings of a set of at least 5 events.
- **T1.5** `history.rs` (§7.5). *Tests:* 24 and 7 zero-filled buckets per series; each series carries the correct `source`, `scope`, and `scopeLabel` from the §7.5 table; Codex daily falls back to rollout when app-server daily data is older than 1 day; the Codex `Account` series ends at the latest reported UTC day and never shows a zero for the unreported current day; sources are never mixed within a series; dedupe; prune > 8 days; a DST-transition day (use `chrono_tz` in tests only, with `America/New_York` on 2026-11-01).
- **T1.6** `projection.rs`, `alerts.rs`. *Tests:* projection uses only the Weekly percentage (no token input exists in its signature); upward crossing fires once; the same threshold after a reset fires again; downward doesn't fire.

### M2: Parsers (DTO → domain), fixtures from Appendix A
- **T2.1** `parse/codex_app_server.rs`: `GetAccountRateLimitsResponse` and `AccountRateLimitsUpdatedNotification` → `Reading`. *Tests:* fixture A.2 gives exactly one **full** reading with one `Weekly` window at 2 % and plan "Pro". A sparse notification with only `secondary` gives `partial=true` and one window. A response whose `rateLimitsByLimitId` also has a `"premium"` entry yields no premium windows (test (f), §7.4).
- **T2.2** `parse/codex_rollout.rs`. *Tests:* fixture A.1 gives a full Weekly 1.0 % reading plus a token delta. The `premium` line gives no reading. The `resets_in_seconds` legacy line computes `resets_at` = 1790132400. Fixture A.6 (forked session) counts exactly **3000** tokens, following the four fork-safe rules in §2.2.
- **T2.3** `parse/claude_statusline.rs` (the bridge file format). *Tests:* A.4 gives Session 23.5 and Weekly 41.2. A missing `five_hour` gives a single window.
- **T2.4** `parse/claude_log.rs`. *Tests:* A.3 gives a token sum of 2+797+54097+37690 = 92586 and dedupe key `msg_…:req_…`. Lines with `type != "assistant"` are skipped.
- *(T2.5 moved to M9.)*

### M3: Statusline bridge binary
**All M3 verification uses a temporary `--out-dir`. It never touches the real Application Support directory.**
- **T3.1** Implement §8.3 bridge steps 1–4 (synchronous; deps `serde_json` + `tempfile` only). Use `#[allow(clippy::print_stdout)]` on the single print function.
  *Verify:*
  ```bash
  OUT=$(mktemp -d)
  cat crates/usage-core/tests/fixtures/claude_statusline_input.json | cargo run -q -p statusline-bridge -- --out-dir "$OUT"
  ```
  Expected: it prints `5h 24% · 7d 41%`, `$OUT/claude-rate-limits.json` exists and is valid JSON, and the exit code is 0. With invalid stdin (`echo garbage | … --out-dir "$OUT"`), the exit code is 0 and nothing is printed.
- **T3.2** Chaining (tempdir): put `{"chainedCommand":"cat >/dev/null; echo CHAINED"}` in `$OUT/bridge.json` and verify the output is `CHAINED`. Put `"sleep 5"` there and verify it returns in 2.0–2.3 s and the rate-limit file was still written.
- **T3.3** Concurrency and budget (Rust integration test in `crates/statusline-bridge/tests/`, using `env!("CARGO_BIN_EXE_headroom-statusline")`):
  - Launch **50 overlapping invocations** (threads, each spawning the binary with the fixture on stdin and the same tempdir `--out-dir`). Kill every 5th one at a random 0–10 ms delay. Afterwards: the target file parses as valid JSON, and **no `.tmp*` files remain** except from killed processes (these must be removed by the startup cleanup function, which is also tested here).
  - Budget: 100 sequential unchained runs of the **release** binary → p95 < 20 ms (`cargo test --release -p statusline-bridge`).

### M4: I/O infrastructure (`usage-sources`)
- **T4.1** `paths.rs` + `event.rs` + `store.rs` actor (§9.1). *Tests:* send `Reading`s through the channel and assert the derived watch snapshot; cancel → the task ends within 100 ms.
- **T4.2** `tail.rs`. *Tests* (tempdir): appended lines are read once; a partial line is completed on the next append; truncation resets; a line > 16 MB is skipped.
- **T4.3** `watch.rs` (notify wrapper → bounded mpsc of paths, 500 ms per-path debounce). *Test:* writing a file in a tempdir yields one event.
- **T4.4** `scheduler.rs` (§9.4). Use `tokio::time::pause()` in tests. *Tests:* active vs idle intervals, 15 s min gap, and backoff doubling with a cap.

### M5: Sources (v1: no OAuth)
- **T5.1** `codex/rpc.rs` over `tokio::io::duplex`. *Tests:* request/response correlation, timeout removes the pending entry (assert the map is empty), server→client request gets `-32601`, and notifications are forwarded.
- **T5.2** `codex/app_server.rs` + `discover.rs` (§8.1), including the limit-ID filter, `-32601` → `Unsupported`, and PID recording. *Manual verify* with the probe (T5.6).
- **T5.3** `codex/rollout.rs` (§8.2). *Test:* in a temp `sessions/2026/09/23/`, append fixture A.1 lines → full `Reading` + `Tokens` events; the premium line → tokens only.
- **T5.4** `claude/bridge_install.rs`. *Tests* (tempdir as the claude dir and app-support dir):
  1. Install into a missing `settings.json` creates `{"statusLine":{"type":"command","command":"'…'"}}`.
  2. An existing `statusLine` `{"type":"command","command":"x.sh","padding":2,"refreshInterval":5,"hideVimModeIndicator":true,"futureKey":{"a":1}}` → after install, **every** key except `type`/`command` is preserved exactly, and `chainedCommand == "x.sh"`.
  3. Unrelated top-level keys are untouched (compare as `serde_json::Value`).
  4. Invalid JSON or a non-object top level → error, and the file is unchanged (compare bytes).
  5. Uninstall after the user added a new top-level key `"theme":"dark"` post-install → `statusLine` is restored to exactly the previous `Value` (or absent), and `"theme"` is **kept**.
  6. Uninstall when `statusLine.command` was changed by the user → the file is unchanged (bytes) and the result says "left untouched".
- **T5.5** `bridge_source.rs` + `claude/history.rs` + the effectiveness state machine (§8.3). *Tests:* `Unverified → Confirmed` on the first write after `installedAt`; `Unverified → LikelyOverridden` when log activity is more than 10 min after install with no write (use `tokio::time::pause()` and injected clocks).
- **T5.6** `examples/probe.rs`: runs all enabled sources + the store and prints each derived snapshot as pretty JSON.
  *Verify:*
  ```bash
  cargo run -p usage-sources --example probe
  ```
  It shows the Codex weekly % matching `codex` `/status`. Run one Claude Code prompt in another terminal and a Claude reading appears within about 1 s (bridge installed).

### M6: Tauri shell
- **T6.1** Settings load, migrate, and save (§9.3). *Tests:* missing file → defaults with `onboarding_completed=false`; a v0 file (no `schema_version`) → migrated to v1; `schema_version: 99` → `readOnly=true` and the file is unchanged; corrupt JSON → renamed to `.corrupt-*`; clamping.
- **T6.2** Commands (§10.3, the full table) + forwarder (§10.4) + events. `pick_path` uses the non-blocking dialog callback (never `blocking_*` on an async command).
- **T6.3** Tray (icon states, title, left click → popover, right-click menu → the §10.3 mapping) + popover positioning + hide-on-blur.
- **T6.4** Main window hide-on-close, **toolbar drag region** + `core:window:allow-start-dragging`, first-launch onboarding route, widget window (drag, persisted position, always on top), activation policy.
- **T6.5** Plugins: liquid glass, notification, autostart, positioner, dialog, opener. `externalBin` via `scripts/build-sidecar.sh`.
  *Verify:* `cargo tauri dev`. The tray shows a %. Clicking opens the popover under the icon. The main window can be dragged by its toolbar. Closing main leaves the app running. Quit from the tray menu exits cleanly and no `codex app-server` child is left (§14.3 check).

### M7: UI
> **Before M7:** read `docs/design/README.md` and `docs/design/DESIGN_IMPLEMENTATION_PROMPT.md`, and open `docs/design/preview/index.html`. Every M7 task names the screenshots it must match.

- **T7.1** Tokens, `glass.css`, `NeonBar` (+ a Storybook-free demo route `#/demo`, debug builds only, that shows every state: 0, 30, 76, 91, 100, resetPending). *Match:* `ComponentsDark.png` / `ComponentsLight.png`.
- **T7.2** `ProviderCard`, `Countdown`, `StatusBadge`, `Popover` screen (footer actions wired to commands).
- **T7.3** `HourlyChart`, `DailyChart` (no pace line), `StatTile`, `Dashboard`, hash router, and a per-series scope label.
- **T7.4** `Widget` (3 variants), `Settings` route (Browse…, Detect, bridge status, Diagnostics, Reveal Logs, read-only banner), `Onboarding` route.
- **T7.5** All §11.4 edge states. Light and dark mode. Reduced motion.
  *Verify:* screenshot every screen and state in both color schemes, and compare against the matching file in `docs/design/screenshots/` at the same size. Spacing should be within ±2 pt, radii identical, and every color from a token.

### M8: Hardening and local release
- **T8.1** Leak and soak test (§14.3). Run the **automated 60-minute accelerated soak** (§14.3, "Agent run"), and record the results in `DECISIONS.md`. The 8-hour run goes on the handover checklist.
- **T8.2 Universal local build (ad-hoc signed, NOT distributable):**
  ```bash
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  scripts/build-sidecar.sh universal-apple-darwin   # builds both arches, lipo -create → binaries/headroom-statusline-universal-apple-darwin
  cargo tauri build --target universal-apple-darwin
  ```
  Then run each check separately:
  ```bash
  lipo -archs "src-tauri/target/universal-apple-darwin/release/bundle/macos/Headroom.app/Contents/MacOS/headroom"
  ```
  Expected: `x86_64 arm64`.
  ```bash
  lipo -archs "src-tauri/target/universal-apple-darwin/release/bundle/macos/Headroom.app/Contents/MacOS/headroom-statusline"
  ```
  Expected: `x86_64 arm64`.
  Then ad-hoc sign (`codesign --force --deep -s - "<app>"`), copy to `/Applications`, and smoke test that PATH discovery works without a terminal. *Label the artifact "local build only".*
- **T8.3** Write a README: install, permissions (notifications), the bridge (what it changes, how to uninstall, the project/org override caveat), and the "local build only" note.
- **T8.4 (optional) Distribution build.** Developer ID Application signing + notarization (`xcrun notarytool submit … --wait`, then `xcrun stapler staple`), following the Tauri macOS distribution guide. **Requires the owner's Apple Developer credentials. The implementing agent must not handle them.** It prepares the config (`bundle.macOS.signingIdentity`, the notarization env var names) and the owner runs the signing step.

### M9: Post-v1, Claude OAuth poller (feature `claude-oauth`), only after owner sign-off
- **T9.0 Gate.** The owner records acceptance of the unofficial endpoint's policy risk, response shape, and failure semantics in `DECISIONS.md`. Without that entry, **stop**.
- **T9.1 ⚠ VERIFY the shape.** Add to the probe a `--claude-oauth-dump-keys` flag (feature-gated) that reads the Keychain, calls the endpoint, and prints **only JSON key paths and value types, never values**. Compare with A.5 and update the fixture and parser. Record the result in `DECISIONS.md`.
- **T9.2** `parse/claude_oauth.rs` (lenient). *Tests:* A.5 parses; `resets_at` as an ISO string, as a number, or as null; unknown keys are ignored.
- **T9.3** `claude/keychain.rs` + `claude/oauth.rs` (§8.4). *Tests:* the HTTP layer is behind a small trait with a fake. 401 → `AuthExpired`; **403 and 404 → `Unsupported`**, and polling stops; 429 honors `Retry-After`; 5xx → backoff. `AccessToken`'s `Debug` output is redacted.
- **T9.4** Settings toggle + onboarding step + warning copy (unofficial endpoint, Keychain prompt).

---

## 14. Testing strategy

### 14.1 Unit (usage-core)
The fixture-driven parser tests and the merge, history, projection, and alert tests run with no I/O. Target **≥ 90 % line coverage** for `usage-core` (requires `cargo install cargo-llvm-cov`):
```bash
cargo llvm-cov -p usage-core --fail-under-lines 90
```

### 14.2 Integration (usage-sources, statusline-bridge, src-tauri settings)
- Tempdir-based file tests (tail, watch, rollout, installer, bridge concurrency, settings migration).
- RPC over `tokio::io::duplex` with a scripted fake server.
- Store and scheduler under `tokio::time::pause()`.
- **Never** hit real Anthropic or OpenAI endpoints or the real `~/.claude` / `~/.codex` / Application Support directories in automated tests. Use tempdirs, `--out-dir`, and overrides.

### 14.3 Leak and soak (manual, M8)
1. Point the settings overrides at a temp `codex_home` and `claude_dir`. Run a script that appends one synthetic `token_count` line and one Claude assistant line **every second**, and rewrites the bridge file every 5 s.
2. Run the release build for **8 hours** with the popover toggled every minute (AppleScript or manually for the first 10 min).
3. Every 5 min, record `footprint -p <app_pid> | grep "phys_footprint:"` for the main process into a CSV. Get `<app_pid>` from `pgrep -x headroom` (the exact process name of *our* binary).
4. **Pass criteria:**
   - `leaks <app_pid>` → `0 leaks for 0 total leaked bytes` (main Rust process)
   - Main-process footprint growth from hour 1 to hour 8 is **< 10 %**
   - Idle CPU is < 0.5 % averaged over 10 min (Activity Monitor) with no new events
   - **Our** child `codex app-server` count is always ≤ 1, counted as **children of our PID only**: `pgrep -P <app_pid> -f "app-server" | wc -l`. Cross-check against the PID logged at spawn (§8.1 step 8). Unrelated Codex processes the user runs are ignored.
   - After Quit: `pgrep -P <app_pid>` returns nothing, and the child PID from the log no longer exists (`kill -0 <child_pid>` fails).
5. WebView: in a debug build, use Safari → Develop → Headroom → Timelines/Memory. Heap size must return to baseline after 50 open/close cycles of the dashboard.

**Agent run (automated, required for T8.1).** Add `scripts/soak.sh`:
- Temp `codex_home` and `claude_dir` overrides.
- A synthetic event generator at **10× speed**: one `token_count` line and one Claude assistant line every 100 ms, and a bridge-file rewrite every 500 ms.
- The release build runs for **60 minutes**, with `ui_visible` triggered every 30 s.
- The `footprint` / `pgrep` samples from steps 3–4 are taken every minute into `target/soak/soak.csv`.

Pass criteria are the same as step 4, with growth measured from minute 10 to minute 60. `leaks` runs at the end. The script exits non-zero on any failure. The full 8-hour run and the Safari WebView check go on the handover checklist (§15).

### 14.4 Manual acceptance script
The implementing agent automates what it can (e.g. with fixtures, temp dirs and the probe) and copies every remaining item, unchanged, into the handover checklist (§15). Each item on this list needs a person at the Mac.
- [ ] Fresh install → onboarding → the Codex bar appears within 5 s.
- [ ] Enable the Claude bridge → run a Claude Code prompt → the Claude bars update in < 2 s, and the bridge state becomes `Confirmed`.
- [ ] Create a project with `.claude/settings.local.json` that sets its own `statusLine`, then use Claude Code only there for 10+ min → the bridge state becomes `LikelyOverridden` with the hint.
- [ ] Quit Claude Code → the Claude bars show "Updated Xm ago" and turn Stale after 15 min.
- [ ] Use Codex in a terminal → the Codex bar updates in < 15 s.
- [ ] Kill our `codex app-server` child → the Codex status shows `Degraded`, the bars keep updating from rollout files, and the child restarts with backoff.
- [ ] Sleep the Mac for 10 min → wake → all data refreshes within 5 s.
- [ ] Add a key to `~/.claude/settings.json` after install → uninstall the bridge → `statusLine` is restored and the added key is still there.
- [ ] Rename the `codex` binary → Codex shows `NotConfigured` with a hint, and the app doesn't crash.
- [ ] Settings → Browse… picks a path, Reveal Logs opens Finder, and Quit exits cleanly.

---

## 15. Definition of Done (v1)
- [ ] M0–M8 are complete (T8.4 excluded), and every automatable *Verify* passes.
- [ ] Every phase has a review record in `docs/PROGRESS.md` and one commit.
- [ ] The 60-minute automated soak (§14.3 "Agent run") passes.
- [ ] `docs/HANDOVER.md` exists. It lists every step the agent couldn't do itself, each with exact instructions and expected results: the 8-hour soak, the Safari WebView memory check, each §14.4 manual item, live ⚠ VERIFY checks that couldn't run, and T8.4 if wanted.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean. `tsc` and ESLint are clean.
- [ ] No `unsafe` outside the documented Tauri-macro exception. No `unwrap`/`expect` in non-test code.
- [ ] The v1 build is compiled **without** `claude-oauth`, and `cargo tree -p usage-sources -e features` shows no `reqwest` or `security-framework`.
- [ ] The universal app and sidecar pass the `lipo -archs` checks (T8.2).
- [ ] The soak test passes (§14.3), and the results are in `DECISIONS.md`.
- [ ] No secrets appear in logs (`grep -ri "bearer\|accessToken\|sk-" ~/Library/Logs/dev.headroom.app` returns nothing).
- [ ] The README covers install, permissions, the bridge (including the override caveat), and the local-build note.

---

## 16. Risks and open questions

| Risk | Impact | Mitigation |
|---|---|---|
| Claude OAuth endpoint is undocumented | It may break, change shape, or be disallowed | **Deferred to M9** behind a compile-time feature and an owner sign-off gate. The v1 bars rely only on the documented status line |
| `codex app-server` is `[experimental]` | Method names or shapes may change | Schema regeneration step (§2.1), `Unsupported` handling, a rollout-file fallback (`Degraded`), and a note in the README of which Codex version was tested |
| Liquid Glass plugin uses a private API | May break on a future macOS | Isolated behind one call per window. On failure, windows render with opaque tinted panels (the Reduce Transparency style) |
| Modifying `~/.claude/settings.json` | User trust | Explicit consent, a full-object-preserving install, semantic uninstall that keeps later user edits, and a manual backup |
| Higher-precedence Claude settings (project, local, managed, MDM, server) override our `statusLine` | "Installed" but no data | Behavioral effectiveness check (`Confirmed` / `LikelyOverridden`) with a clear hint (§8.3) |
| Claude bars go stale when Claude Code is idle (v1 has no poller) | Old data shown | Clear "Updated Xm ago" and `Stale` states. This is expected behavior, not a bug |
| Only a local, ad-hoc-signed build in v1 | Gatekeeper warnings on other Macs | T8.4 (Developer ID + notarization) when the owner provides credentials |
| Multiple Claude accounts or config dirs | The wrong account could be shown | Out of scope. One `claude_dir` setting |

---

## 17. Change log

- **v2.2 (2026-09-23), final, cleared for autonomous implementation:**
  - Status set to final for M0–M8, and the re-review gate removed.
  - New §0.1: decision rules (decide → log → continue), hard stops limited to real user data, secrets and `sudo`/GUI toolchain installs, and resuming from `PROGRESS.md`.
  - §0 rules 2–3 point to §0.1. T8.1 and §14.3 gain an automated 60-minute accelerated soak. §14.4 and §15 add `docs/HANDOVER.md` for human-only checks.
- **v2.1 (2026-09-23), design alignment:**
  - Linked the approved design package `docs/design/` (§5, §11, M7, Appendix B). `design-tokens.css` replaces the §11.1 values, and the NeonBar fill animates `width`, not `scaleX` (§11.2).
  - §2.2: fork-safe Codex token counting (the old rule double-counted forked sessions) + fixture A.6 + T2.2 test.
  - §2.1 / §7.5: account daily buckets are UTC days and omit the current day. The Codex `Account` series ends at the latest reported day, never a fake zero (T1.5 test).
  - §10.2: widget window sized per variant, glass radius per variant (36/24/12), hover UI inside the bounds, edge snapping.
  - Appendix B: corrected the menu bar (template icon, uncolored title), Settings, widget and state lists to match §10–§11.
- **v2 (2026-09-23), review #1 corrections:**
  - Merge model rewritten as per-source state + a priority-derived view (§7.4). Full reads now clear absent windows. Fallback-source errors can't override a healthy primary. Non-`codex` limit IDs are filtered.
  - `History` split into `hourly`/`daily` `Series`, each carrying `source`, `scope`, `scopeLabel`, and `observedAt`. `SourceEvent::Daily` carries source and time.
  - Removed the token-chart "limit pace" line. Projection is percentage-only.
  - Bridge installer clones the whole `statusLine` object. Added the settings-precedence effectiveness check. Uninstall is now semantic.
  - Bridge: synchronous (`serde_json` + `tempfile`), separate fast-path and chained budgets, unique temp files, `--out-dir` for tests, and a concurrency test.
  - Frontend contract completed: Settings and Onboarding routes, full IPC table (`pick_path`, `reveal_logs`, `quit_app`, `detect_codex`, …), versioned settings with migrations and `onboarding_completed`.
  - Logging corrected to daily rotation + `max_log_files(7)` + manual truncation.
  - Platform floor set to macOS 26.0. Universal build and sidecar `lipo` procedure added. Ad-hoc signing labeled local-only, and distribution moved to optional T8.4.
  - OAuth: 403/404 → `Unsupported`, and the whole poller moved to post-v1 M9 behind a feature flag and a sign-off gate.
  - Main toolbar drag region + capability. The soak test counts only our child PIDs.
- **v1 (2026-09-23):** initial draft.

---

## Appendix A: Fixtures (copy to `crates/usage-core/tests/fixtures/`)

### A.1 `codex_rollout.jsonl` (real shape, codex-cli 0.154.0)
```jsonl
{"timestamp":"2026-09-23T01:13:09.852Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":12137712,"cached_input_tokens":11909760,"output_tokens":18549,"reasoning_output_tokens":13481,"total_tokens":12156261},"last_token_usage":{"input_tokens":211494,"cached_input_tokens":210688,"output_tokens":60,"reasoning_output_tokens":14,"total_tokens":211554},"model_context_window":258400},"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":1.0,"window_minutes":10080,"resets_at":1790725056},"secondary":null,"credits":{"has_credits":false,"unlimited":false,"balance":"0"},"plan_type":"pro","rate_limit_reached_type":null}}}
{"timestamp":"2026-07-31T19:11:13.991Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"premium","limit_name":null,"primary":null,"secondary":null,"plan_type":null}}}
{"timestamp":"2026-09-23T02:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"limit_id":"codex","primary":{"used_percent":12.5,"window_minutes":300,"resets_in_seconds":3600},"secondary":null}}}
```
(The third line is the synthetic legacy `resets_in_seconds` case. Expected `resets_at` = 1790128800 + 3600 = 1790132400.)

### A.2 `codex_rate_limits_read.json` (real response, `accountId` removed)
```json
{"id":2,"result":{"ordinaryUsageAllowed":true,"rateLimits":{"limitId":"codex","limitName":null,"normalModelSlug":null,"primary":{"usedPercent":2,"windowDurationMins":10080,"resetsAt":1790725056},"secondary":null,"credits":{"hasCredits":false,"unlimited":false,"balance":"0"},"individualLimit":null,"spendControlReached":false,"planType":"pro","rateLimitReachedType":null},"rateLimitsByLimitId":{"codex":{"limitId":"codex","limitName":null,"normalModelSlug":null,"primary":{"usedPercent":2,"windowDurationMins":10080,"resetsAt":1790725056},"secondary":null,"credits":{"hasCredits":false,"unlimited":false,"balance":"0"},"individualLimit":null,"spendControlReached":false,"planType":"pro","rateLimitReachedType":null}},"rateLimitResetCredits":{"availableCount":1,"credits":null},"rateLimitUpsell":null}}
```
`codex_rate_limits_updated.json` (synthetic sparse notification):
```json
{"method":"account/rateLimits/updated","params":{"rateLimits":{"limitId":"codex","secondary":{"usedPercent":37,"windowDurationMins":300,"resetsAt":1790060000}}}}
```

### A.3 `claude_log.jsonl` (real shape, trimmed; the duplicate line is intentional)
```jsonl
{"type":"assistant","timestamp":"2026-08-26T10:25:31.875Z","sessionId":"00000000-0000-4000-8000-000000000001","requestId":"req_01TEST","message":{"id":"msg_01TEST","model":"claude-fable-5","usage":{"input_tokens":2,"cache_creation_input_tokens":54097,"cache_read_input_tokens":37690,"output_tokens":797,"service_tier":"standard"}}}
{"type":"assistant","timestamp":"2026-08-26T10:25:31.875Z","sessionId":"00000000-0000-4000-8000-000000000001","requestId":"req_01TEST","message":{"id":"msg_01TEST","model":"claude-fable-5","usage":{"input_tokens":2,"cache_creation_input_tokens":54097,"cache_read_input_tokens":37690,"output_tokens":797,"service_tier":"standard"}}}
{"type":"user","timestamp":"2026-08-26T10:25:40.000Z","message":{"role":"user","content":"hi"}}
```

### A.4 `claude_statusline_input.json` (shape from the official docs)
```json
{"session_id":"abc123","model":{"id":"claude-opus-5-5","display_name":"Opus"},"workspace":{"current_dir":"/tmp"},"context_window":{"used_percentage":8},"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":1738425600},"seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}
```

### A.5 `claude_oauth_usage.json` (post-v1: ⚠ VERIFY in T9.1, expected shape)
```json
{"five_hour":{"utilization":23.0,"resets_at":"2026-09-23T15:00:00.000000+00:00"},"seven_day":{"utilization":41.0,"resets_at":"2026-09-28T09:00:00.000000+00:00"},"seven_day_opus":null,"seven_day_sonnet":{"utilization":12.0,"resets_at":"2026-09-28T09:00:00.000000+00:00"}}
```
Only map `five_hour` → Session and `seven_day` → Weekly. Ignore the other keys for now, but log their names once at `debug` level.

### A.6 `codex_rollout_fork.jsonl` (synthetic; one file of a forked session whose first total carries the parent's history)
```jsonl
{"timestamp":"2026-09-23T03:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":4990000,"cached_input_tokens":4900000,"output_tokens":10000,"reasoning_output_tokens":2000,"total_tokens":5000000},"last_token_usage":{"input_tokens":900,"cached_input_tokens":800,"output_tokens":100,"reasoning_output_tokens":10,"total_tokens":1000},"model_context_window":258400},"rate_limits":{"limit_id":"codex","primary":{"used_percent":3.0,"window_minutes":10080,"resets_at":1790725056},"secondary":null}}}
{"timestamp":"2026-09-23T03:00:01.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":4990000,"cached_input_tokens":4900000,"output_tokens":10000,"reasoning_output_tokens":2000,"total_tokens":5000000},"last_token_usage":{"input_tokens":900,"cached_input_tokens":800,"output_tokens":100,"reasoning_output_tokens":10,"total_tokens":1000},"model_context_window":258400},"rate_limits":{"limit_id":"codex","primary":{"used_percent":3.0,"window_minutes":10080,"resets_at":1790725056},"secondary":null}}}
{"timestamp":"2026-09-23T03:05:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":4991800,"cached_input_tokens":4901500,"output_tokens":10200,"reasoning_output_tokens":2040,"total_tokens":5002000},"last_token_usage":{"input_tokens":1800,"cached_input_tokens":1500,"output_tokens":200,"reasoning_output_tokens":40,"total_tokens":2000},"model_context_window":258400},"rate_limits":{"limit_id":"codex","primary":{"used_percent":3.0,"window_minutes":10080,"resets_at":1790725056},"secondary":null}}}
```
Expected tokens: line 1 → 1000 (first line: `last_token_usage` only, **not** 5,000,000), line 2 → 0 (repeat), line 3 → 2000. Total **3000**.

---

## Appendix B: UI design brief

> **Superseded for visuals by `docs/design/`** (canvas version 4, 29 artboards). This brief is kept as a summary. Where the two differ, `docs/design/` and §10–§11 win.

Build to this brief. Where it conflicts with §11 (e.g. the weekly chart covers the last 7 days, not Mon–Sun), §11 wins.

- **Style:** macOS 26 Liquid Glass. Translucent, refractive panels over the wallpaper, continuous-curvature corners (20–28 pt), and soft specular edges.
- **Neon progress bars are the hero.** They are thick capsule tracks with a glowing fill (bright core, outer bloom, faint reflection). Claude uses coral `#FF8A5B` and Codex uses cyan `#3CF2FF`. Amber at ≥ 75 %, magenta-red at ≥ 90 %, and a slow pulse at 100 %. Neon is for data only, and the chrome stays calm and native.
- **Typography:** SF Pro, with large tabular numerals for percentages. **Icons:** SF Symbols style, with service badges in small glass chips.
- **Motion:** bars spring-fill on load, countdowns tick live, and hovering shows exact values in a glass tooltip.
- **Main window (720×520):** a glass toolbar with segmented Overview · Claude · Codex, refresh, "last updated", and a collapse-to-widget button. Overview has two cards, each with a service badge, plan, a Session (5h) bar + "Resets in 2h 14m", a Weekly bar + "Resets Mon 9:00 AM", and a today sparkline. Each detail tab has a 24 h hourly chart, a 7-day chart (plain bars, no limit line), and stats (peak hour, average per day, a percentage-based projection: "At this pace: ~130% by reset").
- **Menu bar:** a monochrome **template** icon in 3 bundled states (normal / warning ≥ 75 with a triangle badge / critical ≥ 90 with a dot badge) plus a plain-text title with the highest used %. Neither can be colored (§10.2). The popover (340×420, radius 18) has compact rows per service and a footer: Open Dashboard / Float Widget / Settings / Quit.
- **Floating widget:** Pill 280×72, Stack 160×180, and Mini 200×24, with the window sized to the variant. It's draggable, always on top, and snaps to screen edges. Hovering shows close/expand/opacity controls **inside** the widget; Mini shows its values inline.
- **Settings (route in the main window):** Accounts (Codex path + Browse… / Detect, Claude bridge Enable/Disable with its effectiveness state; Claude OAuth only post-v1), Display (launch at login, Dock icon, widget show/style/opacity), Alerts (75/90/100, reset), Refresh intervals, and Diagnostics (per-source health, Reveal Logs, version). Read-only banner when the settings file is newer.
- **States:** normal, warning, critical, limit reached (the countdown becomes the headline), loading shimmer, not configured (Claude: enable real-time updates; Codex: Detect / Choose Path), degraded (badge, data still shown), stale (dimmed + "Updated 14m ago"), single-window plan, reset pending, bridge likely overridden, unsupported, auth expired (post-v1), dark (full neon), and light (toned-down glow).
- **Accessibility:** % text and an icon always accompany color. Respect Reduce Motion and Reduce Transparency (solid tinted panels). Text sits on sufficiently opaque glass.
