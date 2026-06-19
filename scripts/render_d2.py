#!/usr/bin/env python3
"""
Render a D2 diagram in both light and dark themes, upload the two SVGs to
static.rerun.io, and print a `<div class="d2-diagram">` HTML block that can
be pasted directly into the docs markdown.

The rendered SVGs match the look the rerun.io docs site produces for D2
diagrams: monospaced uppercase labels, muted strokes, transparent node
fills, emerald accent for `<code>` runs inside labels.

Requires:
  - the `d2` CLI on PATH (https://d2lang.com).
  - `scripts/upload_image.py` (same directory) and its dependencies; see
    that file's docstring for GCS authentication setup.

Usage:
    python3 scripts/render_d2.py path/to/diagram.d2
    cat diagram.d2 | python3 scripts/render_d2.py

or via pixi:
    pixi run python scripts/render_d2.py path/to/diagram.d2

The final HTML block is printed to stdout (and stderr when interactive),
mirroring how `upload_image.py` reports its results.

Per theme, this is roughly equivalent to:
    cat <site-theme> diagram.d2 | d2 --pad=0 --scale=0.8 --stdout-format=svg - -
followed by SVG post-processing:
  - Strip D2's embedded <style> blocks.
  - Replace <foreignObject> markdown labels with plain <text> elements
    (preserving `<code>` segments as <tspan class="d2-code">).
  - Rewrite <tspan dy="N"> from D2's 16px user-space units into em units.
  - Inject a <style> block with typography/colors baked in, scaled per
    diagram by viewBox-width / 600 (the target display width) so the
    rendered look stays consistent when an <img> stretches the SVG.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

SVG_NS = "http://www.w3.org/2000/svg"
XLINK_NS = "http://www.w3.org/1999/xlink"
ET.register_namespace("", SVG_NS)
ET.register_namespace("xlink", XLINK_NS)


SITE_THEME = """\
vars: {
  d2-config: {
    layout-engine: elk
    theme-overrides: {
      N1: "#ffffff"
      N2: "#b2b2bb"
      N3: "#8a8a96"
      N4: "#555555"
      N5: "#333333"
      N6: "#1a1a1a"
      N7: "#0c0c0c"
      B1: "#70f2b7"
      B2: "#5cd9a0"
      B3: "#1a3d2e"
      B4: "#162e24"
      B5: "#121f1a"
      B6: "#0c0c0c"
      AA2: "#70f2b7"
      AA4: "#1a3d2e"
      AA5: "#121f1a"
      AB4: "#2a2a3a"
      AB5: "#1a1a2a"
    }
  }
}
"""

# Resolved from src/app.css. Two palettes matching the site's dark/light
# themes; selected per call via the `theme` argument.
THEMES = {
    "dark": {
        "bg": "#000000",
        "muted": "#a3a3a3",  # neutral-400
        "token_string": "#6ee7b7",  # emerald-300
    },
    "light": {
        "bg": "#ffffff",
        "muted": "#525252",  # neutral-600
        "token_string": "#059669",  # emerald-600
    },
}
FONT_FAMILY = 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace'

TARGET_DISPLAY_WIDTH = 600
D2_LAYOUT_FONT_PX = 16


def qname(local: str) -> str:
    return f"{{{SVG_NS}}}{local}"


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def render_d2(source: str) -> str:
    proc = subprocess.run(
        ["d2", "--pad=0", "--scale=0.8", "--stdout-format=svg", "-", "-"],
        input=SITE_THEME + "\n" + source,
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(f"d2 failed with exit code {proc.returncode}")
    return proc.stdout


def strip_styles(root: ET.Element) -> None:
    for parent in root.iter():
        for child in list(parent):
            if local_name(child.tag) == "style":
                parent.remove(child)


def extract_segments(node: ET.Element, in_code: bool) -> list[tuple[str, bool]]:
    is_code = in_code or local_name(node.tag).lower() == "code"
    out: list[tuple[str, bool]] = []
    if node.text:
        out.append((node.text, is_code))
    for child in node:
        out.extend(extract_segments(child, is_code))
        if child.tail:
            out.append((child.tail, in_code))
    return out


def normalize_segments(segs: list[tuple[str, bool]]) -> list[tuple[str, bool]]:
    norm = [(re.sub(r"\s+", " ", t), c) for t, c in segs]
    # Merge runs of same `code` flag.
    merged: list[tuple[str, bool]] = []
    for t, c in norm:
        if merged and merged[-1][1] == c:
            prev_t, prev_c = merged[-1]
            merged[-1] = (prev_t + t, prev_c)
        else:
            merged.append((t, c))
    if merged:
        merged[0] = (merged[0][0].lstrip(), merged[0][1])
        merged[-1] = (merged[-1][0].rstrip(), merged[-1][1])
    return [(t, c) for t, c in merged if t]


def replace_foreign_objects(root: ET.Element) -> None:
    # Collect (parent, child, index) first; mutating during ET iteration is unsafe.
    targets: list[tuple[ET.Element, ET.Element, int]] = []
    for parent in root.iter():
        for i, child in enumerate(list(parent)):
            if local_name(child.tag).lower() == "foreignobject":
                targets.append((parent, child, i))

    for parent, fobj, _ in targets:
        x = float(fobj.get("x", "0"))
        y = float(fobj.get("y", "0"))
        w = float(fobj.get("width", "0"))
        h = float(fobj.get("height", "0"))
        segs = normalize_segments(extract_segments(fobj, False))
        idx = list(parent).index(fobj)
        parent.remove(fobj)
        if not segs:
            continue
        text = ET.Element(
            qname("text"),
            {
                "x": f"{x + w / 2}",
                "y": f"{y + h / 2}",
                "style": "text-anchor:middle;dominant-baseline:central",
            },
        )
        last: ET.Element | None = None
        for t, code in segs:
            if code:
                ts = ET.SubElement(text, qname("tspan"), {"class": "d2-code"})
                ts.text = t
                last = ts
            else:
                if last is None:
                    text.text = (text.text or "") + t
                else:
                    last.tail = (last.tail or "") + t
        parent.insert(idx, text)


def rewrite_tspan_dy(root: ET.Element) -> None:
    for el in root.iter():
        if local_name(el.tag) != "tspan":
            continue
        dy = el.get("dy")
        if dy is None:
            continue
        try:
            v = float(dy)
        except ValueError:
            continue
        if v == 0:
            continue
        em = round(v / D2_LAYOUT_FONT_PX, 3)
        el.set("dy", f"{em}em")


def viewbox_width(root: ET.Element) -> float | None:
    vb = root.get("viewBox")
    if not vb:
        return None
    parts = vb.strip().split()
    if len(parts) < 3:
        return None
    try:
        return float(parts[2])
    except ValueError:
        return None


def build_style_css(scale: float, theme: str) -> str:
    colors = THEMES[theme]
    font = round(10 * scale, 3)
    stroke = round(1 * scale, 3)
    halo = round(6 * scale, 3)
    letter = round(1 * scale, 3)
    return f"""
