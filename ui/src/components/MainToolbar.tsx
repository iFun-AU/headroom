/**
 * @file MainToolbar.tsx
 * @description Native-frame toolbar for dashboard routes and window actions.
 */
import { useState } from "react";
import type { Route } from "../bindings/Route";
import { collapseToWidget, isTauriRuntime, refreshNow } from "../ipc";
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
  const requestRefresh = () => {
    if (!isTauriRuntime()) return;
    setRefreshing(true);
    void refreshNow().catch(() => undefined).finally(() => { setRefreshing(false); });
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
        <span className="num"><UpdatedAgo timestamp={updatedAt} /></span>
        <span className="toolbar-button-group ctl">
          <button className={refreshing ? "is-refreshing" : undefined} aria-label="Refresh usage" onClick={requestRefresh} disabled={refreshing}>
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
