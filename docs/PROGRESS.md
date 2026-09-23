# Progress

Current: Phase 5 — M4 I/O infrastructure

## Phase 0 — Understand and prepare
Status: committed (4504609)

### Tasks

- [x] P0.1 Read the complete contract and design package — Acceptance: all required documents and 29 artboards inspected — Verify: reading order reconciled against `AGENT_KICKOFF_PROMPT.md` — Result: complete; no contract files changed.
- [x] P0.2 Toolchain preflight — Acceptance: macOS/Xcode/Rust/Node meet minimums and both Rust targets are installed — Verify: every mandated version/target command recorded in `UNDERSTANDING.md` — Result: pass; installed `x86_64-apple-darwin` without sudo or GUI.
- [x] P0.3 Write implementation understanding — Acceptance: product, sources, update path, windows/routes, settings, 13 answers and all verification items are covered with references — Verify: manually cross-checked against cited sections — Result: complete.
- [x] P0.4 Prepare repository records — Acceptance: Git initialized; required ignore rules, progress and append-only decision log exist — Verify: `git status --short --ignored` — Result: complete.
- [x] P0.5 Review and commit — Acceptance: only allowed Phase 0 files changed and every answer matches its citation — Verify: staged diff review, `git diff --cached --check`, 29-artboard count, `git log -1 --stat`, clean status — Result: review passed; commit follows this record.

### Review

- Checks: documentation-only phase; toolchain preflight ✅
- Diff review: all implementation notes re-read; corrected the Claude bridge source table to avoid claiming a plan field and chose a neutral full-opacity widget default rather than copying sample UI data. Immutable contract/design files are staged as the initial repository baseline, not modified.
- Decisions: D-001 · ⚠ VERIFY: identified and routed to fixture/fake tests or handover
- New handover items: live Codex account check; old-Codex compatibility check; post-v1 OAuth shape; native Reduce Transparency; §14.4; 8-hour soak; Safari memory; native design; real log grep; `/Applications` smoke; optional signing/notarization

### Blocked (hard stops only)

- None.

## Phase 1 — Scaffold (M0)
Status: committed (090fbf1)

### Tasks

- [x] T0.1 Workspace — Goal: establish the linted Rust workspace and three empty crates — Files: root `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`, `crates/{usage-core,usage-sources,statusline-bridge}/**` — Acceptance: workspace members and off-by-default `claude-oauth` feature match §13 M0 — Verify: `cargo build --workspace` succeeds with zero warnings — Result: split into two ≤8-file batches; `cargo build --workspace` ✅ (4 crates, zero warnings).
- [x] T0.2 Tauri + UI — Goal: scaffold Tauri 2 with React 19/strict TypeScript and the sidecar build hook — Files: `ui/**`, `src-tauri/**`, `scripts/build-sidecar.sh` — Acceptance: exact three-window shell configuration, strict TS/ESLint, typed npm scripts, and native sidecar naming are wired — Verify: `npm --prefix ui run typecheck`, `npm --prefix ui run lint`, `cargo tauri dev` opens a window — Result: official generator 4.7.4 used as the baseline; React 19.3.0, TypeScript 5.9.3, Tauri CLI 2.11.5 installed; typecheck ✅; lint ✅; `cargo tauri dev` ✅ (sidecar built, Vite ready, native process launched and remained live; unbundled debug app was not addressable by the desktop UI inventory).
- [x] T0.3 Decisions — Goal: record the three mandated foundational choices — Files: `docs/DECISIONS.md` — Acceptance: ts-rs, macOS 26, and deferred OAuth entries include context/rule/reason/evidence — Verify: manual review against §4 and §13 M0 — Result: D-002 through D-004 added and cross-checked ✅.

### Review

