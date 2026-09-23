/**
 * @file StatusBadge.tsx
 * @description Text-and-symbol status treatments so color never carries meaning alone.
 */
import type { ConnectionStatus } from "../bindings/ConnectionStatus";
import type { Provider } from "../bindings/Provider";
import { UpdatedAgo } from "./Countdown";
import { Icon } from "./Icon";

export type UsageLevel = "normal" | "warning" | "critical" | "limit";

export function usageLevel(percent: number): UsageLevel {
  if (percent >= 100) return "limit";
  if (percent >= 90) return "critical";
  if (percent >= 75) return "warning";
  return "normal";
}

export function UsageStatusIcon({ level }: { readonly level: Exclude<UsageLevel, "normal"> }) {
  if (level === "warning") return <Icon name="warning" size={16} label="Warning" strokeWidth={2.1} />;
  if (level === "critical") return <Icon name="critical" size={16} label="Critical" strokeWidth={2.1} />;
  return <Icon name="hourglass" size={16} label="Limit reached" strokeWidth={2.1} />;
}

export function UsageValue({
  percent,
  provider,
  compact = false,
  dimmed = false,
}: {
  readonly percent: number;
  readonly provider: Provider;
  readonly compact?: boolean;
  readonly dimmed?: boolean;
}) {
  const level = usageLevel(percent);
  return (
    <span className="usage-value num neon" data-level={level} data-provider={provider} data-compact={compact} data-dimmed={dimmed}>
      {level === "normal" ? null : <UsageStatusIcon level={level} />}
      <span>{String(percent)}<small>%</small></span>
    </span>
  );
}

export function StatusChip({
  tone,
  children,
}: {
  readonly tone: "ok" | "warning" | "neutral";
  readonly children: React.ReactNode;
}) {
  return (
    <span className="status-chip ctl" data-tone={tone}>
      <span className="status-chip__dot" aria-hidden="true" />
      {children}
    </span>
  );
}

export function ConnectionBadge({
  status,
  lastUpdated,
}: {
  readonly status: ConnectionStatus;
  readonly lastUpdated: number | null;
}) {
  if (status.state === "degraded") return <StatusChip tone="warning">Degraded</StatusChip>;
  if (status.state === "stale") return <StatusChip tone="neutral"><UpdatedAgo timestamp={lastUpdated} /></StatusChip>;
  return null;
}
