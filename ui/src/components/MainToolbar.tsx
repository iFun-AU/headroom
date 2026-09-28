/**
 * @file MainToolbar.tsx
 * @description Native-frame toolbar for dashboard routes and window actions.
 */
import { useState } from "react";
import type { Route } from "../bindings/Route";
import { collapseToWidget, commandErrorMessage, isTauriRuntime, refreshNow } from "../ipc";
import { useSettings } from "../hooks/useSettings";
import { UpdatedAgo } from "./Countdown";
import { Icon } from "./Icon";

const TABS: readonly Route[] = ["Overview", "Claude", "Codex"];

export function MainToolbar({
  route,
  updatedAt,
  navigate,
}: {
  readonly route: Route;
  readonly updatedAt: number | null;
  readonly navigate: (route: Route) => void;
}) {
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const settingsState = useSettings();
  const settings = settingsState.state?.settings;
  const pinned = settings?.mainAlwaysOnTop === true;
  const togglePin = () => {
    if (settings === undefined || settingsState.state?.readOnly === true) return;
    void settingsState.save({ ...settings, mainAlwaysOnTop: !pinned });
  };
  const requestRefresh = () => {
    if (!isTauriRuntime()) return;
    setRefreshing(true);
    setRefreshError(null);
    void refreshNow().catch((error: unknown) => { setRefreshError(commandErrorMessage(error)); }).finally(() => { setRefreshing(false); });
  };

  return (
    <header className="main-toolbar" data-tauri-drag-region>
      <span className="main-toolbar__traffic-space" aria-hidden="true" />
      <nav className="segmented-control ctl" aria-label="Dashboard view">
        {TABS.map((tab) => (
          <button
            className={route === tab ? "ctl-sel" : undefined}
            aria-current={route === tab ? "page" : undefined}
            onClick={() => { navigate(tab); }}
            key={tab}
          >
            {tab}
          </button>
        ))}
      </nav>
      <div className="main-toolbar__actions">
        <span className="num" role={refreshError === null ? undefined : "status"} title={refreshError ?? undefined} aria-label={refreshError ?? undefined}>{refreshError === null ? <UpdatedAgo timestamp={updatedAt} /> : "Refresh failed"}</span>
        <span className="toolbar-button-group ctl">
          <button aria-label="Keep dashboard on top" aria-pressed={pinned} title={pinned ? "Unpin from top" : "Keep on top"} onClick={togglePin} disabled={settings === undefined || settingsState.saving}>
            <Icon name="pin" size={15} />
          </button>
          <button className={refreshing ? "is-refreshing" : undefined} aria-label="Refresh usage" title={refreshError ?? "Refresh usage"} onClick={requestRefresh} disabled={refreshing}>
            <Icon name="refresh" size={15} />
          </button>
          <button aria-label="Collapse to floating widget" onClick={() => { if (isTauriRuntime()) void collapseToWidget().catch(() => undefined); }}>
            <Icon name="pip" size={15} />
          </button>
        </span>
      </div>
    </header>
  );
}