- Checks: `cargo fmt --all -- --check` ✅ · `cargo clippy --workspace --all-targets -- -D warnings` ✅ · `cargo test --workspace` ✅ · UI typecheck ✅ lint ✅ build ✅ · `cargo tauri dev` ✅
- Diff review: all scaffold files read in full; generated dependency locks retained; generated schemas, sidecars, dependencies and build output ignored; no contract/design edits; no forbidden Rust constructs, secrets, unbounded resources or unrelated changes. Fixed root release-profile placement, added build-script docs, corrected Tauri command working directories, and scoped type-aware ESLint to TS modules.
- Decisions: D-002, D-003, D-004 · ⚠ VERIFY: none in M0
- New handover items: none; native debug-window inventory limitation is a development-tooling observation, not an acceptance blocker

### Blocked (hard stops only)

- None.

## Phase 2 — Core domain (M1)
Status: committed (7163e95)

### Tasks

- [x] T1.1 Domain types and bounded logging — Goal: implement the strict, documented §7.1 domain model, validated unit newtypes, and UTF-8-safe external-text truncation — Files: `crates/usage-core/src/{lib.rs,domain.rs}` plus focused tests — Acceptance: `Percent` rejects non-finite input and clamps finite values; all IPC fields follow the prescribed JSON/TypeScript representations; `log_trunc` returns at most 2 KiB without splitting a character — Verify: red/green unit tests for NaN, infinities, 120, −1, short text, ASCII boundary, and multibyte boundary — Result: 5 focused tests pass; T1.1 clippy is warning-free.
- [x] T1.2 Window classification — Goal: implement the pure duration classifier — Files: `crates/usage-core/src/window.rs` plus focused tests — Acceptance: session, weekly, other, absent/hinted, negative, and oversized durations follow §7.2 without lossy casts — Verify: 5 boundary-focused tests pass.
- [x] T1.3 TypeScript bindings — Goal: generate reproducible `ts-rs` exports for every IPC type — Files: `ui/src/bindings/**` plus export contract test — Acceptance: every IPC type is exported; 64-bit JSON values use TypeScript `number` — Verify: `cargo test -p usage-core export_bindings` runs 17 export tests; `UnixSeconds.ts` contains `number` and no `bigint`.
- [x] T1.4 Source-state merge — Goal: implement §7.4 ingest and derive semantics without I/O — Files: `crates/usage-core/src/merge.rs` and its tests — Acceptance: rules I1–I4 and D1–D5, races (a)–(e), deterministic source selection, rollover/refinement/reset handling, and at least five-event permutation invariance are covered — Verify: 15 named tests pass, including every rule, all required races, and all 120 orderings of five full-reading events; focused clippy is warning-free.
- [x] T1.5 History — Goal: aggregate bounded per-source token history and produce correctly scoped 24-hour/7-day views — Files: `crates/usage-core/src/history.rs` and its tests — Acceptance: exact zero-filled series, source isolation, Codex account/fallback selection, UTC account days, dedupe, 8-day pruning, and DST behavior match §7.5 — Verify: 7 deterministic tests pass, including the 25-hour `America/New_York` day on 2026-11-01; storage metrics prove hour/dedupe pruning; focused clippy is warning-free.
- [x] T1.6 Projection and alerts — Goal: implement weekly pace projection and stateful threshold/reset alerts — Files: `crates/usage-core/src/{projection.rs,alerts.rs}` and tests — Acceptance: projection depends only on weekly percentage/timing; alerts fire only on upward crossings, dedupe per reset, re-arm after reset, and prune expired state — Verify: 5 focused tests pass for weekly-only projection, six-hour suppression, upward/downward transitions, reset re-arming, reset notification, and bounded firing state; focused clippy is warning-free.

### Phase review and commit

- [x] Goal: leave M1 clean, documented, generated, and warning-free — Files: Phase 2 changes only — Acceptance: all M1 acceptance criteria pass; no forbidden constructs or unrelated edits; coverage meets the project gate — Verify: fmt, clippy, workspace tests, binding diff check, UI checks, coverage measurement, staged diff review, commit, `git log -1 --stat`, clean status — Result: all automated gates through the pre-commit review pass; commit follows this record.

