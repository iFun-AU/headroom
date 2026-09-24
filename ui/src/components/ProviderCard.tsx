/**
 * @file ProviderCard.tsx
 * @description Provider usage card covering live, degraded, empty, stale, and limit states.
 */
import type { ReactNode } from "react";
import type { BridgeStatus } from "../bindings/BridgeStatus";
import type { LimitWindow } from "../bindings/LimitWindow";
import type { Provider } from "../bindings/Provider";
import type { ProviderUsage } from "../bindings/ProviderUsage";
import { creditSummary, formatAmount } from "../credits";
import { Countdown } from "./Countdown";
import { Icon, ServiceBadge, type IconName } from "./Icon";
import { NeonBar } from "./NeonBar";
import { Sparkline } from "./Sparkline";
import { ConnectionBadge, StatusChip, UsageValue, usageLevel } from "./StatusBadge";

export type ProviderCardAction = "configure" | "detect" | "details" | "pickPath" | "retry" | "settings";

export interface ProviderCardProps {
  readonly usage: ProviderUsage | null;
  readonly provider: Provider;
  readonly sparkline?: readonly number[];
  readonly peakLabel?: string | null;
  readonly onAction?: (action: ProviderCardAction) => void;
  readonly className?: string;
  readonly showThresholdNotice?: boolean;
  readonly bridgeStatus?: BridgeStatus | null;
}

function providerName(provider: Provider): string {
  return provider === "claude" ? "Claude" : "Codex";
}

function planLabel(provider: Provider, plan: string | null): string | null {
  if (plan === null) return null;
  if (provider === "claude") return plan.startsWith("Claude ") ? plan : `Claude ${plan}`;
  return plan.startsWith("ChatGPT ") ? plan : `ChatGPT ${plan}`;
}

function kind(window: LimitWindow): "session" | "weekly" | "other" {
  return window.kind.kind;
}

function limitLabel(window: LimitWindow): string {
  if (kind(window) === "session") return "5-hour limit";
  if (kind(window) === "weekly") return "weekly limit";
  return "provider limit";
}

function actionButton(label: string, primary: boolean, onClick: (() => void) | undefined) {
  return <button className={primary ? "primary-button" : "capsule-button ctl"} onClick={onClick}>{label}</button>;
}

function EmptyState({
  icon,
  title,
  body,
  primary,
  secondary,
}: {
  readonly icon: IconName;
  readonly title: string;
  readonly body: string;
  readonly primary?: { readonly label: string; readonly onClick: (() => void) | undefined };
  readonly secondary?: { readonly label: string; readonly onClick: (() => void) | undefined };
}) {
  return (
    <div className="provider-empty">
      <span className="provider-empty__icon"><Icon name={icon} size={28} /></span>
      <strong>{title}</strong>
      <p>{body}</p>
      {primary === undefined ? null : actionButton(primary.label, true, primary.onClick)}
      {secondary === undefined ? null : actionButton(secondary.label, false, secondary.onClick)}
    </div>
  );
}

function LoadingCard({ provider, className }: { readonly provider: Provider; readonly className: string | undefined }) {
  return (
    <section className={`provider-card card ${className ?? ""}`} aria-label={`Loading ${providerName(provider)} usage`}>
      <div className="provider-loading__header"><span className="shim" /><span className="shim" /><small>Loading usage…</small></div>
      {[0, 1].map((item) => <div className="provider-loading__block" key={item}><span className="shim" /><span className="shim" /><span className="shim" /></div>)}
      <div className="provider-loading__spark"><span className="shim" /><span className="shim" /></div>
    </section>
  );
}

function ProviderHeader({ usage, onDetails, statusOverride }: { readonly usage: ProviderUsage; readonly onDetails: (() => void) | undefined; readonly statusOverride?: ReactNode }) {
  const dimmed = usage.status.state === "notConfigured";
  const subtitle = usage.status.state === "notConfigured"
    ? usage.provider === "claude" ? "Not set up" : "Not found"
    : planLabel(usage.provider, usage.plan);
  return (
    <header className="provider-card__header">
      <ServiceBadge provider={usage.provider} dimmed={dimmed} />
      <span className="provider-card__identity">
        <strong>{providerName(usage.provider)}</strong>
        {subtitle === null ? null : <small>{subtitle}</small>}
      </span>
      {statusOverride ?? <ConnectionBadge status={usage.status} lastUpdated={usage.lastUpdated} />}
      {onDetails === undefined ? null : (
        <button className="icon-button ctl" aria-label={`Open ${providerName(usage.provider)} details`} onClick={onDetails}>
          <Icon name="chevron" size={14} />
        </button>
      )}
    </header>
  );
}

