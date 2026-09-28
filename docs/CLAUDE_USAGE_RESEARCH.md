# Claude usage retrieval: GitHub implementation research

Research date: 2026-09-28. This is a source review and recommendation, not an implementation change or a claim that these apps were tested against the current account.

Follow-up: the manual CLI approach is now implemented in Headroom; implementation choices and live-probe evidence are recorded in [D-031](DECISIONS.md#d-031-manual-claude-code-usage-refresh-2026-09-28). Comparisons below describe the workspace at the time of research.

## Conclusion

Existing apps implement three ways to obtain subscription limits: Claude Code OAuth credentials, a claude.ai web session, and automated CLI `/usage` output. The proposed temporary CLI session has direct precedents. The earlier local probe reaching a rate-limit error establishes a failure condition to handle; it does not disprove the approach.

For Headroom, use CodexBar as the main architectural reference for a bounded manual CLI refresh. Retain the existing status-line and optional OAuth sources. A separate browser-session connection is the relevant future option for people who have never installed or signed into Claude Code.

## Repositories inspected

GitHub API metadata and raw source were read at these revisions. No third-party app was installed or executed.

| Repository | Inspected commit | Main approach |
| --- | --- | --- |
| [steipete/CodexBar](https://github.com/steipete/CodexBar) | `c33760ecc34b122f985327aa917557b7753691f7` | OAuth, CLI PTY, and browser session |
| [lionhylra/cc-usage-bar](https://github.com/lionhylra/cc-usage-bar) | `c483481c0fabc0057a4f72ff45fe098cc2f93f15` | Interactive CLI in a PTY |
| [Bread-bang/claude-usage](https://github.com/Bread-bang/claude-usage) | `93b9a0b858db2e475f3d144b7277db9b502a9c64` | Claude Code OAuth credential |
| [thinshaw/claude-usage-widget](https://github.com/thinshaw/claude-usage-widget) | `271e06ff205516e9c6437230e2c56ee8da4c1335` | claude.ai session cookie |

The implementation files below are stronger evidence than README feature claims. Repository presence and code paths do not establish a successful live request on this machine.

## 1. CC Usage Bar: the direct precedent for the proposed CLI flow

Its [UsageViewModel](https://github.com/lionhylra/cc-usage-bar/blob/c483481c0fabc0057a4f72ff45fe098cc2f93f15/CCUsageBar/CCUsageBar/UsageViewModel.swift#L76-L175) opens a PTY, launches `claude` through a login shell in an empty temporary directory, then drives the terminal with a state machine. It waits for startup, writes `/usage`, and waits for the command echo before sending Enter.

The [capture logic](https://github.com/lionhylra/cc-usage-bar/blob/c483481c0fabc0057a4f72ff45fe098cc2f93f15/CCUsageBar/CCUsageBar/UsageViewModel.swift#L236-L375) detects a usage panel, changes terminal dimensions to force a full redraw, and waits for 1.5 seconds of quiet. There is a 30-second timeout. It renders ANSI output instead of returning a stable usage JSON contract.

**README discrepancy:** successful captures keep the process alive. Popover dismissal sends Escape; later refreshes reuse the session. Explicit teardown handles timeout, setup, and rate-limit errors. This differs from the README's claim of immediate termination. Its hard-coded prompt recognition also warrants version-specific tests before adaptation.

## 2. CodexBar: the most complete reference reviewed

The main app's [source planner](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeSourcePlanner.swift#L175-L205) orders automatic candidates as OAuth, CLI, then Web. Availability and error rules govern which fallback actually runs. This is not a rule to try every source after every error.

### CLI process management and parsing

The [status probe](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeStatusProbe.swift#L75-L191) runs `/usage`, validates the result before optionally requesting identity through `/status`, and defaults to ending the temporary session on both success and failure. Keeping sessions alive is optional. It also cleans probe-owned conversation artifacts.

The [session implementation](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeCLISession.swift#L315-L428) uses an app-owned working directory, PTY descriptors, fixed launch arguments, and app-shutdown process tracking. Its process-local settings disable Remote Control startup. These controls explain why a managed subprocess is preferable to opening and minimizing Terminal.app.

Its [terminal screen parser](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeCLIScreen.swift#L3-L90) replays cursor movement and erase operations into a bounded screen. Stripping escape sequences alone can combine old and new frames or remove spacing needed to associate percentages with labels.

There is also a [direct subprocess fallback](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeUsageFetcher.swift#L1328-L1351): invoke `claude` with `/usage`, null stdin, a timeout, and captured stdout. It does not pass `-p`. Its existence is worth investigating, but compatibility with the installed CLI has not been verified here.

### Polling and failures

The [background spawn throttle](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeCLIUsageSpawnThrottle.swift#L3-L43) caches successful CLI results for up to 15 minutes, expiring them sooner when a usage window resets. Manual refresh can bypass that spawn cache.

A separate [CLI rate-limit gate](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeCLIRateLimitGate.swift#L3-L40) applies a five-minute background cooldown. The [OAuth gate](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthUsageRateLimitGate.swift#L3-L43) records a provider retry deadline or a default cooldown. Those gates exempt user-initiated attempts; Headroom should retain its own server-backoff policy rather than copy that exemption automatically.

### OAuth and Web

The [OAuth fetcher](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/Sources/CodexBarCore/Providers/Claude/ClaudeOAuth/ClaudeOAuthUsageFetcher.swift#L61-L117) calls `GET https://api.anthropic.com/api/oauth/usage` with a bearer token and `anthropic-beta: oauth-2025-04-20`. It handles 401 and 429 separately and records successful decoding.

The [provider documentation](https://github.com/steipete/CodexBar/blob/c33760ecc34b122f985327aa917557b7753691f7/docs/claude.md) describes browser-session import and claude.ai organization usage requests as another source. Its Web path distinguishes a Cloudflare challenge from an expired login and retains prior measurements. These are internal service interfaces, not evidence of a documented third-party subscription-usage API.

## 3. Bread-bang Claude Usage: direct OAuth polling

Its [UsageClient](https://github.com/Bread-bang/claude-usage/blob/93b9a0b858db2e475f3d144b7277db9b502a9c64/Sources/ClaudeUsageMiniBar/Networking/UsageClient.swift#L3-L85) calls the same OAuth usage endpoint, with bearer authentication, the beta header, a 15-second timeout, and explicit 401/429 handling.

Its [TokenProvider](https://github.com/Bread-bang/claude-usage/blob/93b9a0b858db2e475f3d144b7277db9b502a9c64/Sources/ClaudeUsageMiniBar/Auth/TokenProvider.swift#L6-L83) normally reads Claude Code's credential and leaves renewal to Claude Code. Self-refresh defaults to disabled. The maintainer documents that independently redeeming the shared rotating refresh token caused Claude Code sign-in failures in their tests; this is their reported evidence, not a test performed here. It supports preserving Headroom's existing read-only credential ownership.

The [view model](https://github.com/Bread-bang/claude-usage/blob/93b9a0b858db2e475f3d144b7277db9b502a9c64/Sources/ClaudeUsageMiniBar/ViewModel/UsageViewModel.swift#L20-L108) retains the last successful report across requests and errors, persists its timestamp, and lengthens polling after a rate-limit error. Displaying cached usage is distinct from a successful current refresh.

## 4. Claude Usage Widget: usage without a CLI installation

Its [provider](https://github.com/thinshaw/claude-usage-widget/blob/271e06ff205516e9c6437230e2c56ee8da4c1335/Sources/ClaudeAIUsageProvider.swift#L3-L145) authenticates using a `sessionKey` cookie, discovers organizations through `GET https://claude.ai/api/organizations`, and fetches `GET /api/organizations/{org_uuid}/usage`. Requests explicitly isolate account cookies. The first successful organization is selected initially; settings can override that choice.

Its [embedded sign-in view](https://github.com/thinshaw/claude-usage-widget/blob/271e06ff205516e9c6437230e2c56ee8da4c1335/Sources/InAppAuthView.swift#L3-L169) opens claude.ai in a WKWebView and observes its cookie store after sign-in. Thus this approach does not depend on Claude Code credentials. The implementation supports a manual cookie fallback too; an ordinary sign-in flow would be the preferable Headroom product experience.

This provider maps both 401 and 403 to session expiry. CodexBar's more specific challenge handling is a better reference for avoiding unnecessary sign-in loops.

## Comparison with Headroom today

These facts were checked in the current workspace:

- `crates/usage-sources/src/claude/oauth.rs` already implements the same OAuth endpoint and beta header used above, with bounded requests and scheduler backoff.
- `crates/usage-sources/src/claude/keychain.rs` reads the `Claude Code-credentials` item without refreshing or rotating it.
- `src-tauri/Cargo.toml` puts this behind `claude-oauth`; `src-tauri/src/settings.rs` defaults the runtime setting to false. This source inspection does not establish the feature flags or settings of a running app.
- The existing status-line source receives account limits after qualifying terminal activity. Headroom currently has neither a CLI usage probe nor a browser-session provider.

## Recommended next change

Implement manual **Refresh through Claude Code** as a bounded background PTY probe, using CodexBar's lifecycle and parsing lessons. Parse `/usage` directly; do not rely on it incidentally populating the status-line bridge. Use a dedicated working directory, one refresh at a time, terminal-screen reconstruction, explicit setup/authentication errors, and cleanup of only probe-owned processes and artifacts. Preserve valid windows and their original timestamps on failure. Apply a cooldown to server rate limits and do not send a model prompt to obtain limits.

Keep automatic CLI polling conservative because startup has a cost. Preserve the passive bridge for normal CLI activity and the existing explicitly enabled OAuth path. Do not automatically broaden credential access when switching sources or merge readings from unrelated accounts.

For users without Claude Code installed and authenticated, investigate an explicit **Connect Claude account** browser-session option separately. That addresses a different prerequisite from users who merely do not keep the CLI open.

Acceptance should cover recorded terminal redraws, missing/independent windows, unparseable reset times, setup prompts, concurrent clicks, timeout/shutdown, rate-limit responses, and a successful comparison with Claude's own usage display. These are proposed implementation checks; no app changes or live-provider acceptance were completed by this research.

## Implementation follow-up (2026-09-29)

The recommendation above was implemented in D-031, followed by Fable support in D-032. The current app has a bounded manual CLI refresh path. Live capture from Claude Code 2.1.281 returned session, all-model weekly, and Fable weekly percentages and reset times. Fable uses its own reported `weekly_scoped` model allowance, also documented by [claude-fable-usage](https://github.com/T0mSIlver/claude-fable-usage); it is not inferred from the weekly percentage or a promotional credit. Dashboard bridge diagnostics no longer suppress valid usage windows. See DECISIONS.md and HANDOVER.md for verification and remaining native acceptance checks.
