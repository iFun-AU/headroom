/**
 * @file SettingsGroup.tsx
 * @description Reusable labelled settings rows and grouped material.
 */
import { Children, type ReactNode } from "react";

export function SettingsGroup({ heading, foot, children }: { readonly heading?: string; readonly foot?: ReactNode; readonly children: ReactNode }) {
  return (
    <section className="settings-group">
      {heading === undefined ? null : <h2>{heading}</h2>}
      <div className="settings-group__rows">{Children.toArray(children)}</div>
      {foot === undefined ? null : <footer>{foot}</footer>}
    </section>
  );
}

export function SettingsRow({
  label,
  description,
  leading,
  children,
}: {
  readonly label: string;
  readonly description?: ReactNode;
  readonly leading?: ReactNode;
  readonly children?: ReactNode;
}) {
  return (
    <div className="settings-row">
      {leading}
      <span className="settings-row__label"><span>{label}</span>{description === undefined ? null : <small>{description}</small>}</span>
      {children}
    </div>
  );
}
