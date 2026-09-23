/**
 * @file HourlyChart.tsx
 * @description Accessible 24-hour token histogram with a highlighted current hour.
 */
import type { CSSProperties } from "react";
import type { Provider } from "../bindings/Provider";
import type { Series } from "../bindings/Series";

type BarStyle = CSSProperties & {
  readonly "--bar-delay": string;
  readonly "--bar-height": string;
};

export function formatTokenCount(tokens: number): string {
  if (tokens <= 0) return "0";
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
  if (tokens >= 1_000) return `${String(Math.round(tokens / 1_000))}k`;
  return new Intl.NumberFormat().format(tokens);
}

function niceAxisMaximum(peak: number): number {
  const target = Math.max(1, peak * 1.1);
  const magnitude = 10 ** Math.floor(Math.log10(target));
  const fraction = target / magnitude;
  const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 2.5 ? 2.5 : fraction <= 5 ? 5 : 10;
  return nice * magnitude;
}

function hourLabel(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, { hour: "numeric" }).format(new Date(timestamp * 1_000));
}

export function HourlyChart({ series, provider }: { readonly series: Series; readonly provider: Provider }) {
  const buckets = series.buckets;
  const peak = buckets.reduce((maximum, bucket) => Math.max(maximum, bucket.tokens), 0);
  const maximum = niceAxisMaximum(peak);
  const currentIndex = Math.max(0, buckets.length - 1);
  const ticks = [maximum, maximum / 2, 0];

  return (
    <div className="hourly-chart" data-provider={provider} aria-label="Tokens used in each of the last 24 hours">
      <div className="hourly-chart__grid" aria-hidden="true">
        {ticks.map((tick) => <span key={tick}><b className="num">{formatTokenCount(tick)}</b></span>)}
      </div>
      <div className="hourly-chart__bars">
        {buckets.map((bucket, index) => {
          const current = index === currentIndex;
          const style: BarStyle = {
            "--bar-delay": `${String(index * 25)}ms`,
            "--bar-height": `${String((bucket.tokens / maximum) * 100)}%`,
          };
          const label = hourLabel(bucket.start);
          return (
            <div className="chart-slot" data-current={current} key={bucket.start}>
              <div className="chart-slot__plot" style={style}>
                {current ? <span className="chart-slot__value num neon">{formatTokenCount(bucket.tokens)}</span> : null}
                <button
                  className="chart-bar chart-bar--hourly"
                  aria-label={`${formatTokenCount(bucket.tokens)} tokens at ${label}${current ? ", current hour" : ""}`}
                >
                  <span className="tipbox" role="tooltip"><strong>{formatTokenCount(bucket.tokens)} tokens</strong><small>{label}{current ? " · now" : ""}</small></span>
                </button>
              </div>
              <span className="chart-slot__label">{current ? "Now" : index % 6 === 0 ? label : ""}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
