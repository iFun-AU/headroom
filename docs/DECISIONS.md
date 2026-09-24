# Decisions

This append-only log records choices and deviations made under `docs/DEVELOPMENT.md` §0.1.

## D-001 Define omitted widget defaults and coordinate types (2026-09-23, phase 0)

- Context: `DEVELOPMENT.md` §9.3 names `WidgetSettings { visible, x, y, variant, opacity }` but does not specify defaults for those fields or concrete coordinate types.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail”.
- Decision: use `visible = false`, `x = None`, `y = None`, `variant = Pill`, `opacity = 1.0`; represent saved coordinates as nullable signed 32-bit integers.
- Why: hidden is non-intrusive on first launch; absent coordinates let macOS choose an initial placement; Pill is the configured native default size; full opacity is the neutral default and avoids treating the artboard's sample 75% as product data; signed integer physical coordinates map directly to Tauri move events and support monitors left/above the primary display. Alternatives such as visible-by-default or zero coordinates would surprise users or force the widget onto the primary screen.
- Evidence: `DEVELOPMENT.md` §§9.3, 10.1–10.2 and `docs/design/screenshots/SettingsDisplay{Dark,Light}.png`.

## D-002 Use ts-rs for IPC bindings (2026-09-23, phase 1)

- Context: every Rust type crossing the Tauri IPC boundary needs a stable generated TypeScript representation; `tauri-specta` remains a release candidate.
- Rule applied: DEVELOPMENT.md §§0 “The documents are the contract”, 4.1 and 6.2.
- Decision: use `ts-rs` 12 for exported domain and IPC types and keep command wrappers hand-written in `ui/src/ipc.ts`.
- Why: it is stable, small in scope, and gives compile-time drift checks without adopting a release-candidate command-generation layer.
- Evidence: `DEVELOPMENT.md` §4.1 explicitly selects ts-rs; `.cargo/config.toml` fixes `TS_RS_EXPORT_DIR` at `ui/src/bindings`.

## D-003 Require macOS 26.0 (2026-09-23, phase 1)

- Context: the approved native presentation uses the macOS 26 Liquid Glass API and the product scope explicitly excludes older macOS releases.
- Rule applied: DEVELOPMENT.md §§0 “The documents are the contract”, 1 and 10.1.
- Decision: set the bundle minimum system version to 26.0 and do not add compatibility code for older systems.
- Why: supporting older releases would contradict product scope and would require a materially different native-window implementation.
- Evidence: `src-tauri/tauri.conf.json` sets `bundle.macOS.minimumSystemVersion` to `26.0`; the preflight host is macOS 27.0.

## D-004 Defer Claude OAuth to gated M9 (2026-09-23, phase 1)

- Context: the Claude OAuth endpoint and response shape are undocumented, and v1 has no owner acceptance entry for its policy and credential-access risks.
- Rule applied: DEVELOPMENT.md §§0 “The documents are the contract”, 1, 2.4 and 13 M9.
- Decision: keep an empty, off-by-default `claude-oauth` Cargo feature for future compatibility, but compile no OAuth, Keychain, `reqwest`, or `security-framework` implementation in v1.
- Why: the documented status-line bridge provides the consent-based v1 path without relying on an unstable endpoint or touching credentials.
- Evidence: `crates/usage-sources/Cargo.toml` defines `default = []` and `claude-oauth = []`; the v1 dependency graph contains neither network nor Keychain crates from our source crate.

## D-005 Add checked numeric conversion and DST-only test dependencies (2026-09-23, phase 2)

- Context: weekly projection converts a bounded floating-point duration into Unix seconds, while T1.5 explicitly requires a deterministic `America/New_York` DST-transition test. Neither conversion support nor an IANA timezone database is in the §4 direct-dependency table.
- Rule applied: DEVELOPMENT.md §0.1 “A dependency seems necessary”.
- Decision: add `num-traits` 0.2 as a direct `usage-core` dependency for checked `f64`→`i64` conversion and `chrono-tz` 0.10 as a test-only dependency.
- Why: `num-traits` is already a maintained transitive dependency of the mandated `chrono` crate and avoids a lossy `as` cast or panic-prone conversion; `chrono-tz` is isolated to tests and is the fixture-independent way to exercise the mandated 25-hour day. Neither adds network, cryptography, I/O, or unsafe code to this project.
- Evidence: `project_weekly` uses `ToPrimitive::to_i64`; the T1.5 test verifies 25 hourly events aggregate into 2026-11-01 in `America/New_York`; the dependency graph remains local-only.

## D-006 Normalize generated TypeScript without a formatter dependency (2026-09-23, phase 2)

