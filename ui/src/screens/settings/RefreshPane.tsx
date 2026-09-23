/** @file RefreshPane.tsx @description Active/idle safety-net polling controls. */
import type { Settings } from "../../bindings/Settings";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { isTauriRuntime, refreshNow } from "../../ipc";

const ACTIVE = [60, 120, 300, 600] as const;
const IDLE = [120, 300, 600, 1_800] as const;
const intervalLabel = (seconds: number) => seconds < 60 ? `${String(seconds)} seconds` : `${String(seconds / 60)} ${seconds === 60 ? "minute" : "minutes"}`;

export function RefreshPane({ settings, disabled, save }: { readonly settings: Settings; readonly disabled: boolean; readonly save: (settings: Settings) => void }) {
  const select = (value: number, active: boolean) => { save(active ? { ...settings, pollActiveSecs: value } : { ...settings, pollIdleSecs: value }); };
  return (
    <>
      <SettingsGroup heading="Codex polling" foot="Updates are pushed in real time whenever possible; polling is the safety net (minimum 1 and 2 minutes). Claude is not polled in this version: it updates when Claude Code reports.">
        <SettingsRow label="When active" description="Any Claude or Codex activity in the last 10 minutes"><select className="settings-select ctl" aria-label="Active polling interval" value={settings.pollActiveSecs} disabled={disabled} onChange={(event) => { select(Number(event.currentTarget.value), true); }}>{ACTIVE.map((value) => <option value={value} key={value}>Every {intervalLabel(value)}</option>)}</select></SettingsRow>
        <SettingsRow label="When idle"><select className="settings-select ctl" aria-label="Idle polling interval" value={settings.pollIdleSecs} disabled={disabled} onChange={(event) => { select(Number(event.currentTarget.value), false); }}>{IDLE.map((value) => <option value={value} key={value}>Every {intervalLabel(value)}</option>)}</select></SettingsRow>
      </SettingsGroup>
      <SettingsGroup><SettingsRow label="Refresh now" description="Reads every source immediately"><button className="capsule-button ctl" disabled={disabled} onClick={() => { if (isTauriRuntime()) void refreshNow().catch(() => undefined); }}>Refresh Now</button></SettingsRow></SettingsGroup>
    </>
  );
}
