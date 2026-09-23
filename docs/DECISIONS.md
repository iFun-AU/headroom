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
- Decision: the bridge binary derives `~/Library/Application Support/dev.howisit.app` from `HOME` using `std` and directly depends only on `serde_json` and `tempfile`; the later `usage-sources` path module may use `dirs` as specified.
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
