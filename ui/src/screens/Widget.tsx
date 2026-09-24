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
import type { WidgetWindows } from "../bindings/WidgetWindows";
import { Icon } from "../components/Icon";
import { NeonBar } from "../components/NeonBar";
import { UsageValue } from "../components/StatusBadge";
import { useSettings } from "../hooks/useSettings";
import { expandFromWidget, hideWindow, isTauriRuntime } from "../ipc";

type WindowKey = "session" | "weekly";

/** One widget row: a limit window to draw, or `undefined` for a dash. */
interface WidgetRow {
  readonly key: WindowKey;
  readonly label: "5h" | "wk";
  readonly window: LimitWindow | undefined;
  /** The provider lacks the chosen window, so its other window stands in. */
  readonly substitute: boolean;
}

function usageWindow(usage: ProviderUsage, kind: WindowKey): LimitWindow | undefined {
  return usage.windows.find((window) => window.kind.kind === kind);
}

function row(key: WindowKey, window: LimitWindow | undefined, substitute = false): WidgetRow {
  return { key, label: key === "session" ? "5h" : "wk", window, substitute };
}

/**
 * The rows a provider shows for the chosen windows. A single choice the plan
 * lacks (e.g. 5-hour on a weekly-only Codex plan) falls back to the window the
 * plan has, flagged as a substitute. With both chosen, a plan without a 5-hour
 * limit drops that row rather than show an empty bar.
 */
function widgetRows(usage: ProviderUsage, windows: WidgetWindows): readonly WidgetRow[] {
  const session = usageWindow(usage, "session");
  const weekly = usageWindow(usage, "weekly");
  if (windows === "Both") return session !== undefined || weekly === undefined ? [row("session", session), row("weekly", weekly)] : [row("weekly", weekly)];
  const [chosen, other]: readonly [WindowKey, WindowKey] = windows === "FiveHour" ? ["session", "weekly"] : ["weekly", "session"];
  const chosenWindow = chosen === "session" ? session : weekly;
  const otherWindow = other === "session" ? session : weekly;
  if (chosenWindow === undefined && otherWindow !== undefined) return [row(other, otherWindow, true)];
  return [row(chosen, chosenWindow)];
}

/** Single-window rows and 5-hour rows get the tall bar and the prominent value. */
function isPrimary(entry: WidgetRow, windows: WidgetWindows): boolean {
  return windows !== "Both" || entry.key === "session";
}

function ThinBar({ provider, window, height }: { readonly provider: Provider; readonly window: LimitWindow | undefined; readonly height: 3 | 4 | 6 }) {
  if (window === undefined) return <span className="widget-empty-bar" data-height={height} />;
  return <NeonBar percent={window.used} accent={provider} label={window.kind.kind === "session" ? "5-hour limit" : "weekly limit"} resetsAt={window.resetsAt} resetPending={window.resetPending} height={height} showValue={false} reflect={false} />;
}

function WidgetLetter({ provider }: { readonly provider: Provider }) {
  return <span className="widget-letter" data-provider={provider}>{provider === "claude" ? "C" : "X"}</span>;
}

function PillProvider({ usage, windows }: { readonly usage: ProviderUsage; readonly windows: WidgetWindows }) {
  const rows = widgetRows(usage, windows);
  return (
    <div className="widget-pill__provider" data-provider={usage.provider}>
      <WidgetLetter provider={usage.provider} />
      <span className="widget-pill__bars">{rows.map((entry) => <ThinBar key={entry.key} provider={usage.provider} window={entry.window} height={isPrimary(entry, windows) ? 6 : 4} />)}</span>
      <span className="widget-pill__values">
        {rows.map((entry) => {
          if (!isPrimary(entry, windows)) return <small key={entry.key} className="num">{entry.window === undefined ? "—" : `${String(entry.window.used)}%`}</small>;
          return entry.window === undefined ? <small key={entry.key}>—</small> : <UsageValue key={entry.key} percent={entry.window.used} provider={usage.provider} compact />;
        })}
        {rows.map((entry) => entry.substitute && <small key={`${entry.key}-tag`} className="widget-window-tag">{entry.label}</small>)}
      </span>
    </div>
  );
}

function StackProvider({ usage, windows }: { readonly usage: ProviderUsage; readonly windows: WidgetWindows }) {
  return (
    <section className="widget-stack__provider" data-provider={usage.provider}>
      <header><WidgetLetter provider={usage.provider} /><strong>{usage.provider === "claude" ? "Claude" : "Codex"}</strong></header>
      {widgetRows(usage, windows).map((entry) => (
        <div key={entry.key} className="widget-stack__row"><small>{entry.label}</small><ThinBar provider={usage.provider} window={entry.window} height={isPrimary(entry, windows) ? 6 : 4} />{entry.window === undefined ? <span>—</span> : <UsageValue percent={entry.window.used} provider={usage.provider} compact />}</div>
      ))}
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

/** Mini fits one window per provider, so "both" shows the 5-hour limit. */
function MiniWidget({ snapshot, windows }: { readonly snapshot: UsageSnapshot; readonly windows: WidgetWindows }) {
  const single = windows === "Both" ? "FiveHour" : windows;
  const [claude, codex] = [snapshot.claude, snapshot.codex].map((usage) => widgetRows(usage, single)[0]);
  const value = (entry: WidgetRow | undefined) => {
    if (entry?.window === undefined) return "—";
    return `${entry.substitute ? `${entry.label} ` : ""}${String(entry.window.used)}%`;
  };
  return (
    <>
      <div className="widget-mini__bars"><ThinBar provider="claude" window={claude?.window} height={3} /><ThinBar provider="codex" window={codex?.window} height={3} /></div>
      <div className="widget-mini__values"><strong className="num" data-provider="claude">C {value(claude)}</strong><span>·</span><strong className="num" data-provider="codex">X {value(codex)}</strong></div>
    </>
  );
}

export function Widget({
  snapshot,
  fixtureSettings,
  variantOverride,
  windowsOverride,
  forceHover = false,
}: {
  readonly snapshot: UsageSnapshot | null;
  readonly fixtureSettings?: SettingsState;
  readonly variantOverride?: WidgetVariant;
  readonly windowsOverride?: WidgetWindows;
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
  const windows = windowsOverride ?? settings.widget.windows;
  return (
    <section className="widget-root glass" data-variant={variant} data-force-hover={forceHover} data-tauri-drag-region aria-label={`${variant} usage widget`} style={{ opacity: opacity / 100 }} onPointerLeave={releaseFocus}>
      {variant === "Mini" ? <MiniWidget snapshot={snapshot} windows={windows} /> : (
        <>
          <div className="widget-content">
            {variant === "Pill" ? <><PillProvider usage={snapshot.claude} windows={windows} /><span className="widget-divider" /><PillProvider usage={snapshot.codex} windows={windows} /></> : <><StackProvider usage={snapshot.claude} windows={windows} /><span className="widget-divider" /><StackProvider usage={snapshot.codex} windows={windows} /></>}
          </div>
          <WidgetControls opacity={opacity} setOpacity={setOpacity} commitOpacity={commitOpacity} />
        </>
      )}
    </section>
  );
}
