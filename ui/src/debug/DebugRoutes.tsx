/**
 * @file DebugRoutes.tsx
 * @description Development-only visual routes; this module is absent from production bundles.
 */
import { useEffect, useState } from "react";
import { NeonBar } from "../components/NeonBar";
import { ProviderCard } from "../components/ProviderCard";
import { ThresholdLegend } from "../components/ThresholdLegend";
import { useRoute } from "../hooks/useRoute";
import {
  MOCK_BRIDGE_STATUS,
  MOCK_CARD_STATES,
  MOCK_CODEX_DETECTION,
  MOCK_HISTORIES,
  MOCK_READ_ONLY_SETTINGS,
  MOCK_SETTINGS_STATE,
  MOCK_SNAPSHOT,
  MOCK_SPARKLINES,
  mockProviderUsage,
} from "../mocks";
import { Dashboard } from "../screens/Dashboard";
import { Onboarding } from "../screens/Onboarding";
import { Popover } from "../screens/Popover";
import { Settings, type SettingsSection } from "../screens/Settings";
import { Widget } from "../screens/Widget";

const RESET_AT = 1_790_132_400;

function DemoBar({
  name,
  percent,
  accent,
  note,
  resetPending = false,
}: {
  readonly name: string;
  readonly percent: number;
  readonly accent: "claude" | "codex";
  readonly note: string;
  readonly resetPending?: boolean;
}) {
  return (
    <div className="demo-bar-row">
      <strong>{name}</strong>
      <NeonBar
        percent={percent}
        accent={accent}
        label={name.includes("Weekly") ? "weekly limit" : "5-hour limit"}
        resetsAt={resetPending ? null : RESET_AT}
        resetPending={resetPending}
      />
      <span>{note}</span>
    </div>
  );
}

function DemoScreen() {
  return (
    <main className="demo-screen">
      <section className="demo-panel glass dense" aria-labelledby="neon-title">
        <div className="demo-heading">
          <h1 id="neon-title">Neon progress bar</h1>
          <p>The hero element. Glow scales with value; thresholds override the service accent.</p>
        </div>
        <div className="demo-bars">
          <DemoBar name="Claude · 0%" percent={0} accent="claude" note="empty track" />
          <DemoBar name="Claude · 30%" percent={30} accent="claude" note="accent · low bloom" />
          <DemoBar name="Warning · 76%" percent={76} accent="codex" note="amber · icon + word" />
          <DemoBar name="Critical · 91%" percent={91} accent="claude" note="magenta-red · icon + word" />
          <DemoBar name="Limit · 100%" percent={100} accent="codex" note="pulse 1.8s + halo" />
          <DemoBar name="Reset pending" percent={0} accent="claude" note="Reset — waiting for new data" resetPending />
        </div>
      </section>
    </main>
  );
}

function AccessibilityScreen() {
  const sampleUsage = mockProviderUsage("claude", { state: "connected" }, 62, 78);
  return (
    <main className="accessibility-demo">
      <section className="accessibility-panel glass dense accessibility-panel--transparency" aria-labelledby="transparency-title">
        <header><h1 id="transparency-title">Reduce Transparency</h1><p>Glass becomes a solid tinted panel; glow and accents are unchanged.</p></header>
        <div className="no-glass accessibility-card-preview">
          <ProviderCard usage={sampleUsage} provider="claude" />
        </div>
      </section>
      <section className="accessibility-panel glass dense accessibility-panel--motion accessibility-motion" aria-labelledby="motion-title">
        <header><h1 id="motion-title">Reduce Motion</h1><p>Honoured via prefers-reduced-motion. The limit state keeps its meaning without animating.</p></header>
        <NeonBar percent={100} accent="claude" label="5-hour limit" resetsAt={null} resetPending={false} />
        <dl className="accessibility-comparison">
          <div><dt>Bar fill</dt><dd>Appears at value with no spring</dd></div>
          <div><dt>Limit reached</dt><dd>Static ring and hourglass icon</dd></div>
          <div><dt>Loading</dt><dd>Static skeleton</dd></div>
          <div><dt>Countdown</dt><dd>Text updates without animated motion</dd></div>
        </dl>
      </section>
      <section className="accessibility-panel glass dense accessibility-panel--contrast" aria-labelledby="contrast-title">
        <header><h1 id="contrast-title">Contrast over any wallpaper</h1><p>Text never sits on the thinnest glass.</p></header>
        <ul>
          <li>Primary and secondary labels sit on card glass or denser material.</li>
          <li>Secondary labels retain readable contrast in both appearances.</li>
          <li>Accent numbers use text tokens, not the raw neon fills.</li>
          <li>Desktop captions and the widget use dense glass.</li>
        </ul>
      </section>
      <section className="accessibility-panel glass dense accessibility-panel--colour" aria-labelledby="colour-title">
        <header><h1 id="colour-title">Never colour alone</h1><p>The same four states in greyscale: percentage, icon and wording still tell them apart.</p></header>
        <div className="accessibility-greyscale">
          <DemoBar name="Normal" percent={52} accent="codex" note="Normal" />
          <DemoBar name="Warning" percent={80} accent="codex" note="Warning" />
          <DemoBar name="Critical" percent={95} accent="codex" note="Critical" />
          <DemoBar name="Limit reached" percent={100} accent="codex" note="Limit reached" />
        </div>
      </section>
    </main>
  );
}