function UsageBlock({ window, provider, dimmed }: { readonly window: LimitWindow; readonly provider: Provider; readonly dimmed: boolean }) {
  const isSession = kind(window) === "session";
  return (
    <div className="usage-block">
      <div className="usage-block__heading">
        <span><strong>{isSession ? "Session" : kind(window) === "weekly" ? "Weekly" : "Other"}</strong><small>{isSession ? "5-hour window" : kind(window) === "weekly" ? "7-day window" : "Provider window"}</small></span>
        <UsageValue percent={window.used} provider={provider} dimmed={dimmed} />
      </div>
      <NeonBar
        percent={window.used}
        accent={provider}
        label={limitLabel(window)}
        resetsAt={window.resetsAt}
        resetPending={window.resetPending}
        showValue={false}
        dimmed={dimmed}
      />
      <span className="usage-block__reset"><Icon name={window.resetPending ? "refresh" : "clock"} size={13} /><Countdown resetsAt={window.resetsAt} resetPending={window.resetPending} /></span>
    </div>
  );
}

/** Claude extra usage (spent of its limit) or Codex credits (balance); the dashboard always shows them when reported (D-030). */
function CreditsBlock({ usage, dimmed }: { readonly usage: ProviderUsage; readonly dimmed: boolean }) {
  const credits = usage.credits;
  if (credits === null) return null;
  const extraUsage = credits.source === "claudeOAuth";
  const summary = creditSummary(providerName(usage.provider), credits);
  if (summary === null) return <div className="provider-muted"><Icon name="info" size={14} />{extraUsage ? "Extra usage is off" : "No Codex credits"}</div>;
  return (
    <div className="usage-block credits-block" title={summary.long}>
      <div className="usage-block__heading">
        <span><strong>{extraUsage ? "Extra usage" : "Credits"}</strong><small>{extraUsage ? "Spent beyond plan limits" : credits.unlimited ? "Unlimited" : "Remaining balance"}</small></span>
        <span className="credits-block__amount num" data-provider={usage.provider} data-dimmed={dimmed}>{summary.amount}{extraUsage && credits.limit !== null ? <small> of {formatAmount(credits.limit)}</small> : null}</span>
      </div>
      {summary.percent === null ? null : <NeonBar percent={summary.percent} accent={usage.provider} label={summary.long} resetsAt={null} resetPending={false} showValue={false} dimmed={dimmed} />}
    </div>
  );
}

type BridgeProblem = "likelyOverridden" | "headlessOnly";

/** Chip and explanation for each bridge state in which Claude limits stop updating. */
const BRIDGE_PROBLEM_COPY: Readonly<Record<BridgeProblem, { readonly chip: string; readonly message: string }>> = {
  likelyOverridden: {
    chip: "No updates",
    message: "No updates received from Claude Code. A project or organization setting may override your status line, or your plan doesn’t report limits.",
  },
  headlessOnly: {
    chip: "Terminal only",
    message: "Claude Code is running only in the Claude desktop app, an IDE, or the SDK, which don’t run status lines. Limits update while you use claude in a terminal.",
  },
};

function bridgeProblem(provider: Provider, status: BridgeStatus | null): BridgeProblem | null {
  if (provider !== "claude" || status === null) return null;
  return status.effective === "likelyOverridden" || status.effective === "headlessOnly" ? status.effective : null;
}

function BridgeNotice({ problem, usage, onAction }: { readonly problem: BridgeProblem; readonly usage: ProviderUsage; readonly onAction: ((action: ProviderCardAction) => void) | undefined }) {
  const session = usage.windows.find((window) => kind(window) === "session");
  const copy = BRIDGE_PROBLEM_COPY[problem];
  return (
    <>
      <ProviderHeader usage={usage} onDetails={undefined} statusOverride={<StatusChip tone="warning">{copy.chip}</StatusChip>} />
      <div className="bridge-warning" role="status">
        <Icon name="warning" size={15} />
        <span>{copy.message}</span>
      </div>
      {session === undefined ? null : <UsageBlock window={session} provider="claude" dimmed />}
      {actionButton("Open Settings", false, onAction === undefined ? undefined : () => { onAction("settings"); })}
    </>
  );
}

function StatusNotice({ window }: { readonly window: LimitWindow }) {
  const level = usageLevel(window.used);
  if (level === "normal" || level === "limit") return null;
  const critical = level === "critical";
  const windowName = kind(window) === "session" ? "session" : kind(window) === "weekly" ? "weekly" : "provider";
  return (
    <div className="provider-notice" data-level={level}>
      <Icon name={critical ? "critical" : "warning"} size={15} />
      <strong>{critical ? `Almost out: ${String(Math.max(0, 100 - window.used))}% of ${windowName} left` : `Approaching ${windowName} limit`}</strong>
    </div>
  );
}

function LimitBar({ window, provider, showReset }: { readonly window: LimitWindow; readonly provider: Provider; readonly showReset: boolean }) {
  const isSession = kind(window) === "session";
  return (
    <div className="limit-reached__bar">
      <span><strong>{isSession ? "Session" : "Weekly"}</strong><UsageValue percent={window.used} provider={provider} compact /></span>
      <NeonBar
        percent={window.used}
        accent={provider}
        label={isSession ? "5-hour limit" : "weekly limit"}
        resetsAt={window.resetsAt}
        resetPending={window.resetPending}
        height={isSession ? 14 : 8}
        showValue={false}
        reflect={isSession}
      />
      {showReset ? <small><Countdown resetsAt={window.resetsAt} resetPending={window.resetPending} /></small> : null}
    </div>
  );
}

