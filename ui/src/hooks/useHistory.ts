/**
 * @file useHistory.ts
 * @description Rate-limited provider history loading keyed by snapshot freshness.
 */
import { useEffect, useRef, useState } from "react";
import type { History } from "../bindings/History";
import type { Provider } from "../bindings/Provider";
import { getHistory, isTauriRuntime } from "../ipc";

const MINIMUM_FETCH_INTERVAL_MS = 60_000;

export interface HistoryState {
  readonly history: History | null;
  readonly loading: boolean;
  readonly error: string | null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "History is temporarily unavailable.";
}

export function useHistory(provider: Provider, lastUpdated: number | null): HistoryState {
  const [history, setHistory] = useState<History | null>(null);
  const [loading, setLoading] = useState(isTauriRuntime());
  const [error, setError] = useState<string | null>(null);
  const lastFetch = useRef<Record<Provider, number>>({ claude: 0, codex: 0 });
  const requestSequence = useRef(0);

  useEffect(() => {
    setHistory(null);
    setError(null);
  }, [provider]);

  useEffect(() => {
    if (!isTauriRuntime()) return undefined;

    let cancelled = false;
    let timer: number | undefined;
    const sequence = ++requestSequence.current;
    const load = () => {
      lastFetch.current[provider] = Date.now();
      setLoading(true);
      void getHistory(provider).then((next) => {
        if (cancelled || sequence !== requestSequence.current) return;
        setHistory(next);
        setLoading(false);
        setError(null);
      }).catch((cause: unknown) => {
        if (cancelled || sequence !== requestSequence.current) return;
        setLoading(false);
        setError(errorMessage(cause));
      });
    };

    const elapsed = Date.now() - lastFetch.current[provider];
    if (elapsed >= MINIMUM_FETCH_INTERVAL_MS) load();
    else timer = window.setTimeout(load, MINIMUM_FETCH_INTERVAL_MS - elapsed);

    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [lastUpdated, provider]);

  return { history, loading, error };
}
