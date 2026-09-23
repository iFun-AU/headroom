/**
 * @file Onboarding.tsx
 * @description Three-step first-run flow for Codex, Claude updates, and notifications.
 */
import { useEffect, useState } from "react";
import type { BridgeStatus } from "../bindings/BridgeStatus";
import type { CodexDetection } from "../bindings/CodexDetection";
import type { Route } from "../bindings/Route";
import type { SettingsState } from "../bindings/SettingsState";
import { Icon, ServiceBadge, type IconName } from "../components/Icon";
import { RouteToolbar } from "../components/RouteToolbar";
import { SettingsGroup, SettingsRow } from "../components/SettingsGroup";
import { StatusChip } from "../components/StatusBadge";
import { useBridgeStatus } from "../hooks/useBridgeStatus";
import { useSettings } from "../hooks/useSettings";
import { completeOnboarding, detectCodex, isTauriRuntime, pickPath, requestNotificationAccess } from "../ipc";

interface OnboardingProps {
  readonly navigate: (route: Route) => void;
  readonly fixtureState?: SettingsState;
  readonly fixtureDetection?: CodexDetection;
  readonly fixtureBridge?: BridgeStatus;
  readonly initialStep?: 1 | 2 | 3;
}

const STEP_COPY: Readonly<Record<1 | 2 | 3, { readonly icon: IconName; readonly title: string; readonly body: string }>> = {
  1: { icon: "search", title: "Find Codex", body: "How Is It shows your Codex session and weekly limits from the Codex CLI on this Mac." },
  2: { icon: "bell", title: "Real-time Claude updates", body: "Claude Code can report your limits after every response through its status line." },
  3: { icon: "bell", title: "Notifications", body: "Get a heads-up before you run out, and when a limit resets." },
};

function StepHeader({ step }: { readonly step: 1 | 2 | 3 }) {
  const copy = STEP_COPY[step];
  return <header className="onboarding-heading"><span><Icon name={copy.icon} size={28} /></span><h1>{copy.title}</h1><p>{copy.body}</p></header>;
}

function StepDots({ step }: { readonly step: 1 | 2 | 3 }) {
  return <div className="onboarding-step"><span>Step {String(step)} of 3</span><span aria-hidden="true">{[1, 2, 3].map((item) => <i data-active={item === step} key={item} />)}</span></div>;
}

function ClaudeExplanation() {
  const items: readonly [IconName, string][] = [
    ["gear", "Adds a status line command to ~/.claude/settings.json. Any status line you already use keeps running after ours."],
    ["info", "While a custom status line is set, Claude Code hides most footer keyboard hints."],
    ["warning", "A project or organization setting can override it. If updates stop arriving, How Is It tells you."],
    ["refresh", "You can turn it off at any time in Settings → Accounts; your previous status line is restored."],
  ];
  return <section className="onboarding-explanation card">{items.map(([icon, text]) => <p key={text}><Icon name={icon} size={15} /><span>{text}</span></p>)}</section>;
}

export function Onboarding({ navigate, fixtureState, fixtureDetection, fixtureBridge, initialStep = 1 }: OnboardingProps) {
  const [step, setStep] = useState<1 | 2 | 3>(initialStep);
  const [detection, setDetection] = useState<CodexDetection | null>(fixtureDetection ?? null);
  const [error, setError] = useState<string | null>(null);
  const settingsState = useSettings(fixtureState);
  const bridge = useBridgeStatus(fixtureBridge);
  const settings = settingsState.state?.settings;
  useEffect(() => { setStep(initialStep); }, [initialStep]);
  const runDetection = () => {
    if (!isTauriRuntime()) return;
    setDetection(null);
    setError(null);
    void detectCodex().then(setDetection).catch(() => { setError("Codex detection failed. You can choose it manually."); });
  };
  useEffect(() => { if (fixtureDetection === undefined) runDetection(); }, [fixtureDetection]);
  const browse = () => {
    if (!isTauriRuntime() || settings === undefined) return;
    void pickPath("File", "CodexBinary").then((path) => { if (path !== null) void settingsState.save({ ...settings, codexPath: path }); }).catch(() => { setError("The path picker closed unexpectedly."); });
  };
  const finish = async () => {
    try {
      if (isTauriRuntime()) await completeOnboarding();
      else if (settings !== undefined) await settingsState.save({ ...settings, onboardingCompleted: true });
      navigate("Overview");
    } catch {
      setError("Onboarding could not be completed. Try again.");
    }
  };
  const allowAndFinish = async () => {
    try { if (isTauriRuntime()) await requestNotificationAccess(); } finally { await finish(); }
  };

  return (
    <main className="onboarding-root glass">
      <RouteToolbar title="Welcome to How Is It" right={<StepDots step={step} />} />
      <div className="onboarding-content">
        <StepHeader step={step} />
        {step === 1 ? <SettingsGroup><SettingsRow label="Codex CLI" description={detection?.path ?? settings?.codexPath ?? "Automatic discovery"} leading={<ServiceBadge provider="codex" size="regular" />}><StatusChip tone={detection === null || detection.path === null ? "neutral" : "ok"}>{detection?.path === null ? "Not found" : detection === null ? "Detecting…" : `Found${detection.version === null ? "" : ` · ${detection.version}`}`}</StatusChip></SettingsRow><SettingsRow label="Not the right one?"><button className="capsule-button ctl" onClick={runDetection}>Detect Again</button><button className="capsule-button ctl" onClick={browse}>Browse…</button></SettingsRow><footer>Limits are read through <span className="mono">codex app-server</span>. Your Codex sign-in stays with Codex.</footer></SettingsGroup> : null}
        {step === 2 ? <ClaudeExplanation /> : null}
        {step === 3 ? <SettingsGroup foot="You can change these in Settings → Alerts."><SettingsRow label="75% and 90% used" description="One alert per threshold per window" leading={<span className="settings-icon--warn"><Icon name="warning" size={18} /></span>} /><SettingsRow label="Limit reached and reset" description="So you know when capacity is back" leading={<span className="settings-icon--crit"><Icon name="hourglass" size={18} /></span>} /></SettingsGroup> : null}
        {error === null && bridge.error === null && settingsState.error === null ? null : <div className="settings-error" role="status">{error ?? bridge.error ?? settingsState.error}</div>}
        <footer className="onboarding-actions">
          {step === 1 ? null : <button className="capsule-button ctl" onClick={() => { setStep(step === 3 ? 2 : 1); }}>Back</button>}
          {step === 2 ? <button className="capsule-button ctl" onClick={() => { setStep(3); }}>Skip</button> : null}
          {step === 3 ? <button className="capsule-button ctl" onClick={() => { void finish(); }}>Not Now</button> : null}
          {step === 1 ? <button className="primary-button" onClick={() => { setStep(2); }}>Continue</button> : null}
          {step === 2 ? <button className="primary-button" disabled={bridge.busy} onClick={() => { void bridge.setInstalled(true).then((installed) => { if (installed) setStep(3); }); }}>Enable</button> : null}
          {step === 3 ? <button className="primary-button" onClick={() => { void allowAndFinish(); }}>Allow Notifications</button> : null}
        </footer>
      </div>
    </main>
  );
}
