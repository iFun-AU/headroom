# How Is It: design package

Approved UI design, exported from the design canvas
(<https://claude.ai/artifact/SbtKJhjwPQHcs9Dbs9UFnU>, version 4, 29 artboards, 2026-09-23).
It matches `docs/DEVELOPMENT.md` **v2.2**:
- **Main window:** Overview, Claude and Codex tabs, with token charts labelled "this Mac" or "account".
- **Settings and Onboarding:** routes inside the main window.
- **Menu bar:** template icon + title, and the popover.
- **Floating widget:** Pill, Stack and Mini.
- **States:** every §11.4 edge state.
- **Reference sheets:** components, tokens and accessibility.

| Path | Contents |
|---|---|
| [`DESIGN_IMPLEMENTATION_PROMPT.md`](DESIGN_IMPLEMENTATION_PROMPT.md) | **Start here for UI work (M7).** Precedence, design decisions, component specs, task order |
| [`preview/index.html`](preview/index.html) | Browsable gallery. Each artboard is a standalone HTML page (hover tooltips work) |
| `screenshots/` | 29 PNG references (1×), one per artboard |
| [`design-tokens.css`](design-tokens.css) | Colors, glass materials, glow, motion, and reduced-motion/transparency rules |
| `canvas/` | Raw canvas source (`*.dc.html` + `canvas.json`). Reference only |
| `source/gen.py` | Generator for every artboard: the canonical component math |
| `source/export.py` | Rebuilds `preview/`, `screenshots/` and `design-tokens.css` from `canvas/` |

## Viewing
```bash
open preview/index.html
```

## Re-exporting after a design change
Download the updated canvas files into `canvas/`, then run this from `docs/design`:
```bash
python3 source/export.py canvas .
```
Screenshots need Google Chrome in `/Applications`.
