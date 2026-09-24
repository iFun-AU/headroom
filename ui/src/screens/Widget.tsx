/**
 * @file Widget.tsx
 * @description The three exact-size floating widget variants and in-bounds hover controls.
 */
import { useEffect, useState, type PointerEvent } from "react";
import type { LimitWindow } from "../bindings/LimitWindow";
import type { Provider } from "../bindings/Provider";
import type { ProviderUsage } from "../bindings/ProviderUsage";
import type { SettingsState } from "../bindings/SettingsState";
import type { UsageSnapshot } from "../bindings/UsageSnapshot";
import type { WidgetVariant } from "../bindings/WidgetVariant";
import { Icon } from "../components/Icon";
import { NeonBar } from "../components/NeonBar";
import { UsageValue } from "../components/StatusBadge";
import { useSettings } from "../hooks/useSettings";
import { expandFromWidget, hideWindow, isTauriRuntime } from "../ipc";

function usageWindow(usage: ProviderUsage, kind: "session" | "weekly"): LimitWindow | undefined {
  return usage.windows.find((window) => window.kind.kind === kind);
}

function ThinBar({ provider, window, height }: { readonly provider: Provider; readonly window: LimitWindow | undefined; readonly height: 3 | 4 | 6 }) {
  if (window === undefined) return <span className="widget-empty-bar" data-height={height} />;
  return <NeonBar percent={window.used} accent={provider} label={window.kind.kind === "session" ? "5-hour limit" : "weekly limit"} resetsAt={window.resetsAt} resetPending={window.resetPending} height={height} showValue={false} reflect={false} />;
}

/** Some plans (e.g. Codex) have no 5-hour limit; drop that row rather than show an empty bar. */
function showsSession(session: LimitWindow | undefined, weekly: LimitWindow | undefined): boolean {
  return session !== undefined || weekly === undefined;
}

function WidgetLetter({ provider }: { readonly provider: Provider }) {
  return <span className="widget-letter" data-provider={provider}>{provider === "claude" ? "C" : "X"}</span>;
}

function PillProvider({ usage }: { readonly usage: ProviderUsage }) {
  const session = usageWindow(usage, "session");
  const weekly = usageWindow(usage, "weekly");
  const withSession = showsSession(session, weekly);
  return (
    <div className="widget-pill__provider" data-provider={usage.provider}>
      <WidgetLetter provider={usage.provider} />
      <span className="widget-pill__bars">{withSession && <ThinBar provider={usage.provider} window={session} height={6} />}<ThinBar provider={usage.provider} window={weekly} height={4} /></span>
      <span className="widget-pill__values">
        {!withSession ? null : session === undefined ? <small>—</small> : <UsageValue percent={session.used} provider={usage.provider} compact />}
        <small className="num">{weekly === undefined ? "—" : `${String(weekly.used)}%`}</small>
      </span>
    </div>
  );
}

function StackProvider({ usage }: { readonly usage: ProviderUsage }) {
  const session = usageWindow(usage, "session");
  const weekly = usageWindow(usage, "weekly");
  const row = (label: string, window: LimitWindow | undefined, height: 4 | 6) => (
    <div className="widget-stack__row"><small>{label}</small><ThinBar provider={usage.provider} window={window} height={height} />{window === undefined ? <span>—</span> : <UsageValue percent={window.used} provider={usage.provider} compact />}</div>
  );
  return (
    <section className="widget-stack__provider" data-provider={usage.provider}>
      <header><WidgetLetter provider={usage.provider} /><strong>{usage.provider === "claude" ? "Claude" : "Codex"}</strong></header>
      {showsSession(session, weekly) && row("5h", session, 6)}
      {row("wk", weekly, 4)}
    </section>
  );
}

