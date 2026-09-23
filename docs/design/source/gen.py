#!/usr/bin/env python3
"""
@file gen.py
@description Generates the "How Is It" Design canvas: one .dc.html per artboard plus
             project/canvas.json. Shared building blocks (neon bar, glass card, charts,
             menu bar icon) are Python functions so every artboard stays consistent;
             theming is done with CSS custom properties (.t-dark / .t-light).
"""
import datetime
import json
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.join(HERE, "canvas", "project")

# ---------------------------------------------------------------- stylesheet
CSS = r"""
body{margin:0;background:#0a0b16}
.scene,.scene *{box-sizing:border-box}
.scene{position:relative;overflow:hidden;font-family:-apple-system,BlinkMacSystemFont,'SF Pro Text','SF Pro',system-ui,sans-serif;color:var(--label);-webkit-font-smoothing:antialiased;font-size:13px;line-height:1.3}
.scene button{font:inherit;color:inherit;border:0;background:none;padding:0;margin:0;cursor:pointer}
.t-dark{color-scheme:dark;--label:rgba(255,255,255,0.94);--label2:rgba(235,235,245,0.68);--label3:rgba(235,235,245,0.36);--sep:rgba(255,255,255,0.1);--glass-win:rgba(24,26,42,0.44);--glass-dense:rgba(16,18,32,0.58);--glass-card:rgba(12,14,26,0.5);--glass-tile:rgba(255,255,255,0.055);--glass-pop:rgba(22,24,38,0.72);--glass-edge:rgba(255,255,255,0.14);--card-edge:rgba(255,255,255,0.09);--hl-a:rgba(255,255,255,0.16);--hl-b:rgba(255,255,255,0.05);--spec:rgba(255,255,255,0.32);--spec-soft:rgba(255,255,255,0.1);--spec-low:rgba(255,255,255,0.05);--shadow:0 40px 90px rgba(0,0,0,0.5),0 10px 24px rgba(0,0,0,0.3);--track:rgba(255,255,255,0.08);--claude:#FF8A5B;--codex:#3CF2FF;--warn:#FFB020;--crit:#FF2D6F;--claude-text:#FF9A70;--codex-text:#62F5FF;--warn-text:#FFC247;--crit-text:#FF5C8E;--core:rgba(255,255,255,0.9);--gk:1;--control:rgba(255,255,255,0.08);--control-sel:rgba(255,255,255,0.2);--control-edge:rgba(255,255,255,0.1);--group:rgba(255,255,255,0.05);--sel:rgba(255,255,255,0.12);--accent:#0A84FF;--ok:#30D158;--mb:#ffffff;--mb-shadow:0 1px 2px rgba(0,0,0,0.35);--mb-sel:rgba(255,255,255,0.22);--sk1:rgba(255,255,255,0.06);--sk2:rgba(255,255,255,0.17);--solid-win:#1C1E2B;--solid-card:#252838;--blur:44px;--sat:190%;--switch-off:rgba(255,255,255,0.18)}
.t-light{color-scheme:light;--label:rgba(0,0,0,0.88);--label2:rgba(50,50,60,0.8);--label3:rgba(60,60,67,0.4);--sep:rgba(0,0,0,0.08);--glass-win:rgba(255,255,255,0.42);--glass-dense:rgba(255,255,255,0.62);--glass-card:rgba(255,255,255,0.68);--glass-tile:rgba(255,255,255,0.55);--glass-pop:rgba(255,255,255,0.78);--glass-edge:rgba(255,255,255,0.75);--card-edge:rgba(255,255,255,0.9);--hl-a:rgba(255,255,255,0.7);--hl-b:rgba(255,255,255,0.25);--spec:rgba(255,255,255,1);--spec-soft:rgba(255,255,255,0.9);--spec-low:rgba(0,0,0,0.04);--shadow:0 30px 70px rgba(50,40,90,0.22),0 8px 20px rgba(50,40,90,0.12);--track:rgba(0,0,0,0.07);--claude:#EE6431;--codex:#0099BA;--warn:#E08A00;--crit:#E0194F;--claude-text:#B9461C;--codex-text:#00708A;--warn-text:#955800;--crit-text:#BF1041;--core:rgba(255,255,255,0.6);--gk:0.38;--control:rgba(255,255,255,0.55);--control-sel:#ffffff;--control-edge:rgba(0,0,0,0.06);--group:rgba(255,255,255,0.6);--sel:rgba(0,0,0,0.07);--accent:#007AFF;--ok:#248A3D;--mb:rgba(0,0,0,0.85);--mb-shadow:none;--mb-sel:rgba(0,0,0,0.12);--sk1:rgba(0,0,0,0.05);--sk2:rgba(255,255,255,0.85);--solid-win:#F2F2F6;--solid-card:#FFFFFF;--blur:40px;--sat:180%;--switch-off:rgba(0,0,0,0.13)}
.wall-dark{background:radial-gradient(60% 55% at 16% 20%,rgba(255,72,140,0.55),rgba(0,0,0,0) 70%),radial-gradient(55% 60% at 84% 16%,rgba(64,86,255,0.7),rgba(0,0,0,0) 70%),radial-gradient(50% 55% at 74% 90%,rgba(255,140,56,0.55),rgba(0,0,0,0) 70%),radial-gradient(45% 50% at 10% 92%,rgba(0,200,180,0.45),rgba(0,0,0,0) 70%),radial-gradient(40% 40% at 50% 52%,rgba(150,60,255,0.4),rgba(0,0,0,0) 70%),linear-gradient(160deg,#140a30,#07162e 55%,#1c0a26)}
.wall-light{background:radial-gradient(60% 55% at 14% 18%,rgba(255,150,120,0.75),rgba(255,255,255,0) 70%),radial-gradient(55% 60% at 86% 14%,rgba(120,170,255,0.8),rgba(255,255,255,0) 70%),radial-gradient(50% 55% at 76% 92%,rgba(255,205,110,0.8),rgba(255,255,255,0) 70%),radial-gradient(45% 50% at 8% 92%,rgba(110,220,200,0.7),rgba(255,255,255,0) 70%),radial-gradient(40% 40% at 50% 55%,rgba(200,160,255,0.55),rgba(255,255,255,0) 70%),linear-gradient(160deg,#f6efe9,#e8eefb 55%,#f3ece6)}
.glass{background:linear-gradient(155deg,var(--hl-a) 0%,rgba(255,255,255,0) 30%,rgba(255,255,255,0) 72%,var(--hl-b) 100%),var(--glass-win);-webkit-backdrop-filter:blur(var(--blur)) saturate(var(--sat));backdrop-filter:blur(var(--blur)) saturate(var(--sat));border:1px solid var(--glass-edge);box-shadow:var(--shadow),inset 0 1px 0 var(--spec),inset 0 -1px 0 var(--spec-low),inset 0 0 20px rgba(255,255,255,0.04)}
.glass.dense{background:linear-gradient(155deg,var(--hl-a) 0%,rgba(255,255,255,0) 30%,rgba(255,255,255,0) 72%,var(--hl-b) 100%),var(--glass-dense)}
.glass.pop{background:linear-gradient(155deg,var(--hl-a) 0%,rgba(255,255,255,0) 30%),var(--glass-pop)}
.card{background:linear-gradient(170deg,var(--hl-b) 0%,rgba(255,255,255,0) 40%),var(--glass-card);border:1px solid var(--card-edge);box-shadow:inset 0 1px 0 var(--spec-soft),0 1px 2px rgba(0,0,0,0.08)}
.tile{background:var(--glass-tile);border:1px solid var(--card-edge);border-radius:16px;box-shadow:inset 0 1px 0 var(--spec-soft)}
.ctl{background:var(--control);border:1px solid var(--control-edge);box-shadow:inset 0 1px 0 var(--spec-soft)}
.ctl-sel{background:var(--control-sel);box-shadow:0 1px 3px rgba(0,0,0,0.18),inset 0 1px 0 var(--spec-soft)}
.tipbox{background:var(--glass-pop);-webkit-backdrop-filter:blur(20px) saturate(180%);backdrop-filter:blur(20px) saturate(180%);border:1px solid var(--glass-edge);box-shadow:0 12px 30px rgba(0,0,0,0.3),inset 0 1px 0 var(--spec-soft)}
.num{font-family:'SF Pro Rounded',ui-rounded,-apple-system,BlinkMacSystemFont,system-ui,sans-serif;font-variant-numeric:tabular-nums;letter-spacing:-0.01em}
.mono{font-family:'SF Mono',ui-monospace,Menlo,monospace;font-variant-numeric:tabular-nums}
.neon{text-shadow:0 0 calc(var(--gk) * 16px) color-mix(in srgb,currentColor 55%,transparent)}
.cap{font-size:11px;font-weight:600;letter-spacing:0.03em;color:var(--label2);text-transform:uppercase}
.switch{position:relative;width:40px;height:24px;border-radius:12px;background:var(--switch-off);flex-shrink:0;box-shadow:inset 0 1px 2px rgba(0,0,0,0.15)}
.switch.on{background:var(--accent)}
.switch .knob{position:absolute;top:2px;left:2px;width:26px;height:20px;border-radius:10px;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,0.3)}
.switch.on .knob{left:12px}
.menu-row:hover{background:var(--sel)}
.range{-webkit-appearance:none;appearance:none;width:100%;height:4px;border-radius:2px;background:linear-gradient(90deg,var(--accent) 0%,var(--accent) 75%,var(--switch-off) 75%,var(--switch-off) 100%);outline:none;margin:0}
.range::-webkit-slider-thumb{-webkit-appearance:none;width:22px;height:16px;border-radius:8px;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,0.35)}
.range::-moz-range-thumb{width:22px;height:16px;border-radius:8px;background:#fff;border:0}
.hov{position:relative}
.hov .tip{opacity:0;transform:translateY(4px);transition:opacity .16s ease,transform .16s ease;pointer-events:none;z-index:20}
.hov:hover .tip{opacity:1;transform:none}
@keyframes fillIn{from{transform:scaleX(0)}}
.nbf{transform-origin:left center;animation:fillIn 1.2s cubic-bezier(.3,1.5,.5,1) backwards}
@keyframes pulse{0%,100%{filter:brightness(1)}50%{filter:brightness(1.55) saturate(1.3)}}
.pulse{transform-origin:left center;animation:fillIn 1.2s cubic-bezier(.3,1.5,.5,1) backwards,pulse 1.8s ease-in-out 1.2s infinite}
@keyframes halo{0%,100%{opacity:.25}50%{opacity:.9}}
.halo{animation:halo 1.8s ease-in-out infinite}
@keyframes rise{from{transform:scaleY(0)}}
.hb{transform-origin:center bottom;animation:rise .9s cubic-bezier(.3,1.5,.5,1) backwards}
@keyframes shimmer{from{background-position:130% 0}to{background-position:-30% 0}}
.shim{background:linear-gradient(100deg,var(--sk1) 30%,var(--sk2) 50%,var(--sk1) 70%);background-size:250% 100%;animation:shimmer 1.6s linear infinite}
@keyframes draw{from{stroke-dashoffset:1000}}
.spark{stroke-dasharray:1000;animation:draw 1.6s ease-out backwards}
@media (prefers-reduced-motion:reduce){.nbf,.pulse,.halo,.hb,.shim,.spark{animation:none !important}}
@media (prefers-reduced-transparency:reduce){.glass,.glass.dense,.glass.pop{background:var(--solid-win) !important;-webkit-backdrop-filter:none !important;backdrop-filter:none !important}.card,.tile{background:var(--solid-card) !important}}
.rt.glass,.rt .glass{background:var(--solid-win) !important;-webkit-backdrop-filter:none !important;backdrop-filter:none !important}
.rt .card,.rt .tile{background:var(--solid-card) !important}
.rm .nbf,.rm .pulse,.rm .halo,.rm .hb,.rm .shim,.rm .spark{animation:none !important}
.gray{filter:grayscale(1)}
"""
# Holes use {{ }} — keep stray double braces out of the stylesheet.
CSS = CSS.replace("}}", "} }").replace("{{", "{ {").strip()

# ---------------------------------------------------------------- data
SVC = {
    "claude": {"name": "Claude", "plan": "Claude Max", "letter": "C"},
    "codex": {"name": "Codex", "plan": "ChatGPT Pro", "letter": "X"},
}
# Sample values for the mockups. Limits are account-wide %; token series always carry a scope label.
CLAUDE = {
    "s": 62, "w": 41, "s_reset": "Resets in 2h 14m", "w_reset": "Resets Mon 9:00 AM",
    "s_tip": ["62% of 5-hour limit", "Window 12:00 – 5:00 PM", "About 38% left"],
    "w_tip": ["41% of weekly limit", "Since Mon 9:00 AM", "About 59% left"],
    # Tokens per hour (millions), 3 PM yesterday .. 2 PM today; index 23 = current hour.
    "hourly": [3.1, 5.2, 7.8, 4.4, 1.2, 2.5, 3.9, 1.8, 0.4, 0, 0, 0, 0, 0, 0, 0, 0, 2.1, 6.3, 9.8, 8.7, 14.2, 16.9, 10.4],
    "hourly_scope": "Claude Code on this Mac",
    "daily": [48.2, 61.5, 12.3, 0.0, 35.6, 88.1, 71.2],
    "daily_labels": ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Today"],
    "daily_scope": "Claude Code on this Mac",
    "daily_asof": None,
    "peak": ("1 PM", "16.9M tokens · this Mac"),
    "avg": ("45.3M", "tokens per day · this Mac"),
    "projection": ("~96% by reset", "Stays under the limit at this pace", None),
}
CODEX = {
    "s": 48, "w": 78, "s_reset": "Resets in 3h 41m", "w_reset": "Resets Mon 11:40 AM",
    "s_tip": ["48% of 5-hour limit", "Window 1:27 – 6:27 PM", "About 52% left"],
    "w_tip": ["78% of weekly limit", "Since Mon 11:40 AM", "22% left"],
    "hourly": [8.4, 11.2, 6.1, 2.3, 0, 3.9, 9.7, 7.5, 3.1, 1.2, 0, 0, 0, 0, 0, 0, 0, 4.6, 9.1, 13.4, 15.8, 4.9, 14.1, 19.6],
    "hourly_scope": "Codex CLI on this Mac",
    "daily": [162.4, 148.9, 71.3, 39.5, 128.7, 284.1, 176.8],
    "daily_labels": ["Wed", "Thu", "Fri", "Sat", "Sun", "Mon", "Sep 22"],
    "daily_scope": "All Codex usage (account)",
    "daily_asof": "as of 2:31 PM",
    "daily_note": "tokens per UTC day",
    "peak": ("2 PM", "19.6M tokens · this Mac"),
    "avg": ("144.5M", "tokens per day · account"),
    "projection": ("~130% by reset", "Limit reached ≈ Thu 4:00 PM", "warn"),
}
DATA = {"claude": CLAUDE, "codex": CODEX}


def level(p):
    """0 normal, 1 warning (>=75), 2 critical (>=90), 3 limit (>=100)."""
    return 3 if p >= 100 else 2 if p >= 90 else 1 if p >= 75 else 0


def cvar(svc, p):
    """Colour token for a value: the service accent until thresholds override it."""
    return [svc, "warn", "crit", "crit"][level(p)]


LEVEL_ICON = {1: "warn", 2: "crit", 3: "hourglass"}
LEVEL_WORD = {1: "Warning", 2: "Critical", 3: "Limit reached"}


