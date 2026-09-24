/**
 * @file mocks.ts
 * @description Development-only representative data for visual routes and screenshots.
 */
import type { BridgeStatus } from "./bindings/BridgeStatus";
import type { CodexDetection } from "./bindings/CodexDetection";
import type { ConnectionStatus } from "./bindings/ConnectionStatus";
import type { History } from "./bindings/History";
import type { LimitWindow } from "./bindings/LimitWindow";
import type { Provider } from "./bindings/Provider";
import type { ProviderUsage } from "./bindings/ProviderUsage";
import type { SettingsState } from "./bindings/SettingsState";
import type { SourceKind } from "./bindings/SourceKind";
import type { UsageSnapshot } from "./bindings/UsageSnapshot";

const NOW = Math.floor(Date.now() / 1_000);

function sourceFor(provider: Provider): SourceKind {
  return provider === "claude" ? "claudeStatusline" : "codexAppServer";
}

function limitWindow(provider: Provider, kind: "session" | "weekly", used: number, remainingSeconds: number): LimitWindow {
  return {
    kind: { kind },
    used,
    resetsAt: NOW + remainingSeconds,
    resetPending: false,
    source: sourceFor(provider),
    observedAt: NOW - 12,
  };
}

export function mockProviderUsage(
  provider: Provider,
  status: ConnectionStatus = { state: "connected" },
  session = provider === "claude" ? 62 : 48,
  weekly = provider === "claude" ? 41 : 78,
): ProviderUsage {
  const source = sourceFor(provider);
  return {
    provider,
    plan: provider === "claude" ? "Max" : "Pro",
    windows: [
      limitWindow(provider, "session", session, provider === "claude" ? 8_040 : 13_260),
      limitWindow(provider, "weekly", weekly, provider === "claude" ? 411_600 : 421_200),
    ],
    status,
    authoritativeSource: source,
    lastUpdated: NOW - (status.state === "stale" ? 14 * 60 : 12),
    sources: provider === "claude"
      ? [{ source, status, lastSuccess: NOW - 12 }, { source: "claudeLocalLogs", status: { state: "connected" }, lastSuccess: NOW - 40 }]
      : [{ source, status, lastSuccess: NOW - 12 }, { source: "codexRollout", status: { state: "connected" }, lastSuccess: NOW - 3 }],
  };
}

export const MOCK_SNAPSHOT: UsageSnapshot = {
  claude: mockProviderUsage("claude"),
  codex: mockProviderUsage("codex"),
  generatedAt: NOW - 12,
};

export const MOCK_SPARKLINES: Readonly<Record<Provider, readonly number[]>> = {
  claude: [0, 0, 0, 0, 0, 0, 0, 0, 2_100_000, 6_300_000, 9_800_000, 8_700_000, 14_200_000, 16_900_000, 10_400_000],
  codex: [1_200_000, 0, 0, 0, 0, 0, 0, 0, 4_600_000, 9_100_000, 13_400_000, 15_800_000, 4_900_000, 14_100_000, 19_600_000],
};

const CLAUDE_HOURLY = [3.1, 5.2, 7.8, 4.4, 1.2, 2.5, 3.9, 1.8, 0.4, 0, 0, 0, 0, 0, 0, 0, 0, 2.1, 6.3, 9.8, 8.7, 14.2, 16.9, 10.4];
const CODEX_HOURLY = [8.4, 11.2, 6.1, 2.3, 0, 3.9, 9.7, 7.5, 3.1, 1.2, 0, 0, 0, 0, 0, 0, 0, 4.6, 9.1, 13.4, 15.8, 4.9, 14.1, 19.6];
const CLAUDE_DAILY = [48.2, 61.5, 12.3, 0, 35.6, 88.1, 71.2];
const CODEX_DAILY = [162.4, 148.9, 71.3, 39.5, 128.7, 284.1, 176.8];

function mockHistory(provider: Provider): History {
  const source = provider === "claude" ? "claudeLocalLogs" : "codexRollout";
  const hourly = provider === "claude" ? CLAUDE_HOURLY : CODEX_HOURLY;
  const daily = provider === "claude" ? CLAUDE_DAILY : CODEX_DAILY;
  const hourStart = Math.floor(NOW / 3_600) * 3_600;
  const localMidnight = new Date(NOW * 1_000);
  localMidnight.setHours(0, 0, 0, 0);
  const dailyEnd = localMidnight.getTime() / 1_000 - (provider === "codex" ? 86_400 : 0);
  return {
    provider,
    hourly: {
      source,
      scope: "thisMac",
      scopeLabel: provider === "claude" ? "Claude Code on this Mac" : "Codex CLI on this Mac",
      observedAt: NOW - 12,
      buckets: hourly.map((tokens, index) => ({ start: hourStart - (23 - index) * 3_600, tokens: tokens * 1_000_000 })),
    },
    daily: {
      source,
      scope: provider === "claude" ? "thisMac" : "account",
      scopeLabel: provider === "claude" ? "Claude Code on this Mac" : "All Codex usage (account)",
      observedAt: NOW - (provider === "claude" ? 12 : 15 * 60),
      buckets: daily.map((tokens, index) => ({ start: dailyEnd - (6 - index) * 86_400, tokens: tokens * 1_000_000 })),
    },
    projection: provider === "claude"
      ? { projectedPercentAtReset: 96, hitsLimitAt: null }
      : { projectedPercentAtReset: 130, hitsLimitAt: NOW + 38 * 3_600 },
  };
}