- Context: the default ts-rs 12 renderer leaves trailing spaces around documented object fields, so reproducible generated bindings failed `git diff --check` even though TypeScript and ESLint accepted them.
- Rule applied: DEVELOPMENT.md §§0.1 “A dependency seems necessary” and 6.2 generated IPC bindings.
- Decision: make the `export_bindings` integration test export both top-level IPC graphs and normalize trailing whitespace across every generated `.ts` file using `std`; do not enable ts-rs's optional formatter feature.
- Why: generation remains a single deterministic command and later regeneration cannot silently restore whitespace defects, without pulling a full TypeScript parser/formatter graph (57 additional packages) into the Rust dependency lock for a whitespace-only problem. This is mechanical generated-file normalization, not hand editing or a weakened check.
- Evidence: two consecutive `cargo test -p usage-core export_bindings` runs produce identical SHA-256 lists, and the generated directory passes `git diff --check` after restaging.

## D-007 Parse both Claude bridge boundary shapes (2026-09-23, phase 3)

- Context: T2.3 names the persisted bridge-file format (`rateLimits`, `writtenAt`), but its required Appendix A.4 fixture is the raw Claude status-line stdin format (`rate_limits`) from which that file is produced.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail”.
- Decision: keep one lenient parser that accepts both field names; use `writtenAt` when present and otherwise require the caller to supply the observation time used for raw fixture/probe parsing.
- Why: the production source can parse exactly what the bridge persists, the required documented fixture remains executable, and both paths normalize into the same strict full `Reading` without another public DTO or inferred timestamp.
- Evidence: `claude_statusline_parser` verifies Appendix A.4 and a schema-1 persisted envelope independently, including timestamp precedence and independently missing windows.

## D-008 Keep the bridge binary to its explicit two dependencies (2026-09-23, phase 4)

- Context: DEVELOPMENT.md §4.1 broadly lists `dirs` for “sources, bridge”, while the more specific §8.3 and T3.1 requirements say the synchronous status-line bridge has `serde_json` and `tempfile` only.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail”, resolved in favor of the task-specific acceptance rule.
- Decision: the bridge binary derives `~/Library/Application Support/dev.headroom.app` from `HOME` using `std` and directly depends only on `serde_json` and `tempfile`; the later `usage-sources` path module may use `dirs` as specified.
- Why: this meets T3.1 literally, keeps the latency-sensitive process small, and avoids a third runtime dependency for one fixed macOS path. A missing `HOME` is suppressed like every other bridge error so Claude Code is never disrupted.
- Evidence: `cargo tree -p statusline-bridge --depth 1` shows exactly the two required direct dependencies, and every M3 integration test supplies a temporary `--out-dir` rather than resolving a real user path.

## D-009 Resolve path settings before inherited environment values (2026-09-23, phase 5)

- Context: DEVELOPMENT.md §8.6 requires one path module to combine Settings overrides, environment-aware tool roots, and platform directories, but does not state which wins when a saved setting and an inherited environment variable are both present.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail”.
- Decision: resolve explicit app Settings first, then `CODEX_HOME` / `CLAUDE_CONFIG_DIR`, then the documented home-directory defaults.
- Why: a path the user selected in onboarding or Settings must remain effective when the GUI happens to inherit a conflicting shell environment; honoring environment variables before defaults still supports standard tool layouts without duplicating path construction elsewhere.
- Evidence: `Paths::resolve` is the only path constructor, and its temp-root tests exercise settings > environment > default precedence without reading real user directories.

## D-010 Preserve absent versus null Claude status-line state (2026-09-23, phase 6)

- Context: DEVELOPMENT.md §8.3 requires `bridge-install.json` to store `previousStatusLine: <value> | null`, while also requiring uninstall to distinguish a previously absent key from every prior JSON value, including JSON `null`.
- Rule applied: DEVELOPMENT.md §0.1 “Two document requirements conflict”.
- Decision: retain the required `previousStatusLine` field and add `previousStatusLinePresent: bool`; uninstall removes the key only when that bit is false and otherwise restores the saved JSON value exactly.
- Why: JSON `null` alone cannot encode both states. The additive presence bit preserves forward-readable state, makes semantic uninstall lossless, and avoids treating a legitimate user value as absence.
- Evidence: bridge installer tests cover missing settings, arbitrary prior status-line objects, idempotent reinstallation, semantic uninstall after later edits, and byte-for-byte no-op when the command was changed by the user.

## D-011 Bound the diagnostic probe and add a fixture completion condition (2026-09-23, phase 6)

- Context: DEVELOPMENT.md T5.6 requires an all-source probe but does not define how it terminates, while automated tests must never inspect real Codex or Claude data.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail” and “The step needs a human”.
- Decision: live mode runs for 30 seconds by default and accepts `--seconds N`; `--fixture` uses only a temporary root and exits when both provider readings, both token histories, the primary Codex app-server reading, and Claude bridge confirmation are all observed, or fails after five seconds.
- Why: both modes retain one cancellation owner and a three-second join deadline, fixture verification is deterministic, and the default live command remains long enough for the documented immediate comparisons without becoming an orphaned background process.
- Evidence: `cargo run -q -p usage-sources --example probe -- --fixture` progresses from empty state through Claude and rollout readings to an authoritative 2% fake app-server snapshot, then exits successfully; the real-data live comparison is deferred to handover.

