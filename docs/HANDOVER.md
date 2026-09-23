# How Is It v1 handover

All automatable M0–M8 checks passed on 2026-09-24. The local universal app is:

```text
target/universal-apple-darwin/release/bundle/macos/How Is It.app
```

It is ad-hoc signed and intended only for local use. The unchecked work below requires a person because it changes real files, reads real provider data, uses native macOS UI, needs eight hours of elapsed time, or requires owner credentials. Prefer a disposable macOS account for mutation tests, keep recoverable backups, and never paste credentials into this repository or its logs.

## 1. Manual acceptance from DEVELOPMENT.md §14.4

### Install and launch preflight

1. Copy the reviewed bundle to `/Applications`:

   ```bash
   ditto \
     "target/universal-apple-darwin/release/bundle/macos/How Is It.app" \
     "/Applications/How Is It.app"
   ```

2. Close any development-launched copy, then open **How Is It** from Finder or Spotlight, not from a terminal. This proves GUI path discovery. Expected: onboarding opens and Codex is detected from a fixed candidate or bounded login-shell lookup; if it is not, **Browse…** can select the executable.
3. Perform the fresh-install and bridge-mutation checks in a disposable macOS account if existing How Is It or Claude settings must be preserved.

The ten checkbox lines below are copied unchanged and remain in contract order.

- [ ] Fresh install → onboarding → the Codex bar appears within 5 s.

  **Instructions:** launch the `/Applications` copy with no existing How Is It settings, complete the Codex step, skip or accept the optional Claude bridge, and finish notification onboarding. Time from completion to the first Codex bar.

  **Expected:** the dashboard appears, the Codex bar has real data within five seconds, and no Claude settings are changed unless bridge consent was given.

- [ ] Enable the Claude bridge → run a Claude Code prompt → the Claude bars update in < 2 s, and the bridge state becomes `Confirmed`.

  **Instructions:** duplicate `~/.claude/settings.json` first if it exists. Enable **Settings → Accounts → Claude → Real-time updates**, start Claude Code, and submit one ordinary prompt while watching How Is It.

  **Expected:** the session/weekly bars update within two seconds of Claude's response; Diagnostics reports `Confirmed`; the backup, installed bridge, and unrelated settings remain intact.

- [ ] Create a project with `.claude/settings.local.json` that sets its own `statusLine`, then use Claude Code only there for 10+ min → the bridge state becomes `LikelyOverridden` with the hint.

  **Instructions:** use a disposable project and configure a harmless project-local status-line command, for example:

  ```json
  {"statusLine":{"type":"command","command":"printf 'project override\\n'"}}
  ```

  Keep Claude Code activity confined to that project for more than ten minutes after the user bridge was installed.

  **Expected:** How Is It keeps the last good limits, changes bridge effectiveness to `LikelyOverridden`, dims the affected session value, and explains that project or managed settings may take precedence.

- [ ] Quit Claude Code → the Claude bars show "Updated Xm ago" and turn Stale after 15 min.

  **Instructions:** first obtain a confirmed bridge reading, quit every Claude Code process, and leave How Is It running without new Claude events.

  **Expected:** the age text advances and, strictly after 15 minutes, Claude is visually stale without losing the last good values.

- [ ] Use Codex in a terminal → the Codex bar updates in < 15 s.

  **Instructions:** note the current Codex percentage, use Codex in a terminal, and watch both the menu item and dashboard.

  **Expected:** rollout activity triggers reconciliation and the visible Codex value or timestamp changes within 15 seconds.

- [ ] Kill our `codex app-server` child → the Codex status shows `Degraded`, the bars keep updating from rollout files, and the child restarts with backoff.

  **Instructions:** list running copies, choose the PID of the installed How Is It app, then list only that process's children:

  ```bash
  pgrep -x how-is-it
  HOWISIT_PID='replace-with-verified-app-pid'
  ps -p "$HOWISIT_PID" -o pid=,command=
  pgrep -P "$HOWISIT_PID" -fl 'app-server'
  ```

  Replace the quoted placeholder with the numeric app PID before running the remaining commands. Set `HOWISIT_CHILD_PID` to the single verified `codex app-server` child PID. Confirm its PPID equals `HOWISIT_PID` with `ps -p "$HOWISIT_CHILD_PID" -o pid=,ppid=,command=`, then run `kill "$HOWISIT_CHILD_PID"`. Never target unrelated Codex processes.

  **Expected:** status becomes `Degraded`, rollout-file activity still updates the bars, and exactly one replacement child appears after bounded backoff.