export const MOCK_HISTORIES: Readonly<Record<Provider, History>> = {
  claude: mockHistory("claude"),
  codex: mockHistory("codex"),
};

export const MOCK_SETTINGS_STATE: SettingsState = {
  settings: {
    schemaVersion: 1,
    onboardingCompleted: true,
    codexPath: "/opt/homebrew/bin/codex",
    codexHome: null,
    claudeDir: null,
    claudeBridgeEnabled: true,
    claudeOauthEnabled: false,
    thresholds: [75, 90, 100],
    notifyOnReset: true,
    launchAtLogin: true,
    showDockIcon: false,
    mainAlwaysOnTop: false,
    trayStyle: "Numbers",
    widget: { visible: true, x: null, y: null, variant: "Pill", opacity: 0.75 },
    pollActiveSecs: 120,
    pollIdleSecs: 600,
  },
  readOnly: false,
  notice: null,
  claudeOauthAvailable: true,
};

export const MOCK_READ_ONLY_SETTINGS: SettingsState = {
  ...MOCK_SETTINGS_STATE,
  readOnly: true,
  notice: "Settings were created by a newer version of How Is It. Changes can’t be saved.",
};

export const MOCK_BRIDGE_STATUS: BridgeStatus = {
  installed: true,
  chained: true,
  effective: "confirmed",
  settingsPath: "~/.claude/settings.json",
};

export const MOCK_OVERRIDDEN_BRIDGE: BridgeStatus = {
  ...MOCK_BRIDGE_STATUS,
  effective: "likelyOverridden",
};

export const MOCK_HEADLESS_BRIDGE: BridgeStatus = {
  ...MOCK_BRIDGE_STATUS,
  effective: "headlessOnly",
};

export const MOCK_CODEX_DETECTION: CodexDetection = {
  path: "/opt/homebrew/bin/codex",
  version: "0.154.0",
};

function withWindows(usage: ProviderUsage, updates: Readonly<Partial<Record<"session" | "weekly", Partial<LimitWindow>>>>): ProviderUsage {
  return {
    ...usage,
    windows: usage.windows.map((window) => {
      if (window.kind.kind === "other") return window;
      return { ...window, ...updates[window.kind.kind] };
    }),
  };
}

function withStatus(usage: ProviderUsage, status: ConnectionStatus): ProviderUsage {
  return { ...usage, status, lastUpdated: status.state === "stale" ? NOW - 14 * 60 : usage.lastUpdated };
}

const singleWindow = withWindows(mockProviderUsage("codex"), { weekly: { used: 3 } });

export const MOCK_CARD_STATES: readonly {
  readonly title: string;
  readonly provider: Provider;
  readonly usage: ProviderUsage | null;
  readonly notice?: boolean;
  readonly bridgeStatus?: BridgeStatus;
}[] = [
  { title: "Normal · 30–60%", provider: "claude", usage: mockProviderUsage("claude", { state: "connected" }, 46, 32) },
  { title: "Warning · ≥ 75%", provider: "codex", usage: mockProviderUsage("codex", { state: "connected" }, 81, 64), notice: true },
  { title: "Critical · ≥ 90%", provider: "claude", usage: mockProviderUsage("claude", { state: "connected" }, 95, 88), notice: true },
  { title: "Limit reached · 100%", provider: "codex", usage: withWindows(mockProviderUsage("codex"), { session: { used: 100 }, weekly: { used: 91 } }) },
  { title: "Loading", provider: "claude", usage: null },
  { title: "Not configured · Claude", provider: "claude", usage: { ...mockProviderUsage("claude"), windows: [], status: { state: "notConfigured", hint: "How Is It gets Claude limits from Claude Code’s status line. Enable it once and your bars update after every response." } } },
  { title: "Not configured · Codex", provider: "codex", usage: { ...mockProviderUsage("codex"), windows: [], status: { state: "notConfigured", hint: "Install Codex CLI or set its path in Settings." } } },
  { title: "Degraded · data still shown", provider: "codex", usage: withStatus(mockProviderUsage("codex"), { state: "degraded", reason: "Codex app-server stopped; using session files" }) },
  { title: "Stale · no poller in v1", provider: "claude", usage: withStatus(mockProviderUsage("claude"), { state: "stale" }) },
  { title: "Single-window plan", provider: "codex", usage: { ...singleWindow, windows: singleWindow.windows.filter((window) => window.kind.kind === "weekly") } },
  { title: "Reset pending", provider: "claude", usage: withWindows(mockProviderUsage("claude"), { session: { used: 0, resetsAt: NOW - 30, resetPending: true } }) },
  { title: "Bridge likely overridden", provider: "claude", usage: mockProviderUsage("claude"), bridgeStatus: MOCK_OVERRIDDEN_BRIDGE },
  { title: "Bridge terminal only", provider: "claude", usage: { ...mockProviderUsage("claude"), windows: [] }, bridgeStatus: MOCK_HEADLESS_BRIDGE },
  { title: "Unsupported", provider: "codex", usage: { ...mockProviderUsage("codex"), windows: [], status: { state: "unsupported", reason: "This Codex version doesn’t support reading rate limits. Update Codex CLI, then retry." } } },
  { title: "Auth expired", provider: "claude", usage: { ...mockProviderUsage("claude"), windows: [], status: { state: "authExpired", hint: "Open Claude Code to refresh sign-in." } } },
];
