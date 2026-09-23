# Kickoff prompt: build How Is It to completion, phase by phase

> **Author:** AI Assistant · **Version:** 2.0 · **Date:** 2026-09-23 · **Spec:** `docs/DEVELOPMENT.md` v2.2 (final) · **Design:** `docs/design/` (canvas v4, 29 artboards)
> **Changes in 2.0:**
> - Runs **to completion without approval gates**, using the decision rules in `DEVELOPMENT.md` §0.1.
> - Adds a toolchain preflight, a resume procedure, a screenshot method for design checks, and a final handover phase.
> - Forbids editing the spec and design.

## Launcher prompt (paste this into the implementing model)

```text
You are the implementing engineer for the "How Is It" macOS app in this repository.
Read docs/AGENT_KICKOFF_PROMPT.md completely and follow it exactly, starting at the
"Start or resume" step. Work phase by phase until the Definition of Done in
docs/DEVELOPMENT.md §15 is met. Review and commit at the end of every phase. Do not
wait for approval between phases. Use the decision rules in DEVELOPMENT.md §0.1 and
stop only for the hard stops listed there. Keep docs/PROGRESS.md current after every
task so that any later session can resume where you stopped.
```

---

You are the implementing engineer for **How Is It**, a macOS 26 menu bar app (Rust + Tauri 2 + React/TypeScript). It shows how much of the user's Claude and Codex plan limits are used. The spec and design are **final**. Your job is to build all of milestones M0–M8 to the Definition of Done (`DEVELOPMENT.md` §15), phase by phase, with a review and a commit at the end of every phase.

## Ground rules

1. **The documents are the contract.** `docs/DEVELOPMENT.md` decides behavior, data, types, IPC and tasks. `docs/design/` decides the look. **Don't edit either one**, nor `docs/design/**` or `docs/TOKEN_USAGE_FINDINGS.md`. You write only code, tests, scripts, and `docs/{UNDERSTANDING,PROGRESS,DECISIONS,HANDOVER}.md` (plus the README in T8.3).
2. **Don't wait for approval.** When something is unclear or differs from reality, apply the matching rule in `DEVELOPMENT.md` §0.1, log it in `docs/DECISIONS.md`, and continue. Never guess silently, and never invent API fields, formats, commands or settings.
3. **Hard stops (the only reasons to halt):**
   - the next step would touch **real user data**: the real `~/.claude/settings.json`, `~/.codex/auth.json`, Keychain items, or files outside the repo and temp dirs
   - it would need a **secret, password or credential**
   - the toolchain can't be installed without `sudo` or a GUI prompt

   Record the blocker in `PROGRESS.md` before halting.
4. **Out of scope:** M9 (Claude OAuth poller, feature `claude-oauth`) and T8.4 (signing and notarization). Don't build them.
5. **One task at a time**, in the order of `DEVELOPMENT.md` §13. A task is done only when its *Acceptance* and *Verify* steps pass.
6. **Small changes:** ≤ ~400 lines per file, ≤ ~60 lines per function, ≤ 8 files per task. Pure logic goes in `usage-core` (no I/O).
7. **Tests never touch real data.** Use temp dirs, `--out-dir`, and settings overrides. **Never** run the Claude bridge installer against the real `~/.claude/settings.json`. **Never** read `~/.codex/auth.json`. **Never** log or commit secrets.
8. **Git:**
   - One commit per phase, after its review passes.
   - Never use `--no-verify`, force-push, rewrite history, or commit `target/`, `node_modules/`, `dist/`, `.env*` or build output.
9. **Failing checks:** fix the root cause. Never delete, skip or weaken a test or lint. After **three** focused attempts, follow the §0.1 "check fails" rule.

## Reading order (Phase 0)

Read each file completely. For files over 500 lines, read in chunks with offset + limit to the end.

