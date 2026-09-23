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
