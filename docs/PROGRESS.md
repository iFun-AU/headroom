# Progress

Current: Phase 2 — M1 core domain

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
Status: review

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