# ---------------------------------------------------------------- icons
ICONS = {
    "clock": '<circle cx="12" cy="12" r="8.5"></circle><path d="M12 7.5V12l3 2"></path>',
    "refresh": '<path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3"></path><path d="M19.5 4v4.5H15"></path>',
    "pip": '<rect x="3" y="5" width="18" height="14" rx="3.5"></rect><rect x="11.5" y="11.5" width="7" height="5" rx="1.5"></rect>',
    "gear": '<circle cx="12" cy="12" r="3"></circle><path d="M12 3.5v2.2M12 18.3v2.2M3.5 12h2.2M18.3 12h2.2M6 6l1.6 1.6M16.4 16.4L18 18M6 18l1.6-1.6M16.4 7.6L18 6"></path>',
    "power": '<path d="M12 4v7.5"></path><path d="M7.2 7a7 7 0 1 0 9.6 0"></path>',
    "xmark": '<path d="M7.5 7.5l9 9M16.5 7.5l-9 9"></path>',
    "expand": '<path d="M14 4.5h5.5V10M10 19.5H4.5V14M19.5 4.5L13.5 10.5M4.5 19.5l6-6"></path>',
    "warn": '<path d="M10.3 4.9a2 2 0 0 1 3.4 0l7.1 12.3a2 2 0 0 1-1.7 3H4.9a2 2 0 0 1-1.7-3Z"></path><path d="M12 9.5v4.5"></path><path d="M12 17.2v.1"></path>',
    "crit": '<path d="M8.6 3.5h6.8l5.1 5.1v6.8l-5.1 5.1H8.6l-5.1-5.1V8.6Z"></path><path d="M12 8v5"></path><path d="M12 16.3v.1"></path>',
    "hourglass": '<path d="M6.5 3.5h11M6.5 20.5h11"></path><path d="M8 3.5v2.8c0 1.4.7 2.6 1.9 3.4L12 11.2l2.1-1.5c1.2-.8 1.9-2 1.9-3.4V3.5"></path><path d="M8 20.5v-2.8c0-1.4.7-2.6 1.9-3.4l2.1-1.5 2.1 1.5c1.2.8 1.9 2 1.9 3.4v2.8"></path>',
    "wifi": '<path d="M3.5 9.5a12.5 12.5 0 0 1 17 0"></path><path d="M6.5 12.8a8 8 0 0 1 11 0"></path><path d="M9.5 16a3.5 3.5 0 0 1 5 0"></path><path d="M12 19.2v.1"></path>',
    "wifislash": '<path d="M3.5 9.5a12.5 12.5 0 0 1 17 0"></path><path d="M6.5 12.8a8 8 0 0 1 11 0"></path><path d="M9.5 16a3.5 3.5 0 0 1 5 0"></path><path d="M12 19.2v.1"></path><path d="M4.5 4.5l15 15"></path>',
    "link": '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"></path><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"></path>',
    "bell": '<path d="M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 2h-15Z"></path><path d="M10 20.5a2 2 0 0 0 4 0"></path>',
    "person": '<circle cx="9" cy="8.5" r="3.2"></circle><path d="M3.5 19a5.5 5.5 0 0 1 11 0"></path><circle cx="16.5" cy="9.5" r="2.6"></circle><path d="M15.5 14.2a4.6 4.6 0 0 1 5 4.8"></path>',
    "menubar": '<rect x="3" y="4.5" width="18" height="15" rx="3"></rect><path d="M3 8.5h18"></path>',
    "info": '<circle cx="12" cy="12" r="8.5"></circle><path d="M12 11v5.5"></path><path d="M12 7.8v.1"></path>',
    "trend": '<path d="M4 16.5l5-5 3.5 3L20 7"></path><path d="M15 7h5v5"></path>',
    "chevdown": '<path d="M7 9.5l5 5 5-5"></path>',
    "window": '<rect x="3" y="5" width="18" height="14" rx="3.5"></rect><path d="M3 9.5h18"></path>',
    "check": '<path d="M5 12.5l4.5 4.5L19 7.5"></path>',
    "opacity": '<circle cx="12" cy="12" r="8"></circle><path d="M12 4v16"></path><path d="M12 8h5.5M12 12h8M12 16h5.5"></path>',
    "cc": '<rect x="3" y="5.5" width="18" height="5.5" rx="2.75"></rect><circle cx="7.5" cy="8.25" r="1.2"></circle><rect x="3" y="13" width="18" height="5.5" rx="2.75"></rect><circle cx="16.5" cy="15.75" r="1.2"></circle>',
    "back": '<path d="M14 6.5L8.5 12l5.5 5.5"></path>',
    "chevron": '<path d="M10 6.5l5.5 5.5-5.5 5.5"></path>',
    "lock": '<rect x="5" y="10.5" width="14" height="10" rx="2.5"></rect><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"></path>',
    "flame": '<path d="M12 3.5c.8 2.6 5 4.8 5 9.5a5 5 0 0 1-10 0c0-2.1.9-3.7 2.1-4.7.2 1.7.9 2.7 1.9 3C10.9 9.2 11 6.2 12 3.5Z"></path>',
    "avg": '<path d="M4 19.5h16"></path><path d="M7.5 16v-5M12 16V7.5M16.5 16v-7"></path>',
    "search": '<circle cx="10.5" cy="10.5" r="6"></circle><path d="M15 15l5 5"></path>',
    "calendar": '<rect x="3.5" y="5" width="17" height="15" rx="3"></rect><path d="M3.5 9.5h17M8 3v4M16 3v4"></path>',
    "motion": '<path d="M4 12h3l2-5 3 10 2-5h6"></path>',
    "eye": '<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z"></path><circle cx="12" cy="12" r="3"></circle>',
    "contrast": '<circle cx="12" cy="12" r="8.5"></circle><path d="M12 3.5v17"></path><path d="M12 7h4M12 11h6M12 15h5"></path>',
}


def icon(name, size=16, color="currentColor", sw=1.8, label=None):
    """Inline stroke icon in the SF Symbols idiom. Decorative unless `label` is given."""
    a = f'role="img" aria-label="{label}"' if label else 'aria-hidden="true"'
    return (f'<svg width="{size}" height="{size}" viewBox="0 0 24 24" {a} style="flex-shrink:0;display:block;'
            f'fill:none;stroke:{color};stroke-width:{sw};stroke-linecap:round;stroke-linejoin:round;">{ICONS[name]}</svg>')


def glyph(svc, size, color=None):
    """Service marks (placeholders — swap in official marks): Claude spark, Codex prompt."""
    col = color or f"var(--{svc})"
    if svc == "claude":
        rays = ""
        for k in range(8):
            a = math.radians(k * 45 + 22.5)
            r = 8.2 if k % 2 == 0 else 6.2
            rays += f'<path d="M12 12L{12 + r * math.cos(a):.2f} {12 + r * math.sin(a):.2f}"></path>'
        body = rays
        sw = 2.4
    else:
        body = '<path d="M6.5 8l4.5 4-4.5 4"></path><path d="M13 16.5h5"></path>'
        sw = 2.3
    return (f'<svg width="{size}" height="{size}" viewBox="0 0 24 24" aria-hidden="true" style="display:block;fill:none;'
            f'stroke:{col};stroke-width:{sw};stroke-linecap:round;stroke-linejoin:round;">{body}</svg>')


def badge(svc, size=32, dim=False):
    """Service logo in a small tinted glass badge (continuous-corner square)."""
    r = round(size * 0.3)
    tint = "var(--label3)" if dim else f"var(--{svc})"
    return (f'<div style="width:{size}px;height:{size}px;border-radius:{r}px;display:flex;align-items:center;justify-content:center;flex-shrink:0;'
            f'background:linear-gradient(160deg,var(--hl-a),rgba(255,255,255,0) 60%),color-mix(in srgb,{tint} 16%,var(--glass-card));'
            f'border:1px solid color-mix(in srgb,{tint} 30%,var(--card-edge));box-shadow:inset 0 1px 0 var(--spec-soft);">'
            f'{glyph(svc, round(size * 0.6), "var(--label3)" if dim else None)}</div>')


def letter_badge(svc, size=22):
    """Minimal C / X badge used by the floating widget."""
    return (f'<div style="width:{size}px;height:{size}px;border-radius:{round(size * 0.32)}px;display:flex;align-items:center;justify-content:center;flex-shrink:0;'
            f'background:color-mix(in srgb,var(--{svc}) 18%,var(--glass-card));border:1px solid color-mix(in srgb,var(--{svc}) 35%,var(--card-edge));'
            f'font-size:{round(size * 0.5)}px;font-weight:700;color:var(--{svc}-text);">{SVC[svc]["letter"]}</div>')


# ---------------------------------------------------------------- neon bar
def tipbox(lines, style, always=False):
    """Glass tooltip. `lines[0]` is the headline value."""
    cls = "tipbox" if always else "tip tipbox"
    body = f'<div style="font-size:13px;font-weight:600;color:var(--label);">{lines[0]}</div>'
    body += "".join(f'<div>{t}</div>' for t in lines[1:])
    return (f'<div class="{cls}" style="position:absolute;{style}padding:9px 11px;border-radius:12px;display:flex;'
            f'flex-direction:column;gap:2px;font-size:11px;color:var(--label2);text-align:left;white-space:nowrap;">{body}</div>')


def bar(pct, c, w, h=14, dim=False, delay=0.0, tip=None, reflect=True, head=True, tip_always=False, shimmer=False):
    """
    Neon capsule progress bar: track, gradient fill with white core, specular head,
    two-layer bloom whose radius grows with the value, and a blurred reflection below.
    `c` is a colour token name (claude / codex / warn / crit). `w` is px or a CSS width.
    """
    r = h / 2
    p = max(0, min(100, pct))
    wcss = f"{w}px" if isinstance(w, (int, float)) else w
    cv = f"var(--{c})"
    pulse = pct >= 100 and not dim
    track = (f'<div style="position:absolute;left:0;top:0;right:0;bottom:0;border-radius:{r}px;background:var(--track);'
             f'box-shadow:inset 0 1px 2px rgba(0,0,0,0.25),inset 0 0 0 0.5px var(--sep);"></div>')
    if shimmer:
        return (f'<div aria-hidden="true" style="position:relative;width:{wcss};height:{h}px;flex-shrink:0;">'
                f'<div class="shim" style="position:absolute;left:0;top:0;right:0;bottom:0;border-radius:{r}px;"></div></div>')
    b1 = 3 + p * 0.07
    b2 = 8 + p * 0.2
    if c in ("warn", "crit"):
        b1, b2 = b1 * 1.2, b2 * 1.25
    parts = [track]
    if dim:
        fill_bg = "var(--label3)"
        glow = "none"
    else:
        fill_bg = (f"linear-gradient(90deg,color-mix(in srgb,{cv} 50%,transparent) 0%,{cv} 65%,"
                   f"color-mix(in srgb,{cv} 70%,#fff) 100%)")
        glow = (f"0 0 calc(var(--gk) * {b1:.1f}px) color-mix(in srgb,{cv} 85%,transparent),"
                f"0 0 calc(var(--gk) * {b2:.1f}px) color-mix(in srgb,{cv} 50%,transparent)")
        if reflect and p > 0:
            parts.append(f'<div style="position:absolute;left:{r}px;top:{h + 2}px;width:calc({p}% - {h}px);height:{max(4, round(h * 0.7))}px;'
                         f'border-radius:{r}px;background:linear-gradient(180deg,color-mix(in srgb,{cv} 50%,transparent),rgba(0,0,0,0));'
                         f'filter:blur({max(2, round(h / 4))}px);opacity:calc(0.25 + var(--gk) * 0.4);"></div>')
    if pulse:
        parts.append(f'<div class="halo" style="position:absolute;left:-3px;top:-3px;right:-3px;bottom:-3px;border-radius:{r + 3}px;'
                     f'box-shadow:0 0 calc(var(--gk) * 26px) {cv},0 0 0 1px color-mix(in srgb,{cv} 60%,transparent);"></div>')
    inner = ""
    if not dim and h >= 6 and p > 0:
        ch = max(1.5, round(h * 0.2, 1))
        inner += (f'<div style="position:absolute;left:{r}px;right:{round(h * 0.9)}px;top:50%;height:{ch}px;margin-top:-{ch / 2}px;'
                  f'border-radius:{ch}px;background:linear-gradient(90deg,rgba(255,255,255,0),var(--core));"></div>')
        if head and h >= 8:
            inner += (f'<div style="position:absolute;right:1px;top:1px;width:{h - 2}px;height:{h - 2}px;border-radius:50%;'
                      f'background:radial-gradient(circle,#fff 0%,color-mix(in srgb,{cv} 50%,#fff) 40%,rgba(255,255,255,0) 72%);"></div>')
    if p > 0:
        cls = "pulse" if pulse else "nbf"
        d = f"animation-delay:{delay}s;" if delay and not pulse else ""
        parts.append(f'<div class="{cls}" style="position:absolute;left:0;top:0;bottom:0;width:{p}%;min-width:{h}px;border-radius:{r}px;'
                     f'background:{fill_bg};box-shadow:{glow};{d}">{inner}</div>')
    if tip:
        if isinstance(w, (int, float)):
            left = min(max(p / 100 * w - 90, -10), w - 170)
            pos = f"left:{left:.0f}px;bottom:{h + 12}px;"
        else:
            pos = f"left:calc({p}% - 90px);bottom:{h + 12}px;"
        parts.append(tipbox(tip, pos, always=tip_always))
    cls = ' class="hov"' if tip else ""
    op = "opacity:0.55;" if dim else ""
    return f'<div{cls} aria-hidden="true" style="position:relative;width:{wcss};height:{h}px;flex-shrink:0;{op}">{"".join(parts)}</div>'


def level_icon(p, svc, size=16):
    lv = level(p)
    if not lv:
        return ""
    return icon(LEVEL_ICON[lv], size, f"var(--{cvar(svc, p)}-text)", 2.1, label=LEVEL_WORD[lv])


def usage(svc, kind, pct, reset, w, tip=None, delay=0.0, dim=False):
    """One labelled usage block for a card: title, big %, neon bar, reset line."""
    c = cvar(svc, pct)
    title = "Session" if kind == "s" else "Weekly"
    sub = "5-hour window" if kind == "s" else "7-day window"
    pc = "var(--label2)" if dim else f"var(--{c}-text)"
    neon = "" if dim else " neon"
    ico = "" if dim else level_icon(pct, svc)
    return (f'<div style="display:flex;flex-direction:column;gap:10px;">'
            f'<div style="display:flex;align-items:flex-end;justify-content:space-between;">'
            f'<div style="display:flex;flex-direction:column;gap:2px;"><span style="font-size:13px;font-weight:600;">{title}</span>'
            f'<span style="font-size:11px;color:var(--label2);">{sub}</span></div>'
            f'<div style="display:flex;align-items:center;gap:6px;color:{pc};">{ico}'
            f'<span class="num{neon}" style="font-size:30px;font-weight:600;line-height:1;">{pct}<span style="font-size:17px;">%</span></span></div></div>'
            f'{bar(pct, c, w, 14, dim=dim, delay=delay, tip=tip)}'
            f'<div style="display:flex;align-items:center;gap:6px;font-size:12px;color:var(--label2);margin-top:4px;">{icon("clock", 13)}<span>{reset}</span></div>'
            f'</div>')


# ---------------------------------------------------------------- projection


