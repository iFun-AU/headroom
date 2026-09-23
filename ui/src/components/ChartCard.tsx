/**
 * @file ChartCard.tsx
 * @description Shared chart material with source scope and freshness disclosure.
 */
import { useId, type ReactNode } from "react";
import type { Series } from "../bindings/Series";
import { useNow } from "../hooks/useNow";
import { Icon } from "./Icon";

function asOfLabel(observedAt: number | null, nowMs: number): string | null {
  if (observedAt === null || nowMs / 1_000 - observedAt <= 300) return null;
  return new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(observedAt * 1_000));
}

export function ChartCard({
  title,
  note,
  series,
  grow = "regular",
  children,
}: {
  readonly title: string;
  readonly note: string;
  readonly series: Series;
  readonly grow?: "regular" | "wide";
  readonly children: ReactNode;
}) {
  const titleId = useId();
  const now = useNow(60_000);
  const asOf = asOfLabel(series.observedAt, now);
  return (
    <section className="chart-card card" data-grow={grow} aria-labelledby={titleId}>
      <header className="chart-card__header">
        <strong id={titleId}>{title}</strong>
        <span>{note}</span>
      </header>
      {children}
      <footer className="chart-card__scope">
        <Icon name="info" size={12} />
        <span>{series.scopeLabel}{asOf === null ? null : ` · as of ${asOf}`}</span>
      </footer>
    </section>
  );
}
