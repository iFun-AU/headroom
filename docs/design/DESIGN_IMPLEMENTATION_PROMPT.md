# Prompt: Build the Headroom UI to match the design

> **How to use:** give this file to the coding agent at the start of milestone **M7 (UI)** in `docs/DEVELOPMENT.md`. The agent needs read access to this repo.
> **Author:** AI Assistant · **Version:** 3.0 · **Date:** 2026-09-23 · **Design:** canvas version 4 (29 artboards) · **Spec:** `DEVELOPMENT.md` v2.2
> **Changes in 3.0:** the design now follows `DEVELOPMENT.md` exactly:
> - the Claude / Codex tabs with scope-labelled token charts are back
> - Settings and Onboarding are routes inside the main window
> - the menu bar uses template icons + a title
> - widget hover stays inside the window bounds
> - every §11.4 edge state is drawn
>
> The v2.0 scope overrides are withdrawn.

---

## 1. Role and goal

You are building the React UI of **Headroom**, a macOS 26 menu bar app for Claude and Codex plan usage. `docs/DEVELOPMENT.md` defines what the app does, meaning data, types, IPC, routes and behavior. `docs/design/` defines how it looks. Your job is to make each screen match its design artboard while using only the types and commands in the spec.

Work in small steps. After each component, compare it with its screenshot at the same size, in dark **and** light mode, before moving on.

---

## 2. Precedence

| Topic | Source that wins |
|---|---|
| Data, types, IPC commands and events, routes, windows, settings fields, behavior, security | `docs/DEVELOPMENT.md` |
| Layout, spacing, radii, color, materials, glow, typography, motion, copy tone | `docs/design/` + this prompt |
| Anything this prompt doesn't settle | Apply `DEVELOPMENT.md` §0.1: pick the simplest option that keeps the drawn look and the specified behavior, log it in `docs/DECISIONS.md`, and continue |

---

## 3. The design package

| Path | Use it for |
|---|---|
| `preview/index.html` | Browsable gallery. Each artboard opens as a full-size HTML page with working hover tooltips. |
| `screenshots/*.png` | Pixel reference (1×). |
| `design-tokens.css` | The only token source (§5.1). |
| `source/gen.py` | Exact numbers for every component: `bar()`, `usage()`, `hourly_chart()`, `daily_chart()`, `chart_card()`, `stat()`, `sparkline()`, `template_icon()`, `status_item()`, `popover()`, `widget_*()`, `settings_pane()`, `onboarding_scene()`, `state_card()`, `ICONS`. |
| `canvas/` | Raw canvas source. Reference only; never import it into the app. |

### Artboard → spec → code

| Screenshots | Spec | Build |
|---|---|---|
| `Main.png`, `OverviewLight.png` | §11.4 Overview (`#/`) | `Dashboard.tsx`, `ProviderCard.tsx` (with the "Today" sparkline) |
| `DetailClaude{Dark,Light}.png`, `DetailCodex{Dark,Light}.png` | §11.4 detail (`#/claude`, `#/codex`), §7.5 series | `HourlyChart.tsx`, `DailyChart.tsx`, `StatTile.tsx` |
| `MenuBar{Dark,Light}.png` | §10.2 tray + popover | `Popover.tsx`; 3 template PNGs. The left panel is a reference, not UI. |
| `Widget{Dark,Light}.png` | §10.2 widget, §11.4 | `Widget.tsx` (Pill, Stack, Mini + hover) |
| `Settings{Accounts,Display,Alerts,Refresh,Diagnostics}{Dark,Light}.png`, `SettingsReadOnlyDark.png` | §9.3, §10.3, §11.4 Settings route (`#/settings`) | `Settings.tsx` |
| `Onboarding{Dark,Light}.png` | §11.4 Onboarding route (`#/onboarding`) | `Onboarding.tsx` |
| `States{Dark,Light}.png` | §11.4 edge states | `ProviderCard.tsx`, `StatusBadge.tsx`, `Countdown.tsx` |
| `Components{Dark,Light}.png` | §11.2 | The `#/demo` route (T7.1) must reproduce this sheet |
| `Tokens.png`, `Accessibility.png` | §11.1, §5 below | `styles/*.css`; acceptance checks |

---

## 4. Decisions the spec leaves to the design