## D-012 Serialize shared settings as camelCase and accept snake_case aliases (2026-09-23, phase 7)

- Context: DEVELOPMENT.md §9.3 names Rust settings fields in snake_case, while §10.3 exposes the same struct to TypeScript and explicitly names wrapper fields such as `readOnly` in camelCase; the on-disk spelling is not stated.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail”.
- Decision: serialize the shared Settings/SettingsState IPC and JSON representation as camelCase, while accepting documented snake_case aliases during load and recognizing either `schemaVersion` or `schema_version`.
- Why: this follows the existing domain IPC convention and produces idiomatic generated TypeScript without making older or hand-authored settings brittle. Every successful load rewrites one canonical current-schema form.
- Evidence: T6.1 migration tests cover a versionless document, generated TypeScript exposes camelCase fields, and `schema_version` aliases are declared on every multiword setting.

## D-013 Reuse tempfile for atomic shell settings writes (2026-09-23, phase 7)

- Context: DEVELOPMENT.md §9.3 requires atomic settings writes, but §4.1 lists `tempfile` directly for the bridge and sources rather than explicitly for `src-tauri`.
- Rule applied: DEVELOPMENT.md §0.1 “A dependency seems necessary”.
- Decision: add the already pinned `tempfile` 3.23 runtime crate to `src-tauri` and use `NamedTempFile::new_in`, file sync, atomic persist, and parent-directory sync for settings.
- Why: reusing the workspace's maintained same-directory atomic-write primitive avoids collision-prone custom temporary names and adds no new package family, network access, cryptography, or project unsafe code.
- Evidence: the T6.1 suite exercises missing, migrated, corrupt, future-version, clamped, save, and reset paths entirely in temporary directories; the workspace dependency lock reuses the existing tempfile version.

## D-014 Restart one bounded source generation when source settings change (2026-09-23, phase 7)

- Context: DEVELOPMENT.md requires path, Codex executable, and Claude bridge settings to take effect without defining how already-running filesystem watchers and child processes are reconfigured.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail”.
- Decision: own all four concrete sources under one cancellable source generation; replace that generation when a source input (`codex_path`, `codex_home`, `claude_dir`, the future OAuth toggle) or the bridge installation timestamp changes, while keeping the store, scheduler, settings, and effectiveness actors alive.
- Why: paths and process discovery are immutable inputs to individual source loops. A bounded cancel-and-join restart applies them consistently, prevents split generations from observing different settings, and leaves accumulated store/history state intact. It is simpler and safer than teaching each watcher and process a separate reconfiguration protocol.
- Evidence: `SourceSupervisor` derives every generation from the latest validated settings, ignores display/alert/onboarding changes, cancels and joins the prior generation before a relevant replacement, and the runtime owns the supervisor under the same three-second shutdown boundary as every other actor.

## D-015 Use the system transparency preference with an explicit opaque fallback (2026-09-24, phase 8)

- Context: the approved accessibility design requires opaque surfaces under Reduce Transparency, but browser rendering can prove only the CSS media query; the final behavior of that query in the native macOS 26 WKWebView still needs a person at the Mac.
- Rule applied: DEVELOPMENT.md §0.1 “The step needs a human”.
- Decision: implement both `prefers-reduced-transparency: reduce` and the same tokenized `.no-glass` fallback used when native Liquid Glass setup fails; do not add a second preference store or a JavaScript-only substitute.
- Why: the webview follows the user's system preference when WKWebView exposes it, while the explicit class guarantees the identical opaque `--solid-win`/`--solid-card` rendering for plugin failure. A second setting could drift from macOS and would create an unnecessary source of truth.
- Evidence: browser media emulation matches the query and computes `backdrop-filter: none`, no background image, and opaque token surfaces; the dedicated accessibility route and all state routes pass dark/light semantic checks. Native System Settings → Accessibility → Display verification remains on the handover checklist.

## D-016 Gate soak instrumentation behind a canonical temporary root (2026-09-24, phase 9)

- Context: DEVELOPMENT.md §14.3 requires a packaged-release soak with synthetic provider data and real `ui_visible` scheduling, while automated verification must not read user data or mutate launch-at-login state. The clean release build also failed because the workspace's global `strip = true` stripped a host proc-macro artifact before Rust could load it.
- Rule applied: DEVELOPMENT.md §0.1 “Spec is silent on a detail” and “A check fails and the cause is unclear”.
- Decision: enable the native soak seam only when `HEADROOM_SOAK_ROOT` canonicalizes to an existing child of the system temporary directory; derive all home/data roots from it, skip autostart convergence, and own the visibility timer under the normal runtime cancellation tree. Keep release application binaries stripped, but set `profile.release.build-override.strip = false` so host build scripts and proc macros remain loadable.
- Why: a malformed or accidental soak variable fails closed before application setup, normal launches have no changed behavior, and the harness exercises the real scheduler without touching home-directory state. A build override is narrower than disabling stripping for the shipped binaries and adds no dependency or special build command.
- Evidence: focused tests reject relative, missing and outside-temp roots and constrain the smoke-only timer interval; `cargo clippy -p headroom --all-targets -- -D warnings` and a clean isolated `cargo tauri build --bundles app` pass. The same isolated build reproduced `E0463: can't find crate for ctor_proc_macro` without the build override and succeeds with it.

