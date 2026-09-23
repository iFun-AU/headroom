/**
 * @file Dashboard.tsx
 * @description Overview and provider detail routes backed by live usage and scoped history.
 */
import type { History } from "../bindings/History";
import type { BridgeStatus } from "../bindings/BridgeStatus";
import type { LimitWindow } from "../bindings/LimitWindow";
import type { Provider } from "../bindings/Provider";
import type { ProviderUsage } from "../bindings/ProviderUsage";
import type { Route } from "../bindings/Route";
import type { UsageSnapshot } from "../bindings/UsageSnapshot";
import { ChartCard } from "../components/ChartCard";
import { Countdown } from "../components/Countdown";
import { DailyChart } from "../components/DailyChart";
import { formatTokenCount, HourlyChart } from "../components/HourlyChart";
import { MainToolbar } from "../components/MainToolbar";
import { NeonBar } from "../components/NeonBar";
import { ProviderCard, type ProviderCardAction } from "../components/ProviderCard";
import { ServiceBadge } from "../components/Icon";
import { StatTile } from "../components/StatTile";
import { UsageValue } from "../components/StatusBadge";
import { useHistory } from "../hooks/useHistory";
import { useBridgeStatus } from "../hooks/useBridgeStatus";
import { isTauriRuntime, refreshNow } from "../ipc";

interface DashboardProps {
  readonly route: Extract<Route, "Overview" | "Claude" | "Codex">;
  readonly snapshot: UsageSnapshot | null;
  readonly navigate: (route: Route) => void;
  readonly histories?: Partial<Readonly<Record<Provider, History>>>;
  readonly error?: string | null;
  readonly bridgeStatus?: BridgeStatus;
}

function providerName(provider: Provider): string {
  return provider === "claude" ? "Claude" : "Codex";
}

function displayPlan(provider: Provider, plan: string | null): string | null {
  if (plan === null) return null;
  if (provider === "claude") return plan.startsWith("Claude ") ? plan : `Claude ${plan}`;
  return plan.startsWith("ChatGPT ") ? plan : `ChatGPT ${plan}`;
}

function windowOf(usage: ProviderUsage, wanted: "session" | "weekly"): LimitWindow | undefined {
  return usage.windows.find((window) => window.kind.kind === wanted);
}

function todayValues(history: History | null): readonly number[] {
  if (history === null) return [];
  const today = new Date();
  return history.hourly.buckets.filter((bucket) => {
    const date = new Date(bucket.start * 1_000);
    return date.getFullYear() === today.getFullYear()
      && date.getMonth() === today.getMonth()
      && date.getDate() === today.getDate();
  }).map((bucket) => bucket.tokens);
}

function peakLabel(history: History | null): string | null {
  if (history === null) return null;
  const today = new Date();
  const buckets = history.hourly.buckets.filter((bucket) => {
    const date = new Date(bucket.start * 1_000);
    return date.getFullYear() === today.getFullYear()
      && date.getMonth() === today.getMonth()
      && date.getDate() === today.getDate();
  });
  if (buckets.length === 0) return null;
  const peak = buckets.reduce((best, bucket) => bucket.tokens > best.tokens ? bucket : best);
  return new Intl.DateTimeFormat(undefined, { hour: "numeric" }).format(new Date(peak.start * 1_000));
}

function DetailLimit({ window, provider, label }: { readonly window: LimitWindow; readonly provider: Provider; readonly label: string }) {
  return (
    <div className="detail-limit">
      <div><span>{label} · <Countdown resetsAt={window.resetsAt} resetPending={window.resetPending} /></span><UsageValue percent={window.used} provider={provider} compact /></div>
      <NeonBar percent={window.used} accent={provider} label={label === "Session" ? "5-hour limit" : "weekly limit"} resetsAt={window.resetsAt} resetPending={window.resetPending} height={8} showValue={false} reflect={false} />
    </div>
  );
}

function DetailHeader({ usage }: { readonly usage: ProviderUsage }) {
  const session = windowOf(usage, "session");
  const weekly = windowOf(usage, "weekly");
  const date = new Intl.DateTimeFormat(undefined, { weekday: "long", month: "short", day: "numeric" }).format(new Date());
  return (
    <header className="detail-header" data-provider={usage.provider}>
      <ServiceBadge provider={usage.provider} size="large" />
      <div className="detail-header__identity">
        <span><strong>{providerName(usage.provider)}</strong>{displayPlan(usage.provider, usage.plan) === null ? null : <small className="ctl">{displayPlan(usage.provider, usage.plan)}</small>}</span>
        <small>{date}</small>
      </div>
      <div className="detail-header__limits">
        {session === undefined ? null : <DetailLimit window={session} provider={usage.provider} label="Session" />}
        {weekly === undefined ? null : <DetailLimit window={weekly} provider={usage.provider} label="Weekly" />}
      </div>
    </header>
  );
}

function scopeWord(scope: History["hourly"]["scope"]): string {
  return scope === "account" ? "account" : "this Mac";
}

