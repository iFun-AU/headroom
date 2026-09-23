/**
 * @file RouteToolbar.tsx
 * @description Native-frame toolbar shared by settings and onboarding routes.
 */
import type { ReactNode } from "react";
import { Icon } from "./Icon";

export function RouteToolbar({
  title,
  backLabel,
  onBack,
  right,
}: {
  readonly title: string;
  readonly backLabel?: string;
  readonly onBack?: () => void;
  readonly right?: ReactNode;
}) {
  return (
    <header className="route-toolbar" data-tauri-drag-region>
      <div className="route-toolbar__left">
        <span className="route-toolbar__traffic-space" aria-hidden="true" />
        {onBack === undefined ? null : <button className="capsule-button ctl" onClick={onBack}><Icon name="back" size={14} />{backLabel ?? "Back"}</button>}
      </div>
      <strong>{title}</strong>
      <div className="route-toolbar__right">{right}</div>
    </header>
  );
}