| # | File | Take away |
|---|---|---|
| 1 | `docs/DEVELOPMENT.md` §0–§1 (incl. **§0.1**) | Rules, decision rules, hard stops, v1 scope |
| 2 | `docs/DEVELOPMENT.md` §2 | Every data source, verified facts, fork-safe token rule, UTC-day rule, ⚠ VERIFY items |
| 3 | `docs/DEVELOPMENT.md` §3–§6 | Architecture, update strategy, stack and versions, repo layout, lints, memory and security rules |
| 4 | `docs/DEVELOPMENT.md` §7–§9 | Types, merge rules I1–I4 / D1–D5, history series, projection, alerts, sources, store, settings |
| 5 | `docs/DEVELOPMENT.md` §10–§12 | Three windows (sizes, widget per variant), tray, IPC, routes, notifications, logs |
| 6 | `docs/DEVELOPMENT.md` §13–§17 + Appendix A | Every task, test strategy, soak "Agent run", Definition of Done, risks, fixtures A.1–A.6 |
| 7 | `docs/design/README.md` | Contents of the design package |
| 8 | `docs/design/DESIGN_IMPLEMENTATION_PROMPT.md` | UI rules and decisions D1–D10 |
| 9 | `docs/design/screenshots/*.png` (or `docs/design/preview/index.html`) | Look at **all 29** artboards |
| 10 | `docs/TOKEN_USAGE_FINDINGS.md` | Why token charts are labelled "this Mac" or "account" |

---

## Start or resume (do this at the beginning of every session)

1. If `docs/PROGRESS.md` exists, read it. Find the first phase that isn't `committed`, and within it the first unchecked task. Read the spec sections for that phase again, then continue from there. Skip Phase 0 if it's committed.
2. Run `git status`. If there are uncommitted changes from an earlier session, compare them with `PROGRESS.md` and finish or repair that task before starting a new one.
3. If nothing exists yet, start at Phase 0.

## Phase 0: Understand and prepare (no application code)

1. **Read** everything in the reading order.
2. **Toolchain preflight.** Run each command and record the output in `UNDERSTANDING.md`:
   ```bash
   sw_vers -productVersion
   ```
   ```bash
   xcode-select -p
   ```
   ```bash
   rustc --version && cargo --version
   ```
   ```bash
   rustup target list --installed
   ```
   ```bash
   node --version && npm --version
   ```
   ```bash
   codex --version
   ```
   ```bash
   claude --version
   ```
   - You need macOS ≥ 26, the Xcode Command Line Tools, Rust ≥ 1.95, and Node ≥ 20.
   - Missing Rust targets can be added without sudo:
     ```bash
     rustup target add aarch64-apple-darwin x86_64-apple-darwin
     ```
   - If `codex` or `claude` is missing, that isn't a stop. Use fixtures (§0.1) and add the live checks to the handover list.
   - If the OS, Xcode tools or Rust can't be provided without sudo or a GUI → **hard stop**.
3. **Write `docs/UNDERSTANDING.md`**, short and in your own words:
   - **Product in 5 lines.**
   - **Data sources:** a table of source → provides → push or poll → priority.
   - **One Codex update, end to end:** name each component from the child process to the UI.
   - **Windows and routes**, with sizes (widget per variant).
   - **Settings fields**, with defaults.
   - **Answers**, each with the spec section it comes from:
     1. Why classify windows by duration, not `primary` / `secondary`?
     2. The app-server is 20 min stale and the rollout is fresh. Which source is authoritative, and what status shows?
     3. What does a full reading do to missing windows? What does a partial reading do?
     4. What does the bridge write, where, and how does it stay atomic under overlapping runs?
     5. How does bridge uninstall treat later user edits to `settings.json`?
     6. What is the fork-safe Codex token rule, and what does fixture A.6 total?
     7. Why does the Codex account daily series end yesterday, and how is its last column labelled?
     8. Which colors and icons apply at 0–74, 75–89, 90–99 and 100 %?
     9. Why does the neon bar animate `width` and not `scaleX`?
     10. What must stay inside the widget window on hover, and why?
     11. What is behind the `claude-oauth` feature, and is it built?
     12. What must never be logged, and how is external text truncated?
     13. Which steps will end up on the handover checklist?
   - **Open items:** every ⚠ VERIFY, with the task and the §0.1 rule you'll apply.
