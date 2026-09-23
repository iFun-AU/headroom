/**
 * @file Settings.tsx
 * @description Five-pane settings route backed by the versioned Rust settings contract.
 */
import { useEffect, useState } from "react";
import type { BridgeStatus } from "../bindings/BridgeStatus";
import type { CodexDetection } from "../bindings/CodexDetection";
import type { Settings as SettingsValue } from "../bindings/Settings";
import type { SettingsState } from "../bindings/SettingsState";
import type { UsageSnapshot } from "../bindings/UsageSnapshot";
import { Icon, type IconName } from "../components/Icon";
import { RouteToolbar } from "../components/RouteToolbar";
import { useSettings } from "../hooks/useSettings";
import { AccountsPane } from "./settings/AccountsPane";
import { AlertsPane } from "./settings/AlertsPane";
import { DiagnosticsPane } from "./settings/DiagnosticsPane";
import { DisplayPane } from "./settings/DisplayPane";
import { RefreshPane } from "./settings/RefreshPane";

export type SettingsSection = "Accounts" | "Display" | "Alerts" | "Refresh" | "Diagnostics";

const SECTIONS: readonly { readonly name: SettingsSection; readonly icon: IconName }[] = [
  { name: "Accounts", icon: "search" },
  { name: "Display", icon: "menubar" },
  { name: "Alerts", icon: "bell" },
  { name: "Refresh", icon: "refresh" },
  { name: "Diagnostics", icon: "info" },
];

interface SettingsProps {
  readonly snapshot: UsageSnapshot | null;
  readonly navigateOverview: () => void;
  readonly fixtureState?: SettingsState;
  readonly fixtureBridge?: BridgeStatus;
  readonly fixtureDetection?: CodexDetection;
  readonly initialSection?: SettingsSection;
}

export function Settings({ snapshot, navigateOverview, fixtureState, fixtureBridge, fixtureDetection, initialSection = "Accounts" }: SettingsProps) {
  const [section, setSection] = useState<SettingsSection>(initialSection);
  useEffect(() => { setSection(initialSection); }, [initialSection]);
  const settingsState = useSettings(fixtureState);
  const current = settingsState.state;
  const save = (settings: SettingsValue) => { void settingsState.save(settings); };
  let pane = null;
  if (current !== null) {
    switch (section) {
      case "Accounts": pane = <AccountsPane settings={current.settings} disabled={current.readOnly || settingsState.saving} save={save} fixtureBridge={fixtureBridge} fixtureDetection={fixtureDetection} />; break;
      case "Display": pane = <DisplayPane settings={current.settings} disabled={current.readOnly || settingsState.saving} save={save} />; break;
      case "Alerts": pane = <AlertsPane settings={current.settings} disabled={current.readOnly || settingsState.saving} save={save} />; break;
      case "Refresh": pane = <RefreshPane settings={current.settings} disabled={current.readOnly || settingsState.saving} save={save} />; break;
      case "Diagnostics": pane = <DiagnosticsPane snapshot={snapshot} />; break;
    }
  }

  return (
    <main className="settings-root glass">
      <RouteToolbar title="Settings" backLabel="Overview" onBack={navigateOverview} />
      <div className="settings-layout">
        <nav className="settings-sidebar card" aria-label="Settings sections">
          {SECTIONS.map((item) => <button aria-current={section === item.name ? "page" : undefined} data-section={item.name.toLowerCase()} onClick={() => { setSection(item.name); }} key={item.name}><span><Icon name={item.icon} size={14} /></span>{item.name}</button>)}
        </nav>
        <section className="settings-pane">
          {current?.readOnly === true ? <div className="settings-readonly" role="alert"><Icon name="warning" size={16} /><strong>{current.notice ?? "Settings were created by a newer version of How Is It. Changes can’t be saved."}</strong><button className="primary-button" disabled={settingsState.saving} onClick={() => { void settingsState.reset(); }}>Reset Settings</button></div> : null}
          {settingsState.error === null ? null : <div className="settings-error" role="status">{settingsState.error}</div>}
          {settingsState.loading || current === null ? <div className="settings-loading"><span className="shim" /><span className="shim" /></div> : <fieldset disabled={current.readOnly || settingsState.saving}>{pane}</fieldset>}
        </section>
      </div>
    </main>
  );
}
