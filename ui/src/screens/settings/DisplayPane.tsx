/** @file DisplayPane.tsx @description General and floating-widget display preferences. */
import { useEffect, useState } from "react";
import type { CreditsDisplay } from "../../bindings/CreditsDisplay";
import type { Settings } from "../../bindings/Settings";
import type { WidgetVariant } from "../../bindings/WidgetVariant";
import type { WidgetWindows } from "../../bindings/WidgetWindows";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { Switch } from "../../components/Switch";

const CREDITS_OPTIONS: readonly { readonly value: CreditsDisplay; readonly label: string }[] = [
  { value: "Off", label: "Off" },
  { value: "WithLimits", label: "With limits" },
  { value: "Only", label: "Instead of limits" },
];

/** Credits choice; a provider with no credits to show always keeps its limit. */
function CreditsSelect({ label, value, disabled, onChange }: { readonly label: string; readonly value: CreditsDisplay; readonly disabled: boolean; readonly onChange: (value: CreditsDisplay) => void }) {
  return <select className="settings-select ctl" aria-label={label} value={value} disabled={disabled} onChange={(event) => { onChange(CREDITS_OPTIONS.find((option) => option.value === event.currentTarget.value)?.value ?? "Off"); }}>{CREDITS_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select>;
}

const CREDITS_DESCRIPTION = "Claude extra usage or Codex credits, when reported";

export function DisplayPane({ settings, disabled, save }: { readonly settings: Settings; readonly disabled: boolean; readonly save: (settings: Settings) => void }) {
  const [opacity, setOpacity] = useState(Math.round(settings.widget.opacity * 100));
  useEffect(() => { setOpacity(Math.round(settings.widget.opacity * 100)); }, [settings.widget.opacity]);
  const commitOpacity = () => {
    const nextOpacity = opacity / 100;
    if (!disabled && settings.widget.opacity !== nextOpacity) save({ ...settings, widget: { ...settings.widget, opacity: nextOpacity } });
  };
  return (
    <>
      <SettingsGroup heading="General">
        <SettingsRow label="Launch at login"><Switch checked={settings.launchAtLogin} label="Launch at login" disabled={disabled} onChange={(launchAtLogin) => { save({ ...settings, launchAtLogin }); }} /></SettingsRow>
        <SettingsRow label="Show Dock icon" description="Off keeps Headroom in the menu bar only"><Switch checked={settings.showDockIcon} label="Show Dock icon" disabled={disabled} onChange={(showDockIcon) => { save({ ...settings, showDockIcon }); }} /></SettingsRow>
        <SettingsRow label="Menu bar" description="Limits as text or stacked bars (Claude on top)"><select className="settings-select ctl" aria-label="Menu bar style" value={settings.trayStyle} disabled={disabled} onChange={(event) => { save({ ...settings, trayStyle: event.currentTarget.value === "Bars" ? "Bars" : "Numbers" }); }}><option value="Numbers">Numbers</option><option value="Bars">Bars</option></select></SettingsRow>
        <SettingsRow label="Menu bar shows" description="A plan without this limit shows its other one, labeled"><select className="settings-select ctl" aria-label="Menu bar limit" value={settings.trayWindow} disabled={disabled} onChange={(event) => { save({ ...settings, trayWindow: event.currentTarget.value === "FiveHour" ? "FiveHour" : "Weekly" }); }}><option value="FiveHour">5-hour limit</option><option value="Weekly">Weekly limit</option></select></SettingsRow>
        <SettingsRow label="Menu bar credits" description={CREDITS_DESCRIPTION}><CreditsSelect label="Menu bar credits" value={settings.trayCredits} disabled={disabled} onChange={(trayCredits) => { save({ ...settings, trayCredits }); }} /></SettingsRow>
        <SettingsRow label="Keep dashboard on top" description="Also available from the pin button in the dashboard toolbar"><Switch checked={settings.mainAlwaysOnTop} label="Keep dashboard on top" disabled={disabled} onChange={(mainAlwaysOnTop) => { save({ ...settings, mainAlwaysOnTop }); }} /></SettingsRow>
      </SettingsGroup>
      <SettingsGroup heading="Floating widget" foot="The widget always stays on top. Drag it anywhere; it snaps to screen edges.">
        <SettingsRow label="Show floating widget"><Switch checked={settings.widget.visible} label="Show floating widget" disabled={disabled} onChange={(visible) => { save({ ...settings, widget: { ...settings.widget, visible } }); }} /></SettingsRow>
        <SettingsRow label="Style"><select className="settings-select ctl" aria-label="Widget style" value={settings.widget.variant} disabled={disabled} onChange={(event) => { save({ ...settings, widget: { ...settings.widget, variant: event.currentTarget.value as WidgetVariant } }); }}><option>Pill</option><option>Stack</option><option>Mini</option></select></SettingsRow>
        <SettingsRow label="Shows" description="Mini fits one limit and shows the 5-hour one when both are chosen"><select className="settings-select ctl" aria-label="Widget limits" value={settings.widget.windows} disabled={disabled} onChange={(event) => { save({ ...settings, widget: { ...settings.widget, windows: event.currentTarget.value as WidgetWindows } }); }}><option value="Both">5-hour and weekly</option><option value="FiveHour">5-hour limit</option><option value="Weekly">Weekly limit</option></select></SettingsRow>
        <SettingsRow label="Credits" description={CREDITS_DESCRIPTION}><CreditsSelect label="Widget credits" value={settings.widget.credits} disabled={disabled} onChange={(credits) => { save({ ...settings, widget: { ...settings.widget, credits } }); }} /></SettingsRow>
        <SettingsRow label="Opacity"><input className="range" type="range" min="40" max="100" value={opacity} disabled={disabled} aria-label="Widget opacity" onChange={(event) => { setOpacity(Number(event.currentTarget.value)); }} onPointerUp={commitOpacity} onBlur={commitOpacity} /><span className="settings-value num">{String(opacity)}%</span></SettingsRow>
      </SettingsGroup>
    </>
  );
}