function WidgetControls({ opacity, setOpacity, commitOpacity }: { readonly opacity: number; readonly setOpacity: (value: number) => void; readonly commitOpacity: () => void }) {
  return (
    <div className="widget-controls">
      <button className="icon-button ctl" aria-label="Close widget" onClick={() => { if (isTauriRuntime()) void hideWindow("Widget").catch(() => undefined); }}><Icon name="xmark" size={13} /></button>
      <span><Icon name="opacity" size={14} /><input type="range" min="40" max="100" value={opacity} aria-label="Widget opacity" onChange={(event) => { setOpacity(Number(event.currentTarget.value)); }} onPointerUp={commitOpacity} onBlur={commitOpacity} /><small className="num">{String(opacity)}%</small></span>
      <button className="icon-button ctl" aria-label="Expand to dashboard" onClick={() => { if (isTauriRuntime()) void expandFromWidget().catch(() => undefined); }}><Icon name="expand" size={13} /></button>
    </div>
  );
}

function MiniWidget({ snapshot }: { readonly snapshot: UsageSnapshot }) {
  const claude = usageWindow(snapshot.claude, "session");
  const codex = usageWindow(snapshot.codex, "session");
  return (
    <>
      <div className="widget-mini__bars"><ThinBar provider="claude" window={claude} height={3} /><ThinBar provider="codex" window={codex} height={3} /></div>
      <div className="widget-mini__values"><strong className="num" data-provider="claude">C {claude === undefined ? "—" : `${String(claude.used)}%`}</strong><span>·</span><strong className="num" data-provider="codex">X {codex === undefined ? "—" : `${String(codex.used)}%`}</strong></div>
    </>
  );
}

export function Widget({
  snapshot,
  fixtureSettings,
  variantOverride,
  forceHover = false,
}: {
  readonly snapshot: UsageSnapshot | null;
  readonly fixtureSettings?: SettingsState;
  readonly variantOverride?: WidgetVariant;
  readonly forceHover?: boolean;
}) {
  const settingsState = useSettings(fixtureSettings);
  const settings = settingsState.state?.settings;
  const [opacity, setOpacity] = useState(Math.round((settings?.widget.opacity ?? 1) * 100));
  useEffect(() => { setOpacity(Math.round((settings?.widget.opacity ?? 1) * 100)); }, [settings?.widget.opacity]);
  const commitOpacity = () => {
    if (settings === undefined || settingsState.state?.readOnly === true) return;
    const nextOpacity = opacity / 100;
    if (settings.widget.opacity === nextOpacity) return;
    void settingsState.save({ ...settings, widget: { ...settings.widget, opacity: nextOpacity } });
  };

  // A dragged slider keeps focus, and `:focus-within` would then pin the hover
  // controls after the pointer leaves. Blurring also commits via `onBlur`.
  const releaseFocus = (event: PointerEvent<HTMLElement>) => {
    const active = document.activeElement;
    if (active instanceof HTMLElement && event.currentTarget.contains(active)) active.blur();
  };

  if (snapshot === null || settings === undefined) return <section className="widget-root glass" data-variant={variantOverride ?? "Loading"} aria-label="Usage widget loading"><span className="shim" /></section>;
  const variant = variantOverride ?? settings.widget.variant;
  return (
    <section className="widget-root glass" data-variant={variant} data-force-hover={forceHover} data-tauri-drag-region aria-label={`${variant} usage widget`} style={{ opacity: opacity / 100 }} onPointerLeave={releaseFocus}>
      {variant === "Mini" ? <MiniWidget snapshot={snapshot} /> : (
        <>
          <div className="widget-content">
            {variant === "Pill" ? <><PillProvider usage={snapshot.claude} /><span className="widget-divider" /><PillProvider usage={snapshot.codex} /></> : <><StackProvider usage={snapshot.claude} /><span className="widget-divider" /><StackProvider usage={snapshot.codex} /></>}
          </div>
          <WidgetControls opacity={opacity} setOpacity={setOpacity} commitOpacity={commitOpacity} />
        </>
      )}
    </section>
  );
}