## D-017 Suspend hidden WebView work and refresh on presentation (2026-09-24, phase 9)

- Context: the first exact 10-minute no-event measurement averaged 0.555% CPU, narrowly above the §14.3 limit. Process sampling showed the owned Rust/Tokio work blocked while JavaScriptCore scavenging and WebKit display callbacks accounted for the recurring work; snapshot events and one-second clocks were still delivered to three hidden windows.
- Rule applied: DEVELOPMENT.md §0.1 “A check fails and the cause is unclear” and “Spec is silent on a detail”.
- Decision: keep tray updates live, but emit `usage-updated` only to native windows that are visible. Hidden webviews pause shared clocks and rendering; on visibility or focus, each window fetches the current store snapshot and invokes the documented `ui_visible` refresh path immediately.
- Why: this preserves current data whenever a surface is presented and avoids maintaining hidden React layout/GC activity. Destroying and recreating windows would complicate state, positioning and native-glass lifecycle for no user-visible benefit.
- Evidence: Rust Clippy/tests and UI typecheck/lint/build pass. The rebuilt packaged-app smoke retained one owned app-server child, logged two native visibility triggers during its 60-second event segment, and averaged 0.125% CPU over the exact 600-second no-event window; the prior bundle averaged 0.555% under the same measurement.

## D-018 Fail closed around macOS 27 AppIntents leak-tool cycles (2026-09-24, phase 9)

- Context: DEVELOPMENT.md §14.3 requires `leaks` to report zero. A fully inspectable, debug-entitled copy of the release bundle instead reports three stable root cycles (416–417 nodes, about 20 KB total), all owned by macOS 27 AppIntents' `LNProcessInstanceRegistryClient` connection to `com.apple.linkd.autoShortcut`; there are no Rust, WebKit or project allocation roots. The three cycles are already present after a few seconds and do not grow with the workload.
- Rule applied: DEVELOPMENT.md §0.1 “A check fails and the cause is unclear” followed by “Spec is silent on a detail”.
- Decision: preserve the raw `leaks` report and accept this host-runtime exception only when every top-level leak root is an `LNDaemonApplicationInterface` `NSXPCConnection` cycle to the exact `com.apple.linkd.autoShortcut` service. Any other root, a partial inspection, or an unparsable report fails the soak. Record project leaks and platform AppIntents nodes separately; never relabel the raw platform count as zero.
- Why: `leaks -exclude` requires allocation backtraces and therefore malloc stack logging for the full run, which materially distorts the memory criterion. The structural allowlist is narrower and auditable, while keeping the requested check sensitive to every application-controlled allocation. A raw-zero confirmation on target macOS 26 remains appropriate handover work.
- Evidence: linker-signed execution first produced a partial-inspection warning; signing the temp copy with `com.apple.security.get-task-allow` removed that warning and exposed the named service. An exact-symbol `leaks -exclude` probe did not alter the result. The corrected end-to-end smoke passed with `project_leaks=0`, `platform_appintents=416`, one owned child, clean child reaping, −1.100% footprint growth and 0.033% short-window idle CPU. The full run reported 296 platform nodes across the same three allowed roots and no other root.

## D-019 Accept the automated 60-minute soak (2026-09-24, phase 9)

- Context: DEVELOPMENT.md §§13 M8, 14.3 and 15 require the accelerated release soak to pass and its results to be recorded before the local universal build.
- Rule applied: DEVELOPMENT.md §14.3 “Agent run” pass criteria, with only the host-framework treatment documented in D-018.
- Decision: accept T8.1. Retain the generated evidence under ignored `target/soak/`; carry the longer eight-hour run, Safari heap check and raw-zero `leaks` confirmation on target macOS 26 to `docs/HANDOVER.md` as required.
- Why: every application-controlled criterion passed with substantial margin, all generated provider/log data stayed under one temporary root, and the app/child exited cleanly. The remaining checks explicitly require a person, a longer elapsed run, a target-OS comparison, or Safari Develop tooling.
- Evidence: the release app processed 31,414 Codex lines, 31,414 Claude lines and 6,282 atomic bridge rewrites over 3,600 seconds; it logged 120 real `ui_visible` triggers. All 61 per-minute samples had exactly one child. Footprint was 35,095,728 bytes at minute 10 and 35,030,192 at minute 60 (−0.187%). Ten idle minutes consumed 0.08 CPU seconds (0.013% average). The raw leak report had only D-018's three AppIntents roots (`project_leaks=0`, `platform_appintents_leaks=296`), isolated logs contained no secret-shaped values, and both the main process and logged child were gone after Quit.