# ---------------------------------------------------------------- chrome
def waves(mode, w, h):
    """Soft, blurred wallpaper ribbons so the glass has something to refract."""
    if mode == "dark":
        cols = [("#ff4f9a", 0.5), ("#5b6cff", 0.55), ("#ff9a3c", 0.4)]
    else:
        cols = [("#ff9f80", 0.55), ("#8fb4ff", 0.6), ("#ffd27a", 0.6)]
    p1 = (f"M -100 {h * 0.72:.0f} C {w * 0.25:.0f} {h * 0.38:.0f}, {w * 0.55:.0f} {h * 1.02:.0f}, {w + 100} {h * 0.48:.0f} "
          f"L {w + 100} {h + 100} L -100 {h + 100} Z")
    p2 = (f"M -100 {h * 0.22:.0f} C {w * 0.3:.0f} {h * 0.02:.0f}, {w * 0.62:.0f} {h * 0.58:.0f}, {w + 100} {h * 0.12:.0f} "
          f"L {w + 100} {h * 0.3:.0f} C {w * 0.62:.0f} {h * 0.76:.0f}, {w * 0.3:.0f} {h * 0.2:.0f}, -100 {h * 0.42:.0f} Z")
    p3 = (f"M {w * 0.35:.0f} {h + 50} C {w * 0.45:.0f} {h * 0.6:.0f}, {w * 0.8:.0f} {h * 0.7:.0f}, {w + 100} {h * 0.62:.0f} "
          f"L {w + 100} {h + 50} Z")
    return (f'<svg width="{w}" height="{h}" viewBox="0 0 {w} {h}" aria-hidden="true" preserveAspectRatio="none" '
            f'style="position:absolute;left:0;top:0;filter:blur(36px);pointer-events:none;">'
            f'<path d="{p2}" style="fill:{cols[0][0]};opacity:{cols[0][1]};"></path>'
            f'<path d="{p1}" style="fill:{cols[1][0]};opacity:{cols[1][1]};"></path>'
            f'<path d="{p3}" style="fill:{cols[2][0]};opacity:{cols[2][1]};"></path></svg>')


def scene_open(mode, w, h, extra=""):
    return f'<div class="scene t-{mode} wall-{mode}" style="width:{w}px;height:{h}px;{extra}">' + waves(mode, w, h)




def battery():
    return ('<svg width="26" height="13" viewBox="0 0 26 13" aria-hidden="true" style="display:block;">'
            '<rect x="0.5" y="0.5" width="22" height="12" rx="3.5" style="fill:none;stroke:var(--mb);opacity:0.5;"></rect>'
            '<rect x="2.5" y="2.5" width="14" height="8" rx="2" style="fill:var(--mb);"></rect>'
            '<path d="M24 4.5v4" style="stroke:var(--mb);stroke-width:1.5;stroke-linecap:round;opacity:0.5;"></path></svg>')


def menubar(active=False, pct=62, state=0):
    """Transparent Tahoe menu bar with the How Is It status item."""
    left = "".join(f'<span style="font-weight:{700 if i == 0 else 500};">{t}</span>'
                   for i, t in enumerate(["Finder", "File", "Edit", "View", "Go", "Window", "Help"]))
    item = (f'<button aria-label="How Is It, highest usage {pct} percent" aria-expanded="{"true" if active else "false"}" '
            f'style="display:flex;align-items:center;">{status_item(pct, state, 1.0, "var(--mb)", active)}</button>')
    right = (f'{item}{battery()}{icon("wifi", 17, "var(--mb)", 2)}{icon("search", 16, "var(--mb)", 2)}{icon("cc", 17, "var(--mb)", 1.8)}'
             f'<span class="num" style="font-weight:500;">Wed Sep 23  2:46 PM</span>')
    return (f'<div style="position:absolute;left:0;top:0;right:0;height:30px;display:flex;align-items:center;justify-content:space-between;'
            f'padding:0 14px;font-size:13px;color:var(--mb);text-shadow:var(--mb-shadow);z-index:5;">'
            f'<div style="display:flex;gap:20px;align-items:center;">{left}</div>'
            f'<div style="display:flex;gap:14px;align-items:center;">{right}</div></div>')


def traffic(active=True):
    cols = ["#FF5F57", "#FEBC2E", "#28C840"] if active else ["var(--label3)"] * 3
    return ('<div style="display:flex;gap:8px;align-items:center;">' +
            "".join(f'<button aria-label="{l}" style="width:12px;height:12px;border-radius:50%;background:{c};'
                    f'box-shadow:inset 0 0 0 0.5px rgba(0,0,0,0.2);"></button>'
                    for c, l in zip(cols, ["Close", "Minimize", "Zoom"])) + "</div>")


def toolbar_buttons():
    return ('<div class="ctl" style="display:flex;border-radius:99px;padding:2px;">'
            f'<button aria-label="Refresh" style="width:32px;height:28px;display:flex;align-items:center;justify-content:center;border-radius:99px;color:var(--label);">{icon("refresh", 15)}</button>'
            f'<button aria-label="Collapse to floating widget" style="width:32px;height:28px;display:flex;align-items:center;justify-content:center;border-radius:99px;color:var(--label);">{icon("pip", 15)}</button>'
            '</div>')


def window(content, left=280, top=154, center=None, left_extra="", right=None, w=720, h=520, updated="Updated 12s ago"):
    """Main window 720x520 (native frame; the traffic lights are native, drawn here for context).
    Toolbar: segmented control (dashboard routes) or a title (Settings / Onboarding routes)."""
    right_html = right if right is not None else (
        f'<span class="num" style="font-size:12px;color:var(--label2);white-space:nowrap;">{updated}</span>{toolbar_buttons()}')
    return (f'<div class="glass" style="position:absolute;left:{left}px;top:{top}px;width:{w}px;height:{h}px;border-radius:26px;display:flex;flex-direction:column;">'
            f'<div style="height:56px;flex-shrink:0;display:flex;align-items:center;padding:0 12px 0 18px;gap:12px;">'
            f'<div style="flex:1;display:flex;align-items:center;gap:16px;">{traffic()}{left_extra}</div>{center or ""}'
            f'<div style="flex:1;display:flex;align-items:center;justify-content:flex-end;gap:10px;">{right_html}</div></div>'
            f'{content}</div>')


def card_header(svc, right="", dim=False, sub=None):
    s = SVC[svc]
    return (f'<div style="display:flex;align-items:center;gap:10px;">{badge(svc, 34, dim)}'
            f'<div style="display:flex;flex-direction:column;gap:1px;flex:1;min-width:0;"><span style="font-size:15px;font-weight:600;">{s["name"]}</span>'
            f'<span style="font-size:12px;color:var(--label2);">{sub or s["plan"]}</span></div>{right}</div>')


def overview_card(svc, s_reset):
    d = DATA[svc]
    inner_w = 290
    return (f'<section class="card" style="flex:1;min-width:0;border-radius:22px;padding:18px 20px;display:flex;flex-direction:column;justify-content:space-between;">'
            f'{card_header(svc, details_btn(svc))}'
            f'{usage(svc, "s", d["s"], s_reset, inner_w, d["s_tip"], 0.05)}'
            f'{usage(svc, "w", d["w"], d["w_reset"], inner_w, d["w_tip"], 0.15)}'
            f'{spark_section(svc, d["hourly"][9:], inner_w, d["peak"][0])}</section>')


# ---------------------------------------------------------------- page wrapper
STATIC_LOGIC = """class Component extends DCLogic {
  renderVals() {
    return {};
  }
}"""

LIVE_LOGIC = """class Component extends DCLogic {
  componentDidMount() {
    // One tick per second drives the "updated" label and the reset countdowns.
    this.timer = setInterval(() => {
      const t = (this.state && this.state.t) || 0;
      this.setState({ t: t + 1 });
    }, 1000);
  }
  componentWillUnmount() {
    clearInterval(this.timer);
  }
  renderVals() {
    const t = (this.state && this.state.t) || 0;
    const hm = (s) => {
      const h = Math.floor(s / 3600);
      const m = Math.floor((s % 3600) / 60);
      return h + 'h ' + String(m).padStart(2, '0') + 'm';
    };
    const hms = (s) => {
      const h = Math.floor(s / 3600);
      const m = Math.floor((s % 3600) / 60);
      const r = s % 60;
      return h + ':' + String(m).padStart(2, '0') + ':' + String(r).padStart(2, '0');
    };
    const ago = (12 + t) % 60;
    return {
      updated: ago === 0 ? 'Updated just now' : 'Updated ' + ago + 's ago',
      claudeReset: 'Resets in ' + hm(Math.max(0, 8049 - t)),
      codexReset: 'Resets in ' + hm(Math.max(0, 13269 - t)),
      limitClock: hms(Math.max(0, 5527 - t))
    };
  }
}"""