### Review

- Checks: `cargo fmt --all -- --check` ✅ · workspace clippy with `-D warnings` ✅ · workspace tests ✅ (58 `usage-core` tests, none skipped) · `cargo llvm-cov -p usage-core --fail-under-lines 90` ✅ (95.89% lines; domain/window/projection 100%, merge 97.19%) · generated-binding SHA-256 regeneration check ✅ · UI typecheck/lint/build ✅ · `git diff --cached --check` ✅.
- Diff review: every M1 source and test file re-read; the highest-risk merge paths have one test per I1–I4/D1–D5 plus five named races and all 120 permutations. History never mixes sources, retains bounded state, preserves UTC account days and passes a 25-hour DST fixture. All source files stay below 400 lines after splitting time helpers and merge test groups. No I/O entered `usage-core`; no non-test unwrap/expect, unsafe, secrets, unbounded collections, contract edits, or unrelated files were introduced.
- Decisions: D-005, D-006. The review rejected ts-rs's heavyweight formatter graph (57 packages) and retained a std-only deterministic generated-whitespace normalizer instead.
- New handover items: none; M1 is fully fixture/pure-logic verifiable.

### Blocked (hard stops only)

- None.

## Phase 3 — Parsers (M2)
Status: committed (bf538a9)

### Tasks

- [x] T2.1 Codex app-server parser — Goal: convert lenient camelCase RPC DTOs into strict full/sparse readings while enforcing the `codex` limit-ID filter — Files: `crates/usage-core/src/parse/{mod.rs,codex_app_server.rs}`, focused tests, Appendix A.2 fixture — Acceptance: the documented response yields exactly one full Weekly 2% reading with plan `Pro`; a secondary-only notification is one-window partial; `premium` never reaches the store — Verify: red/green fixture and sparse/filter tests — Result: 4 focused tests pass; the exact Appendix A.2 response yields one full Weekly 2% `Pro` reading, sparse secondary data remains partial, and both mapped and notification `premium` snapshots are excluded; full fmt, strict workspace Clippy, and workspace tests pass.
- [x] T2.2 Codex rollout parser — Goal: parse snake_case JSONL into full limit readings and fork-safe token deltas — Files: `crates/usage-core/src/parse/codex_rollout.rs`, focused tests, Appendix A.1/A.6 fixtures — Acceptance: A.1 yields Weekly 1% plus its delta; premium yields tokens only; legacy `resets_in_seconds` computes 1790132400; forked sessions count exactly 3000 under all four §2.2 rules — Verify: fixture-driven red/green tests — Result: 4 focused tests pass; the exact fixtures yield the documented full reading, 211,554 first-line delta, 1,790,132,400 legacy reset, and fork-safe deltas `[1000, 0, 2000]`; an added counter-reset case proves rule 3, and a `premium` case proves token events survive the limit filter; full task gates pass.
- [x] T2.3 Claude status-line parser — Goal: convert the bridge-file envelope and lenient Claude rate-limit shape into full readings — Files: `crates/usage-core/src/parse/claude_statusline.rs`, focused tests, Appendix A.4 fixture — Acceptance: A.4 yields Session 23.5 and Weekly 41.2; an absent `five_hour` yields only Weekly — Verify: exact fixture and missing-window tests — Result: 4 focused tests pass; A.4 yields the exact Session/Weekly values, independently missing windows are omitted, empty shapes are ignored, and the parser accepts both raw `rate_limits` input and the persisted `rateLimits` envelope while honoring `writtenAt`; full task gates pass.
- [x] T2.4 Claude local-log parser — Goal: extract assistant token events without accepting non-assistant records or losing dedupe provenance — Files: `crates/usage-core/src/parse/claude_log.rs`, focused tests, Appendix A.3 fixture — Acceptance: A.3 totals 92,586 tokens and produces `message_id:request_id`; non-assistant lines are skipped — Verify: exact fixture and negative tests — Result: 4 focused tests pass; the exact duplicate fixture emits identical 92,586-token events with `msg_01TEST:req_01TEST`, the history store counts them once, non-assistant/no-usage records are skipped, missing counters default to zero, and overflow is rejected; full task gates pass.

