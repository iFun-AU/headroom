/** @file AccountsPane.tsx @description Codex discovery and Claude bridge controls. */
import { useEffect, useState } from "react";
import type { BridgeStatus } from "../../bindings/BridgeStatus";
import type { CodexDetection } from "../../bindings/CodexDetection";
import type { Settings } from "../../bindings/Settings";
import { ServiceBadge, Icon } from "../../components/Icon";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { StatusChip } from "../../components/StatusBadge";
import { useBridgeStatus } from "../../hooks/useBridgeStatus";
import { detectCodex, isTauriRuntime, pickPath } from "../../ipc";

function effectiveness(status: BridgeStatus | null): { readonly tone: "ok" | "warning" | "neutral"; readonly text: string } {
  if (status === null || !status.installed) return { tone: "neutral", text: "Disabled" };
  if (status.effective === "confirmed") return { tone: "ok", text: "Confirmed" };
  if (status.effective === "likelyOverridden") return { tone: "warning", text: "Likely overridden" };
  if (status.effective === "headlessOnly") return { tone: "warning", text: "Terminal only" };
  return { tone: "neutral", text: "Unverified" };
}

export function AccountsPane({ settings, disabled, save, fixtureBridge, fixtureDetection }: { readonly settings: Settings; readonly disabled: boolean; readonly save: (settings: Settings) => void; readonly fixtureBridge?: BridgeStatus | undefined; readonly fixtureDetection?: CodexDetection | undefined }) {
  const bridge = useBridgeStatus(fixtureBridge);
  const [detection, setDetection] = useState<CodexDetection | null>(fixtureDetection ?? null);
  const [detecting, setDetecting] = useState(false);
  const runDetection = () => {
    if (!isTauriRuntime()) return;
    setDetecting(true);
    void detectCodex().then(setDetection).catch(() => { setDetection({ path: null, version: null }); }).finally(() => { setDetecting(false); });
  };
  useEffect(() => { if (fixtureDetection === undefined) runDetection(); }, [fixtureDetection]);
  const browse = () => {
    if (!isTauriRuntime()) return;
    void pickPath("File", "CodexBinary").then((path) => { if (path !== null) save({ ...settings, codexPath: path }); }).catch(() => undefined);
  };
  const codexPath = settings.codexPath ?? detection?.path;
  const bridgeLabel = effectiveness(bridge.status);
  return (
    <>
      <SettingsGroup heading="Codex" foot={<>Limits are read through <span className="mono">codex app-server</span>. Your Codex sign-in stays with Codex.</>}>
        <SettingsRow label="Codex CLI" leading={<ServiceBadge provider="codex" size="compact" />}><StatusChip tone={detection?.path !== null && detection !== null ? "ok" : "neutral"}>{detection?.path === null ? "Not found" : detection === null ? "Detecting…" : `Found${detection.version === null ? "" : ` · ${detection.version}`}`}</StatusChip><button className="capsule-button ctl" disabled={disabled || detecting} onClick={runDetection}>Detect</button></SettingsRow>
        <SettingsRow label="Path"><span className="settings-value mono">{codexPath ?? "Automatic discovery"}</span><button className="capsule-button ctl" disabled={disabled} onClick={browse}>Browse…</button></SettingsRow>
      </SettingsGroup>
      <SettingsGroup heading="Claude" foot={<>Adds a status line command to <span className="mono">~/.claude/settings.json</span>. Claude Code hides most footer keyboard hints while a custom status line is set. A project or organization setting can override it.</>}>
        <SettingsRow label="Real-time updates" description="Status line bridge" leading={<ServiceBadge provider="claude" size="compact" />}><StatusChip tone={bridgeLabel.tone}>{bridgeLabel.text}</StatusChip><button className="capsule-button ctl" disabled={disabled || bridge.busy} onClick={() => { void bridge.setInstalled(!(bridge.status?.installed ?? false)); }}>{bridge.status?.installed === true ? "Disable" : "Enable"}</button></SettingsRow>
        <SettingsRow label="Previous status line" description="Still runs after ours"><span className="settings-value mono">{bridge.status?.chained === true ? "Preserved and chained" : "None detected"}</span></SettingsRow>
        {bridge.status?.effective === "likelyOverridden" ? <div className="settings-warning"><Icon name="warning" size={15} /><span><strong>No updates received from Claude Code.</strong> A project or organization setting may override your status line, or your plan doesn’t report limits.</span></div> : null}
        {bridge.status?.effective === "headlessOnly" ? <div className="settings-warning"><Icon name="warning" size={15} /><span><strong>Limits update only from Claude Code in a terminal.</strong> The Claude desktop app, IDE extensions, and SDK don’t run status lines. Use <span className="mono">claude</span> in a terminal to refresh limits.</span></div> : null}
        {bridge.error === null ? null : <div className="settings-error" role="status">{bridge.error}</div>}
      </SettingsGroup>
    </>
  );
}
