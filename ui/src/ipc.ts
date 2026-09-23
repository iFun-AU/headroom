/**
 * @file ipc.ts
 * @description Typed wrappers around Tauri invoke/listen; the only module allowed to call Tauri APIs.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { BridgeStatus } from "./bindings/BridgeStatus";
import type { CodexDetection } from "./bindings/CodexDetection";
import type { History } from "./bindings/History";
import type { PathKind } from "./bindings/PathKind";
import type { PathPurpose } from "./bindings/PathPurpose";
import type { Provider } from "./bindings/Provider";
import type { Route } from "./bindings/Route";
import type { Settings } from "./bindings/Settings";
import type { SettingsState } from "./bindings/SettingsState";
import type { UsageSnapshot } from "./bindings/UsageSnapshot";
import type { WindowTarget } from "./bindings/WindowTarget";

export const getSnapshot = (): Promise<UsageSnapshot> =>
  invoke<UsageSnapshot>("get_snapshot");

export const getHistory = (provider: Provider): Promise<History> =>
  invoke<History>("get_history", { provider });

export const refreshNow = (): Promise<null> => invoke<null>("refresh_now");

export const getSettings = (): Promise<SettingsState> =>
  invoke<SettingsState>("get_settings");

export const setSettings = (settings: Settings): Promise<SettingsState> =>
  invoke<SettingsState>("set_settings", { settings });

export const resetSettings = (): Promise<SettingsState> =>
  invoke<SettingsState>("reset_settings");

export const completeOnboarding = (): Promise<SettingsState> =>
  invoke<SettingsState>("complete_onboarding");

export const getClaudeBridgeStatus = (): Promise<BridgeStatus> =>
  invoke<BridgeStatus>("claude_bridge_status");

export const installClaudeBridge = (): Promise<BridgeStatus> =>
  invoke<BridgeStatus>("install_claude_bridge");

export const uninstallClaudeBridge = (): Promise<BridgeStatus> =>
  invoke<BridgeStatus>("uninstall_claude_bridge");

export const detectCodex = (): Promise<CodexDetection> =>
  invoke<CodexDetection>("detect_codex");

export const pickPath = (
  kind: PathKind,
  purpose: PathPurpose,
): Promise<string | null> =>
  invoke<string | null>("pick_path", { kind, purpose });

export const showWindow = (
  which: WindowTarget,
  route: Route | null = null,
): Promise<null> => invoke<null>("show_window", { which, route });

export const hideWindow = (which: WindowTarget): Promise<null> =>
  invoke<null>("hide_window", { which });

export const revealLogs = (): Promise<null> => invoke<null>("reveal_logs");

export const quitApp = (): Promise<null> => invoke<null>("quit_app");

export const uiVisible = (label: string): Promise<null> =>
  invoke<null>("ui_visible", { label });

export const onUsageUpdated = (
  callback: (snapshot: UsageSnapshot) => void,
): Promise<UnlistenFn> =>
  listen<UsageSnapshot>("usage-updated", (event) => {
    callback(event.payload);
  });

export const onSettingsChanged = (
  callback: (state: SettingsState) => void,
): Promise<UnlistenFn> =>
  listen<SettingsState>("settings-changed", (event) => {
    callback(event.payload);
  });

export const onBridgeStatusChanged = (
  callback: (status: BridgeStatus) => void,
): Promise<UnlistenFn> =>
  listen<BridgeStatus>("bridge-status-changed", (event) => {
    callback(event.payload);
  });

export const onNavigate = (
  callback: (route: Route) => void,
): Promise<UnlistenFn> =>
  listen<Route>("navigate", (event) => {
    callback(event.payload);
  });
