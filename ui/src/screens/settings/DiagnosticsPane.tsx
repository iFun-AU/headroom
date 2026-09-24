/** @file DiagnosticsPane.tsx @description Source health, log access, and build identity. */
import { useEffect, useState } from "react";
import type { SourceHealth } from "../../bindings/SourceHealth";
import type { SourceKind } from "../../bindings/SourceKind";
import type { UsageSnapshot } from "../../bindings/UsageSnapshot";
import { UpdatedAgo } from "../../components/Countdown";
import { SettingsGroup, SettingsRow } from "../../components/SettingsGroup";
import { StatusChip } from "../../components/StatusBadge";
import { getAppVersion, isTauriRuntime, revealLogs } from "../../ipc";

const SOURCE_NAMES: Readonly<Record<SourceKind, string>> = { codexAppServer: "Codex app-server", codexRollout: "Codex session files", claudeStatusline: "Claude status line", claudeOAuth: "Claude OAuth", claudeLocalLogs: "Claude local logs" };

function sourceStatus(source: SourceHealth): { readonly tone: "ok" | "warning" | "neutral"; readonly text: string } {
  switch (source.status.state) {
    case "connected": return { tone: "ok", text: "Connected" };
    case "degraded": return { tone: "warning", text: "Degraded" };
    case "stale": return { tone: "neutral", text: "Stale" };
    case "notConfigured": return { tone: "neutral", text: "Not configured" };
    case "authExpired": return { tone: "warning", text: "Sign-in expired" };
    case "unsupported": return { tone: "neutral", text: "Unsupported" };
    case "error": return { tone: "warning", text: "Error" };
  }
}

function uniqueSources(snapshot: UsageSnapshot | null): readonly SourceHealth[] {
  if (snapshot === null) return [];
  const sources = new Map<SourceKind, SourceHealth>();
  for (const source of [...snapshot.codex.sources, ...snapshot.claude.sources]) sources.set(source.source, source);
  return [...sources.values()];
}

export function DiagnosticsPane({ snapshot }: { readonly snapshot: UsageSnapshot | null }) {
  const [version, setVersion] = useState<string | null>(import.meta.env.DEV ? "1.0.0" : null);
  useEffect(() => { if (isTauriRuntime()) void getAppVersion().then(setVersion).catch(() => undefined); }, []);
  const sources = uniqueSources(snapshot);
  return (
    <>
      <SettingsGroup heading="Sources">
        {sources.length === 0 ? <SettingsRow label="Waiting for source health…" /> : sources.map((source) => { const status = sourceStatus(source); return <SettingsRow key={source.source} label={SOURCE_NAMES[source.source]} description={<UpdatedAgo timestamp={source.lastSuccess} />}><StatusChip tone={status.tone}>{status.text}</StatusChip></SettingsRow>; })}
      </SettingsGroup>
      <SettingsGroup heading="About">
        <SettingsRow label="Logs" description="~/Library/Logs/dev.headroom.app · last 7 days"><button className="capsule-button ctl" onClick={() => { if (isTauriRuntime()) void revealLogs().catch(() => undefined); }}>Reveal Logs</button></SettingsRow>
        <SettingsRow label="Version"><span className="settings-value">{version === null ? "Loading…" : `${version} · local build`}</span></SettingsRow>
      </SettingsGroup>
    </>
  );
}
