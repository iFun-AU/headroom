/**
 * @file Countdown.tsx
 * @description Live reset and last-updated labels driven by the shared window clock.
 */
import { useNow } from "../hooks/useNow";

function formatDuration(totalSeconds: number, withSeconds: boolean): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(seconds / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  if (withSeconds) {
    return `${String(hours)}:${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
  }
  if (hours > 0) return `${String(hours)}h ${String(minutes).padStart(2, "0")}m`;
  return `${String(minutes)}m`;
}

export function Countdown({
  resetsAt,
  resetPending,
  precise = false,
}: {
  readonly resetsAt: number | null;
  readonly resetPending: boolean;
  readonly precise?: boolean;
}) {
  const now = useNow(1_000);
  if (resetPending) return <>Reset — waiting for new data</>;
  if (resetsAt === null) return <>Reset time unavailable</>;

  const remaining = resetsAt - now / 1_000;
  if (remaining <= 0) return <>Reset — waiting for new data</>;
  if (remaining < 86_400) return <>{precise ? formatDuration(remaining, true) : `Resets in ${formatDuration(remaining, false)}`}</>;

  const reset = new Intl.DateTimeFormat(undefined, {
    weekday: "short",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(resetsAt * 1_000));
  return <>Resets {reset}</>;
}

export function UpdatedAgo({ timestamp }: { readonly timestamp: number | null }) {
  const now = useNow(1_000);
  if (timestamp === null) return <>Waiting for data</>;
  const seconds = Math.max(0, Math.floor(now / 1_000 - timestamp));
  if (seconds < 2) return <>Updated just now</>;
  if (seconds < 60) return <>Updated {String(seconds)}s ago</>;
  if (seconds < 3_600) return <>Updated {String(Math.floor(seconds / 60))}m ago</>;
  return <>Updated {String(Math.floor(seconds / 3_600))}h ago</>;
}