**D1 — Toolbar per route.** Dashboard routes show the segmented control **Overview · Claude · Codex** in the center, and on the right "Updated … ago", Refresh and Collapse-to-widget. The Settings and Onboarding routes show a title in the center ("Settings" / "Welcome to Headroom"). Settings adds a "‹ Overview" back button after the traffic lights; Onboarding shows "Step n of 3" + dots on the right. The toolbar root has `data-tauri-drag-region`, and its buttons and segmented control do not (§10.2).

**D2 — Token numbers.** Show compact figures (`16.9M`, `842k`, `0`) with tabular numerals. Axis ticks are `0 / 10M / 20M` (pick a round max ≥ 1.1 × the peak). Tooltips read `{n} tokens` / `{hour or day}`.

**D3 — Chart footers.** Every chart shows `series.scopeLabel` with an info icon, plus ` · as of {time}` when `series.observedAt` is older than 5 min (§11.4). The Codex `Account` daily series labels its last column by date (e.g. "Sep 22"), and its header note says "tokens per UTC day" (§7.5).

**D4 — Stat row (detail tab).**
- **Peak hour:** the max hourly bucket, captioned `{n} tokens · this Mac`.
- **7-day average:** tokens per day, captioned with the scope.
- **At this pace:** from `History.projection`:
  - `hitsLimitAt` set → value `~{projected}% by reset`, caption `Limit reached ≈ {Thu 4:00 PM}`, warn styling + triangle.
  - Otherwise → `~{projected}% by reset` / `Stays under the limit at this pace`.
  - `None` → hide the tile's value, with the caption `Not enough data yet`.

**D5 — Menu bar.** The icon is 3 template PNGs drawn from `template_icon()`: 18×18 box, outer ring r 7, inner r 3.4, stroke 1.8, fixed sweeps; warning = triangle badge, critical = dot badge. Export @1x and @2x. The title is plain text with the highest %. No color anywhere in the menu bar.

**D6 — Popover** is exactly 340×420 with radius 18. Its content must fit without scrolling. The footer has no keyboard shortcuts, because the spec defines none.

**D7 — Widget hover** never draws outside the window:
- **Pill:** the controls row (close · opacity slider 40–100 % · expand) replaces the bars.
- **Stack:** the controls row replaces the divider.
- **Mini:** shows `C 62% · X 48%` inline.

**D8 — Plan label** renders `Claude {plan}` / `ChatGPT {plan}`, and is hidden when `plan` is `null`.

**D9 — Service marks** (spark and `>_`) are placeholders until the owner supplies approved logos.

**D10 — Settings copy.** Use the artboard text for labels and footnotes. The bridge effectiveness chip shows `Confirmed` (green), `Unverified` (grey) or `Likely overridden` (amber) from `BridgeStatus.effective`.

---

## 5. Translating the mockups into React + CSS

### 5.1 Tokens and materials
- Split `design-tokens.css` into `ui/src/styles/tokens.css`, `glass.css` and `neon.css` (§11.1). Use the design's variable names only.
- `--gk` scales every glow (1 dark, 0.38 light). Numbers in an accent color use `--*-text` (≥ 4.5:1). Bars use the raw accents.
- WKWebView needs `-webkit-backdrop-filter` next to `backdrop-filter`.
- The `.no-glass` class (§10.2) reuses the Reduce Transparency styles (`--solid-win`, `--solid-card`).

### 5.2 Don't copy from the mockups
- **The wallpaper, blurred waves, fake menu bar and drawn traffic lights.** Window glass is the native plugin (§10.2); CSS can't blur the desktop. Window roots stay transparent, and only `.card/.tile/.ctl/.tipbox` are CSS layers.
- The sample numbers. Everything comes from `UsageSnapshot`, `History` and `SettingsState`.
- `aria-hidden` on bars. Bars get `role="progressbar"`, `aria-valuenow/min/max` and an `aria-label` (§11.2).
- The status-item reference panel and the "caption chips" on the widget board.

### 5.3 NeonBar (the hero)
Follow §11.2 and `bar()`. The layers are track, reflection (h ≥ 8), gradient fill, white core, specular head (h ≥ 8), two-layer bloom, and a halo + pulse at 100 %. The fill **animates `width`**. Sizes: 14 (cards), 8 (popover, detail header), 6 + 4 (widget, no head or reflection), 3 (mini). Hover tooltip: `{n}% of 5-hour limit` / `Window {start} – {end}` / `About {100−n}% left`.

