/**
 * @file useRoute.ts
 * @description Dependency-free hash routing synchronized with native navigation events.
 */
import { useCallback, useEffect, useState } from "react";
import type { Route } from "../bindings/Route";
import { isTauriRuntime, onNavigate } from "../ipc";

const ROUTE_HASH: Readonly<Record<Route, string>> = {
  Overview: "#/",
  Claude: "#/claude",
  Codex: "#/codex",
  Settings: "#/settings",
  Onboarding: "#/onboarding",
};

function routeFromHash(hash: string): Route {
  switch (hash.toLowerCase()) {
    case "#/claude": return "Claude";
    case "#/codex": return "Codex";
    case "#/settings": return "Settings";
    case "#/onboarding": return "Onboarding";
    case "#/":
    case "":
    default: return "Overview";
  }
}

export function useRoute(): { readonly route: Route; readonly navigate: (route: Route) => void } {
  const [route, setRoute] = useState<Route>(() => routeFromHash(window.location.hash));
  const navigate = useCallback((next: Route) => {
    const hash = ROUTE_HASH[next];
    if (window.location.hash === hash) setRoute(next);
    else window.location.hash = hash;
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const readHash = () => { setRoute(routeFromHash(window.location.hash)); };
    window.addEventListener("hashchange", readHash);

    if (isTauriRuntime()) {
      void onNavigate(navigate).then((dispose) => {
        if (cancelled) dispose();
        else unlisten = dispose;
      }).catch(() => undefined);
    }

    return () => {
      cancelled = true;
      unlisten?.();
      window.removeEventListener("hashchange", readHash);
    };
  }, [navigate]);

  return { route, navigate };
}