### Phase review and commit

- [x] Goal: leave M2 lenient at external boundaries and strict internally — Files: Phase 3 changes only — Acceptance: every Appendix fixture is checked in verbatim, malformed/unknown fields cannot panic, parser output observes unit/source/filter contracts, and coverage remains above the core gate — Verify: fmt, workspace clippy/tests, UI checks if bindings change, coverage, staged diff review, commit, `git log -1 --stat`, clean status — Result: all automated gates through the pre-commit review pass; commit follows this record.

### Review

- Checks: `cargo fmt --all -- --check` ✅ · workspace Clippy with `-D warnings` ✅ · workspace tests ✅ (17 parser-focused/robustness tests; none skipped) · `cargo llvm-cov -p usage-core --fail-under-lines 90` ✅ (95.53% lines; every provider parser ≥96%) · generated bindings unchanged ✅ · `git diff --check` ✅.
- Diff review: all parser sources and tests re-read; every external DTO field is optional with serde defaults and unknown fields remain accepted. Appendix A.1–A.4 and A.6 files compare byte-for-byte with their documentation code blocks. Codex limit IDs are filtered before readings, token deltas remain independent of that filter and per-file state is explicit. Claude duplicates retain a stable key for store dedupe, arithmetic is checked, malformed JSON returns typed errors, and no parser performs I/O, logs secrets, panics, or exceeds 400 lines.
- Decisions: D-007 records the raw-status-line versus persisted-bridge shape ambiguity and the additive parser behavior.
- New handover items: none; M2 is fully fixture/pure-logic verifiable.

### Blocked (hard stops only)

- None.

## Phase 4 — Status-line bridge (M3)
Status: committed (c0ccd5b)

### Tasks

- [x] T3.1 Synchronous bridge fast path — Goal: accept bounded Claude status-line stdin, atomically persist the documented schema-1 bridge file, and emit the compact fallback text without ever failing Claude Code — Files: `crates/statusline-bridge/{Cargo.toml,src/**,tests/basic.rs}` (split to keep resource and process logic small) — Acceptance: only `serde_json` and `tempfile` are runtime dependencies; input is capped at 4 MiB; a temp `--out-dir` run prints `5h 24% · 7d 41%`, writes valid complete JSON with verbatim rate limits, and exits 0; invalid/oversized input exits 0 silently — Verify: test-first binary integration coverage, the exact §13 command in a temp directory, and standard workspace gates — Result: 2 binary tests pass; the exact temp probe prints `5h 24% · 7d 41%`, `jq` validates the schema/session/time and verbatim window object, invalid and >4 MiB inputs exit 0 silently, atomic writes use unique same-directory temp files plus `sync_all`, and full fmt/strict Clippy/workspace tests pass.
- [x] T3.2 Chained status line — Goal: forward the original stdin to an existing configured command, return its stdout verbatim, and kill it at the hard two-second deadline after the bridge file is already durable — Files: bridge process module and focused tempdir tests — Acceptance: `cat >/dev/null; echo CHAINED` returns `CHAINED`; `sleep 5` finishes in 2.0–2.3 seconds with the rate-limit file present; child stdio cannot deadlock — Verify: red/green integration tests, the two §13 shell probes, and standard workspace gates — Result: 2 focused chain tests pass; the exact command returned `CHAINED`, the direct-binary sleep probe returned in 2.01 s, and `jq` confirmed the bridge file was already valid; stdin/stdout workers avoid pipe deadlock, stderr is discarded, timeout kills/reaps the child, and full task gates pass.
- [x] T3.3 Overlap safety and performance — Goal: prove atomic last-writer-wins behavior, abandoned-temp cleanup, and the unchained latency budget under realistic process concurrency — Files: `crates/statusline-bridge/tests/{concurrency.rs,budget.rs}` plus minimal production fixes — Acceptance: 50 overlapping invocations with every fifth killed leave a valid target; startup cleanup removes stale `.tmp*`; 100 release runs have p95 <20 ms — Verify: debug concurrency test, `cargo test --release -p statusline-bridge`, and standard workspace gates — Result: synchronized 50-process overlap with 10 varied kill attempts leaves a valid schema-1 target and at most one temp per killed writer; future-time startup cleanup removes all abandoned temp files; 100 optimized invocations pass the p95 <20 ms assertion; release and full workspace gates pass.

