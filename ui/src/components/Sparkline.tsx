/**
 * @file Sparkline.tsx
 * @description Compact today-token series used at the foot of provider cards.
 */
import { useId } from "react";
import type { Provider } from "../bindings/Provider";

export function Sparkline({
  values,
  provider,
  dimmed = false,
}: {
  readonly values: readonly number[];
  readonly provider: Provider;
  readonly dimmed?: boolean;
}) {
  const gradientId = `spark${useId().replaceAll(":", "")}`;
  const series = values.length === 0 ? [0] : values;
  const width = 290;
  const height = 42;
  const slots = 24;
  const step = width / (slots - 1);
  const max = Math.max(...series, 1) * 1.15;
  const points = series.map((value, index) => ({
    x: Number((index * step).toFixed(1)),
    y: Number((height - 2 - value / max * (height - 8)).toFixed(1)),
  }));
  const line = points.map(({ x, y }) => `${String(x)},${String(y)}`).join(" ");
  const last = points.at(-1) ?? { x: 0, y: height - 2 };
  const area = `M0,${String(height)} L${points.map(({ x, y }) => `${String(x)},${String(y)}`).join(" L")} L${String(last.x)},${String(height)} Z`;
  const total = series.reduce((sum, value) => sum + value, 0);

  return (
    <svg
      className="sparkline"
      data-provider={provider}
      data-dimmed={dimmed}
      viewBox={`0 0 ${String(width)} ${String(height)}`}
      role="img"
      aria-label={`${Math.round(total).toLocaleString()} tokens shown for today`}
      preserveAspectRatio="none"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" className="sparkline__area-start" />
          <stop offset="1" className="sparkline__area-end" />
        </linearGradient>
      </defs>
      <line className="sparkline__future" x1={last.x} y1={height - 0.5} x2={width} y2={height - 0.5} />
      <path className="sparkline__area" d={area} style={{ fill: `url(#${gradientId})` }} />
      <polyline className="sparkline__line spark" points={line} />
      <circle className="sparkline__point" cx={last.x} cy={last.y} r="3.5" />
    </svg>
  );
}
