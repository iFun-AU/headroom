/**
 * @file Popover.tsx
 * @description Fixed-size menu-bar popover with compact live usage and typed actions.
 */
import { useState } from "react";
import type { LimitWindow } from "../bindings/LimitWindow";
import type { Provider } from "../bindings/Provider";
import type { ProviderUsage } from "../bindings/ProviderUsage";
import type { UsageSnapshot } from "../bindings/UsageSnapshot";
import { Countdown, UpdatedAgo } from "../components/Countdown";
import { Icon, ServiceBadge, type IconName } from "../components/Icon";
import { NeonBar } from "../components/NeonBar";
import { UsageValue } from "../components/StatusBadge";
import { isTauriRuntime, quitApp, refreshNow, showWindow } from "../ipc";

function findWindow(usage: ProviderUsage, kind: "session" | "weekly"): LimitWindow | undefined {
  return usage.windows.find((window) => window.kind.kind === kind);
}

function planLabel(provider: Provider, plan: string | null): string | null {
  if (plan === null) return null;
  if (provider === "claude") return plan.startsWith("Claude ") ? plan : `Claude ${plan}`;
  return plan.startsWith("ChatGPT ") ? plan : `ChatGPT ${plan}`;
}

function CompactWindow({ usage, kind }: { readonly usage: ProviderUsage; readonly kind: "session" | "weekly" }) {
  const window = findWindow(usage, kind);
  if (window === undefined) {
    return <div className="popover-limit popover-limit--empty"><span>{kind === "session" ? "Session" : "Weekly"}</span><small>Not reported</small></div>;
  }
  return (
    <div className="popover-limit">
      <div className="popover-limit__row">
        <span>{kind === "session" ? "Session" : "Weekly"}</span>
        <NeonBar
          percent={window.used}
          accent={usage.provider}
          label={kind === "session" ? "5-hour limit" : "weekly limit"}
          resetsAt={window.resetsAt}
          resetPending={window.resetPending}
          height={8}
          showValue={false}
          reflect={false}
          dimmed={usage.status.state === "stale"}
        />
        <UsageValue percent={window.used} provider={usage.provider} compact dimmed={usage.status.state === "stale"} />
      </div>
      <small><Countdown resetsAt={window.resetsAt} resetPending={window.resetPending} /></small>
    </div>
  );
}

function CompactProvider({ usage }: { readonly usage: ProviderUsage }) {
  return (
    <section className="popover-provider" data-provider={usage.provider}>
      <header>
        <ServiceBadge provider={usage.provider} size="compact" />
        <strong>{usage.provider === "claude" ? "Claude" : "Codex"}</strong>
        {planLabel(usage.provider, usage.plan) === null ? null : <small>{planLabel(usage.provider, usage.plan)}</small>}
      </header>
      <CompactWindow usage={usage} kind="session" />
      <CompactWindow usage={usage} kind="weekly" />
    </section>
  );
}

function MenuButton({ icon, children, onClick }: { readonly icon: IconName; readonly children: React.ReactNode; readonly onClick: () => void }) {
  return <button className="popover-menu__item" onClick={onClick}><Icon name={icon} size={15} /><span>{children}</span></button>;
}

export function Popover({ snapshot }: { readonly snapshot: UsageSnapshot | null }) {
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const run = (action: () => Promise<unknown>, message: string): void => {
    if (!isTauriRuntime()) return;
    setError(null);
    void action().catch(() => {
      setError(message);
    });
  };
  const refresh = (): void => {
    if (!isTauriRuntime()) return;
    setRefreshing(true);
    setError(null);
    void refreshNow()
      .catch(() => { setError("Couldn’t refresh usage."); })
      .finally(() => { setRefreshing(false); });
  };

  return (
    <main className="popover-root glass pop">
      <header className="popover-header">
        <span><strong>Headroom</strong><small role={error === null ? undefined : "alert"}>{error ?? (snapshot === null ? "Loading usage…" : <UpdatedAgo timestamp={snapshot.generatedAt} />)}</small></span>
        <button className="icon-button ctl" aria-label="Refresh usage" disabled={refreshing} onClick={refresh}>
          <Icon name="refresh" size={14} />
        </button>
      </header>
      {snapshot === null ? (
        <div className="popover-loading" aria-label="Loading provider usage"><span className="shim" /><span className="shim" /><span className="shim" /></div>
      ) : (
        <>
          <CompactProvider usage={snapshot.claude} />
          <div className="popover-separator" />
          <CompactProvider usage={snapshot.codex} />
        </>
      )}
      <div className="popover-separator" />
      <nav className="popover-menu" aria-label="Headroom menu">
        <MenuButton icon="window" onClick={() => { run(() => showWindow("Main", "Overview"), "Couldn’t open the dashboard."); }}>Open Dashboard</MenuButton>
        <MenuButton icon="pip" onClick={() => { run(() => showWindow("Widget"), "Couldn’t show the widget."); }}>Float Widget</MenuButton>
        <MenuButton icon="gear" onClick={() => { run(() => showWindow("Main", "Settings"), "Couldn’t open Settings."); }}>Settings…</MenuButton>
        <div className="popover-menu__separator" />
        <MenuButton icon="power" onClick={() => { run(quitApp, "Couldn’t quit Headroom."); }}>Quit Headroom</MenuButton>
      </nav>
    </main>
  );
}
