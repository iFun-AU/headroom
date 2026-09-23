/**
 * @file useSettings.ts
 * @description Live settings state with typed save/reset operations and testable browser fixtures.
 */
import { useCallback, useEffect, useState } from "react";
import type { Settings } from "../bindings/Settings";
import type { SettingsState } from "../bindings/SettingsState";
import { getSettings, isTauriRuntime, onSettingsChanged, resetSettings, setSettings } from "../ipc";

export interface SettingsHookState {
  readonly state: SettingsState | null;
  readonly loading: boolean;
  readonly saving: boolean;
  readonly error: string | null;
  readonly save: (settings: Settings) => Promise<SettingsState | null>;
  readonly reset: () => Promise<SettingsState | null>;
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : "Settings could not be saved.";
}

export function useSettings(fixture?: SettingsState): SettingsHookState {
  const native = isTauriRuntime();
  const [state, setState] = useState<SettingsState | null>(fixture ?? null);
  const [loading, setLoading] = useState(native && fixture === undefined);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (fixture !== undefined) setState(fixture);
  }, [fixture]);

  useEffect(() => {
    if (!native) return undefined;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const accept = (next: SettingsState) => {
      if (!cancelled) {
        setState(next);
        setLoading(false);
        setError(null);
      }
    };
    void getSettings().then(accept).catch((cause: unknown) => {
      if (!cancelled) { setLoading(false); setError(message(cause)); }
    });
    void onSettingsChanged(accept).then((dispose) => {
      if (cancelled) dispose(); else unlisten = dispose;
    }).catch((cause: unknown) => { if (!cancelled) setError(message(cause)); });
    return () => { cancelled = true; unlisten?.(); };
  }, [native]);

  const save = useCallback(async (settings: Settings) => {
    setSaving(true);
    setError(null);
    try {
      const next = native ? await setSettings(settings) : { settings, readOnly: false, notice: null };
      setState(next);
      return next;
    } catch (cause) {
      setError(message(cause));
      return null;
    } finally {
      setSaving(false);
    }
  }, [native]);

  const reset = useCallback(async () => {
    setSaving(true);
    setError(null);
    try {
      const next = native ? await resetSettings() : fixture ?? null;
      if (next !== null) setState(next);
      return next;
    } catch (cause) {
      setError(message(cause));
      return null;
    } finally {
      setSaving(false);
    }
  }, [fixture, native]);

  return { state, loading, saving, error, save, reset };
}