4. **Repo setup:**
   - If `.git` is missing, run `git init`.
   - Create a `.gitignore` covering `target/`, `node_modules/`, `dist/`, `.DS_Store`, `*.log`, `src-tauri/binaries/`, `.env*`, `docs/design/source/__pycache__/`.
   - Create `docs/PROGRESS.md` and `docs/DECISIONS.md` from the templates below.
5. **Review:** check each answer against the section it cites, and fix any mismatch.
6. **Commit:** `docs: add understanding notes, progress and decision logs`.

## Phases 1–9: build (`DEVELOPMENT.md` §13)

| Phase | Milestone | Scope | Re-read before starting |
|---|---|---|---|
| 1 | M0 | Workspace, Tauri + UI scaffold, sidecar script, `DECISIONS.md` first entries | §4, §5, §6.1, §10.1 |
| 2 | M1 | Types, `log_trunc`, `classify`, bindings, merge, history, projection, alerts | §6.2, §7 |
| 3 | M2 | Parsers + fixtures A.1–A.4 and **A.6** | §2, Appendix A |
| 4 | M3 | Status line bridge binary | §2.3, §8.3 |
| 5 | M4 | Paths, events, store actor, tail, watch, scheduler | §3.3, §6.3, §9 |
| 6 | M5 | RPC, app-server, rollout, bridge installer, bridge source + history, probe | §2, §8 |
| 7 | M6 | Settings, commands, tray, windows (widget sized per variant), plugins | §9.3, §10, §12 |
| 8 | M7 | UI to the design | `docs/design/DESIGN_IMPLEMENTATION_PROMPT.md` in full, §11 |
| 9 | M8 | Automated soak, universal local build, README (T8.4 excluded) | §14, §15 |

### The task loop (for every task)
1. **Restate** the task's Goal, Files, Acceptance and Verify under the current phase in `PROGRESS.md`.
2. **Plan** the smallest file set (≤ 8). Split larger tasks and note why.
3. **Tests first** where the task lists tests: write them, run them, and see them fail for the right reason.
4. **Implement** until they pass. Give every public Rust item a `///` doc and every TS module a header comment.
5. **Verify** with the task's own *Verify* commands, plus:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
   After touching `ui/`:
   ```bash
   npm --prefix ui run typecheck && npm --prefix ui run lint
   ```
   Read the output. Printed errors mean it failed.
6. **Tick** the task in `PROGRESS.md` with one line per command and its result. **Update `PROGRESS.md` after every task**; it's how a new session resumes.

### Comparing the UI with the design (Phase 8)
The native window glass can't be captured headlessly, so compare the **CSS layers** in a browser:
- Add a **dev-only** fixture adapter in `ui/src/ipc.ts`: when `window.__TAURI_INTERNALS__` is absent **and** `import.meta.env.DEV` is true, `ipc.ts` returns fixture `UsageSnapshot` / `History` / `SettingsState` values that mirror the artboards' sample data. The adapter must be excluded from production builds. Log it in `DECISIONS.md`.
- Serve the UI with the Vite dev server. Capture each route and state at the artboard size (main 720×520, popover 340×420, widget 280×72 / 160×180 / 200×24), in dark and light (`prefers-color-scheme` emulation). Use headless Chrome if it's installed:
  ```bash
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --window-size=720,520 --screenshot=target/ui-shots/overview-dark.png "http://localhost:1420/#/"
  ```
  Otherwise use any available headless browser.
- Compare each capture with its file in `docs/design/screenshots/`, ignoring the wallpaper and the native glass. Spacing should be within ±2 pt, radii identical, every color from a token, and the glow visibly matching.
- The **native** look (real glass, the tray, the widget over the desktop) goes on the handover checklist.

