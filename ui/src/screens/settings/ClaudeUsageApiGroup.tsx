/** @file ClaudeUsageApiGroup.tsx @description Opt-in toggle and warning for Claude's unofficial OAuth usage API (D-023). */
import type { Settings } from "../../bindings/Settings";
import { ServiceBadge } from "../../components/Icon";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { Switch } from "../../components/Switch";

export function ClaudeUsageApiGroup({ settings, disabled, save }: { readonly settings: Settings; readonly disabled: boolean; readonly save: (settings: Settings) => void }) {
  return (
    <SettingsGroup heading="Claude usage API" foot={<>Unofficial and may stop working without notice. Headroom reads Claude Code’s sign-in from your Keychain (read-only) and checks your limits every few minutes. macOS will ask to allow access to <span className="mono">Claude Code-credentials</span>; choose Always Allow. Works with the Claude desktop app and IDEs.</>}>
      <SettingsRow label="Limits from Claude’s servers" description="Off by default" leading={<ServiceBadge provider="claude" size="compact" />}>
        <Switch checked={settings.claudeOauthEnabled} label="Limits from Claude’s servers" disabled={disabled} onChange={(claudeOauthEnabled) => { save({ ...settings, claudeOauthEnabled }); }} />
      </SettingsRow>
    </SettingsGroup>
  );
}