function peakStat(history: History): { readonly value: string; readonly caption: string } {
  const peak = history.hourly.buckets.reduce((best, bucket) => bucket.tokens > best.tokens ? bucket : best);
  return {
    value: new Intl.DateTimeFormat(undefined, { hour: "numeric" }).format(new Date(peak.start * 1_000)),
    caption: `${formatTokenCount(peak.tokens)} tokens · ${scopeWord(history.hourly.scope)}`,
  };
}

function averageStat(history: History): { readonly value: string; readonly caption: string } {
  const total = history.daily.buckets.reduce((sum, bucket) => sum + bucket.tokens, 0);
  const divisor = Math.max(1, history.daily.buckets.length);
  return {
    value: formatTokenCount(total / divisor),
    caption: `tokens per day · ${scopeWord(history.daily.scope)}`,
  };
}

function projectionStat(history: History): { readonly value: string | null; readonly caption: string; readonly warning: boolean } {
  const projection = history.projection;
  if (projection === null) return { value: null, caption: "Not enough data yet", warning: false };
  if (projection.hitsLimitAt === null) {
    return { value: `~${String(Math.round(projection.projectedPercentAtReset))}% by reset`, caption: "Stays under the limit at this pace", warning: false };
  }
  const reached = new Intl.DateTimeFormat(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" }).format(new Date(projection.hitsLimitAt * 1_000));
  return { value: `~${String(Math.round(projection.projectedPercentAtReset))}% by reset`, caption: `Limit reached ≈ ${reached}`, warning: true };
}

function HistoryLoading() {
  return <div className="history-loading" role="status"><span className="shim" /><span className="shim" /><span className="sr-only">Loading history</span></div>;
}

function DetailView({ usage, history, error }: { readonly usage: ProviderUsage; readonly history: History | null; readonly error: string | null }) {
  if (history === null) return <><DetailHeader usage={usage} />{error === null ? <HistoryLoading /> : <div className="dashboard-error" role="alert">{error}</div>}</>;
  const peak = peakStat(history);
  const average = averageStat(history);
  const projection = projectionStat(history);
  const dailyNote = history.daily.scope === "account" && usage.provider === "codex" ? "tokens per UTC day" : "tokens per day";
  return (
    <>
      <DetailHeader usage={usage} />
      <div className="detail-charts">
        <ChartCard title="Last 24 hours" note="tokens per hour" series={history.hourly} grow="wide"><HourlyChart series={history.hourly} provider={usage.provider} /></ChartCard>
        <ChartCard title="Last 7 days" note={dailyNote} series={history.daily}><DailyChart series={history.daily} provider={usage.provider} /></ChartCard>
      </div>
      <div className="detail-stats">
        <StatTile icon="flame" label="Peak hour" value={peak.value} caption={peak.caption} />
        <StatTile icon="average" label="7-day average" value={average.value} caption={average.caption} />
        <StatTile icon="trend" label="At this pace" value={projection.value} caption={projection.caption} tone={projection.warning ? "warning" : usage.provider} warning={projection.warning} />
      </div>
    </>
  );
}

export function Dashboard({ route, snapshot, navigate, histories, error = null, bridgeStatus }: DashboardProps) {
  const bridge = useBridgeStatus(bridgeStatus);
  const claudeState = useHistory("claude", snapshot?.claude.lastUpdated ?? null);
  const codexState = useHistory("codex", snapshot?.codex.lastUpdated ?? null);
  const claudeHistory = histories?.claude ?? claudeState.history;
  const codexHistory = histories?.codex ?? codexState.history;
  const cardAction = (provider: Provider, action: ProviderCardAction) => {
    if (action === "details") navigate(provider === "claude" ? "Claude" : "Codex");
    else if (action === "retry" && isTauriRuntime()) void refreshNow().catch(() => undefined);
    else navigate("Settings");
  };
  const updatedAt = snapshot?.generatedAt ?? null;

  return (
    <main className="dashboard-root glass">
      <MainToolbar route={route} updatedAt={updatedAt} navigate={navigate} />
      {error === null ? null : <div className="dashboard-error" role="status">{error}</div>}
      {route === "Overview" ? (
        <div className="overview-grid">
          <ProviderCard usage={snapshot?.claude ?? null} provider="claude" sparkline={todayValues(claudeHistory)} peakLabel={peakLabel(claudeHistory)} onAction={(action) => { cardAction("claude", action); }} bridgeStatus={bridge.status} />
          <ProviderCard usage={snapshot?.codex ?? null} provider="codex" sparkline={todayValues(codexHistory)} peakLabel={peakLabel(codexHistory)} onAction={(action) => { cardAction("codex", action); }} />
        </div>
      ) : snapshot === null ? <HistoryLoading /> : (
        <div className="detail-content">
          {route === "Claude"
            ? <DetailView usage={snapshot.claude} history={claudeHistory} error={histories?.claude === undefined ? claudeState.error : null} />
            : <DetailView usage={snapshot.codex} history={codexHistory} error={histories?.codex === undefined ? codexState.error : null} />}
        </div>
      )}
    </main>
  );
}
