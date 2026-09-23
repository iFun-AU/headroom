/**
 * @file NeonBar.tsx
 * @description Layered, accessible plan-usage bar from the approved component anatomy.
 */
import { useEffect, useMemo, useState, type CSSProperties } from "react";

export interface NeonBarProps {
  readonly percent: number;
  readonly accent: "claude" | "codex";
  readonly label: string;
  readonly resetsAt: number | null;
  readonly resetPending: boolean;
  readonly height?: 3 | 4 | 6 | 8 | 14 | 24;
  readonly showValue?: boolean;
  readonly dimmed?: boolean;
  readonly reflect?: boolean;
}

type BarLevel = "normal" | "warning" | "critical" | "limit";
type AnimationPhase = "idle" | "entering" | "settled";

interface BarStyle extends CSSProperties {
  "--bar-bloom-far": string;
  "--bar-bloom-near": string;
  "--bar-height": string;
  "--bar-width": string;
}

function clampPercent(value: number): number {
  return Number.isFinite(value) ? Math.min(100, Math.max(0, value)) : 0;
}

function levelFor(percent: number): BarLevel {
  if (percent >= 100) return "limit";
  if (percent >= 90) return "critical";
  if (percent >= 75) return "warning";
  return "normal";
}

function statusWord(level: BarLevel): string | null {
  switch (level) {
    case "warning":
      return "Warning";
    case "critical":
      return "Critical";
    case "limit":
      return "Limit reached";
    case "normal":
      return null;
  }
}

function formatWindow(label: string, resetsAt: number): string {
  const duration = label.toLowerCase().includes("week") ? 7 * 86_400 : 5 * 3_600;
  const formatter = new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
  return `Window ${formatter.format(new Date((resetsAt - duration) * 1000))} – ${formatter.format(new Date(resetsAt * 1000))}`;
}

function StatusIcon({ level }: { readonly level: Exclude<BarLevel, "normal"> }) {
  const word = statusWord(level) ?? "";

  if (level === "warning") {
    return (
      <svg className="neon-bar__status-icon" viewBox="0 0 24 24" role="img" aria-label={word}>
        <path d="M10.3 4.9a2 2 0 0 1 3.4 0l7.1 12.3a2 2 0 0 1-1.7 3H4.9a2 2 0 0 1-1.7-3Z" />
        <path d="M12 9.5v4.5M12 17.2v.1" />
      </svg>
    );
  }

  if (level === "critical") {
    return (
      <svg className="neon-bar__status-icon" viewBox="0 0 24 24" role="img" aria-label={word}>
        <path d="M8.6 3.5h6.8l5.1 5.1v6.8l-5.1 5.1H8.6l-5.1-5.1V8.6Z" />
        <path d="M12 8v5M12 16.3v.1" />
      </svg>
    );
  }

  return (
    <svg className="neon-bar__status-icon" viewBox="0 0 24 24" role="img" aria-label={word}>
      <path d="M6.5 3.5h11M6.5 20.5h11" />
      <path d="M8 3.5v2.8c0 1.4.7 2.6 1.9 3.4l2.1 1.5 2.1-1.5c1.2-.8 1.9-2 1.9-3.4V3.5" />
      <path d="M8 20.5v-2.8c0-1.4.7-2.6 1.9-3.4l2.1-1.5 2.1 1.5c1.2.8 1.9 2 1.9 3.4v2.8" />
    </svg>
  );
}

export function NeonBar({
  percent,
  accent,
  label,
  resetsAt,
  resetPending,
  height = 14,
  showValue = true,
  dimmed = false,
  reflect = true,
}: NeonBarProps) {
  const safePercent = clampPercent(percent);
  const level = levelFor(safePercent);
  const [phase, setPhase] = useState<AnimationPhase>("idle");

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      setPhase("entering");
    });
    const timer = window.setTimeout(() => {
      setPhase("settled");
    }, 1_300);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
  }, []);

  const style = useMemo<BarStyle>(() => {
    const multiplier = level === "warning" ? 1.2 : level === "critical" || level === "limit" ? 1.25 : 1;
    const near = (3 + safePercent * 0.07) * multiplier;
    const far = (8 + safePercent * 0.2) * multiplier;
    return {
      "--bar-bloom-far": `${far.toFixed(1)}px`,
      "--bar-bloom-near": `${near.toFixed(1)}px`,
      "--bar-height": `${String(height)}px`,
      "--bar-width": phase === "idle" ? "0%" : `${String(safePercent)}%`,
    };
  }, [height, level, phase, safePercent]);

  const levelWord = statusWord(level);
  const ariaLabel = resetPending
    ? `${label}: reset, waiting for new data, 0 percent`
    : `${label}: ${String(safePercent)} percent${levelWord === null ? "" : `, ${levelWord.toLowerCase()}`}`;
  const hasDetails = resetsAt !== null && !resetPending;

  return (
    <div className="neon-bar" data-level={level} data-tone={accent} data-dimmed={dimmed} style={style}>
      <div
        className="neon-bar__visual"
        role="progressbar"
        aria-label={ariaLabel}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={resetPending ? 0 : safePercent}
        tabIndex={hasDetails ? 0 : undefined}
      >
        <div className="neon-bar__track" />
        {!resetPending && safePercent > 0 && reflect ? <div className="neon-bar__reflection" /> : null}
        {!resetPending && level === "limit" ? <div className="neon-bar__halo" /> : null}
        <div
          className="neon-bar__fill"
          data-empty={resetPending || safePercent === 0}
          data-limit={!resetPending && level === "limit"}
          data-phase={phase}
        >
          {!resetPending && safePercent > 0 && height >= 6 ? <span className="neon-bar__core" /> : null}
          {!resetPending && safePercent > 0 && height >= 8 ? <span className="neon-bar__head" /> : null}
        </div>
        {hasDetails ? (
          <span className="neon-bar__tooltip tipbox" role="tooltip">
            <strong>{safePercent}% of {label}</strong>
            <span>{formatWindow(label, resetsAt)}</span>
            <span>About {100 - safePercent}% left</span>
          </span>
        ) : null}
      </div>
      {showValue ? (
        <span className="neon-bar__value num neon" data-pending={resetPending}>
          {!resetPending && level !== "normal" ? <StatusIcon level={level} /> : null}
          {resetPending ? "0%" : `${String(safePercent)}%`}
        </span>
      ) : null}
    </div>
  );
}
