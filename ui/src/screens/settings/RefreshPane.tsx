/** @file RefreshPane.tsx @description Active/idle safety-net polling controls. */
import type { Settings } from "../../bindings/Settings";
import { useState } from "react";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { commandErrorMessage, isTauriRuntime, refreshNow } from "../../ipc";

const ACTIVE = [60, 120, 300, 600] as const;
const IDLE = [120, 300, 600, 1_800] as const;
const intervalLabel = (seconds: number) => seconds < 60 ? `${String(seconds)} seconds` : `${String(seconds / 60)} ${seconds === 60 ? "minute" : "minutes"}`;

export function RefreshPane({ settings, disabled, save }: { readonly settings: Settings; readonly disabled: boolean; readonly save: (settings: Settings) => void }) {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const refresh = () => {
    if (!isTauriRuntime() || busy) return;
    setBusy(true);
    setMessage(null);
    void refreshNow().catch((error: unknown) => { setMessage(commandErrorMessage(error)); }).finally(() => { setBusy(false); });
  };
  const select = (value: number, active: boolean) => { save(active ? { ...settings, pollActiveSecs: value } : { ...settings, pollIdleSecs: value }); };
  return (
    <>
      <SettingsGroup heading="Polling intervals" foot="Codex and the optional Claude usage API use these intervals. Claude Code terminal checks run only when you click Refresh; the status line continues to update as you work.">
        <SettingsRow label="When active" description="Any Claude or Codex activity in the last 10 minutes"><select className="settings-select ctl" aria-label="Active polling interval" value={settings.pollActiveSecs} disabled={disabled} onChange={(event) => { select(Number(event.currentTarget.value), true); }}>{ACTIVE.map((value) => <option value={value} key={value}>Every {intervalLabel(value)}</option>)}</select></SettingsRow>
        <SettingsRow label="When idle"><select className="settings-select ctl" aria-label="Idle polling interval" value={settings.pollIdleSecs} disabled={disabled} onChange={(event) => { select(Number(event.currentTarget.value), false); }}>{IDLE.map((value) => <option value={value} key={value}>Every {intervalLabel(value)}</option>)}</select></SettingsRow>
      </SettingsGroup>
      <SettingsGroup foot="When the Claude usage API is off, Refresh briefly runs Claude Code in the background to check your limits. Retry cooldowns still apply."><SettingsRow label="Refresh now" description="Check current usage"><button className="capsule-button ctl" disabled={disabled || busy} onClick={refresh}>{busy ? "Refreshing…" : "Refresh Now"}</button></SettingsRow>{message === null ? null : <div className="settings-error" role="status">{message}</div>}</SettingsGroup>
    </>
  );
}
