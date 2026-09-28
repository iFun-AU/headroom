/** @file ClaudeCliGroup.tsx @description Explicit manual Claude Code usage refresh and outcome. */
import { useState } from "react";
import { ServiceBadge } from "../../components/Icon";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { commandErrorMessage, isTauriRuntime, refreshClaudeCli } from "../../ipc";

export function ClaudeCliGroup({ disabled }: { readonly disabled: boolean }) {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const refresh = () => {
    if (!isTauriRuntime() || busy) return;
    setBusy(true);
    setMessage(null);
    setFailed(false);
    void refreshClaudeCli().then(() => { setMessage("Claude usage updated."); }).catch((error: unknown) => {
      setFailed(true);
      setMessage(commandErrorMessage(error));
    }).finally(() => { setBusy(false); });
  };
  return (
    <SettingsGroup heading="Claude manual refresh" foot="Requires Claude Code installed and signed in. Briefly runs /usage in the background, then closes it. No model prompt is sent. The main Refresh button uses this when the usage API is off.">
      <SettingsRow label="Refresh with Claude Code" description="Session and weekly limits" leading={<ServiceBadge provider="claude" size="compact" />}>
        <button className="capsule-button ctl" aria-label="Refresh with Claude Code" disabled={disabled || busy} onClick={refresh}>{busy ? "Refreshing…" : "Refresh"}</button>
      </SettingsRow>
      {message === null ? null : <div className={failed ? "settings-error" : "settings-foot"} role="status">{message}</div>}
    </SettingsGroup>
  );
}