## D-020 Preserve both architecture sidecars during the universal build (2026-09-24, phase 9)

- Context: the exact T8.2 universal command compiles the Tauri app once per architecture. The original universal branch of `scripts/build-sidecar.sh` emitted only `headroom-statusline-universal-apple-darwin`, so Tauri's x86_64 slice build stopped because `headroom-statusline-x86_64-apple-darwin` was absent.
- Rule applied: DEVELOPMENT.md §0.1 “A check fails and the cause is unclear”.
- Decision: after building the status-line bridge for both Rust targets, install both architecture-suffixed executables into `src-tauri/binaries/` and lipo those retained files into the universal-suffixed executable.
- Why: Tauri resolves the architecture-specific name while compiling each app slice, then merges the app and sidecar into the final universal bundle. Retaining all three ignored build outputs satisfies both stages without changing the external-bin contract or adding a dependency.
- Evidence: the repaired helper reports `arm64`, `x86_64`, and `x86_64 arm64` for its three outputs. `cargo tauri build --target universal-apple-darwin` completes both the `.app` and DMG outside the packaging sandbox. In the final bundle both `headroom` and `headroom-statusline` report `x86_64 arm64`; `codesign --force --deep -s -` followed by `codesign --verify --deep --strict --verbose=2` passes. As expected for the Cargo workspace, the bundle path is root `target/universal-apple-darwin/...`, not the non-workspace `src-tauri/target/...` example in §13.

## D-021 Require measured wall-clock cadence for the accelerated soak (2026-09-24, phase 9)

- Context: the first otherwise-passing T8.1 run in D-019 slept for 100 ms only after completing each fixture write. Its 31,414 lines per provider over 3,600 seconds therefore represented 8.7 Hz rather than the required one line per 100 ms.
- Rule applied: DEVELOPMENT.md §0.1 “A check fails and the cause is unclear” and §14.3's exact accelerated workload.
- Decision: supersede D-019's acceptance evidence with a persistent Node.js generator driven by a 100 ms wall-clock interval, and make the harness fail unless each provider reaches at least 95% of the nominal 10 Hz count. Retain D-019 as diagnostic history rather than rewriting the append-only record.
- Why: overlapping shell process startup with a deadline still sustained only 8.8 Hz. A persistent timer removes per-line interpreter startup overhead, measures the requirement directly, and leaves the packaged application, temporary-root isolation and every other acceptance check unchanged.
- Evidence: the corrected 3,600-second run produced 35,508 Codex lines, 35,508 Claude lines and 7,101 atomic bridge rewrites (9.86 Hz/provider), plus 120 real `ui_visible` triggers. All 61 footprint samples retained the same single child PID. Minute-10 to minute-60 footprint grew from 34,931,936 to 34,997,472 bytes (0.188%); ten idle minutes consumed 0.08 CPU seconds (0.013% average). The raw leak report contained only D-018's three exact AppIntents roots (`project_leaks=0`, `platform_appintents_leaks=417`), isolated logs passed the secret scan, and both the app and child exited cleanly. This run is the authoritative T8.1 acceptance.

## D-022 Distinguish headless Claude clients from overridden status lines (2026-09-24, post-v1)

- Context: pre-production use showed an installed, correct bridge that never wrote a snapshot. Every Claude Code session on the owner's Mac ran headless inside the Claude desktop app (`--output-format stream-json`, log `entrypoint: "claude-desktop"`); Claude Code runs status-line commands only in its interactive terminal UI. DEVELOPMENT.md §8.3 classifies any assistant activity more than ten minutes past the baseline as `LikelyOverridden`, whose hint wrongly blames project or organization settings.
- Rule applied: owner request (2026-09-24) to fix the diagnosis; this amends the three-state contract in DEVELOPMENT.md §8.3.
- Decision: the Claude log parser classifies each record by `entrypoint`. `cli`, or no entrypoint (older logs), is `Interactive`; every other value (`claude-desktop`, `claude-vscode`, `sdk-cli`, `sdk-ts`, `sdk-py`, `mcp`, `remote`, `local-agent`, unknown) is `Headless`. Only interactive activity past the baseline yields `LikelyOverridden`; headless-only activity yields the new `Effectiveness::HeadlessOnly` ("Terminal only" in the UI), which never downgrades an existing `LikelyOverridden`. A qualifying bridge write still yields `Confirmed`. Out-of-order evidence from concurrently tailed files is evaluated while `last_activity` stays monotonic.
- Why: the new state gives an accurate, actionable explanation without guessing about settings, and treating missing or `cli` entrypoints as interactive preserves the original behavior for every log that is not positively known to be headless. It does not make limits available to desktop-only users; that requires the gated M9 OAuth source.
- Evidence: Claude Code 2.1.280 sets `CLAUDE_CODE_ENTRYPOINT` to `cli` for the interactive UI and `sdk-cli` for non-interactive `-p`. The owner's logs contained 70,023 `claude-desktop` and 1,302 `cli` records, and all four sessions after bridge installation were `claude-desktop`. New parser, tracker, and history-source integration tests cover classification, precedence, out-of-order evidence, and the headless end-to-end path. `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace` (168 passed), `tsc --noEmit`, and ESLint pass; the regenerated `Effectiveness.ts` binding changed only by the new variant.