### Phase review and commit

- [x] Goal: leave M3 synchronous, bounded, atomic, and isolated from real user paths — Files: Phase 4 changes only — Acceptance: no async runtime or extra dependency, no real Application Support access in verification, no panic/output on failure, and all child/temp resources are reaped or cleaned — Verify: fmt, strict workspace Clippy/tests, release M3 tests, dependency/diff/resource review, commit, `git log -1 --stat`, clean status — Result: all automated gates through the pre-commit review pass; commit follows this record.

### Review

- Checks: exact fast-path/invalid/chain/sleep temp probes ✅ · `cargo fmt --all -- --check` ✅ · workspace Clippy with `-D warnings` ✅ · workspace tests ✅ (6 M3 integration tests, none skipped) · `cargo test --release -p statusline-bridge` ✅ (100-process p95 <20 ms assertion, 50-process overlap, process-group timeout) · direct dependency tree exactly `serde_json` + `tempfile` ✅ · `git diff --check` ✅.
- Diff review: every M3 source/test file re-read; stdin is capped at 4 MiB, rate limits are copied semantically unchanged into a unique same-directory temp file, `sync_all` precedes atomic persistence, startup cleanup targets only hour-old `.tmp*` regular files, and all verification paths use tempdirs. Review caught and fixed an unbounded chained-stdout `Vec` and shell-only timeout: stdout now spools to an anonymous temp file, and the whole isolated process group is killed/reaped before the 2.3 s ceiling. No async runtime, panic/unwrap/unsafe in production, real user-data access, secret output, leaked child, or file over 400 lines remains.
- Decisions: D-008 resolves the broad `dirs` table versus M3's explicit two-dependency rule in favor of the task-specific contract.
- New handover items: none; native installer consent and real Claude effectiveness remain later M5/manual acceptance work, not M3 bridge gaps.

### Blocked (hard stops only)

- None.

## Phase 5 — I/O infrastructure (M4)
Status: committed

### Tasks