def page(title, body, w, h, logic=STATIC_LOGIC):
    props = json.dumps({"$preview": {"width": w, "height": h}})
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<style>
{CSS}
</style>
</helmet>
{body}
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{props}'>
{logic}
</script>
</body>
</html>
"""


# ---------------------------------------------------------------- scenes
W, H = 1280, 800




def overview_scene(mode, live):
    content = (f'<div style="flex:1;display:flex;gap:16px;padding:0 20px 20px;">'
               f'{overview_card("claude", "{{claudeReset}}" if live else CLAUDE["s_reset"])}'
               f'{overview_card("codex", "{{codexReset}}" if live else CODEX["s_reset"])}</div>')
    win = window(content, center=seg("Overview"), updated="{{updated}}" if live else "Updated 12s ago")
    return scene_open(mode, W, H) + menubar() + win + "</div>"


def chip(text, left, top):
    """Calm caption chip on the desktop (opaque enough to read over any wallpaper)."""
    return (f'<div class="glass pop" style="position:absolute;left:{left}px;top:{top}px;padding:5px 10px;border-radius:99px;'
            f'font-size:11px;font-weight:500;color:var(--label2);white-space:nowrap;box-shadow:none;">{text}</div>')


def pop_service(svc, s_pct, w_pct, s_reset, w_reset):
    s = SVC[svc]

    def row(label, pct, reset):
        c = cvar(svc, pct)
        return (f'<div style="display:flex;flex-direction:column;gap:1px;">'
                f'<div style="display:flex;align-items:center;gap:10px;">'
                f'<span style="width:54px;font-size:12px;color:var(--label2);">{label}</span>'
                f'<div style="flex:1;min-width:0;">{bar(pct, c, "100%", 8, reflect=False)}</div>'
                f'<span style="width:52px;display:flex;justify-content:flex-end;align-items:center;gap:4px;color:var(--{c}-text);">'
                f'{level_icon(pct, svc, 12)}<span class="num neon" style="font-size:13px;font-weight:700;">{pct}%</span></span></div>'
                f'<span style="margin-left:64px;font-size:11px;color:var(--label2);">{reset}</span></div>')

    return (f'<div style="display:flex;flex-direction:column;gap:6px;padding:6px 16px 8px;">'
            f'<div style="display:flex;align-items:center;gap:8px;">{badge(svc, 24)}<span style="font-size:13px;font-weight:600;">{s["name"]}</span>'
            f'<span style="font-size:11px;color:var(--label2);">{s["plan"]}</span></div>'
            f'{row("Session", s_pct, s_reset)}{row("Weekly", w_pct, w_reset)}</div>')


def menu_item(ic, text, hover=False):
    bg = "background:var(--sel);" if hover else ""
    return (f'<button class="menu-row" style="width:100%;height:26px;padding:0 10px;border-radius:8px;display:flex;align-items:center;gap:10px;{bg}">'
            f'<span style="color:var(--label2);">{icon(ic, 15)}</span><span style="flex:1;text-align:left;font-size:13px;">{text}</span></button>')


def popover(left, top):
    """Popover window: exactly 340x420, liquid-glass corner radius 18 (DEVELOPMENT.md §10.1/§10.2)."""
    head = (f'<div style="display:flex;align-items:center;justify-content:space-between;padding:8px 16px 0;">'
            f'<div style="display:flex;flex-direction:column;gap:1px;"><span style="font-size:13px;font-weight:700;">How Is It</span>'
            f'<span class="num" style="font-size:11px;color:var(--label2);">Updated 12s ago</span></div>'
            f'<button aria-label="Refresh" class="ctl" style="width:28px;height:28px;border-radius:99px;display:flex;align-items:center;justify-content:center;">{icon("refresh", 14)}</button></div>')
    sep = '<div style="height:1px;background:var(--sep);margin:0 16px;flex-shrink:0;"></div>'
    foot = (f'<div style="margin-top:auto;padding:6px;display:flex;flex-direction:column;gap:1px;">'
            f'{menu_item("window", "Open Dashboard", True)}{menu_item("pip", "Float Widget")}'
            f'{menu_item("gear", "Settings…")}<div style="height:1px;background:var(--sep);margin:2px 10px;"></div>'
            f'{menu_item("power", "Quit How Is It")}</div>')
    return (f'<div class="glass pop" style="position:absolute;left:{left}px;top:{top}px;width:340px;height:420px;border-radius:18px;display:flex;flex-direction:column;">'
            f'{head}{pop_service("claude", 62, 41, CLAUDE["s_reset"], CLAUDE["w_reset"])}{sep}'
            f'{pop_service("codex", 48, 78, CODEX["s_reset"], CODEX["w_reset"])}{sep}{foot}</div>')




def menubar_scene(mode):
    return scene_open(mode, W, H) + menubar(active=True) + statusitem_panel(80, 60) + popover(900, 36) + "</div>"


def widget_pill(left, top, cp, cw, xp, xw, hover=False):
    """Pill 280x72: the window IS the pill, so hover controls replace the bars inside its bounds."""
    def col(svc, sp, wp):
        cs, cwv = cvar(svc, sp), cvar(svc, wp)
        return (f'<div style="display:flex;align-items:center;gap:8px;">{letter_badge(svc)}'
                f'<div style="width:52px;display:flex;flex-direction:column;gap:7px;">{bar(sp, cs, 52, 6, reflect=False, head=False)}{bar(wp, cwv, 52, 4, reflect=False, head=False)}</div>'
                f'<div style="width:32px;display:flex;flex-direction:column;align-items:flex-end;gap:1px;">'
                f'<span class="num neon" style="font-size:12px;font-weight:700;color:var(--{cs}-text);">{sp}%</span>'
                f'<span class="num" style="font-size:10px;font-weight:600;color:var(--{cwv}-text);">{wp}%</span></div></div>')

    if hover:
        cb = "width:30px;height:30px;border-radius:50%;display:flex;align-items:center;justify-content:center;flex-shrink:0;"
        inner = (f'<button aria-label="Close widget" class="ctl" style="{cb}">{icon("xmark", 13, sw=2.2)}</button>'
                 f'<div style="flex:1;display:flex;align-items:center;gap:8px;padding:0 10px;">'
                 f'<span style="color:var(--label2);">{icon("opacity", 14)}</span>'
                 f'<input class="range" type="range" min="40" max="100" value="75" aria-label="Widget opacity">'
                 f'<span class="num" style="font-size:11px;color:var(--label2);width:28px;text-align:right;">75%</span></div>'
                 f'<button aria-label="Expand to dashboard" class="ctl" style="{cb}">{icon("expand", 13, sw=2.2)}</button>')
    else:
        inner = f'{col("claude", cp, cw)}<div style="width:1px;height:30px;background:var(--sep);"></div>{col("codex", xp, xw)}'
    return (f'<div class="glass" style="position:absolute;left:{left}px;top:{top}px;width:280px;height:72px;border-radius:36px;padding:0 14px;'
            f'display:flex;align-items:center;justify-content:space-between;">{inner}</div>')


def widget_stack(left, top, cp, cw, xp, xw, hover=False):
    def sec(svc, sp, wp):
        def r(lbl, p, h):
            c = cvar(svc, p)
            return (f'<div style="display:flex;align-items:center;gap:6px;"><span style="width:16px;font-size:10px;color:var(--label2);">{lbl}</span>'
                    f'{bar(p, c, 72, h, reflect=False, head=False)}'
                    f'<span style="width:38px;display:flex;justify-content:flex-end;align-items:center;gap:2px;color:var(--{c}-text);">{level_icon(p, svc, 10)}'
                    f'<span class="num" style="font-size:11px;font-weight:700;">{p}%</span></span></div>')
        return (f'<div style="display:flex;flex-direction:column;gap:8px;"><div style="display:flex;align-items:center;gap:6px;">{letter_badge(svc, 20)}'
                f'<span style="font-size:12px;font-weight:600;">{SVC[svc]["name"]}</span></div>{r("5h", sp, 6)}{r("wk", wp, 4)}</div>')
    if hover:
        cb = "width:24px;height:24px;border-radius:50%;display:flex;align-items:center;justify-content:center;flex-shrink:0;"
        mid = (f'<div style="display:flex;align-items:center;gap:6px;height:26px;">'
               f'<button aria-label="Close widget" class="ctl" style="{cb}">{icon("xmark", 11, sw=2.2)}</button>'
               f'<input class="range" type="range" min="40" max="100" value="75" aria-label="Widget opacity">'
               f'<button aria-label="Expand to dashboard" class="ctl" style="{cb}">{icon("expand", 11, sw=2.2)}</button></div>')
    else:
        mid = '<div style="height:1px;background:var(--sep);"></div>'
    return (f'<div class="glass" style="position:absolute;left:{left}px;top:{top}px;width:160px;height:180px;border-radius:24px;padding:14px;'
            f'display:flex;flex-direction:column;justify-content:space-between;">{sec("claude", cp, cw)}{mid}{sec("codex", xp, xw)}</div>')


def widget_mini(left, top, cp, xp, hover=False):
    """Mini 200x24: two thin lines; hover shows the values inline (the window has no room for a tooltip)."""
    if hover:
        inner = (f'<div style="display:flex;align-items:center;justify-content:center;gap:8px;font-size:11px;">'
                 f'<span class="num" style="font-weight:700;color:var(--claude-text);">C {cp}%</span>'
                 f'<span style="color:var(--label3);">·</span>'
                 f'<span class="num" style="font-weight:700;color:var(--codex-text);">X {xp}%</span></div>')
        just = "justify-content:center;"
    else:
        inner = (f'{bar(cp, cvar("claude", cp), 176, 3, reflect=False, head=False)}'
                 f'{bar(xp, cvar("codex", xp), 176, 3, reflect=False, head=False)}')
        just = "justify-content:center;gap:5px;"
    return (f'<div class="glass" aria-label="Claude session {cp} percent, Codex session {xp} percent" style="position:absolute;left:{left}px;top:{top}px;'
            f'width:200px;height:24px;border-radius:12px;padding:0 12px;display:flex;flex-direction:column;{just}">{inner}</div>')


def widget_scene(mode):
    snap = ('<div style="position:absolute;right:6px;top:42px;bottom:12px;width:2px;'
            'background:repeating-linear-gradient(180deg,var(--accent) 0px,var(--accent) 6px,rgba(0,0,0,0) 6px,rgba(0,0,0,0) 12px);opacity:0.85;"></div>')
    return (scene_open(mode, W, H) + menubar() +
            widget_pill(80, 96, 62, 41, 48, 78) + chip("Pill · 280×72 · top bar session, thin bar weekly", 80, 178) +
            widget_pill(80, 250, 62, 41, 48, 78, hover=True) + chip("Pill · hover: controls replace the bars inside the window", 80, 332) +
            widget_stack(480, 96, 62, 41, 48, 78) + chip("Stack · 160×180", 480, 286) +
            widget_stack(680, 96, 62, 41, 48, 78, hover=True) + chip("Stack · hover", 680, 286) +
            widget_stack(880, 96, 81, 52, 100, 91) + chip("Stack · warning + limit", 880, 286) +
            widget_mini(480, 380, 62, 48) + chip("Mini · 200×24", 480, 412) +
            widget_mini(720, 380, 62, 48, hover=True) + chip("Mini · hover shows values inline", 720, 412) +
            snap + widget_pill(1280 - 280 - 12, 560, 62, 41, 48, 78) + chip("Snaps to the screen edge · 12 pt inset", 1280 - 280 - 12, 642) +
            "</div>")


# ---------- settings
SIDEBAR = [("Accounts", "person", "#0A84FF"), ("Display", "menubar", "#BF5AF2"), ("Alerts", "bell", "#FF453A"),
           ("Refresh", "refresh", "#30B0C7"), ("Diagnostics", "info", "#8E8E93")]


def switch(on, label):
    return (f'<button class="switch{" on" if on else ""}" aria-pressed="{"true" if on else "false"}" aria-label="{label}">'
            f'<span class="knob"></span></button>')


def group(rows, heading=None, foot=None):
    h = f'<div style="font-size:12px;font-weight:600;color:var(--label2);padding:0 4px 6px;">{heading}</div>' if heading else ""
    body = ""
    for i, r in enumerate(rows):
        bt = "border-top:1px solid var(--sep);" if i else ""
        body += f'<div style="display:flex;align-items:center;gap:12px;padding:9px 14px;min-height:44px;{bt}">{r}</div>'
    f = f'<div style="font-size:11px;color:var(--label2);padding:6px 4px 0;line-height:1.4;">{foot}</div>' if foot else ""
    return f'<div>{h}<div style="border-radius:14px;background:var(--group);border:1px solid var(--card-edge);">{body}</div>{f}</div>'


def label_row(text, sub=None, lead=""):
    s = f'<span style="font-size:11px;color:var(--label2);">{sub}</span>' if sub else ""
    return (f'{lead}<div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:1px;">'
            f'<span style="font-size:13px;">{text}</span>{s}</div>')


def popup_btn(text):
    return (f'<button class="ctl" style="height:26px;padding:0 8px 0 12px;border-radius:99px;display:flex;align-items:center;gap:6px;font-size:12px;">'
            f'{text}{icon("chevdown", 12, "var(--label2)", 2)}</button>')


def capsule_btn(text):
    return f'<button class="ctl" style="height:28px;padding:0 14px;border-radius:99px;font-size:12px;font-weight:500;">{text}</button>'


def settings_pane(pane):
    if pane == "Accounts":
        codex = group([
            label_row("Codex CLI", None, badge("codex", 28)) + status_chip("ok", "Found · 0.154.0") + capsule_btn("Detect"),
            label_row("Path") + value_text("/opt/homebrew/bin/codex", True) + capsule_btn("Browse…"),
        ], "Codex", "Limits are read through <span class=\"mono\">codex app-server</span>. Your Codex sign-in stays with Codex.")
        claude = group([
            label_row("Real-time updates", "Status line bridge", badge("claude", 28)) + status_chip("ok", "Confirmed") + capsule_btn("Disable"),
            label_row("Previous status line", "Still runs after ours") + value_text("statusline.sh", True),
        ], "Claude", "Adds a status line command to <span class=\"mono\">~/.claude/settings.json</span>. Claude Code hides most footer "
                     "keyboard hints while a custom status line is set. A project or organization setting can override it.")
        return codex + claude
    if pane == "Display":
        general = group([label_row("Launch at login") + switch(True, "Launch at login"),
                         label_row("Show Dock icon", "Off keeps How Is It in the menu bar only") + switch(False, "Show Dock icon")], "General")
        widget = group([label_row("Show floating widget") + switch(True, "Show floating widget"),
                        label_row("Style") + popup_btn("Pill"),
                        label_row("Opacity") + '<div style="width:150px;"><input class="range" type="range" min="40" max="100" value="75" aria-label="Widget opacity"></div>'
                        + value_text("75%")],
                       "Floating widget", "The widget always stays on top. Drag it anywhere; it snaps to screen edges.")
        return general + widget
    if pane == "Alerts":
        rows = [label_row("75% used", "Approaching the limit", f'<span style="color:var(--warn-text);">{icon("warn", 18, sw=2)}</span>') + switch(True, "Notify at 75%"),
                label_row("90% used", "Almost out", f'<span style="color:var(--crit-text);">{icon("crit", 18, sw=2)}</span>') + switch(True, "Notify at 90%"),
                label_row("Limit reached", "100% of a session or weekly limit", f'<span style="color:var(--crit-text);">{icon("hourglass", 18, sw=2)}</span>') + switch(True, "Notify at 100%")]
        return (group(rows, "Notify me when a limit reaches", "Each alert fires once per window. Notifications respect Focus.") +
                group([label_row("When a limit resets", "After it had reached 90% or more",
                                 f'<span style="color:var(--label2);">{icon("refresh", 18, sw=2)}</span>') + switch(True, "Notify when a limit resets")], "Resets"))
    if pane == "Refresh":
        return (group([label_row("When active", "Any Claude or Codex activity in the last 10 minutes") + popup_btn("Every 2 minutes"),
                       label_row("When idle") + popup_btn("Every 10 minutes")], "Codex polling",
                      "Updates are pushed in real time whenever possible; polling is the safety net (minimum 1 and 2 minutes). "
                      "Claude is not polled in this version: it updates when Claude Code reports.") +
                group([label_row("Refresh now", "Reads every source immediately") + capsule_btn("Refresh Now")]))
    if pane == "Diagnostics":
        src = [("Codex app-server", "pid 48213 · 12s ago", "ok", "Connected"),
               ("Codex session files", "3s ago", "ok", "Connected"),
               ("Claude status line", "Bridge confirmed · 1m ago", "ok", "Connected"),
               ("Claude local logs", "Last activity 40s ago", "ok", "Connected")]
        rows = [label_row(n, s) + status_chip(k, t) for n, s, k, t in src]
        return (group(rows, "Sources") +
                group([label_row("Logs", "~/Library/Logs/dev.howisit.app · last 7 days") + capsule_btn("Reveal Logs"),
                       label_row("Version") + value_text("1.0.0 · local build")], "About"))
    raise ValueError(pane)


def settings_scene(mode, pane, read_only=False):
    """Settings is a route (#/settings) inside the main window, not a separate window."""
    items = ""
    for name, ic, colr in SIDEBAR:
        on = name == pane
        bg = "background:var(--sel);" if on else ""
        items += (f'<button aria-current="{"page" if on else "false"}" style="width:100%;height:32px;padding:0 8px;border-radius:9px;display:flex;align-items:center;gap:9px;{bg}">'
                  f'<span style="width:22px;height:22px;border-radius:6px;background:{colr};display:flex;align-items:center;justify-content:center;">{icon(ic, 14, "#fff", 2)}</span>'
                  f'<span style="font-size:13px;font-weight:{600 if on else 400};">{name}</span></button>')
    sidebar = (f'<nav class="card" aria-label="Settings sections" style="width:172px;flex-shrink:0;border-radius:16px;padding:8px;display:flex;flex-direction:column;gap:2px;">'
               f'{items}</nav>')
    banner = ""
    body = settings_pane(pane)
    if read_only:
        banner = (f'<div style="display:flex;align-items:center;gap:10px;padding:9px 12px;border-radius:12px;font-size:12px;'
                  f'color:var(--warn-text);background:color-mix(in srgb,var(--warn) 13%,transparent);border:1px solid color-mix(in srgb,var(--warn) 35%,transparent);">'
                  f'{icon("warn", 16, sw=2.1)}<span style="flex:1;font-weight:600;">Settings were created by a newer version of How Is It. Changes can’t be saved.</span>'
                  f'<button style="height:26px;padding:0 12px;border-radius:99px;background:var(--accent);color:#fff;font-size:12px;font-weight:600;">Reset Settings</button></div>')
        body = f'<div aria-disabled="true" style="display:flex;flex-direction:column;gap:16px;opacity:0.5;">{body}</div>'
    pane_html = (f'<div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:16px;overflow:hidden;">'
                 f'{banner}{body}</div>')
    content = f'<div style="flex:1;min-height:0;display:flex;gap:16px;padding:0 16px 16px;">{sidebar}{pane_html}</div>'
    center = '<span style="font-size:13px;font-weight:600;">Settings</span>'
    win = window(content, center=center, left_extra=back_btn(), right="")
    return scene_open(mode, W, H) + menubar() + win + "</div>"


# ---------- states
CARD_W, CARD_H = 332, 420


def notice(kind, text, ic):
    return (f'<div style="display:flex;align-items:center;gap:8px;padding:7px 10px;border-radius:10px;font-size:12px;font-weight:600;'
            f'color:var(--{kind}-text);background:color-mix(in srgb,var(--{kind}) 13%,transparent);border:1px solid color-mix(in srgb,var(--{kind}) 35%,transparent);">'
            f'{icon(ic, 15, sw=2.1)}<span>{text}</span></div>')


def state_card(kind, live=False, extra_cls=""):
    inner = 290
    base = (f'class="glass dense{extra_cls}" style="width:{CARD_W}px;height:{CARD_H}px;border-radius:24px;padding:18px 20px;'
            f'display:flex;flex-direction:column;gap:14px;flex-shrink:0;"')
    spark_c = f'<div style="margin-top:auto;">{spark_section("claude", CLAUDE["hourly"][9:], inner, "1 PM")}</div>'
    spark_x = f'<div style="margin-top:auto;">{spark_section("codex", CODEX["hourly"][9:], inner, "2 PM")}</div>'
    if kind == "normal":
        body = (card_header("claude", details_btn("claude")) +
                usage("claude", "s", 46, "Resets in 3h 02m", inner, ["46% of 5-hour limit", "Window 12:48 – 5:48 PM", "About 54% left"]) +
                usage("claude", "w", 32, "Resets Mon 9:00 AM", inner) + spark_c)
    elif kind == "warning":
        body = (card_header("codex", details_btn("codex")) + notice("warn", "Approaching session limit", "warn") +
                usage("codex", "s", 81, "Resets in 1h 48m", inner) + usage("codex", "w", 64, "Resets Mon 11:40 AM", inner) + spark_x)
    elif kind == "critical":
        body = (card_header("claude", details_btn("claude")) + notice("crit", "Almost out: 5% of session left", "crit") +
                usage("claude", "s", 95, "Resets in 0h 52m", inner) + usage("claude", "w", 88, "Resets Mon 9:00 AM", inner) + spark_c)
    elif kind == "limit":
        clock = "{{limitClock}}" if live else "1:32:07"
        body = (card_header("codex", details_btn("codex")) +
                f'<div style="display:flex;flex-direction:column;align-items:center;gap:6px;padding:4px 0 2px;">'
                f'<div style="position:relative;width:46px;height:46px;border-radius:50%;display:flex;align-items:center;justify-content:center;'
                f'color:var(--crit-text);background:color-mix(in srgb,var(--crit) 14%,transparent);">'
                f'<div class="halo" style="position:absolute;left:0;top:0;right:0;bottom:0;border-radius:50%;box-shadow:0 0 calc(var(--gk) * 22px) var(--crit),0 0 0 1.5px color-mix(in srgb,var(--crit) 60%,transparent);"></div>'
                f'{icon("hourglass", 22, sw=2)}</div>'
                f'<span style="font-size:14px;font-weight:700;">Session limit reached</span>'
                f'<span style="font-size:11px;color:var(--label2);">Resets in</span>'
                f'<span class="num neon" style="font-size:40px;font-weight:600;line-height:1;color:var(--crit-text);">{clock}</span>'
                f'<span style="font-size:12px;color:var(--label2);">Back at 4:18 PM</span></div>'
                f'<div style="display:flex;flex-direction:column;gap:8px;"><div style="display:flex;justify-content:space-between;align-items:center;">'
                f'<span style="font-size:13px;font-weight:600;">Session</span><span style="display:flex;align-items:center;gap:5px;color:var(--crit-text);">'
                f'{level_icon(100, "codex", 14)}<span class="num neon" style="font-size:17px;font-weight:700;">100%</span></span></div>'
                f'{bar(100, "crit", inner, 14)}</div>'
                f'<div style="margin-top:6px;display:flex;flex-direction:column;gap:8px;"><div style="display:flex;justify-content:space-between;align-items:center;">'
                f'<span style="font-size:13px;font-weight:600;">Weekly</span><span style="display:flex;align-items:center;gap:5px;color:var(--crit-text);">'
                f'{level_icon(91, "codex", 14)}<span class="num neon" style="font-size:17px;font-weight:700;">91%</span></span></div>'
                f'{bar(91, "crit", inner, 10, reflect=False)}<span style="font-size:12px;color:var(--label2);">Resets Mon 11:40 AM</span></div>')
    elif kind == "loading":
        def sk(w, h, r=6):
            return f'<div class="shim" style="width:{w};height:{h}px;border-radius:{r}px;flex-shrink:0;"></div>'

        def blk():
            return (f'<div style="display:flex;flex-direction:column;gap:10px;"><div style="display:flex;justify-content:space-between;align-items:flex-end;">'
                    f'<div style="display:flex;flex-direction:column;gap:6px;">{sk("64px", 12)}{sk("96px", 10)}</div>{sk("64px", 28, 8)}</div>'
                    f'{sk("100%", 14, 7)}{sk("130px", 10)}</div>')
        body = (f'<div style="display:flex;align-items:center;gap:10px;">{sk("34px", 34, 10)}'
                f'<div style="flex:1;display:flex;flex-direction:column;gap:6px;">{sk("80px", 13)}{sk("110px", 10)}</div>'
                f'<span style="font-size:11px;color:var(--label2);">Loading usage…</span></div>'
                f'{blk()}{blk()}<div style="margin-top:auto;display:flex;flex-direction:column;gap:8px;">{sk("60px", 10)}{sk("100%", 42, 10)}</div>')
    elif kind == "claude_nc":
        body = (card_header("claude", "", dim=True, sub="Not set up") +
                empty_state("bell", "Turn on real-time updates",
                            "How Is It gets Claude limits from Claude Code’s status line. Enable it once and your bars update after every response.",
                            "Enable Real-Time Updates", "What changes?"))
    elif kind == "codex_nc":
        body = (card_header("codex", "", dim=True, sub="Not found") +
                empty_state("search", "Codex CLI not found", "Install Codex CLI or set its path in Settings.", "Detect Again", "Choose Path…"))
    elif kind == "degraded":
        body = (card_header("codex", status_chip("warn", "Degraded")) +
                muted_row("info", "Codex app-server stopped; using session files") +
                usage("codex", "s", 48, CODEX["s_reset"], inner) + usage("codex", "w", 78, CODEX["w_reset"], inner))
    elif kind == "stale":
        body = (card_header("claude", status_chip("neutral", "Updated 14m ago")) +
                muted_row("clock", "No new data since 2:32 PM. Claude Code reports when you use it.") +
                usage("claude", "s", 62, "Resets in 2h 14m", inner, dim=True) +
                usage("claude", "w", 41, "Resets Mon 9:00 AM", inner, dim=True))
    elif kind == "single":
        body = (card_header("codex", details_btn("codex")) +
                muted_row("info", "No 5-hour limit on this plan") +
                usage("codex", "w", 3, "Resets Tue 7:37 AM", inner) + spark_x)
    elif kind == "reset":
        body = (card_header("claude", details_btn("claude")) +
                f'<div style="display:flex;flex-direction:column;gap:10px;">'
                f'<div style="display:flex;align-items:flex-end;justify-content:space-between;">'
                f'<div style="display:flex;flex-direction:column;gap:2px;"><span style="font-size:13px;font-weight:600;">Session</span>'
                f'<span style="font-size:11px;color:var(--label2);">5-hour window</span></div>'
                f'<span class="num" style="font-size:30px;font-weight:600;line-height:1;color:var(--label2);">0<span style="font-size:17px;">%</span></span></div>'
                f'{bar(0, "claude", inner, 14)}'
                f'<div style="display:flex;align-items:center;gap:6px;font-size:12px;color:var(--label2);margin-top:4px;">{icon("refresh", 13)}'
                f'<span>Reset — waiting for new data</span></div></div>' +
                usage("claude", "w", 41, "Resets Mon 9:00 AM", inner) + spark_c)
    elif kind == "overridden":
        body = (card_header("claude", status_chip("warn", "No updates")) +
                f'<div style="display:flex;gap:10px;padding:10px 12px;border-radius:10px;font-size:12px;line-height:1.45;color:var(--warn-text);'
                f'background:color-mix(in srgb,var(--warn) 13%,transparent);border:1px solid color-mix(in srgb,var(--warn) 35%,transparent);">'
                f'<span style="margin-top:1px;">{icon("warn", 15, sw=2.1)}</span><span><b>No updates received from Claude Code.</b> '
                f'A project or organization setting may override your status line, or your plan doesn’t report limits.</span></div>' +
                usage("claude", "s", 62, "Resets in 2h 14m", inner, dim=True) +
                f'<div style="margin-top:auto;display:flex;justify-content:flex-end;">{capsule_btn("Open Settings")}</div>')
    elif kind == "unsupported":
        body = (card_header("codex", "", sub="ChatGPT Pro") +
                empty_state("info", "Codex doesn’t report limits",
                            "This Codex version doesn’t support reading rate limits. Update Codex CLI, then retry.", None, "Retry"))
    elif kind == "auth":
        body = (card_header("claude", "", sub="Claude Max") +
                empty_state("lock", "Sign-in expired", "Open Claude Code to refresh sign-in.", None, "Retry"))
    else:  # thresholds legend
        rows = ""
        for rng, desc, p, svc, ic in [("0–74%", "Service accent, glow grows with use", 52, "claude", None),
                                       ("75–89%", "Amber + warning triangle", 80, "codex", "warn"),
                                       ("90–99%", "Magenta-red + octagon", 95, "claude", "crit"),
                                       ("100%", "Slow pulse + reset countdown", 100, "codex", "hourglass")]:
            c = cvar(svc, p)
            ico = icon(ic, 14, f"var(--{c}-text)", 2.1) if ic else '<span style="width:14px;"></span>'
            rows += (f'<div style="display:flex;flex-direction:column;gap:7px;">'
                     f'<div style="display:flex;align-items:center;gap:6px;">{ico}<span class="num" style="font-size:13px;font-weight:700;color:var(--{c}-text);">{rng}</span>'
                     f'<span style="font-size:12px;color:var(--label2);">{desc}</span></div>{bar(p, c, inner, 8, reflect=False)}</div>')
        body = (f'<div style="display:flex;flex-direction:column;gap:3px;"><span style="font-size:15px;font-weight:700;">Thresholds</span>'
                f'<span style="font-size:12px;color:var(--label2);">Applied to session and weekly independently.</span></div>{rows}'
                f'<div style="margin-top:auto;font-size:11px;color:var(--label2);line-height:1.45;padding-top:10px;border-top:1px solid var(--sep);">'
                f'Colour is never the only signal: the percentage is always shown and an icon appears from 75%.</div>')
    return f'<section {base}>{body}</section>'


STATES = [("normal", "Normal · 30–60%"), ("warning", "Warning · ≥ 75%"), ("critical", "Critical · ≥ 90%"), ("limit", "Limit reached · 100%"),
          ("loading", "Loading"), ("claude_nc", "Not configured · Claude"), ("codex_nc", "Not configured · Codex"),
          ("degraded", "Degraded · data still shown"), ("stale", "Stale · no poller in v1"), ("single", "Single-window plan"),
          ("reset", "Reset pending"), ("overridden", "Bridge likely overridden"), ("unsupported", "Unsupported"),
          ("auth", "Auth expired · OAuth, post-v1"), ("legend", "Thresholds")]
STATES_W, STATES_H = 1932, 1610


def states_scene(mode, live=True):
    cells = ""
    for kind, title in STATES:
        cells += f'<div style="display:flex;flex-direction:column;gap:12px;">{chip_inline(title)}{state_card(kind, live)}</div>'
    grid = (f'<div style="position:absolute;left:64px;top:64px;display:grid;grid-template-columns:repeat(5,minmax(0,1fr));'
            f'column-gap:36px;row-gap:48px;width:{STATES_W - 128}px;">{cells}</div>')
    return scene_open(mode, STATES_W, STATES_H) + grid + "</div>"


def chip_inline(text):
    return (f'<div class="glass pop" style="align-self:flex-start;padding:5px 12px;border-radius:99px;font-size:12px;font-weight:600;'
            f'color:var(--label);white-space:nowrap;box-shadow:none;">{text}</div>')


# ---------- component sheet
COMPONENTS_H = 1560

def panel(title, sub, inner, extra=""):
    return (f'<section class="glass dense" style="border-radius:28px;padding:24px 28px;display:flex;flex-direction:column;gap:18px;{extra}">'
            f'<div style="display:flex;flex-direction:column;gap:3px;"><span style="font-size:17px;font-weight:700;">{title}</span>'
            f'<span style="font-size:12px;color:var(--label2);">{sub}</span></div>{inner}</section>')


def marker(n, left, top):
    return (f'<div class="num" style="position:absolute;left:{left}px;top:{top}px;width:20px;height:20px;border-radius:50%;background:var(--label);'
            f'color:var(--solid-win);font-size:11px;font-weight:700;display:flex;align-items:center;justify-content:center;">{n}</div>')


def components_scene(mode):
    # 1. neon bar anatomy + states
    aw = 560
    anat = (f'<div style="position:relative;height:120px;padding-top:44px;">'
            f'{bar(64, "claude", aw, 24)}'
            f'{marker(1, 470, 8)}{marker(2, 90, 8)}{marker(3, 230, 8)}{marker(4, 344, 8)}{marker(5, 170, 88)}{marker(6, 280, 88)}'
            f'<div style="position:absolute;left:479px;top:28px;width:1px;height:16px;background:var(--label3);"></div>'
            f'<div style="position:absolute;left:99px;top:28px;width:1px;height:16px;background:var(--label3);"></div>'
            f'<div style="position:absolute;left:239px;top:28px;width:1px;height:26px;background:var(--label3);"></div>'
            f'<div style="position:absolute;left:353px;top:28px;width:1px;height:18px;background:var(--label3);"></div></div>')
    legend_items = [("1", "Track", "8% white (dark) · 7% black (light), inset shadow"),
                    ("2", "Fill", "accent 50% → 100% → 70% white tip"),
                    ("3", "Core", "white line, 20% of height, fades in"),
                    ("4", "Head", "specular dot at the leading edge"),
                    ("5", "Bloom", "two shadows: 3+0.07·p and 8+0.2·p px"),
                    ("6", "Reflection", "blurred fill on the glass below")]
    legend = '<div style="display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px 18px;">'
    for n, t, dsc in legend_items:
        legend += (f'<div style="display:flex;gap:8px;"><span class="num" style="font-size:12px;font-weight:700;width:14px;">{n}</span>'
                   f'<div style="display:flex;flex-direction:column;gap:1px;"><span style="font-size:12px;font-weight:600;">{t}</span>'
                   f'<span style="font-size:11px;color:var(--label2);line-height:1.35;">{dsc}</span></div></div>')
    legend += "</div>"
    rows_spec = [("Claude · 35%", 35, "claude", {}, "accent · bloom 5/15"),
                 ("Claude · 62%", 62, "claude", {}, "accent · bloom 7/20"),
                 ("Codex · 48%", 48, "codex", {}, "accent · bloom 6/18"),
                 ("Warning · 80%", 80, "codex", {}, "amber · bloom ×1.2"),
                 ("Critical · 95%", 95, "claude", {}, "magenta-red · bloom ×1.25"),
                 ("Limit · 100%", 100, "codex", {}, "pulse 1.8s + halo"),
                 ("Stale", 62, "claude", {"dim": True}, "grey fill, no glow, 55%"),
                 ("Reset pending", 0, "claude", {}, "0% + “Reset — waiting”"),
                 ("Loading", 0, "claude", {"shimmer": True}, "shimmer 1.6s")]
    rows = ""
    for name, p, svc, kw, spec in rows_spec:
        c = cvar(svc, p)
        dim = kw.get("dim")
        shim = kw.get("shimmer")
        pcol = "var(--label2)" if (dim or p == 0) else f"var(--{c}-text)"
        pct_txt = "" if shim else (f'<span style="display:flex;align-items:center;gap:4px;color:{pcol};">'
                                   f'{"" if dim else level_icon(p, svc, 13)}<span class="num" style="font-size:14px;font-weight:700;">{p}%</span></span>')
        rows += (f'<div style="display:flex;align-items:center;gap:12px;min-height:34px;">'
                 f'<span style="width:118px;font-size:12px;font-weight:600;">{name}</span>'
                 f'{bar(p, c, 330, 14, dim=bool(dim), shimmer=bool(shim))}'
                 f'<span style="width:64px;display:flex;justify-content:flex-end;">{pct_txt}</span>'
                 f'<span style="flex:1;font-size:11px;color:var(--label2);">{spec}</span></div>')
    sizes = '<div style="display:flex;gap:22px;align-items:flex-end;">'
    for lbl, h, w in [("14pt · card", 14, 150), ("8pt · popover, detail", 8, 130), ("6 / 4pt · widget", 6, 90), ("3pt · mini", 3, 110)]:
        extra = bar(41, "claude", w, 4, reflect=False, head=False) if h == 6 else ""
        sizes += (f'<div style="display:flex;flex-direction:column;gap:8px;"><div style="display:flex;flex-direction:column;gap:6px;">'
                  f'{bar(62, "claude", w, h, reflect=h >= 8, head=h >= 8)}{extra}</div>'
                  f'<span style="font-size:11px;color:var(--label2);">{lbl}</span></div>')
    sizes += "</div>"
    tipdemo = (f'<div style="position:relative;height:110px;padding-top:78px;">'
               f'{bar(62, "claude", 330, 14, tip=CLAUDE["s_tip"], tip_always=True)}</div>')
    p1 = panel("Neon progress bar", "The hero element. Glow scales with value; thresholds override the service accent.",
               anat + legend + f'<div style="display:flex;flex-direction:column;gap:6px;padding-top:6px;border-top:1px solid var(--sep);">{rows}</div>' +
               f'<div style="display:flex;flex-direction:column;gap:18px;padding-top:12px;border-top:1px solid var(--sep);">'
               f'<div style="display:flex;flex-direction:column;gap:10px;"><span class="cap">Sizes</span>{sizes}</div>'
               f'<div style="display:flex;flex-direction:column;gap:6px;"><span class="cap">Hover tooltip</span>{tipdemo}</div></div>')

    # 2. glass materials
    mats = ""
    for name, cls, spec in [("Window glass", "glass", "Native plugin in the app"),
                            ("Card glass", "card", "Text-bearing · denser tint"),
                            ("Tile / control", "tile", "Stats, controls · lightest")]:
        mats += (f'<div class="{cls}" style="flex:1;height:120px;border-radius:20px;padding:14px;display:flex;flex-direction:column;justify-content:flex-end;gap:2px;">'
                 f'<span style="font-size:13px;font-weight:600;">{name}</span><span style="font-size:11px;color:var(--label2);">{spec}</span></div>')
    solid = (f'<div class="glass rt" style="flex:1;height:120px;border-radius:20px;padding:14px;display:flex;flex-direction:column;justify-content:flex-end;gap:2px;">'
             f'<span style="font-size:13px;font-weight:600;">Reduce Transparency</span><span style="font-size:11px;color:var(--label2);">Also used for .no-glass</span></div>')
    p2 = panel("Glass materials", "Layered depth: window → card → tile. Text always sits on card glass or denser.",
               f'<div class="wall-{mode}" style="position:relative;overflow:hidden;border-radius:20px;padding:22px;display:flex;gap:16px;">'
               f'{waves(mode, 684, 164)}<div style="position:relative;display:flex;gap:14px;width:100%;">{mats}{solid}</div></div>')

    # 3. charts (tokens, always with a scope label; no limit/pace line)
    charts = (chart_card("Hourly · last 24 h", "current hour: brighter core, 1.6× glow, value label",
                         hourly_chart(CLAUDE["hourly"], "claude", 650, 170), CLAUDE["hourly_scope"]) +
              f'<div style="display:flex;gap:16px;">'
              f'{chart_card("Daily · last 7 days", "plain bars", daily_chart(CODEX["daily"], CODEX["daily_labels"], "codex", 290, 180), CODEX["daily_scope"], CODEX["daily_asof"])}'
              f'<div style="flex:1;display:flex;flex-direction:column;gap:16px;">'
              f'<div class="card" style="border-radius:20px;padding:14px 16px;display:flex;flex-direction:column;gap:10px;">'
              f'<span style="font-size:13px;font-weight:600;">Sparkline · today</span>{sparkline(CLAUDE["hourly"][9:], "claude", 290, 42)}'
              f'{sparkline(CODEX["hourly"][9:], "codex", 290, 42)}</div>'
              f'<div class="card" style="position:relative;border-radius:20px;padding:14px 16px;height:118px;">'
              f'<span style="font-size:13px;font-weight:600;">Glass tooltip</span>'
              f'{tipbox(["16.9M tokens", "1 PM"], "left:16px;top:44px;", always=True)}'
              f'{tipbox(["88.1M tokens", "Tue"], "left:150px;top:44px;", always=True)}</div></div></div>')
    p3 = panel("Chart styles", "Token charts carry their scope (this Mac or account). No limit or pace line on token charts.", charts)

    # 4. stat tiles
    p4 = panel("Stat tile", "Icon + caption label, rounded tabular value, one-line explanation.",
               f'<div style="display:flex;gap:14px;">{stat("flame", "Peak hour", "1 PM", "16.9M tokens · this Mac")}'
               f'{stat("avg", "7-day average", "45.3M", "tokens per day · this Mac")}'
               f'{stat("trend", "At this pace", "~130% by reset", "Limit reached ≈ Thu 4:00 PM", "warn", icon("warn", 18, "var(--warn-text)", 2.1, label="Warning"))}</div>')

    # 5. menu bar: 3 template states + title, on both menu bar appearances
    def strip(m):
        items = ""
        for name, pct, st in [("Normal", 62, 0), ("Warning ≥ 75", 81, 1), ("Critical ≥ 90", 94, 2)]:
            items += (f'<div style="flex:1;display:flex;flex-direction:column;align-items:center;gap:8px;">'
                      f'<div style="height:34px;display:flex;align-items:center;">{status_item(pct, st, 1.3)}</div>'
                      f'<span style="font-size:10px;color:var(--label2);text-align:center;">{name}</span></div>')
        return (f'<div class="t-{m} wall-{m}" style="border-radius:16px;padding:12px 8px 10px;display:flex;color:var(--label);">'
                f'{items}</div>')
    p5 = panel("Menu bar icon", "Three bundled template PNGs (monochrome, tinted by macOS) plus a title with the highest used %.",
               f'<div style="display:flex;flex-direction:column;gap:10px;">{strip("dark")}{strip("light")}</div>')

    # 6. controls + badges
    ctrls = (f'<div style="display:flex;flex-wrap:wrap;gap:14px;align-items:center;">{seg("Overview")}{toolbar_buttons()}{back_btn()}'
             f'{badge("claude", 32)}{badge("codex", 32)}{letter_badge("claude")}{letter_badge("codex")}'
             f'{switch(True, "Example on")}{switch(False, "Example off")}{popup_btn("Every 2 minutes")}{capsule_btn("Browse…")}'
             f'{status_chip("ok", "Confirmed")}{status_chip("neutral", "Unverified")}{status_chip("warn", "Likely overridden")}'
             f'<button style="height:32px;padding:0 18px;border-radius:99px;background:var(--accent);color:#fff;font-size:13px;font-weight:600;">Enable</button></div>')
    p6 = panel("Controls & badges", "Calm and native: no neon on chrome. Service marks are placeholders for the official logos.", ctrls)

    col_a = f'<div style="width:740px;display:flex;flex-direction:column;gap:40px;">{p1}{p2}</div>'
    col_b = f'<div style="width:740px;display:flex;flex-direction:column;gap:40px;">{p3}{p4}{p5}{p6}</div>'
    return (scene_open(mode, 1680, COMPONENTS_H) +
            f'<div style="position:absolute;left:60px;top:60px;display:flex;gap:40px;">{col_a}{col_b}</div></div>')


# ---------- tokens
def swatch(hexv, label):
    return (f'<div style="display:flex;align-items:center;gap:8px;"><span style="width:22px;height:22px;border-radius:7px;background:{hexv};'
            f'box-shadow:0 0 0 1px var(--sep),0 0 10px color-mix(in srgb,{hexv} 50%,transparent);flex-shrink:0;"></span>'
            f'<span class="mono" style="font-size:11px;">{label}</span></div>')


def table(headers, rows, widths):
    head = "".join(f'<span class="cap" style="width:{w};">{h}</span>' for h, w in zip(headers, widths))
    body = ""
    for r in rows:
        cells = "".join(f'<div style="width:{w};font-size:12px;display:flex;align-items:center;">{c}</div>' for c, w in zip(r, widths))
        body += f'<div style="display:flex;gap:12px;padding:8px 0;border-top:1px solid var(--sep);align-items:center;">{cells}</div>'
    return f'<div style="display:flex;flex-direction:column;"><div style="display:flex;gap:12px;padding-bottom:8px;">{head}</div>{body}</div>'


def tokens_scene():
    mode = "dark"
    colors = [("accent.claude", "#FF8A5B", "#EE6431", "Claude bars, lines"),
              ("accent.codex", "#3CF2FF", "#0099BA", "Codex bars, lines"),
              ("state.warning", "#FFB020", "#E08A00", "≥ 75%"),
              ("state.critical", "#FF2D6F", "#E0194F", "≥ 90%, 100% pulse"),
              ("text.claude", "#FF9A70", "#B9461C", "Key numbers ≥ 4.5:1"),
              ("text.codex", "#62F5FF", "#00708A", "Key numbers ≥ 4.5:1"),
              ("text.warning", "#FFC247", "#955800", "Warning labels"),
              ("text.critical", "#FF5C8E", "#BF1041", "Critical labels"),
              ("bar.core", "rgba(255,255,255,.9)", "rgba(255,255,255,.6)", "Inner bright line"),
              ("bar.track", "rgba(255,255,255,.08)", "rgba(0,0,0,.07)", "Capsule track")]
    crow = [(f'<span class="mono">{n}</span>', swatch(d, d), swatch(l, l), f'<span style="color:var(--label2);">{u}</span>') for n, d, l, u in colors]
    p_col = panel("Color · data", "Neon is reserved for data. Light mode uses the same hues as saturated fills.",
                  table(["Token", "Dark", "Light", "Use"], crow, ["130px", "170px", "170px", "140px"]))
    glow_rows = [("< 50%", "3–6 / 8–18 px", "× 0.38", "calm"),
                 ("50–74%", "6–8 / 18–23 px", "× 0.38", "brighter"),
                 ("75–89%", "× 1.2 / × 1.25", "× 0.38", "amber"),
                 ("90–99%", "× 1.2 / × 1.25", "× 0.38", "magenta-red"),
                 ("100%", "+ halo 26 px", "× 0.38", "pulse 1.8 s")]
    grow = [(f'<span class="num" style="font-weight:600;">{a}</span>', f'<span class="mono">{b}</span>', f'<span class="mono">{c}</span>',
             f'<span style="color:var(--label2);">{d}</span>') for a, b, c, d in glow_rows]
    p_glow = panel("Glow radii", "Bloom = two box-shadows (inner 85%, outer 50% alpha). Reflection blur = height ÷ 4.",
                   table(["Value", "Bloom (dark)", "Light", "Feel"], grow, ["90px", "170px", "90px", "120px"]))
    mats = [("Window", "rgba(24,26,42,.44)", "rgba(255,255,255,.42)", "44 / 190%"),
            ("Card (text)", "rgba(12,14,26,.50)", "rgba(255,255,255,.68)", "inherits"),
            ("Dense float", "rgba(16,18,32,.58)", "rgba(255,255,255,.62)", "44 / 190%"),
            ("Popover / tip", "rgba(22,24,38,.72)", "rgba(255,255,255,.78)", "20 / 180%"),
            ("Tile", "white 5.5%", "white 55%", "—"),
            ("Edge", "white 14%", "white 75%", "1 px"),
            ("Specular", "inset 0 1 white 32%", "inset 0 1 white 100%", "top rim"),
            ("Reduce Transp.", "#1C1E2B / #252838", "#F2F2F6 / #FFFFFF", "solid")]
    mrow = [(f'<span style="font-weight:600;">{a}</span>', f'<span class="mono">{b}</span>', f'<span class="mono">{c}</span>',
             f'<span class="mono" style="color:var(--label2);">{d}</span>') for a, b, c, d in mats]
    p_mat = panel("Glass materials", "SwiftUI: .glassEffect(.regular) for chrome; cards add a tint so text holds contrast on any wallpaper.",
                  table(["Layer", "Dark tint", "Light tint", "Blur / sat"], mrow, ["110px", "180px", "180px", "110px"]))
    shape = [("Window", "26 pt"), ("Card", "22 pt"), ("Floating card", "24 pt"), ("Tile", "16 pt"), ("Badge", "30% of size"),
             ("Widget pill", "36 pt (capsule)"), ("Bars, controls", "capsule")]
    srow = [(f'<span>{a}</span>', f'<span class="mono">{b}</span>') for a, b in shape]
    p_shape = panel("Radii", "Continuous corners throughout.", table(["Element", "Radius"], srow, ["200px", "200px"]))
    type_rows = [("Large %", "SF Pro Rounded 30 / semibold · tabular", "30px", 600, "num"),
                 ("Countdown", "SF Pro Rounded 40 / semibold · tabular", "22px", 600, "num"),
                 ("Title", "SF Pro 20 / bold", "20px", 700, ""),
                 ("Headline", "SF Pro 15 / semibold", "15px", 600, ""),
                 ("Body", "SF Pro 13 / regular", "13px", 400, ""),
                 ("Caption", "SF Pro 11 / semibold · caps +3%", "11px", 600, ""),
                 ("Secondary", "vibrant label 2 (68% / 80%)", "12px", 400, "")]
    trow = [(f'<span class="{c}" style="font-size:{s};font-weight:{wt};">{n}</span>', f'<span style="color:var(--label2);">{d}</span>')
            for n, d, s, wt, c in type_rows]
    p_type = panel("Typography", "SF Pro everywhere; numbers in SF Pro Rounded with tabular figures so values don’t jitter.",
                   table(["Style", "Spec"], trow, ["200px", "380px"]))
    motion = [("Bar fill", "spring · response 0.55 s · damping 0.72 · 80 ms stagger"),
              ("Limit pulse", "brightness 1 → 1.55 · 1.8 s ease-in-out · repeat"),
              ("Skeleton", "shimmer sweep 1.6 s linear"),
              ("Chart bars", "rise from baseline · 0.9 s · 25 ms stagger"),
              ("Tooltip", "fade + 4 pt rise · 160 ms"),
              ("Countdown", "ticks every 1 s; ‘updated’ label every 1 s"),
              ("Reduce Motion", "no spring (0.2 s fade), no pulse (static ring), no shimmer")]
    mrows = [(f'<span style="font-weight:600;">{a}</span>', f'<span style="color:var(--label2);">{b}</span>') for a, b in motion]
    p_motion = panel("Motion", "Animation carries meaning (loading, time passing, urgency), never decoration.",
                     table(["Element", "Spec"], mrows, ["140px", "440px"]))
    col_a = f'<div style="width:740px;display:flex;flex-direction:column;gap:40px;">{p_col}{p_glow}{p_type}</div>'
    col_b = f'<div style="width:740px;display:flex;flex-direction:column;gap:40px;">{p_mat}{p_shape}{p_motion}</div>'
    return (scene_open(mode, 1680, 1520) +
            f'<div style="position:absolute;left:60px;top:60px;display:flex;gap:40px;">{col_a}{col_b}</div></div>')


# ---------- accessibility
def a11y_scene():
    mode = "dark"

    def mini_card(m, cls):
        return (f'<div class="t-{m}{cls}" style="color:var(--label);">'
                f'<section class="glass dense" style="width:{CARD_W}px;border-radius:24px;padding:18px 20px;display:flex;flex-direction:column;gap:14px;">'
                f'{card_header("claude")}{usage("claude", "s", 62, CLAUDE["s_reset"], 290)}{usage("codex", "w", 78, CODEX["w_reset"], 290)}</section></div>')

    rt = panel("Reduce Transparency", "Glass becomes solid tinted panels; glow and accents are unchanged.",
               f'<div style="display:flex;gap:24px;">{mini_card("dark", " rt")}{mini_card("light", " rt")}</div>')
    rm_rows = ""
    for name, before, after in [("Bar fill", "spring from 0", "appears at value, 0.2 s fade"),
                                ("Limit reached", "1.8 s pulse", "static ring + hourglass icon"),
                                ("Loading", "shimmer sweep", "static skeleton"),
                                ("Countdown", "ticks each second", "unchanged (text, not motion)")]:
        rm_rows += (f'<div style="display:flex;gap:12px;padding:8px 0;border-top:1px solid var(--sep);font-size:12px;">'
                    f'<span style="width:120px;font-weight:600;">{name}</span><span style="width:150px;color:var(--label2);">{before}</span>'
                    f'<span style="flex:1;">{after}</span></div>')
    rm = panel("Reduce Motion", "Honoured via prefers-reduced-motion. The limit state keeps its meaning without animating.",
               f'<div class="rm" style="display:flex;flex-direction:column;gap:14px;">'
               f'<div style="display:flex;align-items:center;gap:12px;">{bar(100, "crit", 330, 14)}'
               f'<span style="display:flex;align-items:center;gap:5px;color:var(--crit-text);">{icon("hourglass", 16, sw=2.1, label="Limit reached")}'
               f'<span class="num" style="font-size:15px;font-weight:700;">100%</span></span></div><div>{rm_rows}</div></div>')
    gray_rows = ""
    for p, svc in [(52, "claude"), (80, "codex"), (95, "claude"), (100, "codex")]:
        c = cvar(svc, p)
        lv = level(p)
        word = LEVEL_WORD.get(lv, "Normal")
        gray_rows += (f'<div style="display:flex;align-items:center;gap:12px;">{bar(p, c, 260, 10, reflect=False)}'
                      f'<span style="width:70px;display:flex;align-items:center;gap:4px;color:var(--{c}-text);">{level_icon(p, svc, 14)}'
                      f'<span class="num" style="font-size:14px;font-weight:700;">{p}%</span></span>'
                      f'<span style="font-size:12px;color:var(--label2);">{word}</span></div>')
    gray = panel("Never colour alone", "The same four states in greyscale: percentage, icon and wording still tell them apart.",
                 f'<div class="gray" style="display:flex;flex-direction:column;gap:14px;">{gray_rows}</div>')
    contrast = panel("Contrast over any wallpaper", "Text never sits on the thinnest glass.",
                     f'<div style="display:flex;flex-direction:column;gap:8px;font-size:12px;line-height:1.45;">'
                     f'<span>• Primary and secondary labels sit on card glass (50% dark / 68% light tint) or denser.</span>'
                     f'<span>• Secondary labels: 68% white on dark, 80% ink on light: 6.5:1 or better on the card tint.</span>'
                     f'<span>• Accent numbers use the text.* tokens (4.5:1 or better), not the raw neon fills.</span>'
                     f'<span>• Desktop captions and the widget use dense glass (58–62%), never the clear window tint.</span></div>')
    col_a = f'<div style="width:760px;display:flex;flex-direction:column;gap:40px;">{rt}{contrast}</div>'
    col_b = f'<div style="width:740px;display:flex;flex-direction:column;gap:40px;">{rm}{gray}</div>'
    return (scene_open(mode, 1680, 1060) +
            f'<div style="position:absolute;left:60px;top:60px;display:flex;gap:40px;">{col_a}{col_b}</div></div>')


_uid = [0]


def uid(prefix="g"):
    """Unique id for SVG gradients inside one file."""
    _uid[0] += 1
    return f"{prefix}{_uid[0]}"


def hour_label(i):
    """Label for hourly slot i (0 = 3 PM yesterday, 23 = current hour 2 PM)."""
    h = (15 + i) % 24
    return f"{h % 12 or 12} {'AM' if h < 12 else 'PM'}"


def fmt_tok(m):
    """Compact token count from millions: 16.9M, 842k, 0."""
    if m <= 0:
        return "0"
    if m >= 1:
        return f"{m:.1f}M".replace(".0M", "M")
    return f"{m * 1000:.0f}k"


def sparkline(vals, svc, w, h, slots=24, dim=False):
    """Today's hourly tokens; the rest of the day is a dotted baseline."""
    gid = uid("sp")
    step = w / (slots - 1)
    mx = max(max(vals), 1) * 1.15
    pts = [(round(i * step, 1), round(h - 2 - v / mx * (h - 8), 1)) for i, v in enumerate(vals)]
    line = " ".join(f"{x},{y}" for x, y in pts)
    area = f"M0,{h} L" + " L".join(f"{x},{y}" for x, y in pts) + f" L{pts[-1][0]},{h} Z"
    lx, ly = pts[-1]
    col = "var(--label3)" if dim else f"var(--{svc})"
    glow = "" if dim else f"filter:drop-shadow(0 0 calc(var(--gk) * 4px) var(--{svc}));"
    return (f'<svg width="{w}" height="{h}" viewBox="0 0 {w} {h}" aria-hidden="true" style="display:block;overflow:visible;">'
            f'<defs><linearGradient id="{gid}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" style="stop-color:{col};stop-opacity:0.35;"></stop>'
            f'<stop offset="1" style="stop-color:{col};stop-opacity:0;"></stop></linearGradient></defs>'
            f'<line x1="{lx}" y1="{h - 0.5}" x2="{w}" y2="{h - 0.5}" style="stroke:var(--label3);stroke-width:1;stroke-dasharray:2 4;"></line>'
            f'<path d="{area}" style="fill:url(#{gid});stroke:none;"></path>'
            f'<polyline class="spark" points="{line}" style="fill:none;stroke:{col};stroke-width:2;stroke-linecap:round;stroke-linejoin:round;{glow}"></polyline>'
            f'<circle cx="{lx}" cy="{ly}" r="3.5" style="fill:{col};stroke:#fff;stroke-width:1.5;"></circle></svg>')


def hourly_chart(vals, svc, w, h, now=23, mx=20):
    """Last 24 hours of tokens: neon bars, current hour brighter with a value label; hover = exact value."""
    top, bottom, right = 20, 22, 34
    area_h = h - top - bottom
    grid = ""
    for g in (0, 10, 20):
        y = top + area_h * (1 - g / mx)
        style = "solid" if g == 0 else "dashed"
        grid += (f'<div style="position:absolute;left:0;right:{right}px;top:{y:.1f}px;border-top:1px {style} var(--sep);"></div>'
                 f'<div class="num" style="position:absolute;right:0;top:{y - 7:.1f}px;font-size:10px;color:var(--label2);">{fmt_tok(g)}</div>')
    slot_w = (w - right) / len(vals)
    bw = max(4, round(slot_w * 0.56))
    cv = f"var(--{svc})"
    slots, labels = "", ""
    for i, v in enumerate(vals):
        is_now = i == now
        bh = max(3, round(v / mx * area_h))
        if v == 0:
            style = "background:var(--track);"
        else:
            k = 1.6 if is_now else 1.0
            core = "linear-gradient(90deg,rgba(255,255,255,0) 30%,var(--core) 50%,rgba(255,255,255,0) 70%)," if is_now else ""
            style = (f"background:{core}linear-gradient(180deg,color-mix(in srgb,{cv} 70%,#fff) 0%,{cv} 30%,color-mix(in srgb,{cv} 40%,transparent) 100%);"
                     f"box-shadow:0 0 calc(var(--gk) * {5 * k:.0f}px) color-mix(in srgb,{cv} 80%,transparent),0 0 calc(var(--gk) * {12 * k:.0f}px) color-mix(in srgb,{cv} 40%,transparent);")
            if not is_now:
                style += "opacity:0.82;"
        now_lbl = ""
        if is_now:
            now_lbl = (f'<div class="num neon" style="position:absolute;bottom:{bh + 6}px;left:50%;width:44px;margin-left:-22px;text-align:center;'
                       f'font-size:11px;font-weight:700;color:var(--{svc}-text);">{fmt_tok(v)}</div>')
        tip = tipbox([f"{fmt_tok(v)} tokens", hour_label(i) + (" · now" if is_now else "")],
                     f"left:50%;margin-left:-56px;bottom:{bh + 12}px;")
        slots += (f'<div class="hov" style="flex:1;height:100%;display:flex;flex-direction:column;justify-content:flex-end;align-items:center;position:relative;">'
                  f'{now_lbl}<div class="hb" style="width:{bw}px;height:{bh}px;border-radius:{bw / 2}px {bw / 2}px 2px 2px;{style}animation-delay:{i * 0.025:.3f}s;"></div>{tip}</div>')
        txt = "Now" if is_now else (hour_label(i) if i in (0, 6, 12, 18) else "")
        colr = f"var(--{svc}-text)" if is_now else "var(--label2)"
        labels += (f'<div style="flex:1;text-align:center;white-space:nowrap;font-size:10px;font-weight:{700 if is_now else 400};'
                   f'color:{colr};">{txt}</div>')
    return (f'<div style="position:relative;width:{w}px;height:{h}px;">{grid}'
            f'<div style="position:absolute;left:0;right:{right}px;top:{top}px;height:{area_h}px;display:flex;align-items:flex-end;">{slots}</div>'
            f'<div style="position:absolute;left:0;right:{right}px;bottom:0;height:14px;display:flex;">{labels}</div></div>')


def daily_chart(vals, labels, svc, w, h):
    """Last 7 days of tokens as plain bars. No limit or pace line: tokens have no quota denominator."""
    top, bottom = 18, 22
    area_h = h - top - bottom
    mx = max(vals) * 1.12 or 1
    cv = f"var(--{svc})"
    cols, lbls = "", ""
    for i, (v, lab) in enumerate(zip(vals, labels)):
        last = i == len(vals) - 1
        bh = max(3, round(v / mx * area_h))
        if v == 0:
            style = "background:var(--track);"
        else:
            k = 1.4 if last else 1.0
            style = (f"background:linear-gradient(180deg,color-mix(in srgb,{cv} 70%,#fff),{cv} 35%,color-mix(in srgb,{cv} 45%,transparent));"
                     f"box-shadow:0 0 calc(var(--gk) * {6 * k:.0f}px) color-mix(in srgb,{cv} 75%,transparent),"
                     f"0 0 calc(var(--gk) * {14 * k:.0f}px) color-mix(in srgb,{cv} 35%,transparent);")
            if not last:
                style += "opacity:0.82;"
        vcol = f"var(--{svc}-text)" if last else "var(--label2)"
        val = (f'<div class="num" style="font-size:10px;font-weight:{700 if last else 500};color:{vcol};margin-bottom:4px;">'
               f'{fmt_tok(v)}</div>')
        tip = tipbox([f"{fmt_tok(v)} tokens", "Today so far" if lab == "Today" else lab],
                     f"left:50%;margin-left:-56px;bottom:{bh + 26}px;")
        cols += (f'<div class="hov" style="flex:1;height:100%;display:flex;flex-direction:column;justify-content:flex-end;align-items:center;position:relative;">'
                 f'{val}<div class="hb" style="width:20px;height:{bh}px;border-radius:7px 7px 3px 3px;{style}animation-delay:{i * 0.06:.2f}s;"></div>{tip}</div>')
        lcol = "var(--label)" if last else "var(--label2)"
        lbls += (f'<div style="flex:1;text-align:center;white-space:nowrap;font-size:11px;font-weight:{700 if last else 400};'
                 f'color:{lcol};">{lab}</div>')
    return (f'<div style="position:relative;width:{w}px;height:{h}px;">'
            f'<div style="position:absolute;left:0;right:0;top:{top + area_h}px;border-top:1px solid var(--sep);"></div>'
            f'<div style="position:absolute;left:0;right:0;top:{top}px;height:{area_h}px;display:flex;align-items:flex-end;">{cols}</div>'
            f'<div style="position:absolute;left:0;right:0;bottom:0;height:15px;display:flex;">{lbls}</div></div>')


def chart_card(title, note, chart, scope, asof=None, flex="1"):
    """Chart on card glass with its own scope label (and 'as of' time when data is older than 5 min)."""
    meta = scope + (f" · {asof}" if asof else "")
    return (f'<section class="card" style="flex:{flex};min-width:0;border-radius:20px;padding:14px 16px 12px;display:flex;flex-direction:column;gap:8px;">'
            f'<div style="display:flex;justify-content:space-between;align-items:baseline;"><span style="font-size:13px;font-weight:600;">{title}</span>'
            f'<span style="font-size:11px;color:var(--label2);">{note}</span></div>{chart}'
            f'<div style="display:flex;align-items:center;gap:5px;font-size:11px;color:var(--label2);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">'
            f'{icon("info", 12)}<span>{meta}</span></div></section>')


def stat(icon_name, label, value, caption, color=None, lvl_icon=""):
    vcol = f"var(--{color}-text)" if color else "var(--label)"
    neon = " neon" if color else ""
    return (f'<div class="tile" style="flex:1;min-width:0;padding:12px 14px;display:flex;flex-direction:column;gap:5px;">'
            f'<div style="display:flex;align-items:center;gap:6px;color:var(--label2);">{icon(icon_name, 14)}<span class="cap">{label}</span></div>'
            f'<div class="num{neon}" style="font-size:22px;font-weight:600;line-height:1.1;color:{vcol};display:flex;align-items:center;gap:6px;">{lvl_icon}{value}</div>'
            f'<div style="font-size:11px;color:var(--label2);line-height:1.35;">{caption}</div></div>')


def template_icon(state=0, scale=1.0, color="var(--mb)"):
    """Menu bar template glyph (static, monochrome, 3 bundled PNG states): dual ring; warning adds a
    triangle badge, critical a solid dot. It does not show live values; the title text does."""
    s = round(18 * scale, 1)
    c1, c2 = 2 * math.pi * 7, 2 * math.pi * 3.4
    badge_svg = ""
    if state == 1:
        badge_svg = f'<path d="M14 10.6 L17.8 17.4 H10.2 Z" style="fill:{color};stroke:none;"></path>'
    elif state == 2:
        badge_svg = f'<circle cx="14.2" cy="14.2" r="3.6" style="fill:{color};stroke:none;"></circle>'
    return (f'<svg width="{s}" height="{s}" viewBox="0 0 18 18" aria-hidden="true" style="display:block;flex-shrink:0;fill:none;stroke-linecap:round;">'
            f'<circle cx="9" cy="9" r="7" style="stroke:{color};stroke-width:1.8;opacity:0.35;"></circle>'
            f'<circle cx="9" cy="9" r="7" transform="rotate(-90 9 9)" style="stroke:{color};stroke-width:1.8;stroke-dasharray:{c1 * 0.68:.2f} {c1:.2f};"></circle>'
            f'<circle cx="9" cy="9" r="3.4" style="stroke:{color};stroke-width:1.8;opacity:0.35;"></circle>'
            f'<circle cx="9" cy="9" r="3.4" transform="rotate(-90 9 9)" style="stroke:{color};stroke-width:1.8;stroke-dasharray:{c2 * 0.45:.2f} {c2:.2f};"></circle>'
            f'{badge_svg}</svg>')


def status_item(pct, state, scale=1.0, color="var(--mb)", active=False):
    """Template icon + title text (highest used %), exactly what tray.set_title() can show."""
    sel = "background:var(--mb-sel);" if active else ""
    return (f'<span style="display:inline-flex;align-items:center;gap:{round(4 * scale)}px;padding:0 {round(7 * scale)}px;'
            f'height:{round(24 * scale)}px;border-radius:{round(7 * scale)}px;{sel}color:{color};">'
            f'{template_icon(state, scale, color)}<span class="num" style="font-size:{round(13 * scale)}px;font-weight:500;">{pct}%</span></span>')


def seg(active):
    out = '<div class="ctl" style="display:flex;padding:3px;border-radius:99px;gap:2px;">'
    for it in ["Overview", "Claude", "Codex"]:
        on = it == active
        cls = ' class="ctl-sel"' if on else ""
        colr = "var(--label)" if on else "var(--label2)"
        out += (f'<button{cls} aria-pressed="{"true" if on else "false"}" style="height:26px;padding:0 16px;border-radius:99px;'
                f'font-size:13px;font-weight:{600 if on else 500};color:{colr};">{it}</button>')
    return out + "</div>"


def back_btn(text="Overview"):
    return (f'<button class="ctl" style="height:28px;padding:0 12px 0 8px;border-radius:99px;display:flex;align-items:center;gap:4px;'
            f'font-size:12px;font-weight:500;">{icon("back", 14)}{text}</button>')


def spark_section(svc, vals, w, peak, dim=False):
    return (f'<div style="display:flex;flex-direction:column;gap:8px;padding-top:12px;border-top:1px solid var(--sep);">'
            f'<div style="display:flex;justify-content:space-between;font-size:11px;color:var(--label2);">'
            f'<span><span class="cap">Today</span> · tokens on this Mac</span><span>Peak {peak}</span></div>{sparkline(vals, svc, w, 42, dim=dim)}</div>')


def details_btn(svc):
    return (f'<button aria-label="Open {SVC[svc]["name"]} details" class="ctl" style="width:28px;height:28px;border-radius:99px;display:flex;'
            f'align-items:center;justify-content:center;color:var(--label2);">{icon("chevron", 14)}</button>')


def detail_scene(mode, svc):
    d = DATA[svc]
    s = SVC[svc]

    def mini(pct, text):
        c = cvar(svc, pct)
        return (f'<div style="width:168px;display:flex;flex-direction:column;gap:6px;">'
                f'<div style="display:flex;justify-content:space-between;align-items:center;gap:6px;">'
                f'<span style="font-size:11px;color:var(--label2);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">{text}</span>'
                f'<span style="display:flex;align-items:center;gap:4px;color:var(--{c}-text);">{level_icon(pct, svc, 13)}'
                f'<span class="num neon" style="font-size:14px;font-weight:700;">{pct}%</span></span></div>'
                f'{bar(pct, c, 168, 8, reflect=False)}</div>')

    header = (f'<div style="height:44px;display:flex;align-items:center;gap:12px;">{badge(svc, 40)}'
              f'<div style="display:flex;flex-direction:column;gap:2px;flex:1;min-width:0;">'
              f'<div style="display:flex;align-items:center;gap:8px;"><span style="font-size:20px;font-weight:700;">{s["name"]}</span>'
              f'<span class="ctl" style="font-size:11px;padding:2px 8px;border-radius:99px;color:var(--label2);">{s["plan"]}</span></div>'
              f'<span style="font-size:12px;color:var(--label2);">Wednesday, Sep 23</span></div>'
              f'{mini(d["s"], "Session · " + d["s_reset"].replace("Resets in ", "") + " left")}'
              f'{mini(d["w"], "Weekly · " + d["w_reset"].replace("Resets ", ""))}</div>')
    hourly = chart_card("Last 24 hours", "tokens per hour", hourly_chart(d["hourly"], svc, 366, 172), d["hourly_scope"], None, "1.5")
    daily = chart_card("Last 7 days", d.get("daily_note", "tokens per day"), daily_chart(d["daily"], d["daily_labels"], svc, 232, 172),
                       d["daily_scope"], d["daily_asof"], "1")
    pj = d["projection"]
    pj_icon = icon("warn", 18, "var(--warn-text)", 2.1, label="Warning") if pj[2] else ""
    stats = (f'<div style="display:flex;gap:14px;height:96px;">'
             f'{stat("flame", "Peak hour", d["peak"][0], d["peak"][1])}'
             f'{stat("avg", "7-day average", d["avg"][0], d["avg"][1])}'
             f'{stat("trend", "At this pace", pj[0], pj[1], pj[2] or svc, pj_icon)}</div>')
    content = (f'<div style="flex:1;display:flex;flex-direction:column;gap:14px;padding:4px 20px 20px;">'
               f'{header}<div style="display:flex;gap:14px;height:252px;">{hourly}{daily}</div>{stats}</div>')
    return scene_open(mode, W, H) + menubar() + window(content, center=seg(s["name"])) + "</div>"


def native_menu():
    """Right-click tray menu (native NSMenu; shown for its item list and order only)."""
    items = ["Open Dashboard", "Show Floating Widget", "Refresh Now", None, "Settings…", None, "Quit"]
    rows = ""
    for i, it in enumerate(items):
        if it is None:
            rows += '<div style="height:1px;background:var(--sep);margin:4px 8px;"></div>'
        else:
            hl = "background:var(--accent);color:#fff;" if i == 0 else ""
            rows += f'<div style="height:24px;padding:0 10px;border-radius:6px;display:flex;align-items:center;font-size:13px;{hl}">{it}</div>'
    return (f'<div class="glass pop" style="width:220px;border-radius:12px;padding:5px;box-shadow:0 12px 30px rgba(0,0,0,0.3);">{rows}</div>')


def statusitem_panel(left, top):
    """Status item reference at 2.2x plus the right-click menu."""
    rows = ""
    for name, desc, pct, st in [("Normal", "Below 75% everywhere", 62, 0),
                                ("Warning · ≥ 75%", "Any visible window at 75–89%", 81, 1),
                                ("Critical · ≥ 90%", "Any visible window at 90% or more", 94, 2)]:
        rows += (f'<div style="display:flex;align-items:center;gap:14px;">'
                 f'<div style="width:150px;height:52px;border-radius:12px;display:flex;align-items:center;justify-content:center;'
                 f'background:var(--track);box-shadow:inset 0 0 0 1px var(--sep);">{status_item(pct, st, 1.9, "var(--label)")}</div>'
                 f'<div style="display:flex;flex-direction:column;gap:2px;"><span style="font-size:13px;font-weight:600;">{name}</span>'
                 f'<span style="font-size:12px;color:var(--label2);">{desc}</span></div></div>')
    return (f'<div class="glass dense" style="position:absolute;left:{left}px;top:{top}px;width:400px;border-radius:24px;padding:18px 20px;display:flex;flex-direction:column;gap:12px;">'
            f'<div style="display:flex;flex-direction:column;gap:3px;"><span class="cap">Status item · template image at 1.9×</span>'
            f'<span style="font-size:12px;color:var(--label2);line-height:1.4;">Three bundled monochrome PNGs; macOS tints them. The title is the highest used % among visible windows.</span></div>'
            f'{rows}<div style="height:1px;background:var(--sep);margin:4px 0;"></div>'
            f'<span class="cap">Right-click menu (native)</span>{native_menu()}</div>')


def status_chip(kind, text):
    """Small status chip: ok (green dot), warn (amber), neutral (grey)."""
    dot = {"ok": "var(--ok)", "warn": "var(--warn)", "neutral": "var(--label3)"}[kind]
    tcol = "var(--warn-text)" if kind == "warn" else "var(--label2)"
    return (f'<span class="ctl" style="display:inline-flex;align-items:center;gap:6px;padding:3px 9px;border-radius:99px;font-size:11px;'
            f'white-space:nowrap;color:{tcol};"><span style="width:7px;height:7px;border-radius:50%;background:{dot};"></span>{text}</span>')


def value_text(text, mono=False):
    cls = ' class="mono"' if mono else ""
    return f'<span{cls} style="font-size:12px;color:var(--label2);white-space:nowrap;">{text}</span>'


ONB_W = 2360


def onboarding_window(left, step, icon_name, title, body, box, secondary, primary):
    dots = "".join(f'<span style="width:7px;height:7px;border-radius:50%;background:{"var(--label)" if i == step else "var(--label3)"};"></span>'
                   for i in range(1, 4))
    right = (f'<span style="font-size:12px;color:var(--label2);">Step {step} of 3</span>'
             f'<span style="display:flex;gap:5px;align-items:center;">{dots}</span>')
    sec = "".join(capsule_btn(t) for t in secondary)
    content = (f'<div style="flex:1;display:flex;flex-direction:column;padding:6px 40px 24px;gap:16px;">'
               f'<div style="display:flex;flex-direction:column;align-items:center;gap:10px;text-align:center;">'
               f'<div style="width:56px;height:56px;border-radius:18px;display:flex;align-items:center;justify-content:center;background:var(--track);color:var(--label);">'
               f'{icon(icon_name, 28, sw=1.8)}</div>'
               f'<span style="font-size:20px;font-weight:700;">{title}</span>'
               f'<span style="font-size:13px;color:var(--label2);max-width:520px;line-height:1.45;">{body}</span></div>'
               f'{box}'
               f'<div style="margin-top:auto;display:flex;justify-content:flex-end;align-items:center;gap:10px;">{sec}'
               f'<button style="height:30px;padding:0 18px;border-radius:99px;background:var(--accent);color:#fff;font-size:13px;font-weight:600;">{primary}</button></div></div>')
    center = '<span style="font-size:13px;font-weight:600;">Welcome to How Is It</span>'
    return window(content, left=left, top=150, center=center, right=right)


def onboarding_scene(mode):
    """Onboarding route (#/onboarding), shown while onboarding_completed == false. Three steps (§11.4)."""
    step1_box = group([label_row("Codex CLI", "/opt/homebrew/bin/codex", badge("codex", 28)) + status_chip("ok", "Found · 0.154.0"),
                       label_row("Not the right one?") + capsule_btn("Detect Again") + capsule_btn("Browse…")],
                      None, "Limits are read through <span class=\"mono\">codex app-server</span>. Your Codex sign-in stays with Codex.")
    bullets = "".join(f'<div style="display:flex;gap:10px;align-items:flex-start;font-size:12px;line-height:1.45;">'
                      f'<span style="color:var(--label2);margin-top:1px;">{icon(ic, 15)}</span><span>{t}</span></div>'
                      for ic, t in [("gear", "Adds a status line command to <span class=\"mono\">~/.claude/settings.json</span>. Any status line you already use keeps running after ours."),
                                    ("info", "While a custom status line is set, Claude Code hides most footer keyboard hints."),
                                    ("warn", "A project or organization setting can override it. If updates stop arriving, How Is It tells you."),
                                    ("refresh", "You can turn it off at any time in Settings → Accounts; your previous status line is restored.")])
    step2_box = f'<div class="card" style="border-radius:14px;padding:14px 16px;display:flex;flex-direction:column;gap:10px;">{bullets}</div>'
    step3_box = group([label_row("75% and 90% used", "One alert per threshold per window",
                                 f'<span style="color:var(--warn-text);">{icon("warn", 18, sw=2)}</span>'),
                       label_row("Limit reached and reset", "So you know when capacity is back",
                                 f'<span style="color:var(--crit-text);">{icon("hourglass", 18, sw=2)}</span>')],
                      None, "You can change these in Settings → Alerts.")
    wins = (onboarding_window(60, 1, "search", "Find Codex", "How Is It shows your Codex session and weekly limits from the Codex CLI on this Mac.",
                              step1_box, [], "Continue") +
            onboarding_window(820, 2, "bell", "Real-time Claude updates",
                              "Claude Code can report your limits after every response through its status line.",
                              step2_box, ["Back", "Skip"], "Enable") +
            onboarding_window(1580, 3, "bell", "Notifications", "Get a heads-up before you run out, and when a limit resets.",
                              step3_box, ["Back", "Not Now"], "Allow Notifications"))
    return scene_open(mode, ONB_W, H) + menubar() + wins + "</div>"


def empty_state(icon_name, title, body, primary=None, secondary=None):
    p = (f'<button style="height:32px;padding:0 18px;border-radius:99px;background:var(--accent);color:#fff;font-size:13px;font-weight:600;'
         f'box-shadow:inset 0 1px 0 rgba(255,255,255,0.3);">{primary}</button>') if primary else ""
    s = capsule_btn(secondary) if secondary else ""
    return (f'<div style="flex:1;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;text-align:center;">'
            f'<div style="width:80px;height:80px;border-radius:50%;border:1.5px dashed var(--label3);display:flex;align-items:center;justify-content:center;color:var(--label2);">'
            f'{icon(icon_name, 28, sw=1.8)}</div>'
            f'<span style="font-size:15px;font-weight:700;">{title}</span>'
            f'<span style="font-size:12px;color:var(--label2);max-width:250px;line-height:1.45;">{body}</span>{p}{s}</div>')


def muted_row(ic, text):
    return (f'<div style="display:flex;align-items:center;gap:8px;padding:10px 12px;border-radius:10px;font-size:12px;color:var(--label2);'
            f'background:var(--track);">{icon(ic, 14)}<span>{text}</span></div>')


# ---------------------------------------------------------------- build
def main():
    os.makedirs(PROJECT, exist_ok=True)
    boards = {}
    order = []
    files = {}

    def add(name, title, body, w, h, x, y, page_id, logic=STATIC_LOGIC, interactive=False):
        files[name] = page(title, body, w, h, logic)
        _uid[0] = 0
        entry = {"x": x, "y": y, "w": w, "h": h, "title": title, "page": page_id}
        if interactive:
            entry["is_interactive"] = True
        boards[name] = entry
        order.append(name)

    step = W + 80
    row2 = H + 400
    # Page: main window (dashboard routes)
    add("Main.dc.html", "Overview — Dark", overview_scene("dark", True), W, H, 0, 0, "main", LIVE_LOGIC, True)
    add("DetailClaudeDark.dc.html", "Claude tab — Dark", detail_scene("dark", "claude"), W, H, step, 0, "main")
    add("DetailCodexDark.dc.html", "Codex tab — Dark", detail_scene("dark", "codex"), W, H, step * 2, 0, "main")
    add("OverviewLight.dc.html", "Overview — Light", overview_scene("light", True), W, H, 0, row2, "main", LIVE_LOGIC, True)
    add("DetailClaudeLight.dc.html", "Claude tab — Light", detail_scene("light", "claude"), W, H, step, row2, "main")
    add("DetailCodexLight.dc.html", "Codex tab — Light", detail_scene("light", "codex"), W, H, step * 2, row2, "main")
    # Page: menu bar + widget
    add("MenuBarDark.dc.html", "Menu bar & popover — Dark", menubar_scene("dark"), W, H, 0, 0, "menubar")
    add("WidgetDark.dc.html", "Floating widget — Dark", widget_scene("dark"), W, H, step, 0, "menubar")
    add("MenuBarLight.dc.html", "Menu bar & popover — Light", menubar_scene("light"), W, H, 0, row2, "menubar")
    add("WidgetLight.dc.html", "Floating widget — Light", widget_scene("light"), W, H, step, row2, "menubar")
    # Page: settings route
    for i, (pane, _, _) in enumerate(SIDEBAR):
        add(f"Settings{pane}Dark.dc.html", f"Settings · {pane} — Dark", settings_scene("dark", pane), W, H, step * i, 0, "settings")
        add(f"Settings{pane}Light.dc.html", f"Settings · {pane} — Light", settings_scene("light", pane), W, H, step * i, row2, "settings")
    add("SettingsReadOnlyDark.dc.html", "Settings · read-only banner — Dark", settings_scene("dark", "Accounts", True), W, H,
        step * len(SIDEBAR), 0, "settings")
    # Page: onboarding route
    add("OnboardingDark.dc.html", "Onboarding — Dark", onboarding_scene("dark"), ONB_W, H, 0, 0, "onboarding")
    add("OnboardingLight.dc.html", "Onboarding — Light", onboarding_scene("light"), ONB_W, H, 0, row2, "onboarding")
    # Page: states
    add("StatesDark.dc.html", "States — Dark", states_scene("dark"), STATES_W, STATES_H, 0, 0, "states", LIVE_LOGIC, True)
    add("StatesLight.dc.html", "States — Light", states_scene("light"), STATES_W, STATES_H, STATES_W + 80, 0, "states", LIVE_LOGIC, True)
    # Page: system
    add("ComponentsDark.dc.html", "Component sheet — Dark", components_scene("dark"), 1680, COMPONENTS_H, 0, 0, "system")
    add("ComponentsLight.dc.html", "Component sheet — Light", components_scene("light"), 1680, COMPONENTS_H, 1760, 0, "system")
    add("Tokens.dc.html", "Color & material tokens", tokens_scene(), 1680, 1520, 3520, 0, "system")
    add("Accessibility.dc.html", "Accessibility", a11y_scene(), 1680, 1060, 3520, 1640, "system")

    notes = {
        "mainDark": {"x": 0, "y": -300, "text": "Main window · Dark", "kind": "title1", "maxW": step * 3 - 80, "page": "main"},
        "mainLight": {"x": 0, "y": row2 - 300, "text": "Main window · Light", "kind": "title1", "maxW": step * 3 - 80, "page": "main"},
        "mbDark": {"x": 0, "y": -300, "text": "Menu bar & floating widget · Dark", "kind": "title1", "maxW": step * 2 - 80, "page": "menubar"},
        "mbLight": {"x": 0, "y": row2 - 300, "text": "Menu bar & floating widget · Light", "kind": "title1", "maxW": step * 2 - 80, "page": "menubar"},
        "setDark": {"x": 0, "y": -300, "text": "Settings route · Dark", "kind": "title1", "maxW": step * 6 - 80, "page": "settings"},
        "setLight": {"x": 0, "y": row2 - 300, "text": "Settings route · Light", "kind": "title1", "maxW": step * 5 - 80, "page": "settings"},
        "onb": {"x": 0, "y": -300, "text": "Onboarding route · Dark and Light", "kind": "title1", "maxW": ONB_W, "page": "onboarding"},
        "states": {"x": 0, "y": -300, "text": "Card states · Dark and Light", "kind": "title1", "maxW": STATES_W * 2 + 80, "page": "states"},
        "system": {"x": 0, "y": -300, "text": "Components, tokens & accessibility", "kind": "title1", "maxW": 5200, "page": "system"},
    }
    canvas = {
        "v": 3,
        "createdOnFiles": {"v": 1, "at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")},
        "title": "How Is It — macOS Usage Monitor",
        "launch": {"view": "canvas", "page": "main"},
        "pages": [{"id": "main", "name": "Main window"}, {"id": "menubar", "name": "Menu bar & widget"},
                  {"id": "settings", "name": "Settings"}, {"id": "onboarding", "name": "Onboarding"},
                  {"id": "states", "name": "States"}, {"id": "system", "name": "Components & tokens"}],
        "boards": boards,
        "order": order,
        "notes": notes,
        "designSystems": [],
    }
    for name, src in files.items():
        with open(os.path.join(PROJECT, name), "w", encoding="utf-8") as fh:
            fh.write(src)
    with open(os.path.join(PROJECT, "canvas.json"), "w", encoding="utf-8") as fh:
        json.dump(canvas, fh, ensure_ascii=False, indent=1)
    print(f"wrote {len(files)} artboards")


if __name__ == "__main__":
    main()
