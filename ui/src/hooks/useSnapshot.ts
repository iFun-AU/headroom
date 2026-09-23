/**
 * @file useSnapshot.ts
 * @description Live usage snapshot lifecycle shared by every native window.
 */
import { useEffect, useState } from "react";
import type { UsageSnapshot } from "../bindings/UsageSnapshot";
import {
  getCurrentWindowLabel,
  getSnapshot,
  isTauriRuntime,
  onUsageUpdated,
  uiVisible,
} from "../ipc";

export interface SnapshotState {
  readonly snapshot: UsageSnapshot | null;
  readonly loading: boolean;
  readonly error: string | null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Usage is temporarily unavailable.";
}

export function useSnapshot(): SnapshotState {
  const [snapshot, setSnapshot] = useState<UsageSnapshot | null>(null);
  const [loading, setLoading] = useState(isTauriRuntime());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return undefined;

    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const label = getCurrentWindowLabel();
    let deferred: UsageSnapshot | undefined;
    const accept = (next: UsageSnapshot) => {
      if (cancelled) return;
      if (document.visibilityState !== "visible") {
        deferred = next;
        return;
      }
      setSnapshot(next);
      setLoading(false);
      setError(null);
    };
    const fetchSnapshot = () => {
      void getSnapshot().then(accept).catch((cause: unknown) => {
        if (!cancelled) {
          setLoading(false);
          setError(errorMessage(cause));
        }
      });
    };
    const reportVisible = () => {
      if (document.visibilityState === "visible") {
        if (deferred !== undefined) {
          const latest = deferred;
          deferred = undefined;
          accept(latest);
        }
        fetchSnapshot();
        void uiVisible(label).catch(() => undefined);
      }
    };

    void onUsageUpdated(accept).then((dispose) => {
      if (cancelled) dispose();
      else unlisten = dispose;
    }).catch((cause: unknown) => {
      if (!cancelled) setError(errorMessage(cause));
    });

    window.addEventListener("focus", reportVisible);
    document.addEventListener("visibilitychange", reportVisible);
    reportVisible();

    return () => {
      cancelled = true;
      unlisten?.();
      window.removeEventListener("focus", reportVisible);
      document.removeEventListener("visibilitychange", reportVisible);
    };
  }, []);

  return { snapshot, loading, error };
}
