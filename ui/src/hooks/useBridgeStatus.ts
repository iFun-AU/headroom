/**
 * @file useBridgeStatus.ts
 * @description Claude bridge status and mutation lifecycle.
 */
import { useCallback, useEffect, useState } from "react";
import type { BridgeStatus } from "../bindings/BridgeStatus";
import { getClaudeBridgeStatus, installClaudeBridge, isTauriRuntime, onBridgeStatusChanged, uninstallClaudeBridge } from "../ipc";

function message(error: unknown): string {
  return error instanceof Error ? error.message : "The Claude bridge could not be changed.";
}

export function useBridgeStatus(fixture?: BridgeStatus) {
  const native = isTauriRuntime();
  const [status, setStatus] = useState<BridgeStatus | null>(fixture ?? null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (fixture !== undefined) setStatus(fixture);
  }, [fixture]);

  useEffect(() => {
    if (!native) return undefined;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void getClaudeBridgeStatus().then((next) => { if (!cancelled) setStatus(next); }).catch((cause: unknown) => { if (!cancelled) setError(message(cause)); });
    void onBridgeStatusChanged((next) => { if (!cancelled) setStatus(next); }).then((dispose) => {
      if (cancelled) dispose(); else unlisten = dispose;
    }).catch((cause: unknown) => { if (!cancelled) setError(message(cause)); });
    return () => { cancelled = true; unlisten?.(); };
  }, [native]);

  const setInstalled = useCallback(async (installed: boolean) => {
    setBusy(true);
    setError(null);
    try {
      if (native) setStatus(installed ? await installClaudeBridge() : await uninstallClaudeBridge());
      else setStatus((current) => current === null ? null : { ...current, installed, effective: "unverified" });
      return true;
    } catch (cause) {
      setError(message(cause));
      return false;
    } finally {
      setBusy(false);
    }
  }, [native]);

  return { status, busy, error, setInstalled };
}