- [ ] Sleep the Mac for 10 min → wake → all data refreshes within 5 s.

  **Instructions:** leave both providers in a known state, sleep the Mac for at least ten minutes, wake it, and time the first refresh.

  **Expected:** wake drift is detected and both provider snapshots reconcile within five seconds without duplicate children or a frozen UI.

- [ ] Add a key to `~/.claude/settings.json` after install → uninstall the bridge → `statusLine` is restored and the added key is still there.

  **Instructions:** after bridge installation, add a benign top-level key such as `"howIsItHandoverProbe": true` with a JSON-aware editor. Disable real-time updates from How Is It.

  **Expected:** the exact pre-install `statusLine` value is restored (or removed if originally absent), the probe and every unrelated edit remain, and uninstall does not replace the whole file from backup.

- [ ] Rename the `codex` binary → Codex shows `NotConfigured` with a hint, and the app doesn't crash.

  **Instructions:** avoid renaming the primary installation. Create a temporary symlink:

  ```bash
  HOWISIT_CODEX_DIR="$(mktemp -d)"
  ln -s "$(command -v codex)" "$HOWISIT_CODEX_DIR/codex"
  ```

  Select `$HOWISIT_CODEX_DIR/codex` with **Browse…**, confirm Codex is detected, then run `mv "$HOWISIT_CODEX_DIR/codex" "$HOWISIT_CODEX_DIR/codex.off"` and relaunch How Is It. Restore the symlink name after the check and select the real executable again.

  **Expected:** Codex becomes `NotConfigured` with an actionable path hint; the app, Claude data, and settings remain usable.

- [ ] Settings → Browse… picks a path, Reveal Logs opens Finder, and Quit exits cleanly.

  **Instructions:** select a valid Codex executable with the native picker, use **Diagnostics → Reveal Logs**, then quit from the menu-bar action.

  **Expected:** the selected path persists, Finder opens `~/Library/Logs/dev.howisit.app`, and both the app and its owned child are gone after Quit.

## 2. Eight-hour soak and Safari WebView memory

### Eight-hour production-rate soak

- [ ] Run the full §14.3 soak for eight elapsed hours against temporary `codex_home` and `claude_dir` roots.

  **Instructions:** use `scripts/soak.sh` as the audited setup/process-cleanup reference, but do not count its completed 10×/60-minute mode as this result. For this run, generate exactly one synthetic Codex token line and one synthetic Claude assistant line per second, and atomically replace the bridge file every five seconds. Keep all fixtures under one `mktemp -d` root and point app settings overrides there; do not use real provider trees. Toggle the real menu-bar popover every minute (AppleScript after granting Accessibility access, or manually for at least the first ten minutes). Record this every five minutes:

  ```bash
  pgrep -x how-is-it
  HOWISIT_PID='replace-with-verified-app-pid'
  footprint -p "$HOWISIT_PID" | grep 'phys_footprint:'
  pgrep -P "$HOWISIT_PID" -f 'app-server' | wc -l
  ```

  Replace the quoted placeholder with the verified installed-app PID before running the remaining commands. Retain a CSV with timestamp, elapsed seconds, physical footprint, child count, and child PID. Stop synthetic events after hour eight, leave the app idle for ten minutes, record average CPU in Activity Monitor, run `leaks "$HOWISIT_PID"`, quit normally, and check the logged child PID with `kill -0`.

  **Expected:** hour-one to hour-eight main-process footprint growth is under 10%; ten-minute idle CPU averages under 0.5%; every child count is zero or one and matches the logged spawn PID; the app and child exit cleanly. On target macOS 26, `leaks` must say `0 leaks for 0 total leaked bytes`. Do not treat D-018's narrowly classified macOS 27 AppIntents cycles as raw-zero proof for macOS 26.

### Safari WebView memory

- [ ] Verify the dashboard WebView heap returns to baseline after 50 open/close cycles.

  **Instructions:** enable Safari's **Develop** menu, run a debug build with either explicit consent for live provider data or the same isolated temporary roots, and open **Develop → How Is It → dashboard WebView → Timelines/Memory**. Record a post-GC baseline, open and close the dashboard 50 times, force/allow GC, then record the settled heap. Repeat once to distinguish one-time framework allocation from monotonic retention.

  **Expected:** detached React trees/listeners do not accumulate and the settled heap returns close to the post-warm-up baseline rather than growing with each cycle.

## 3. Live ⚠ VERIFY and real-data checks

- [ ] **Codex app-server/account shape:** run the bounded probe only after consenting to reads from the real provider trees:

  ```bash
  cargo run -p usage-sources --example probe -- --seconds 120
  ```

  In a separate Codex session run `/status`. Expected: the probe/app and `/status` agree on the Codex weekly percentage and reset, the account daily series ends on the latest completed UTC day, and no raw RPC payload, account ID, credential, or auth file content is printed.

