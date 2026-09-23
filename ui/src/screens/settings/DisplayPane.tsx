/** @file DisplayPane.tsx @description General and floating-widget display preferences. */
import { useEffect, useState } from "react";
import type { Settings } from "../../bindings/Settings";
import type { WidgetVariant } from "../../bindings/WidgetVariant";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { Switch } from "../../components/Switch";

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
        <SettingsRow label="Show Dock icon" description="Off keeps How Is It in the menu bar only"><Switch checked={settings.showDockIcon} label="Show Dock icon" disabled={disabled} onChange={(showDockIcon) => { save({ ...settings, showDockIcon }); }} /></SettingsRow>
      </SettingsGroup>
      <SettingsGroup heading="Floating widget" foot="The widget always stays on top. Drag it anywhere; it snaps to screen edges.">
        <SettingsRow label="Show floating widget"><Switch checked={settings.widget.visible} label="Show floating widget" disabled={disabled} onChange={(visible) => { save({ ...settings, widget: { ...settings.widget, visible } }); }} /></SettingsRow>
        <SettingsRow label="Style"><select className="settings-select ctl" aria-label="Widget style" value={settings.widget.variant} disabled={disabled} onChange={(event) => { save({ ...settings, widget: { ...settings.widget, variant: event.currentTarget.value as WidgetVariant } }); }}><option>Pill</option><option>Stack</option><option>Mini</option></select></SettingsRow>
        <SettingsRow label="Opacity"><input className="range" type="range" min="40" max="100" value={opacity} disabled={disabled} aria-label="Widget opacity" onChange={(event) => { setOpacity(Number(event.currentTarget.value)); }} onPointerUp={commitOpacity} onBlur={commitOpacity} /><span className="settings-value num">{String(opacity)}%</span></SettingsRow>
      </SettingsGroup>
    </>
  );
}
