/** @file AlertsPane.tsx @description Threshold and reset notification preferences. */
import type { Settings } from "../../bindings/Settings";
import { Icon } from "../../components/Icon";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { Switch } from "../../components/Switch";

const THRESHOLDS = [75, 90, 100] as const;

export function AlertsPane({ settings, disabled, save }: { readonly settings: Settings; readonly disabled: boolean; readonly save: (settings: Settings) => void }) {
  const setThreshold = (threshold: number, enabled: boolean) => {
    const thresholds = enabled ? [...settings.thresholds, threshold] : settings.thresholds.filter((value) => value !== threshold);
    save({ ...settings, thresholds: [...new Set(thresholds)].sort((left, right) => left - right) });
  };
  return (
    <>
      <SettingsGroup heading="Notify me when a limit reaches" foot="Each alert fires once per window. Notifications respect Focus.">
        {THRESHOLDS.map((threshold) => <SettingsRow key={threshold} label={threshold === 100 ? "Limit reached" : `${String(threshold)}% used`} description={threshold === 75 ? "Approaching the limit" : threshold === 90 ? "Almost out" : "100% of a session or weekly limit"} leading={<span className={threshold === 75 ? "settings-icon--warn" : "settings-icon--crit"}><Icon name={threshold === 75 ? "warning" : threshold === 90 ? "critical" : "hourglass"} size={18} /></span>}><Switch checked={settings.thresholds.includes(threshold)} label={`Notify at ${String(threshold)}%`} disabled={disabled} onChange={(enabled) => { setThreshold(threshold, enabled); }} /></SettingsRow>)}
      </SettingsGroup>
      <SettingsGroup heading="Resets">
        <SettingsRow label="When a limit resets" description="After it had reached 90% or more" leading={<Icon name="refresh" size={18} />}><Switch checked={settings.notifyOnReset} label="Notify when a limit resets" disabled={disabled} onChange={(notifyOnReset) => { save({ ...settings, notifyOnReset }); }} /></SettingsRow>
      </SettingsGroup>
    </>
  );
}