### End of every phase: review, then commit
1. **Full check run** from a clean state. All standard commands pass with zero warnings.
2. **Diff review:** run `git status` and `git diff --stat`, then read the whole `git diff` file by file:
   - [ ] Every change traces to a task in this milestone. There are no unrelated edits, and no edits to the spec or design.
   - [ ] Names, fields, rules and numbers match the spec sections for this milestone. Re-open them and compare.
   - [ ] Every listed test exists, including the §7.4 required tests and races.
   - [ ] No `unwrap` / `expect` / `panic!` / `todo!` / `dbg!` outside tests. No `unsafe`, except the documented Tauri-macro case.
   - [ ] Channels are bounded, spawned tasks are owned and cancellable, children use `kill_on_drop`, and stderr is drained.
   - [ ] No secrets, no real user paths in tests, and external text is truncated before logging.
   - [ ] Size limits are respected. No commented-out code. Every TODO has a `PROGRESS.md` note.
   - [ ] Phase 8 only: each screen and state compared in dark and light, with reduced motion and reduced transparency checked.
3. **Fix** every finding, then repeat 1–2.
4. **Record** in `PROGRESS.md`: what was built (one line per task), the check results, deviations (with `DECISIONS.md` ids), ⚠ VERIFY outcomes, known limits, and new handover items.
5. **Commit** once:
   ```text
   <type>(<scope>): <milestone summary> (M<k>)

   - <task one-liner>
   - <task one-liner>

   Verified: cargo fmt/clippy/test clean; <other checks>
   Refs: docs/DEVELOPMENT.md §13 M<k>
   ```
   Types: `feat`, `fix`, `test`, `docs`, `build`, `chore`, `refactor`.
6. **Confirm** the commit (`git log -1 --stat`) and a clean tree (`git status`). Set the phase to `committed (<sha>)` in `PROGRESS.md`. **Go straight on to the next phase.**

## Phase 10: Handover

1. Walk through `DEVELOPMENT.md` §15 item by item. Fix anything that isn't met but can be automated.
2. Write `docs/HANDOVER.md`: every human-only step with exact commands or instructions and the expected result, in this order:
   - the §14.4 manual acceptance items
   - the 8-hour soak and the Safari WebView memory check
   - live ⚠ VERIFY checks that couldn't run here
   - native look vs the design
   - T8.4 (optional)
3. Check that the README (T8.3) covers install, permissions, the bridge (what it changes, how to uninstall, the project or organization override caveat) and "local build only".
4. Review as above, then commit with `docs: handover checklist and final DoD review`, and tag it:
   ```bash
   git tag v1.0.0-local
   ```
5. Final message to the owner: what's built, where the app bundle is, the `DECISIONS.md` highlights, and a pointer to `HANDOVER.md`.

---

### Template: `docs/PROGRESS.md`
```markdown
# Progress
Current: Phase <n> — <task id>   (update after every task)

## Phase <n> — <milestone> (M<k>)
Status: in progress | review | committed (<sha>)

### Tasks
- [ ] T<k>.1 <title> — Acceptance: … — Verify: … — Result: …

### Review
- Checks: fmt ✅ clippy ✅ test ✅ (ui: typecheck ✅ lint ✅)
- Diff review: <findings → fixes>
- Decisions: <D-ids or none> · ⚠ VERIFY: <item → outcome>
- New handover items: <none | list>

### Blocked (hard stops only)
- <none | what, why it is a hard stop, what the owner must do>
```

### Template: `docs/DECISIONS.md` entry
```markdown
## D-<nnn> <title> (<date>, phase <n>)
- Context: <what forced a decision; spec section>
- Rule applied: DEVELOPMENT.md §0.1 "<row>"
- Decision: <what was chosen>
- Why: <reason, alternatives considered>
- Evidence: <probe output shape, test name, command result>
```
