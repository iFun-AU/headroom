#!/usr/bin/env python3
"""
@file export.py
@description Exports the How Is It design canvas into a project folder:
             canvas/      raw .dc.html artboards + canvas.json (editable source)
             preview/     standalone HTML per artboard (opens in any browser) + index.html
             screenshots/ PNG per artboard, rendered with headless Chrome
             design-tokens.css  tokens, materials and motion lifted from the mockups
Usage: python3 export.py <canvas/project dir> <output dir>
"""
import html
import json
import os
import re
import shutil
import subprocess
import sys

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
# Static stand-ins for the live {{holes}} the canvas fills from its logic class.
HOLES = {
    "updated": "Updated 12s ago",
    "claudeReset": "Resets in 2h 14m",
    "codexReset": "Resets in 3h 41m",
    "limitClock": "1:32:07",
}


def to_standalone(src: str) -> str:
    """Strip the Design-canvas runtime so the artboard renders as plain HTML."""
    src = src.replace('<script src="./support.js"></script>\n', "")
    src = re.sub(r'<script type="text/x-dc".*?</script>\n', "", src, flags=re.S)
    return re.sub(r"\{\{\s*(\w+)\s*\}\}", lambda m: HOLES[m.group(1)], src)


def screenshot(html_path: str, png_path: str, w: int, h: int) -> bool:
    """Render one artboard; virtual time lets the fill/rise animations finish first."""
    if not os.path.exists(CHROME):
        return False
    cmd = [CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars", f"--window-size={w},{h}",
           "--virtual-time-budget=4000", f"--screenshot={png_path}", "file://" + html_path]
    res = subprocess.run(cmd, capture_output=True, timeout=90)
    return res.returncode == 0 and os.path.exists(png_path)


def tokens_css(sample_src: str) -> str:
    """Pull the stylesheet out of one artboard and re-home the theme classes on :root."""
    css = sample_src.split("<style>\n", 1)[1].split("\n</style>", 1)[0]
    rules = css.split("\n")
    dark = next(r for r in rules if r.startswith(".t-dark{"))
    light = next(r for r in rules if r.startswith(".t-light{"))
    dark_body = dark[len(".t-dark{"):-1]
    light_body = light[len(".t-light{"):-1]
    keep = [r for r in rules if r.startswith((".glass", ".card", ".tile", ".ctl", ".tipbox", ".num", ".mono", ".neon",
                                             ".cap", "@keyframes", ".nbf", ".pulse", ".halo", ".hb", ".shim", ".spark",
                                             ".hov", "@media"))]
    head = ("/*\n * design-tokens.css — lifted verbatim from the How Is It mockups (docs/design/canvas).\n"
            " * Dark is the default; light applies via prefers-color-scheme. Class names match the mockups\n"
            " * so artboard markup can be read side by side with components.\n */\n")
    return (head + ":root{" + dark_body.replace(";", ";\n  ").replace("{", "{\n  ") + "}\n"
            "@media (prefers-color-scheme: light){\n:root{" + light_body.replace(";", ";\n  ") + "}\n}\n\n" +
            "\n".join(keep).replace("} }", "}}") + "\n")


def copy_if_needed(src: str, dst: str) -> None:
    """Copy unless source and destination are the same file (re-export in place)."""
    if os.path.abspath(src) != os.path.abspath(dst):
        shutil.copy(src, dst)


def main(project_dir: str, out: str) -> None:
    canvas = json.load(open(os.path.join(project_dir, "canvas.json"), encoding="utf-8"))
    for sub in ("canvas", "preview", "screenshots"):
        os.makedirs(os.path.join(out, sub), exist_ok=True)
    copy_if_needed(os.path.join(project_dir, "canvas.json"), os.path.join(out, "canvas", "canvas.json"))
    pages = {p["id"]: p["name"] for p in canvas["pages"]}
    shots_ok = 0
    page_order = [p["id"] for p in canvas["pages"]]
    groups = {pid: [] for pid in page_order}
    for name in canvas["order"]:
        b = canvas["boards"][name]
        src = open(os.path.join(project_dir, name), encoding="utf-8").read()
        copy_if_needed(os.path.join(project_dir, name), os.path.join(out, "canvas", name))
        stem = name.replace(".dc.html", "")
        prev = os.path.abspath(os.path.join(out, "preview", stem + ".html"))
        open(prev, "w", encoding="utf-8").write(to_standalone(src))
        if screenshot(prev, os.path.abspath(os.path.join(out, "screenshots", stem + ".png")), b["w"], b["h"]):
            shots_ok += 1
        groups[b.get("page", "main")].append((stem, b))
    # Gallery
    body = ""
    for pid in page_order:
        body += f"<h2>{html.escape(pages[pid])}</h2><div class='grid'>"
        for stem, b in groups[pid]:
            body += (f"<a href='{stem}.html'><img src='../screenshots/{stem}.png' alt='{html.escape(b['title'])}'>"
                     f"<span>{html.escape(b['title'])} · {b['w']}×{b['h']}</span></a>")
        body += "</div>"
    gallery = ("<!doctype html><html lang='en'><head><meta charset='utf-8'><title>How Is It — design previews</title>"
               "<style>body{margin:0;padding:32px;font:14px -apple-system,system-ui,sans-serif;background:#111;color:#eee}"
               "h1{margin:0 0 8px}h2{margin:32px 0 12px;font-size:16px;color:#aaa}"
               ".grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:16px}"
               "a{color:inherit;text-decoration:none;display:flex;flex-direction:column;gap:6px}"
               "img{width:100%;border-radius:10px;border:1px solid #333;background:#222}</style></head><body>"
               "<h1>How Is It — design previews</h1><p>Click a thumbnail for the full-size HTML mockup (hover works there).</p>"
               f"{body}</body></html>")
    open(os.path.join(out, "preview", "index.html"), "w", encoding="utf-8").write(gallery)
    first = open(os.path.join(project_dir, canvas["order"][0]), encoding="utf-8").read()
    open(os.path.join(out, "design-tokens.css"), "w", encoding="utf-8").write(tokens_css(first))
    print(f"boards: {len(canvas['order'])}, screenshots: {shots_ok}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