## D-023 Accept the M9 Claude OAuth usage source gate (T9.0, 2026-09-24)

- Context: D-022 shows that the status-line bridge cannot serve users who run Claude Code only in the Claude desktop app, IDE extensions, or the SDK; for them the app can at best explain why limits are missing. DEVELOPMENT.md §§2.4, 8.4 and 13 M9 specify an opt-in fallback that reads Claude Code's OAuth token from the Keychain and polls `GET https://api.anthropic.com/api/oauth/usage`.
- Rule applied: DEVELOPMENT.md §13 T9.0 — the owner must accept the policy risk, provisional response shape, and failure semantics before work starts.
- Accepted terms, by area:
  1. **Policy risk.** The endpoint and `anthropic-beta: oauth-2025-04-20` header are undocumented internals of Claude Code and may change or be withdrawn without notice. The app would reuse Claude Code's subscription OAuth credential from a third-party process; the owner should review Anthropic's current terms for Claude subscription credentials before accepting, and accepts that this build stays local-only and is never redistributed while the source exists.
  2. **Credential handling.** Read-only Keychain access to generic password `Claude Code-credentials` after a first-run macOS prompt the onboarding copy explains. Never write, refresh, rotate, log, persist, or send the token anywhere except the single endpoint above. `AccessToken` has redacted `Debug`/`Display`, and log output passes the existing secret scan.
  3. **Provisional response shape.** Appendix A.5 is accepted only as a starting hypothesis. T9.1 must first run the feature-gated probe that prints JSON key paths and value types only (never values) and record the verified shape here before the parser is written. Map only `five_hour` → Session and `seven_day` → Weekly; parse `utilization` as a number and `resets_at` as ISO string, number, or null; ignore unknown keys.
  4. **Failure semantics (as §§2.4 and 8.4).** Local `expiresAt < now` or HTTP 401 → `AuthExpired` ("Open Claude Code to refresh sign-in") and re-check on the next tick; 403/404 → `Unsupported`, stop until restart or toggle; 429 → honor `Retry-After`; 5xx or network error → `Error` with backoff; an unparseable 200 → `Unsupported` rather than guessed values.
  5. **Runtime behavior.** Runtime setting `claudeOauthEnabled` stays off by default and is enabled only after the warning copy in T9.4. Poll every 3–10 minutes, skip when a status-line reading is under 3 minutes old, and never poll while the setting is off. The bridge remains the primary source when it is working.
  6. **Build.** The `claude-oauth` Cargo feature stays off by default; the owner's local universal build enables it explicitly. `reqwest` (rustls) and `security-framework` are admitted only under that feature.
- Why: this is the only path to live limits for desktop-app-only use without asking the user to keep a terminal session open, and the specified gates bound the credential and breakage risk to an opt-in, local, read-only feature.
- Accepted by Ray on 2026-09-24

## D-024 Verified OAuth usage shape and M9 implementation choices (2026-09-24, M9)