- [x] T4.1 Paths, events, and store actor — Goal: centralize all derived filesystem paths and route bounded source events through the sole mutable usage/history owner — Files: `crates/usage-sources/{Cargo.toml,src/{lib.rs,paths.rs,event.rs,store.rs},tests/**}` — Acceptance: overrides/environment/default roots resolve without touching real data in tests; the source channel is capacity 256 and alert channel 32; readings derive through a watch snapshot, history commands reply, and cancellation ends the actor within 100 ms — Verify: red/green temp-root and actor tests plus standard workspace gates — Result: 4 focused tests pass; synthetic roots prove settings > environment > default path resolution, bounded channel constants are 256/32, readings/history/75% alerts traverse the actor, and idle cancellation completes within 100 ms; a package-scoped clean removed a stale pre-M3 test binary and a cold full fmt/strict Clippy/workspace test run passes.
- [x] T4.2 Bounded incremental tailer — Goal: read appended JSONL bytes exactly once while surviving partial lines, truncation/rotation, oversized records, and stale file-state pruning — Files: `crates/usage-sources/src/tail.rs` and focused tempdir tests — Acceptance: appends emit once; partial data completes on the next append; shorter files reset offset; >16 MiB lines are skipped without unbounded buffers; file state older than 8 days is forgotten — Verify: listed red/green tests plus standard workspace gates — Result: 5 focused tempdir tests pass; append/partial delivery is exactly-once, both truncation and inode replacement reset state, oversized lines discard through the next newline while retaining at most 16 MiB per file, complete output batches cap at 4,096 lines/~16 MiB and continue without loss, and modification state older than eight days is pruned; full fmt, strict workspace Clippy, and workspace tests pass.
- [x] T4.3 Debounced filesystem watcher — Goal: adapt `notify` callbacks into a bounded async path stream with recursive watching, relevant-event filtering, and 500 ms per-path coalescing — Files: `crates/usage-sources/src/watch.rs` and focused tempdir tests — Acceptance: a tempdir write yields one path event, bursts coalesce per path, watcher ownership ends on cancellation — Verify: red/green FSEvents test plus standard workspace gates — Result: 2 native FSEvents tempdir tests pass outside the command sandbox; bounded 256-entry callback/output channels and a 256-path debounce set coalesce write bursts to one canonical path, JSONL/exact-file filters reject irrelevant events, and cancellation drops watcher ownership within 100 ms; full fmt, strict workspace Clippy, and workspace tests pass.
- [x] T4.4 Adaptive scheduler — Goal: provide per-source due times for activity-aware polling, immediate triggers, minimum gaps, capped backoff, and wake detection — Files: `crates/usage-sources/src/scheduler.rs` and paused-time tests — Acceptance: active/idle intervals, 15 s minimum gap, 30 s→30 min capped backoff with bounded jitter/Retry-After, UI-visible freshness, and >60 s wall/monotonic wake drift follow §§3.3/9.4 — Verify: deterministic `tokio::time::pause()` tests plus standard workspace gates — Result: 5 paused-time/timing tests pass; Codex switches 600 s idle ↔ 120 s active, triggers retain the 15 s floor, failures double from 30 s to a 30 min cap with ±20% jitter and longer Retry-After precedence, stale visible UI reads immediately, >60 s wall/monotonic drift triggers wake detection, and cancellation is prompt; `SourceEvent::Activity` is routed store→scheduler through bounded actor messages; full fmt, strict workspace Clippy, and workspace tests pass.

### Phase review and commit

- [x] Goal: leave M4 bounded, cancellation-safe, deterministic under tests, and ready for concrete sources — Files: Phase 5 changes only — Acceptance: no unbounded channels/buffers, blocking filesystem work on async tasks, leaked watchers/tasks, real-user path access, or duplicate delivery — Verify: fmt, strict workspace Clippy/tests, dependency/resource/diff review, commit, `git log -1 --stat`, clean status — Result: all automated gates through the pre-commit review pass; commit follows this record.

### Review

- Checks: `cargo fmt --all -- --check` ✅ · workspace Clippy with `-D warnings` ✅ · workspace tests ✅ (17 M4 integration tests; none skipped) · native FSEvents tempdir coverage ✅ outside the command sandbox · UI typecheck/lint/build ✅ · direct dependency tree matches §4.1 ✅ · `git diff --check` ✅.
- Diff review: every M4 source/test file was re-read; paths have one derivation point and tests use synthetic roots, the store/scheduler remain single-owner actors, all mpsc queues and watcher pending state are bounded, every long-lived loop is cancellation-aware, and native watcher ownership stays inside its source task. Review caught an otherwise unbounded vector of completed tail lines: each call now caps both bytes and lines, exposes `has_more`, and a 5,000-line test proves continuation without loss. Partial lines remain capped at 16 MiB; source files remain below 400 lines; production code contains no unwrap/expect/panic/unsafe, real-user test access, leaked task, or unbounded collection.
- Decisions: D-009 records settings > environment > defaults path precedence.
- New handover items: none; native FSEvents behavior was verified on the host rather than replaced with a polling fake.

### Blocked (hard stops only)

- None.
