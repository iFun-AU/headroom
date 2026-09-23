/**
 * @file glass.ts
 * @description Applies native per-window Liquid Glass and maintains the opaque fallback.
 */
import type { WidgetVariant } from "./bindings/WidgetVariant";
import {
  applyLiquidGlass,
  getCurrentWindowLabel,
  getSettings,
  isLiquidGlassSupported,
  isTauriRuntime,
  onSettingsChanged,
} from "./ipc";

function widgetRadius(variant: WidgetVariant): number {
  switch (variant) {
    case "Pill":
      return 36;
    case "Stack":
      return 24;
    case "Mini":
      return 12;
  }
}

function setFallback(enabled: boolean): void {
  document.documentElement.classList.toggle("no-glass", enabled);
}

export async function initializeWindowGlass(): Promise<() => void> {
  if (!isTauriRuntime()) return () => undefined;

  try {
    const supported = await isLiquidGlassSupported();
    if (!supported) {
      setFallback(true);
      return () => undefined;
    }

    const label = getCurrentWindowLabel();
    if (label === "widget") {
      const initial = await getSettings();
      await applyLiquidGlass(widgetRadius(initial.settings.widget.variant));
      const unlisten = await onSettingsChanged((state) => {
        void applyLiquidGlass(widgetRadius(state.settings.widget.variant)).catch(() => {
          setFallback(true);
        });
      });
      setFallback(false);
      return unlisten;
    }

    await applyLiquidGlass(label === "popover" ? 18 : 0);
    setFallback(false);
  } catch {
    setFallback(true);
  }

  return () => undefined;
}
