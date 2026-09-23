/**
 * @file ThresholdLegend.tsx
 * @description Accessible legend proving that threshold meaning survives without colour.
 */
import { NeonBar } from "./NeonBar";

const ROWS = [
  { label: "Normal", percent: 52, description: "Below 75%" },
  { label: "Warning", percent: 80, description: "75–89% · warning symbol" },
  { label: "Critical", percent: 95, description: "90–99% · critical symbol" },
  { label: "Limit reached", percent: 100, description: "100% · hourglass symbol" },
] as const;

export function ThresholdLegend() {
  return (
    <section className="threshold-legend card" aria-labelledby="threshold-legend-title">
      <header>
        <strong id="threshold-legend-title">Thresholds</strong>
        <small>Colour is never the only signal</small>
      </header>
      {ROWS.map((row) => (
        <div className="threshold-legend__row" key={row.label}>
          <span><strong>{row.label}</strong><small>{row.description}</small></span>
          <NeonBar percent={row.percent} accent="codex" label={`${row.label} threshold example`} resetsAt={null} resetPending={false} height={8} />
        </div>
      ))}
    </section>
  );
}