- Context: D-023 opened the M9 gate and requires T9.1 to verify the response shape before the parser, and DEVELOPMENT.md §§2.4 and 8.4 leave several runtime details open.
- Rule applied: DEVELOPMENT.md §13 T9.1–T9.4 and §0.1 “Spec is silent on a detail”.
- Decision: (1) The live shape (probe `--claude-oauth-dump-keys`, key paths and types only, HTTP 200) matches Appendix A.5 for the mapped fields: `five_hour` and `seven_day` are objects with float `utilization` and RFC 3339 `resets_at`. The response also carries many other keys that change over time (`*_dollars`, `locked_reason`, additional null or codename windows such as `nimbus_quill` with a null `resets_at`, `extra_usage`, `limits[]`, `spend`, `seven_day_breakdown`), so the parser maps only the two windows, ignores every other key, and the fixture includes representative extras. (2) The `utilization` scale cannot be proved without printing values; it is parsed as 0–100 per A.5 and the bridge, and a live `/usage` comparison is a handover check. (3) An unparseable or window-less 200 maps to `Unsupported` and stops, instead of guessed values. (4) A denied Keychain prompt stops polling until restart or toggle so the prompt is not repeated every tick; a missing credential shows `NotConfigured` and rechecks. (5) The poller is not triggered by UI visibility, so opening the popover never adds calls to the unofficial endpoint; cadence is the scheduler's 3-minute active / 10-minute idle interval with backoff and `Retry-After`. (6) `SettingsState.claudeOauthAvailable` reports whether the build includes the feature, and the UI shows the toggle only when true. (7) The HTTP client is HTTPS-only, follows no redirects, bounds bodies at 256 KiB, and strips URLs from transport errors; credential parse errors never include provider text, because a serde type error could otherwise quote the token. (8) After security review: the credential is cached in memory and re-read from the Keychain only on first use, local expiry, or a 401, so a one-time "Allow" does not cause a prompt every poll; `Retry-After` is capped at one hour, because an unbounded value would overflow the shared scheduler's deadline arithmetic and stop Codex polling too; and each poll is raced against cancellation.
- Why: the verified extras show the endpoint is actively evolving, so strict unknown-key tolerance and a stop-on-unrecognized policy keep the source safe; the remaining choices minimize calls and prompts to an unofficial, credential-bearing endpoint.
- Evidence: `cargo tree` shows `reqwest` and `security-framework` only with `--features claude-oauth`. Keychain, parser, poller unit tests and seven poller integration tests (reading, 401/expired, 403/404 stop, 429 `Retry-After`, fresh status line skip, denied prompt, credential caching) pass. `cargo fmt --check`, clippy `-D warnings` with and without the feature, `cargo test --workspace` (173 default, 187 with the feature), `tsc --noEmit` and ESLint pass; the Settings group and onboarding step were checked in the browser gallery.

## D-025 Owner manual-test feedback: tray, pinning, widget recovery, bar flashing (2026-09-24)

- Context: owner manual testing reported (1) the menu-bar title looked Codex-only, (2) no always-on-top for the dashboard, (3/4) progress bars briefly dropping to 0% on every update, which the status line triggers every 5–6 s during Claude Code use, and a widget left off-screen after switching from several displays to the laptop display.
- Rule applied: owner request; amends DEVELOPMENT.md §10.2 (tray title = highest used %).
- Decision: (1) The tray title shows each provider's weekly limit (`" C 41% · X 78%"`, provider omitted when it has no weekly window) with a full-name tooltip; the icon badge still reflects the highest window of any kind so a session warning remains visible. Short labels avoid the title being hidden behind the MacBook notch. (2) New persisted setting `mainAlwaysOnTop` (default off, additive under `serde(default)`), toggled by a pin button in the dashboard toolbar or Settings → Display, applied by the window task. (3/4) Root cause was `ProviderCard` keying each bar by `observedAt`, which remounted `NeonBar` and replayed its 0% entrance animation on every reading; keys are now the stable window kind, so updates animate width changes in place and the update frequency needs no throttling. (5) While visible, the widget is checked every 2 s (and when shown or restored) and moved inside the nearest display's work area when its center is on no connected display; macOS offers no display-change notification without `unsafe`, which the workspace forbids.
- Evidence: new tray, recovery-geometry and existing snap tests pass; `cargo fmt --check`, clippy `-D warnings` with and without `claude-oauth`, `cargo test --workspace` (176 default, 190 with the feature), `tsc --noEmit` and ESLint pass; toolbar pin and Display switch checked in the browser.

## D-026 Selectable menu-bar style: numbers or stacked bars (2026-09-24)