function LimitReached({ session, weekly, provider }: { readonly session: LimitWindow; readonly weekly: LimitWindow | undefined; readonly provider: Provider }) {
  return (
    <div className="limit-reached">
      <span className="limit-reached__icon"><span className="limit-reached__halo" /><Icon name="hourglass" size={22} /></span>
      <strong>Session limit reached</strong>
      <small>Resets in</small>
      <span className="limit-reached__count num neon"><Countdown resetsAt={session.resetsAt} resetPending={session.resetPending} precise /></span>
      {session.resetsAt === null ? null : <small>Back at {new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" }).format(new Date(session.resetsAt * 1_000))}</small>}
      <div className="limit-reached__bars">
        <LimitBar window={session} provider={provider} showReset={false} />
        {weekly === undefined ? null : <LimitBar window={weekly} provider={provider} showReset />}
      </div>
    </div>
  );
}

function StaticState({ usage, onAction }: { readonly usage: ProviderUsage; readonly onAction: ((action: ProviderCardAction) => void) | undefined }) {
  const invoke = (action: ProviderCardAction) => onAction === undefined ? undefined : () => { onAction(action); };
  switch (usage.status.state) {
    case "notConfigured":
      return usage.provider === "claude"
        ? <EmptyState icon="bell" title="Turn on real-time updates" body={usage.status.hint} primary={{ label: "Enable Real-Time Updates", onClick: invoke("configure") }} secondary={{ label: "What changes?", onClick: invoke("settings") }} />
        : <EmptyState icon="search" title="Codex CLI not found" body={usage.status.hint} primary={{ label: "Detect Again", onClick: invoke("detect") }} secondary={{ label: "Choose Path…", onClick: invoke("pickPath") }} />;
    case "authExpired":
      return <EmptyState icon="lock" title="Sign-in expired" body={usage.status.hint} secondary={{ label: "Retry", onClick: invoke("retry") }} />;
    case "unsupported":
      return <EmptyState icon="info" title={`${providerName(usage.provider)} doesn’t report limits`} body={usage.status.reason} secondary={{ label: "Retry", onClick: invoke("retry") }} />;
    case "error":
      return <EmptyState icon="info" title={`Can’t read ${providerName(usage.provider)} usage`} body={usage.status.message} secondary={{ label: "Retry", onClick: invoke("retry") }} />;
    case "connected":
    case "stale":
    case "degraded":
      return null;
  }
}

export function ProviderCard({ usage, provider, sparkline = [], peakLabel, onAction, className, showThresholdNotice = false, bridgeStatus = null }: ProviderCardProps) {
  if (usage === null) return <LoadingCard provider={provider} className={className} />;
  const problem = bridgeProblem(provider, bridgeStatus);
  const staticState = <StaticState usage={usage} onAction={onAction} />;
  const disconnected = usage.status.state === "notConfigured" || usage.status.state === "authExpired" || usage.status.state === "unsupported" || usage.status.state === "error";
  const session = usage.windows.find((window) => kind(window) === "session");
  const weekly = usage.windows.find((window) => kind(window) === "weekly");
  const highestWindow = usage.windows.reduce<LimitWindow | undefined>((highest, window) => highest === undefined || window.used > highest.used ? window : highest, undefined);
  const stale = usage.status.state === "stale";
  const details = onAction === undefined ? undefined : () => { onAction("details"); };

  return (
    <section className={`provider-card card ${className ?? ""}`} data-status={problem ?? usage.status.state}>
      {problem === null ? null : <BridgeNotice problem={problem} usage={usage} onAction={onAction} />}
      {problem !== null ? null : (
        <>
      <ProviderHeader usage={usage} onDetails={disconnected ? undefined : details} />
      {disconnected ? staticState : null}
      {!disconnected && usage.status.state === "degraded" ? <div className="provider-muted"><Icon name="info" size={14} />{usage.status.reason}</div> : null}
      {!disconnected && session === undefined ? <div className="provider-muted"><Icon name="info" size={14} />No 5-hour limit on this plan</div> : null}
      {!disconnected && session?.used === 100 ? <><LimitReached session={session} weekly={weekly} provider={provider} /><CreditsBlock usage={usage} dimmed={stale} /></> : null}
      {!disconnected && session?.used !== 100 ? (
        <>
          {showThresholdNotice && highestWindow !== undefined ? <StatusNotice window={highestWindow} /> : null}
          {usage.windows.map((window) => <UsageBlock key={window.kind.kind === "other" ? `other-${String(window.kind.minutes)}` : window.kind.kind} window={window} provider={provider} dimmed={stale} />)}
          <CreditsBlock usage={usage} dimmed={stale} />
          {sparkline.length === 0 ? null : (
            <div className="provider-spark">
              <span><span><b className="cap">Today</b> · tokens on this Mac</span>{peakLabel === null || peakLabel === undefined ? null : <span>Peak {peakLabel}</span>}</span>
              <Sparkline values={sparkline} provider={provider} dimmed={stale} />
            </div>
          )}
        </>
      ) : null}
        </>
      )}
    </section>
  );
}
