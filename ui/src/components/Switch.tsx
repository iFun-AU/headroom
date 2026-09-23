/**
 * @file Switch.tsx
 * @description Accessible macOS-style binary control.
 */
export function Switch({
  checked,
  label,
  disabled = false,
  onChange,
}: {
  readonly checked: boolean;
  readonly label: string;
  readonly disabled?: boolean;
  readonly onChange: (checked: boolean) => void;
}) {
  return (
    <button
      className="switch"
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => { onChange(!checked); }}
    >
      <span className="switch__knob" />
    </button>
  );
}