svg > rect:first-child {{ fill: transparent !important; }}
[stroke]:not([stroke="none"]) {{
  stroke: {colors["muted"]} !important;
  stroke-width: {stroke}px !important;
}}
polygon {{ fill: {colors["muted"]} !important; }}
.shape rect, .shape path, .shape circle, .shape ellipse {{
  fill: transparent !important;
}}
text {{
  fill: {colors["muted"]} !important;
  font-family: {FONT_FAMILY} !important;
  font-size: {font}px !important;
  text-transform: uppercase;
  letter-spacing: {letter}px;
  paint-order: stroke fill;
  stroke: {colors["bg"]};
  stroke-width: {halo}px;
  stroke-linejoin: round;
}}
tspan.d2-code {{
  fill: {colors["token_string"]} !important;
  text-transform: none;
  letter-spacing: normal;
}}
"""


def render(source: str, theme: str = "dark") -> str:
    """Render a single d2 source string to a self-contained SVG (text)."""
    return post_process(render_d2(source), theme)


def post_process(svg_text: str, theme: str = "dark") -> str:
    root = ET.fromstring(svg_text)
    strip_styles(root)
    replace_foreign_objects(root)
    rewrite_tspan_dy(root)

    vb_w = viewbox_width(root)
    scale = vb_w / TARGET_DISPLAY_WIDTH if vb_w else 1.0
    style = ET.Element(qname("style"))
    style.text = build_style_css(scale, theme)
    root.insert(0, style)

    body = ET.tostring(root, encoding="unicode")

    # ET has no CDATA support; unescape and wrap our injected <style> content
    # so the CSS reads cleanly in the file (XML parsers would decode entities
    # before the CSS engine sees them either way, but CDATA is friendlier).
    def _wrap(m: re.Match[str]) -> str:
        css = m.group(1).replace("&gt;", ">").replace("&lt;", "<").replace("&amp;", "&")
        return f"<style><![CDATA[{css}]]></style>"

    body = re.sub(r"<style>(.*?)</style>", _wrap, body, count=1, flags=re.DOTALL)
    return '<?xml version="1.0" encoding="utf-8"?>\n' + body


def main() -> None:
    ap = argparse.ArgumentParser(
        description="Render a D2 diagram in light+dark, upload both SVGs to "
        "static.rerun.io, print an HTML <div> block ready to paste "
        "into docs markdown.",
    )
    ap.add_argument(
        "source",
        nargs="?",
        type=Path,
        help="Path to a .d2 source file. If omitted, source is read from stdin.",
    )
    args = ap.parse_args()

    import logging
    import sys
    import tempfile

    logging.basicConfig(level=logging.INFO)

    if args.source is None:
        if sys.stdin.isatty():
            ap.error("no source file given and stdin is a tty")
        source_text = sys.stdin.read()
    else:
        source_text = args.source.read_text()

    # Imported lazily so the render API can be used as a library without
    # pulling in upload_image's heavier dependency tree (PIL, gcloud, …).
    from upload_image import Uploader

    uploader = Uploader()

    urls: dict[str, str] = {}
    with tempfile.TemporaryDirectory() as td:
        for theme in ("light", "dark"):
            logging.info(f"rendering {theme} theme")
            svg_path = Path(td) / f"d2-{theme}.svg"
            svg_path.write_text(render(source_text, theme))
            object_name = uploader.upload_file(svg_path)
            urls[theme] = f"https://static.rerun.io/{object_name}"

    html = (
        '<div class="d2-diagram">\n'
        f'  <img class="d2-dark" src="{urls["dark"]}" alt="">\n'
        f'  <img class="d2-light" src="{urls["light"]}" alt="">\n'
        "</div>"
    )
    print(f"\n{html}", file=sys.stderr)
    if not sys.stdout.isatty():
        # Allow piping into pbcopy/xclip without the stderr banner.
        print(html)


if __name__ == "__main__":
    main()
