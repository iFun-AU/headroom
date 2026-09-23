/**
 * @file StatTile.tsx
 * @description Compact detail statistic with an optional service or warning tone.
 */
import type { Provider } from "../bindings/Provider";
import { Icon, type IconName } from "./Icon";

export function StatTile({
  icon,
  label,
  value,
  caption,
  tone,
  warning = false,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly value: string | null;
  readonly caption: string;
  readonly tone?: Provider | "warning";
  readonly warning?: boolean;
}) {
  return (
    <section className="stat-tile tile" data-tone={tone ?? "default"}>
      <header><Icon name={icon} size={14} /><span className="cap">{label}</span></header>
      {value === null ? null : (
        <strong className="stat-tile__value num neon">
          {warning ? <Icon name="warning" size={18} label="Warning" strokeWidth={2.1} /> : null}
          {value}
        </strong>
      )}
      <small>{caption}</small>
    </section>
  );
}