function CardsScreen() {
  return (
    <main className="cards-demo">
      <ProviderCard usage={MOCK_SNAPSHOT.claude} provider="claude" sparkline={MOCK_SPARKLINES.claude} peakLabel="1 PM" />
      <ProviderCard usage={MOCK_SNAPSHOT.codex} provider="codex" sparkline={MOCK_SPARKLINES.codex} peakLabel="2 PM" />
    </main>
  );
}

function StatesScreen() {
  return (
    <main className="states-demo">
      {MOCK_CARD_STATES.map((state) => (
        <div className="states-demo__item" key={state.title}>
          <span className="glass pop">{state.title}</span>
          <ProviderCard
            usage={state.usage}
            provider={state.provider}
            sparkline={state.usage === null || state.usage.windows.length === 0 ? [] : MOCK_SPARKLINES[state.provider]}
            peakLabel={state.provider === "claude" ? "1 PM" : "2 PM"}
            showThresholdNotice={state.notice ?? false}
            bridgeStatus={state.bridgeStatus ?? null}
          />
        </div>
      ))}
      <div className="states-demo__item">
        <span className="glass pop">Threshold legend</span>
        <ThresholdLegend />
      </div>
    </main>
  );
}

const WEEKLY_ONLY_CODEX_SNAPSHOT = {
  ...MOCK_SNAPSHOT,
  codex: { ...MOCK_SNAPSHOT.codex, windows: MOCK_SNAPSHOT.codex.windows.filter((window) => window.kind.kind === "weekly") },
};

function WidgetGallery() {
  return (
    <main className="widget-gallery">
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Pill" />
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Pill" forceHover />
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Stack" />
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Stack" forceHover />
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Mini" />
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Mini" forceHover />
      <Widget snapshot={WEEKLY_ONLY_CODEX_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Pill" />
      <Widget snapshot={WEEKLY_ONLY_CODEX_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Stack" />
      <Widget snapshot={WEEKLY_ONLY_CODEX_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Pill" windowsOverride="FiveHour" />
      <Widget snapshot={WEEKLY_ONLY_CODEX_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Stack" windowsOverride="FiveHour" />
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Pill" windowsOverride="Weekly" />
      <Widget snapshot={MOCK_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Stack" windowsOverride="Weekly" />
      <Widget snapshot={WEEKLY_ONLY_CODEX_SNAPSHOT} fixtureSettings={MOCK_SETTINGS_STATE} variantOverride="Mini" windowsOverride="Both" forceHover />
    </main>
  );
}

function settingsSection(hash: string): SettingsSection {
  if (hash.endsWith("display")) return "Display";
  if (hash.endsWith("alerts")) return "Alerts";
  if (hash.endsWith("refresh")) return "Refresh";
  if (hash.endsWith("diagnostics")) return "Diagnostics";
  return "Accounts";
}

export default function DebugRoutes() {
  const { route, navigate } = useRoute();
  const [debugHash, setDebugHash] = useState(window.location.hash);
  useEffect(() => {
    const read = () => { setDebugHash(window.location.hash); };
    window.addEventListener("hashchange", read);
    return () => { window.removeEventListener("hashchange", read); };
  }, []);
  switch (debugHash) {
    case "#/popover":
      return <Popover snapshot={MOCK_SNAPSHOT} />;
    case "#/cards":
      return <CardsScreen />;
    case "#/states":
      return <StatesScreen />;
    case "#/widgets":
      return <WidgetGallery />;
    case "#/settings":
    case "#/settings-display":
    case "#/settings-alerts":
    case "#/settings-refresh":
    case "#/settings-diagnostics":
      return <Settings snapshot={MOCK_SNAPSHOT} navigateOverview={() => { navigate("Overview"); }} fixtureState={MOCK_SETTINGS_STATE} fixtureBridge={MOCK_BRIDGE_STATUS} fixtureDetection={MOCK_CODEX_DETECTION} initialSection={settingsSection(debugHash)} />;
    case "#/settings-readonly":
      return <Settings snapshot={MOCK_SNAPSHOT} navigateOverview={() => { navigate("Overview"); }} fixtureState={MOCK_READ_ONLY_SETTINGS} fixtureBridge={MOCK_BRIDGE_STATUS} fixtureDetection={MOCK_CODEX_DETECTION} />;
    case "#/onboarding":
    case "#/onboarding-1":
      return <Onboarding navigate={navigate} fixtureState={MOCK_SETTINGS_STATE} fixtureBridge={MOCK_BRIDGE_STATUS} fixtureDetection={MOCK_CODEX_DETECTION} initialStep={1} />;
    case "#/onboarding-2":
      return <Onboarding navigate={navigate} fixtureState={MOCK_SETTINGS_STATE} fixtureBridge={MOCK_BRIDGE_STATUS} fixtureDetection={MOCK_CODEX_DETECTION} initialStep={2} />;
    case "#/onboarding-3":
      return <Onboarding navigate={navigate} fixtureState={MOCK_SETTINGS_STATE} fixtureBridge={MOCK_BRIDGE_STATUS} fixtureDetection={MOCK_CODEX_DETECTION} initialStep={3} />;
    case "#/demo":
      return <DemoScreen />;
    case "#/accessibility":
      return <AccessibilityScreen />;
    default:
      if (route === "Overview" || route === "Claude" || route === "Codex") {
        return <Dashboard route={route} snapshot={MOCK_SNAPSHOT} navigate={navigate} histories={MOCK_HISTORIES} bridgeStatus={MOCK_BRIDGE_STATUS} />;
      }
      return <DemoScreen />;
  }
}
