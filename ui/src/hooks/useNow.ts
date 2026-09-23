/**
 * @file useNow.ts
 * @description Shared ticking clock: one interval per cadence in each webview window.
 */
import { useCallback, useSyncExternalStore } from "react";

interface Clock {
  now: number;
  readonly listeners: Set<() => void>;
  timer: number | null;
}

const clocks = new Map<number, Clock>();

function clockFor(intervalMs: number): Clock {
  const existing = clocks.get(intervalMs);
  if (existing !== undefined) return existing;
  const created: Clock = { now: Date.now(), listeners: new Set(), timer: null };
  clocks.set(intervalMs, created);
  return created;
}

function start(clock: Clock, intervalMs: number): void {
  if (clock.timer !== null) return;
  clock.now = Date.now();
  clock.timer = window.setInterval(() => {
    clock.now = Date.now();
    for (const listener of clock.listeners) listener();
  }, intervalMs);
}

function stop(clock: Clock): void {
  if (clock.timer === null) return;
  window.clearInterval(clock.timer);
  clock.timer = null;
}

export function useNow(intervalMs = 1_000): number {
  const clock = clockFor(intervalMs);
  const subscribe = useCallback((listener: () => void) => {
    clock.listeners.add(listener);
    start(clock, intervalMs);
    return () => {
      clock.listeners.delete(listener);
      if (clock.listeners.size === 0) stop(clock);
    };
  }, [clock, intervalMs]);
  return useSyncExternalStore(
    subscribe,
    () => clock.now,
    () => clock.now,
  );
}