### 5.4 Layout numbers
- **Main window 720×520** (min 560×420). Toolbar 56 tall; about 78 pt left inset for the native traffic lights.
- **Overview:** content padding `0 20 20`, two cards with a 16 gap, card radius 22 and padding `18 20`. Each card is header → Session → Weekly → "Today · tokens on this Mac" sparkline (42 tall).
- **Detail:**
  - Header, 44 tall: badge 40, name 20/700, plan chip, two 168-pt mini bars.
  - Charts row, 252 tall: hourly card flex 1.5, daily card flex 1, card radius 20.
  - Stat row, 96 tall: 3 tiles, radius 16.
- **Settings route:** sidebar card 172 wide (radius 16), items 32 tall with 22-pt colored icon squares. Groups have radius 14 and 44-pt rows. Sections: Accounts · Display · Alerts · Refresh · Diagnostics. The read-only banner sits above the pane, and the pane is disabled (0.5 opacity, not focusable).
- **Onboarding route:** centered column with a 56-pt icon tile, 20/700 title, 13 `--label2` body, a grouped box, and buttons bottom-right (secondary capsules + one `--accent` primary).
- **Tooltips:** `.tipbox`, radius 12, padding `9 11`; 160 ms fade + 4 pt rise.

### 5.5 Motion
| Element | Spec | Reduced motion |
|---|---|---|
| Bar fill on first render | `cubic-bezier(.3,1.5,.5,1)`, 1.2 s, 80 ms stagger | appears at value (≤ 0.2 s fade) |
| Bar value change | `width` 600 ms, same curve | none |
| Chart bars | rise 0.9 s, 25 ms stagger | none |
| 100 % pulse | 1.8 s brightness + halo | static halo + hourglass icon |
| Skeleton | shimmer 1.6 s | static |
| Countdown, "Updated Xs ago" | text every 1 s (one `useNow` per window) | unchanged |

---

## 6. Task order (maps onto M7)

| Task | Build | Done when it matches |
|---|---|---|
| T7.1 | tokens / glass / neon CSS, `NeonBar`, `#/demo` | the neon-bar panel of `Components*.png` |
| T7.2 | `ProviderCard`, `Countdown`, `StatusBadge`, `Popover` | `Main.png` cards, `States*.png`, `MenuBar*.png` popover |
| T7.3 | `HourlyChart`, `DailyChart`, `StatTile`, `Dashboard`, hash router, scope labels | `Main/OverviewLight.png`, `Detail*.png` |
| T7.4 | `Widget`, `Settings` route, `Onboarding` route | `Widget*.png`, `Settings*.png`, `Onboarding*.png` |
| T7.5 | every §11.4 edge state, dark + light, reduced motion and transparency | `States*.png`, `SettingsReadOnlyDark.png`, `Accessibility.png` |

For every task: read the `gen.py` function, build it against the generated `ui/src/bindings/` types, then run the checks:
```bash
npm --prefix ui run typecheck && npm --prefix ui run lint
```
Finally, compare screenshots in both color schemes: spacing within ±2 pt, radii identical, every color from a token.

---

## 7. Accessibility (hard requirements)
1. **Never color alone.** The percentage is always visible; at ≥ 75 % there's an icon + `aria-label` word. `#/demo` under `filter: grayscale(1)` must still distinguish the states (`Accessibility.png`).
2. **Reduce Motion:** the `prefers-reduced-motion` rules in the tokens file + §5.5.
3. **Reduce Transparency:** `prefers-reduced-transparency` → solid panels (`.no-glass` covers a failed glass plugin). ⚠ VERIFY by hand that WKWebView on macOS 26 honors the query. If it doesn't, **don't** add native code or dependencies. Record it as a known limitation in `docs/DECISIONS.md` and put the check on the handover checklist.
4. **Contrast:** text only on card, tile, popover or tooltip layers. `--label2` for secondary text, `--*-text` for accent numbers.
5. **Controls:** real `<button>`/`<input>` elements, `aria-label` on icon-only buttons, visible focus, and a logical Tab order.

---

## 8. Definition of done (UI)
- [ ] Every artboard in §3 has a matching screen, route or state in dark and light.
- [ ] `#/demo` reproduces `Components*.png`.
- [ ] No hard-coded color, radius or duration outside `styles/`.
- [ ] No wallpaper, fake chrome or sample data shipped.
- [ ] §7 checks pass. `tsc` and ESLint are clean.
- [ ] Deviations and ⚠ VERIFY results are logged in `docs/DECISIONS.md`.
