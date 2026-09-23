/**
 * @file App.tsx
 * @description Window entry point and debug-only approved component demo.
 */
import { lazy, Suspense, useEffect } from "react";
import { initializeWindowGlass } from "./glass";
import { useRoute } from "./hooks/useRoute";
import { useSnapshot } from "./hooks/useSnapshot";
import { getCurrentWindowLabel, isTauriRuntime } from "./ipc";
import { Dashboard } from "./screens/Dashboard";
import { Onboarding } from "./screens/Onboarding";
import { Popover } from "./screens/Popover";
import { Settings } from "./screens/Settings";
import { Widget } from "./screens/Widget";
import "./styles/app.css";

const DebugRoutes = import.meta.env.DEV ? lazy(() => import("./debug/DebugRoutes")) : null;

export function App() {
  const nativeRuntime = isTauriRuntime();
  const { snapshot, error } = useSnapshot();
  const { route, navigate } = useRoute();

  useEffect(() => {
    let cancelled = false;
    let dispose: () => void = () => undefined;
    void initializeWindowGlass().then((cleanup) => {
      if (cancelled) cleanup();
      else dispose = cleanup;
    });
    return () => {
      cancelled = true;
      dispose();
    };
  }, []);

  const windowLabel = nativeRuntime ? getCurrentWindowLabel() : "main";
  if (windowLabel === "popover") {
    return <Popover snapshot={snapshot} />;
  }
  if (windowLabel === "widget") {
    return <Widget snapshot={snapshot} />;
  }

  if (!nativeRuntime && DebugRoutes !== null) {
    return (
      <Suspense fallback={<main className="demo-screen" />}>
        <DebugRoutes />
      </Suspense>
    );
  }

  if (route === "Overview" || route === "Claude" || route === "Codex") {
    return <Dashboard route={route} snapshot={snapshot} navigate={navigate} error={error} />;
  }
  if (route === "Settings") {
    return <Settings snapshot={snapshot} navigateOverview={() => { navigate("Overview"); }} />;
  }
  return <Onboarding navigate={navigate} />;
}