- Context: the owner asked to choose between the D-025 numeric title and progress bars, with the bars stacked rather than side by side, in provider colors.
- Rule applied: owner request; amends DEVELOPMENT.md §10.2 and Appendix B, which kept the menu bar monochrome.
- Decision: new persisted setting `trayStyle` (`Numbers` default, `Bars`), chosen in Settings → Display → Menu bar. `Bars` replaces the title with a rendered 56×36 px (28×18 pt) non-template image: Claude's weekly limit on top in coral, Codex's below in cyan, each fill turning amber at 75% and magenta-red at 90% (the app's thresholds), over a mid-grey translucent track that remains visible on light and dark menu bars; a missing weekly window draws an empty track. `Numbers` keeps the D-025 title with the monochrome template icon and badge. Both styles keep the full-name weekly tooltip. The image is re-rendered only when a rounded percentage changes, and a style change re-renders immediately from the latest snapshot.
- Why: colored bars distinguish providers at a glance without text; stacking keeps the status item narrow (important beside the MacBook notch); rendering in Rust avoids new image dependencies.
- Evidence: renderer unit tests check size, per-provider colors and position, thresholds and empty tracks; a rendered preview was inspected on dark and light backgrounds. Tray presentation tests cover both styles. `cargo fmt --check`, clippy `-D warnings` with and without `claude-oauth`, `cargo test --workspace` (182 default, 196 with the feature), `tsc --noEmit` and ESLint pass.

## D-027 Owner manual-test feedback: labeled tray bars, widget shadow, hover, session rows (2026-09-24)

- Context: manual testing of D-026 and the widget found that the bars carried no numbers, the widget's outer shadow rendered as a hard grey rectangle on every variant, the hover controls stuck after adjusting opacity, and Codex plans without a 5-hour limit wasted a widget row.
- Rule applied: owner request; amends D-026.
- Decision: (1) each tray bar shows its rounded weekly percentage centered in stroked vector digits drawn in Rust (bars grow to 15 px with a 3 px gap). Over the track the digits follow the system appearance (dark on a light menu bar, white on a dark one) and re-render on `ThemeChanged`; over the fill they are always dark. The owner first asked for white digits over the fill, but previews showed white on cyan and amber unreadable, so the owner chose dark. (2) The widget drops the outer glass `box-shadow` and keeps only the inset edges: the window is exactly the widget's size, so the shadow could only paint into the corners and was clipped into a rectangle; the native window shadow stays off. (3) Leaving the widget blurs any focused control, which commits the opacity and lets `:focus-within` release the hover controls. (4) Pill and Stack omit a provider's 5-hour row when it has a weekly window but no session window; the window size is unchanged.
- Why: digits make the bars readable without the tooltip; a dependency-free stroke font avoids adding a font or CoreText dependency; appearance-aware colors keep contrast on both menu bars.
- Evidence: renderer tests cover centering, dark digits over the fill, appearance-dependent track digits and unlabeled missing usage; previews were inspected on light and dark backgrounds; the widget gallery (`#/widgets`) shows the weekly-only Codex cases. `cargo fmt --check`, clippy `-D warnings` with and without `claude-oauth`, `cargo test --workspace`, `tsc --noEmit` and ESLint pass.

## D-028 Alert fixes: stale baselines, reset-time jitter, clearer wording (2026-09-24)

- Context: the owner received, within 13 minutes, Claude session alerts at 75%, then 100% and 90%, then 75%, 90% and 100% again, and finally "Claude session limit reset · A new usage window has started" at 13:20 with the reset due at 14:00. The log shows an app restart at 13:16. The status-line file was last written at 10:50 (13%, reset 14:00:00), while the usage API reported 100% with a reset of 13:59:59.x (hence "Resets 1:59 PM").
- Rule applied: owner request; amends DEVELOPMENT.md §7.7.
- Decision: (1) after a restart the stale 13% status-line reading became the baseline and the usage API's 100% looked like a fresh climb, so stale (≥ 15 min) and `reset_pending` windows no longer move the baseline; they carry the previous fresh one, and the first fresh reading is silent. (2) The one-second disagreement between Claude's sources looked like a later reset (a reset alert at ≥ 90%) and created a new dedupe key, so reset times within 5 minutes of the established one are the same window and keep the established value. (3) A reset alert also requires the previous reset time to have arrived. (4) Wording: "5-hour" instead of "session" (matching the UI), the reset body states the new usage and next reset, and reset times round to the nearest minute. Deduplication stays in memory: with (1), a restart no longer re-fires unless the first fresh reading after it is itself a new crossing.
- Why: each alert should correspond to a real change the owner can act on; a false "reset" is worse than a late one.
- Evidence: regression tests replay the owner's sequence (stale baseline after restart; 13:59:59/14:00:00 source flips at 100%, 70%, 100%) and fail on the previous implementation; tests cover the stale baseline, a reset that passes while pending, and the early-move guard.

## D-029 Choose the limit window for the menu bar and widget (2026-09-24)

- Context: the owner wants the menu bar and the widget to show the 5-hour or the weekly limit independently; the widget may also show both. The dashboard keeps showing every window a plan has.
- Rule applied: owner request; amends D-025, D-026 (weekly-only menu bar) and D-027 (4).
- Decision: new persisted settings `trayWindow` (`FiveHour` | `Weekly`, default `Weekly`) and `widget.windows` (`FiveHour` | `Weekly` | `Both`, default `Both`), both in Settings → Display. Missing fields load as those defaults, so existing files keep today's display. "Both" is not offered for the menu bar: four bars or two numbers per provider do not fit beside the notch. When a plan lacks the chosen window (e.g. weekly-only Codex), the provider shows its other window, labeled: `X wk 41%` in the Numbers title, "Codex weekly 41%" in the tooltip (the Bars image has room only for digits), a `wk`/`5h` tag in Pill, the row label in Stack, and `X wk 41%` in Mini's values. Mini fits one window, so "Both" shows the 5-hour one there. With "Both", D-027 (4) still drops an absent 5-hour row. Widget sizes are unchanged.
- Why: owner choice between a fallback and a dash; a labeled fallback keeps every provider visible without being mistaken for the chosen window.
- Evidence: tray tests cover both choices, both styles and the labeled fallback; a settings test loads a v1 file without the new fields; the widget gallery (`#/widgets`) shows 5-hour-only, weekly-only and Mini fallback cases and was inspected in the browser, as was Settings → Display. `cargo fmt --check`, clippy `-D warnings` with and without `claude-oauth`, `cargo test --workspace`, `tsc --noEmit` and ESLint pass.
