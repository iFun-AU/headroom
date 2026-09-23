/**
 * @file DailyChart.tsx
 * @description Seven-day token histogram without a fabricated quota or pace line.
 */
import type { CSSProperties } from "react";
import type { Provider } from "../bindings/Provider";
import type { Series } from "../bindings/Series";
import { formatTokenCount } from "./HourlyChart";

type BarStyle = CSSProperties & {
  readonly "--bar-delay": string;
  readonly "--bar-height": string;
};

function isLocalToday(timestamp: number): boolean {
  const then = new Date(timestamp * 1_000);
  const now = new Date();
  return then.getFullYear() === now.getFullYear()
    && then.getMonth() === now.getMonth()
    && then.getDate() === now.getDate();
}

function dayLabel(series: Series, index: number): string {
  const bucket = series.buckets[index];
  if (bucket === undefined) return "";
  const last = index === series.buckets.length - 1;
  if (last && series.scope === "thisMac" && isLocalToday(bucket.start)) return "Today";
  if (last && series.scope === "account") {
    return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(new Date(bucket.start * 1_000));
  }
  return new Intl.DateTimeFormat(undefined, { weekday: "short" }).format(new Date(bucket.start * 1_000));
}

export function DailyChart({ series, provider }: { readonly series: Series; readonly provider: Provider }) {
  const peak = series.buckets.reduce((maximum, bucket) => Math.max(maximum, bucket.tokens), 0);
  const maximum = Math.max(1, peak * 1.12);

  return (
    <div className="daily-chart" data-provider={provider} aria-label="Tokens used on each of the last seven days">
      {series.buckets.map((bucket, index) => {
        const label = dayLabel(series, index);
        const latest = index === series.buckets.length - 1;
        const style: BarStyle = {
          "--bar-delay": `${String(index * 60)}ms`,
          "--bar-height": `${String((bucket.tokens / maximum) * 100)}%`,
        };
        return (
          <div className="chart-slot" data-current={latest} key={bucket.start}>
            <div className="chart-slot__plot" style={style}>
              <span className="chart-slot__value num">{formatTokenCount(bucket.tokens)}</span>
              <button
                className="chart-bar chart-bar--daily"
                aria-label={`${formatTokenCount(bucket.tokens)} tokens, ${label === "Today" ? "today so far" : label}`}
              >
                <span className="tipbox" role="tooltip"><strong>{formatTokenCount(bucket.tokens)} tokens</strong><small>{label === "Today" ? "Today so far" : label}</small></span>
              </button>
            </div>
            <span className="chart-slot__label">{label}</span>
          </div>
        );
      })}
    </div>
  );
}
