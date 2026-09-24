/**
 * @file Icon.tsx
 * @description Shared stroke icons and temporary provider marks from the approved design.
 */
import type { ReactNode } from "react";
import type { Provider } from "../bindings/Provider";

export type IconName =
  | "back"
  | "bell"
  | "calendar"
  | "check"
  | "chevron"
  | "clock"
  | "critical"
  | "expand"
  | "flame"
  | "gear"
  | "hourglass"
  | "info"
  | "link"
  | "lock"
  | "menubar"
  | "average"
  | "opacity"
  | "pin"
  | "pip"
  | "power"
  | "refresh"
  | "search"
  | "trend"
  | "warning"
  | "window"
  | "xmark";

function iconPaths(name: IconName): ReactNode {
  switch (name) {
    case "back":
      return <path d="M14 6.5 8.5 12l5.5 5.5" />;
    case "bell":
      return <><path d="M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 2h-15Z" /><path d="M10 20.5a2 2 0 0 0 4 0" /></>;
    case "calendar":
      return <><rect x="3.5" y="5" width="17" height="15" rx="3" /><path d="M3.5 9.5h17M8 3v4M16 3v4" /></>;
    case "check":
      return <path d="m5 12.5 4.5 4.5L19 7.5" />;
    case "chevron":
      return <path d="m10 6.5 5.5 5.5-5.5 5.5" />;
    case "clock":
      return <><circle cx="12" cy="12" r="8.5" /><path d="M12 7.5V12l3 2" /></>;
    case "critical":
      return <><path d="M8.6 3.5h6.8l5.1 5.1v6.8l-5.1 5.1H8.6l-5.1-5.1V8.6Z" /><path d="M12 8v5M12 16.3v.1" /></>;
    case "expand":
      return <path d="M14 4.5h5.5V10M10 19.5H4.5V14M19.5 4.5 13.5 10.5M4.5 19.5l6-6" />;
    case "flame":
      return <path d="M13.5 3.5c.8 4-2.4 4.8-1.2 8.1.8-1.3 2.2-2.2 3.6-2.7.8 1.2 1.6 2.7 1.6 4.6A5.1 5.1 0 0 1 12 21a5.4 5.4 0 0 1-5.5-5.3c0-3.2 2.2-5.2 4.2-7.1.4 2.3 1.6 2.7 2 2.7.8-2.6-.5-4.8.8-7.8Z" />;
    case "gear":
      return <><circle cx="12" cy="12" r="3" /><path d="M12 3.5v2.2M12 18.3v2.2M3.5 12h2.2M18.3 12h2.2M6 6l1.6 1.6M16.4 16.4 18 18M6 18l1.6-1.6M16.4 7.6 18 6" /></>;
    case "hourglass":
      return <><path d="M6.5 3.5h11M6.5 20.5h11" /><path d="M8 3.5v2.8c0 1.4.7 2.6 1.9 3.4l2.1 1.5 2.1-1.5c1.2-.8 1.9-2 1.9-3.4V3.5M8 20.5v-2.8c0-1.4.7-2.6 1.9-3.4l2.1-1.5 2.1 1.5c1.2.8 1.9 2 1.9 3.4v2.8" /></>;
    case "info":
      return <><circle cx="12" cy="12" r="8.5" /><path d="M12 11v5.5M12 7.8v.1" /></>;
    case "link":
      return <><path d="M10 14a4 4 0 0 0 5.7 0l3-3A4 4 0 0 0 13 5.3l-1 1" /><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1" /></>;
    case "lock":
      return <><rect x="5" y="10.5" width="14" height="10" rx="2.5" /><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5" /></>;
    case "menubar":
      return <><rect x="3" y="4.5" width="18" height="15" rx="3" /><path d="M3 8.5h18" /></>;
    case "average":
      return <><path d="M4 7.5h16M4 16.5h16" /><circle cx="8" cy="7.5" r="2.2" /><circle cx="16" cy="16.5" r="2.2" /></>;
    case "opacity":
      return <><circle cx="12" cy="12" r="8" /><path d="M12 4v16M12 8h5.5M12 12h8M12 16h5.5" /></>;
    case "pin":
      return <><path d="M9 3.5h6l-1 5.5 3.5 3.5v1.5h-11V12.5L10 9Z" /><path d="M12 14v6.5" /></>;
    case "pip":
      return <><rect x="3" y="5" width="18" height="14" rx="3.5" /><rect x="11.5" y="11.5" width="7" height="5" rx="1.5" /></>;
    case "power":
      return <><path d="M12 4v7.5" /><path d="M7.2 7a7 7 0 1 0 9.6 0" /></>;
    case "refresh":
      return <><path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3" /><path d="M19.5 4v4.5H15" /></>;
    case "search":
      return <><circle cx="10.5" cy="10.5" r="6" /><path d="m15 15 5 5" /></>;
    case "trend":
      return <><path d="m4 16 5-5 3.5 3.5L20 7" /><path d="M14.5 7H20v5.5" /></>;
    case "warning":
      return <><path d="M10.3 4.9a2 2 0 0 1 3.4 0l7.1 12.3a2 2 0 0 1-1.7 3H4.9a2 2 0 0 1-1.7-3Z" /><path d="M12 9.5v4.5M12 17.2v.1" /></>;
    case "window":
      return <><rect x="3" y="5" width="18" height="14" rx="3.5" /><path d="M3 9.5h18" /></>;
    case "xmark":
      return <path d="m7.5 7.5 9 9M16.5 7.5l-9 9" />;
  }
}

export function Icon({
  name,
  size = 16,
  label,
  strokeWidth = 1.8,
}: {
  readonly name: IconName;
  readonly size?: number;
  readonly label?: string;
  readonly strokeWidth?: number;
}) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      role={label === undefined ? undefined : "img"}
      aria-hidden={label === undefined ? true : undefined}
      aria-label={label}
      style={{ strokeWidth }}
    >
      {iconPaths(name)}
    </svg>
  );
}

function ServiceMark({ provider }: { readonly provider: Provider }) {
  if (provider === "codex") {
    return (
      <svg className="service-mark" viewBox="0 0 24 24" aria-hidden="true">
        <path d="m6.5 8 4.5 4-4.5 4M13 16.5h5" />
      </svg>
    );
  }

  return (
    <svg className="service-mark" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 12 19.58 15.14M12 12l2.37 5.73M12 12l-3.14 7.58M12 12l-5.73 2.37M12 12 4.42 8.86M12 12 9.63 6.27M12 12l3.14-7.58M12 12l5.73-2.37" />
    </svg>
  );
}

export function ServiceBadge({
  provider,
  size = "regular",
  dimmed = false,
}: {
  readonly provider: Provider;
  readonly size?: "compact" | "regular" | "large";
  readonly dimmed?: boolean;
}) {
  return (
    <span className="service-badge" data-provider={provider} data-size={size} data-dimmed={dimmed}>
      <ServiceMark provider={provider} />
    </span>
  );
}
