# Progress

Current: Phase 1 — review

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
Status: review

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