- [ ] **Claude live comparison:** with the bridge explicitly enabled, run one Claude Code prompt during the probe. Expected: a Claude Session and Weekly reading appears within about one second, then bridge effectiveness becomes `Confirmed`; no OAuth or Keychain access occurs.

- [ ] **Older Codex compatibility:** if an isolated older Codex executable that emits `resets_in_seconds` is available, select only that copy and compare its reset with the app. Expected: the reset is the observation timestamp plus the relative seconds, matching the fixture-tested compatibility path. Do not downgrade or rename the primary Codex installation merely to perform this check.

- [ ] **Real log secret scan:** after the manual scenarios, run:

  ```bash
  grep -Eri 'bearer|accessToken|sk-' "$HOME/Library/Logs/dev.howisit.app"
  ```

  Expected: no output and exit status 1. Inspect any match as sensitive; do not paste it into an issue or commit.

- [ ] **Claude OAuth post-v1 gate:** v1 must continue to show no OAuth toggle, Keychain prompt, `reqwest`, or `security-framework` path. Before any M9 work, the owner must explicitly accept the unofficial endpoint's policy risk, verify only JSON key paths/types with the gated probe described in T9.1, and record that decision. Expected for this v1 handover: M9 remains unbuilt.

## 4. Native look and macOS behavior

- [ ] Compare the installed app over a real desktop with `docs/design/screenshots/`: dashboard 720×520, popover 340×420, and Pill/Stack/Mini widgets at 280×72, 160×180, and 200×24. Expected: real Liquid Glass, radii, neon layers, spacing, threshold text/icons, and light/dark appearance match the approved artboards without fake wallpaper or chrome.
- [ ] Toggle **System Settings → Accessibility → Display → Reduce transparency**, reopen each surface, and toggle it back. Expected: WKWebView honors the system preference with opaque tokenized surfaces; text remains readable and no glass blur remains. This is the native completion of D-015.
- [ ] Toggle Reduce Motion and exercise keyboard navigation in onboarding, Settings, popover, and widget. Expected: entrance/pulse animation is removed, focus order is logical and visible, every threshold remains understandable without color, and no control is clipped.
- [ ] Exercise the tray and native window lifecycle: left-click toggles/positions the popover, right-click exposes the native menu, blur hides the popover, closing the main window hides rather than quits, each widget variant drags/snaps inside its fixed bounds across available monitors, Dock visibility changes activation policy, and launch-at-login converges only when changed.
- [ ] Confirm notification permission is requested only after the onboarding **Allow Notifications** action. Deny once, restore it in System Settings, and cross configured thresholds. Expected: no startup prompt, native alerts include text as well as state, and Focus/permission denial does not crash the app.

## 5. Optional T8.4 Developer ID distribution

Do this only if the owner wants a distributable build and supplies/manages their own Apple Developer credentials. Never commit the identity, certificate, private key, app-specific password, API key, or notarization profile.

- [ ] Install a **Developer ID Application** certificate and confirm its exact identity with:

  ```bash
  security find-identity -v -p codesigning
  ```

- [ ] Set `APPLE_SIGNING_IDENTITY` to that exact identity. For Tauri notarization, choose one owner-managed credential method:

  - App Store Connect API: `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH`; or
  - Apple ID: `APPLE_ID`, an app-specific `APPLE_PASSWORD`, and `APPLE_TEAM_ID`.

  Current variable names and configuration are documented in [Tauri 2 macOS code signing](https://v2.tauri.app/distribute/sign/macos/). Prefer a Keychain/notary profile for interactive manual submission so a password is not left in shell history.

- [ ] Rebuild the universal DMG with the owner-controlled environment:

  ```bash
  cargo tauri build --target universal-apple-darwin
  ```

  If submitting manually instead, use `xcrun notarytool submit <dmg> --keychain-profile <profile> --wait`, then `xcrun stapler staple <dmg>` as described by [Apple's notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).

- [ ] Verify the final app and DMG:

  ```bash
  codesign --verify --deep --strict --verbose=2 \
    "target/universal-apple-darwin/release/bundle/macos/How Is It.app"
  spctl --assess --type execute --verbose=2 \
    "target/universal-apple-darwin/release/bundle/macos/How Is It.app"
  xcrun stapler validate \
    "target/universal-apple-darwin/release/bundle/dmg/How Is It_1.0.0_universal.dmg"
  ```

  **Expected:** Developer ID signature validation passes, the notarization submission is `Accepted`, the ticket staples/validates, and Gatekeeper identifies the app as notarized Developer ID software on another Mac.
