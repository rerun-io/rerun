#!/usr/bin/env python3
"""
Generate the hand-laid-out docs diagrams as SVGs in one shared style.

Every diagram is drawn at the same scale (1 SVG unit = 1 CSS px, and rerun.io
shows `.d2-diagram` images at their intrinsic size), so type size, stroke
weight, node size and arrowheads match across all of them.

Style rules:
  - 8 px layout grid, 40 px tall nodes, 152 px wide where the label allows.
  - 12 px monospace labels. Plain labels are uppercase with tracking; text in
    `backticks` is code: emerald, original case, no tracking.
  - Shapes carry meaning: rectangle = component or step, cylinder = store,
    page = file, parallelogram = input/output. Groups are unfilled frames.
  - Arrows are 1 px orthogonal lines with rounded corners and one arrowhead
    size. Labels sit in a gap in the line, so nothing depends on the page
    background color.
  - Max width 680 px, which fits the 748 px docs prose column.

To add or change a diagram, edit its function below (or add one and list it in
`DIAGRAMS`). The script warns when a label does not fit its node or arrow.

Usage:
    # Write the SVGs to a folder to look at them:
    pixi run python scripts/docs_diagrams.py --out /tmp/docs_diagrams

    # Upload them to static.rerun.io and print the HTML blocks for the docs:
    pixi run python scripts/docs_diagrams.py --upload

Uploading needs `scripts/upload_image.py` (same directory) and its
dependencies; see that file's docstring for GCS authentication setup.
Uploads are content-addressed, so unchanged diagrams keep their URL.
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path
from xml.sax.saxutils import escape

FONT = 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace'
FS = 12  # font size
LH = 15  # line height
TRACK = 0.08  # letter-spacing of plain labels, in em
CHAR = 0.6  # monospace advance, in em
HEAD = 7  # arrowhead length
HEAD_W = 3.5  # arrowhead half width
CORNER = 6  # arrow corner radius
SLANT = 12  # parallelogram slant
LIST_GAP = 5  # extra space between a heading line and its muted list
FOLD = 9  # page corner fold
MAX_W = 680

# Resolved from the site's tokens in src/app.css (neutral + emerald scales).
THEMES = {
    "dark": {
        "stroke": "#a3a3a3",  # --color-muted
        "frame": "#5c5c5c",
        "text": "#e5e5e5",  # --color-subtle
        "muted": "#a3a3a3",
        "code": "#6ee7b7",  # --color-token-string
        "fill": "rgba(255,255,255,0.06)",
    },
    "light": {
        "stroke": "#525252",  # --color-muted
        "frame": "#a3a3a3",
        "text": "#262626",
        "muted": "#525252",
        "code": "#059669",  # --color-token-string
        "fill": "rgba(0,0,0,0.04)",
    },
}

Segment = tuple[str, bool]  # (text, is_code)


def parse_label(label: str) -> list[tuple[list[Segment], bool]]:
    """Split a label into lines of (segments, muted). `~` starts a muted line."""
    lines = []
    for raw in label.split("\n"):
        muted = raw.startswith("~")
        if muted:
            raw = raw[1:]
        segs: list[Segment] = []
        for i, part in enumerate(raw.split("`")):
            if part:
                segs.append((part if i % 2 else part.upper(), bool(i % 2)))
        lines.append((segs, muted))
    return lines


def line_width(segs: list[Segment]) -> float:
    return sum(len(t) * FS * (CHAR if code else CHAR + TRACK) for t, code in segs)


def line_offsets(lines: list[tuple[list[Segment], bool]]) -> tuple[list[float], float]:
    """Top offset of each line plus the total height. A muted block under a
    plain line (a list under its heading) gets a little extra space."""
    offsets, y = [], 0.0
    for i, (_, muted) in enumerate(lines):
        if i > 0 and muted and not lines[i - 1][1]:
            y += LIST_GAP
        offsets.append(y)
        y += LH
    return offsets, y


def label_size(label: str) -> tuple[float, float]:
    lines = parse_label(label)
    return max(line_width(s) for s, _ in lines), line_offsets(lines)[1]


def n(v: float) -> str:
    return f"{v:.2f}".rstrip("0").rstrip(".")


class Diagram:
    def __init__(self, name: str, title: str) -> None:
        self.name = name
        self.title = title
        self.back: list[str] = []  # group frames
        self.mid: list[str] = []  # arrows
        self.front: list[str] = []  # nodes and labels
        self.w = 0.0
        self.h = 0.0
        self.warnings: list[str] = []

    # -- bookkeeping ------------------------------------------------------

    def _extend(self, x: float, y: float) -> None:
        self.w = max(self.w, x)
        self.h = max(self.h, y)

    def _warn(self, msg: str) -> None:
        self.warnings.append(f"{self.name}: {msg}")

    # -- text -------------------------------------------------------------

    def text(self, cx: float, cy: float, label: str, cls: str = "", anchor: str = "middle") -> str:
        lines = parse_label(label)
        offsets, total = line_offsets(lines)
        top = cy - total / 2 + LH / 2 + FS * 0.35
        out = []
        for (segs, muted), offset in zip(lines, offsets):
            y = top + offset
            classes = " ".join(c for c in (cls, "m" if muted else "", "l" if anchor == "start" else "") if c)
            attr = f' class="{classes}"' if classes else ""
            body = "".join(f'<tspan class="c">{escape(t)}</tspan>' if code else escape(t) for t, code in segs)
            out.append(f'<text x="{n(cx)}" y="{n(y)}"{attr}>{body}</text>')
        w, h = label_size(label)
        left = cx if anchor == "start" else cx - w / 2
        self._extend(left + w, cy + h / 2)
        if left < 0:
            self._warn(f"label {label!r} starts left of the canvas")
        return "".join(out)

    # -- shapes -----------------------------------------------------------

    def _shape(self, shape: str, x: float, y: float, w: float, h: float, cls: str, ry: float) -> str:
        if shape == "rect":
            return f'<rect class="{cls}" x="{n(x)}" y="{n(y)}" width="{n(w)}" height="{n(h)}" rx="2"/>'
        if shape == "para":
            return f'<path class="{cls}" d="M{n(x + SLANT)} {n(y)}H{n(x + w)}L{n(x + w - SLANT)} {n(y + h)}H{n(x)}Z"/>'
        if shape == "page":
            return (
                f'<path class="{cls}" d="M{n(x)} {n(y)}H{n(x + w - FOLD)}L{n(x + w)} {n(y + FOLD)}'
                f'V{n(y + h)}H{n(x)}Z"/>'
                f'<path class="e" d="M{n(x + w - FOLD)} {n(y)}V{n(y + FOLD)}H{n(x + w)}"/>'
            )
        if shape == "cyl":
            rx = w / 2
            return (
                f'<path class="{cls}" d="M{n(x)} {n(y + ry)}A{n(rx)} {n(ry)} 0 0 1 {n(x + w)} {n(y + ry)}'
                f'V{n(y + h - ry)}A{n(rx)} {n(ry)} 0 0 1 {n(x)} {n(y + h - ry)}Z"/>'
                f'<path class="{"f" if cls == "g" else "e"}" d="M{n(x)} {n(y + ry)}'
                f'A{n(rx)} {n(ry)} 0 0 0 {n(x + w)} {n(y + ry)}"/>'
            )
        raise ValueError(shape)

    def node(self, x: float, y: float, w: float, h: float, label: str, shape: str = "rect") -> None:
        """A leaf node: filled shape with a centered label."""
        ry = 6
        self.front.append(self._shape(shape, x, y, w, h, "n", ry))
        cy = y + h / 2 + (ry if shape == "cyl" else 0)
        self.front.append(self.text(x + w / 2, cy, label))
        self._extend(x + w, y + h)
        lw, lh = label_size(label)
        room = w - 16 - (2 * SLANT if shape == "para" else 0)
        if lw > room:
            self._warn(f"label {label!r} is {lw:.0f} wide, node has room for {room:.0f}")
        if lh > h - 8:
            self._warn(f"label {label!r} is {lh:.0f} tall, node is {h:.0f}")

    def group(
        self, x: float, y: float, w: float, h: float, label: str, shape: str = "rect", label_dy: float = 18
    ) -> None:
        """An unfilled frame around other nodes. Rect groups are labelled
        top-left, cylinder groups top-center under the rim."""
        ry = 10
        self.back.append(self._shape(shape, x, y, w, h, "g", ry))
        if shape == "cyl":
            self.back.append(self.text(x + w / 2, y + 2 * ry + 18, label, "m"))
        else:
            self.back.append(self.text(x + 12, y + label_dy, label, "m", anchor="start"))
        self._extend(x + w, y + h)

    def panes(self, x: float, y: float, w: float, h: float) -> None:
        """A schematic viewer viewport: one wide view above two smaller ones."""
        split_y = y + round(h * 0.55)
        split_x = x + round(w * 0.5)
        self.front.append(
            f'<rect class="n" x="{n(x)}" y="{n(y)}" width="{n(w)}" height="{n(h)}" rx="2"/>'
            f'<path class="f" d="M{n(x)} {n(split_y)}H{n(x + w)}M{n(split_x)} {n(split_y)}V{n(y + h)}"/>'
        )

    # -- arrows -----------------------------------------------------------

    def arrow(
        self,
        pts: list[tuple[float, float]],
        label: str | None = None,
        seg: int = 0,
        t: float = 0.5,
    ) -> None:
        """Orthogonal polyline through `pts` with an arrowhead at the end.
        `label` sits in a gap on segment `seg`, at fraction `t` along it."""
        last = len(pts) - 2
        d: list[str] = []
        for i in range(len(pts) - 1):
            (ax, ay), (bx, by) = pts[i], pts[i + 1]
            length = abs(bx - ax) + abs(by - ay)
            if ax != bx and ay != by:
                self._warn(f"arrow segment {pts[i]} to {pts[i + 1]} is not orthogonal")
            ux, uy = (bx - ax) / length, (by - ay) / length
            r_in = min(CORNER, length / 2) if i > 0 else 0
            r_out = min(CORNER, length / 2) if i < last else HEAD
            sx, sy = ax + ux * r_in, ay + uy * r_in
            ex, ey = bx - ux * r_out, by - uy * r_out
            if i == 0:
                d.append(f"M{n(sx)} {n(sy)}")
            if label is not None and i == seg:
                lw, lh = label_size(label)
                half = lw / 2 + 6 if uy == 0 else lh / 2 + 3
                cx, cy = ax + (bx - ax) * t, ay + (by - ay) * t
                d.append(f"L{n(cx - ux * half)} {n(cy - uy * half)}M{n(cx + ux * half)} {n(cy + uy * half)}")
                self.mid.append(self.text(cx, cy, label, "m"))
                stub = min(t, 1 - t) * length - half
                if stub < 12:
                    self._warn(f"arrow label {label!r} leaves a {stub:.0f}px stub")
            d.append(f"L{n(ex)} {n(ey)}")
            if i < last:
                cx2, cy2 = pts[i + 2]
                l2 = abs(cx2 - bx) + abs(cy2 - by)
                r2 = min(CORNER, l2 / 2)
                d.append(f"Q{n(bx)} {n(by)} {n(bx + (cx2 - bx) / l2 * r2)} {n(by + (cy2 - by) / l2 * r2)}")
            else:
                px, py = -uy * HEAD_W, ux * HEAD_W
                self.mid.append(
                    f'<path class="h" d="M{n(bx)} {n(by)}L{n(ex + px)} {n(ey + py)}L{n(ex - px)} {n(ey - py)}Z"/>'
                )
        self.mid.insert(0, f'<path class="e" d="{"".join(d)}"/>')
        for x, y in pts:
            self._extend(x, y)

    def caption(self, cx: float, cy: float, label: str) -> None:
        """Free-standing muted label, for example under a loop arrow."""
        self.mid.append(self.text(cx, cy, label, "m"))

    # -- output -----------------------------------------------------------

    def render(self, theme: str) -> str:
        c = THEMES[theme]
        w, h = round(self.w) + 1, round(self.h) + 1
        css = (
            f".n{{fill:{c['fill']};stroke:{c['stroke']}}}"
            f".g{{fill:none;stroke:{c['frame']}}}"
            f".f{{fill:none;stroke:{c['frame']}}}"
            f".e{{fill:none;stroke:{c['stroke']};stroke-linecap:round;stroke-linejoin:round}}"
            f".h{{fill:{c['stroke']}}}"
            f"text{{font-family:{FONT};font-size:{FS}px;letter-spacing:{TRACK}em;"
            f"text-anchor:middle;fill:{c['text']}}}"
            f".m{{fill:{c['muted']}}}"
            f".l{{text-anchor:start}}"
            f".c{{fill:{c['code']};letter-spacing:0}}"
        )
        # The half-pixel viewBox offset puts 1px strokes on whole pixels.
        return (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
            f'viewBox="-0.5 -0.5 {w} {h}" role="img">'
            f"<title>{escape(self.title)}</title>"
            f"<style>{css}</style>" + "".join(self.back) + "".join(self.mid) + "".join(self.front) + "</svg>\n"
        )


# -- diagrams ---------------------------------------------------------------

NW, NH = 152, 40  # default node size


def overview() -> Diagram:
    d = Diagram("overview", "How the Logging SDK, .rrd files, Viewer, catalog server and Catalog SDK connect")
    d.node(16, 32, NW, NH, "Logging SDK")
    d.node(16, 120, NW, NH, ".rrd files", "page")
    d.group(0, 208, 184, 88, "Catalog server")
    d.node(16, 240, NW, NH, "Datasets")

    d.group(280, 0, 400, 176, "Viewer")
    d.node(296, 32, NW, NH, "gRPC endpoint")
    d.node(296, 120, NW, NH, "Chunk store")
    d.node(512, 120, NW, NH, "Renderer")
    d.node(512, 256, NW, NH, "Catalog SDK")

    d.arrow([(168, 52), (296, 52)], "stream")
    d.arrow([(92, 72), (92, 120)], "save")
    d.arrow([(168, 140), (296, 140)], "load")
    d.arrow([(92, 160), (92, 208)], "register")
    d.arrow([(372, 72), (372, 120)])
    d.arrow([(448, 140), (512, 140)])
    d.arrow([(184, 244), (372, 244), (372, 160)], "redap")
    d.arrow([(184, 276), (512, 276)], "redap", t=0.78)
    return d


def log_and_ingest() -> Diagram:
    d = Diagram("log-and-ingest", "Two paths into an .rrd file: a data generator, or existing data through a converter")
    d.node(200, 0, NW, NH, "data generator")
    d.node(0, 56, NW, 88, "Existing data\n~MCAP\n~LeRobot\n~etc.", "page")
    d.node(200, 80, NW, NH, "converter")
    d.node(424, 0, 120, 144, ".RRD", "page")
    d.arrow([(352, 20), (424, 20)])
    d.arrow([(152, 100), (200, 100)])
    d.arrow([(352, 100), (424, 100)])
    return d


def stream_to_viewer() -> Diagram:
    d = Diagram("stream-to-viewer", "The Logging SDK streams to the Viewer")
    d.node(0, 0, NW, NH, "Logging SDK")
    d.node(264, 0, NW, NH, "Viewer")
    d.arrow([(152, 20), (264, 20)], "stream")
    return d


def save_and_load() -> Diagram:
    d = Diagram("save-and-load", "The Logging SDK saves an .rrd file that the Viewer loads")
    d.node(0, 0, NW, NH, "Logging SDK")
    d.node(264, 0, 120, NH, ".rrd", "page")
    d.node(496, 0, NW, NH, "Viewer")
    d.arrow([(152, 20), (264, 20)], "save")
    d.arrow([(384, 20), (496, 20)], "load")
    return d


def viewer_sources() -> Diagram:
    d = Diagram("viewer-sources", "Data sources that feed the Rerun Viewer")
    d.node(0, 88, 168, 88, "Supported formats\n~MCAP\n~LeRobot\n~etc.", "page")
    d.node(216, 0, NW, NH, "data generator")
    d.node(216, 56, NW, NH, ".RRD", "page")
    d.node(216, 112, NW, NH, "Automated\nimporters")
    d.node(216, 168, NW, 48, "Rerun Catalog", "cyl")
    d.group(440, 0, 240, 216, "Rerun Viewer")
    d.panes(456, 32, 208, 168)
    d.arrow([(168, 132), (216, 132)])
    d.arrow([(368, 20), (440, 20)])
    d.arrow([(368, 76), (440, 76)])
    d.arrow([(368, 132), (440, 132)])
    d.arrow([(368, 196), (440, 196)])
    return d


def prepare_catalog() -> Diagram:
    d = Diagram("prepare-catalog", "Creating a dataset in the Rerun Catalog and registering .rrd files")
    d.node(0, 8, 176, NH, "1. create_dataset")
    d.node(0, 64, 176, NH, "2. dataset handle")
    d.node(0, 120, 176, NH, "3. handle.register")
    d.node(424, 0, NW, 168, "Rerun Catalog", "cyl")
    d.arrow([(176, 28), (424, 28)], "dataset name")
    d.arrow([(424, 84), (176, 84)], "returns")
    d.arrow([(176, 140), (424, 140)], "path to .rrd(s)")
    return d


def query_catalog() -> Diagram:
    d = Diagram("query-catalog", "Querying a dataset in the Rerun Catalog through DataFrames")
    d.node(0, 16, 176, NH, "1. dataset handle")
    d.node(0, 72, 176, NH, "2. DataFrame")
    d.node(0, 128, 176, NH, "3. DataFrame")
    d.node(0, 184, 176, NH, "4. query result", "page")
    d.group(424, 0, 256, 240, "Rerun Catalog", "cyl")
    d.group(460, 64, 184, 136, "my_dataset")
    d.node(476, 96, NW, NH, "segment_a")
    d.node(476, 144, NW, NH, "segment_b")
    d.arrow([(176, 36), (424, 36)], "`dataset.reader(index=…)`")
    d.arrow([(424, 92), (176, 92)], "returns")
    d.arrow([(176, 148), (424, 148)], "`df.select(…).where(…)`")
    d.arrow([(424, 204), (176, 204)], "returns")
    return d


def dataloader() -> Diagram:
    d = Diagram("dataloader", "A query feeds a dataloader that reads from the Rerun Catalog and yields batches")
    d.node(0, 4, 120, NH, "query")
    d.node(184, 4, NW, NH, "dataloader")
    d.node(184, 60, NW, NH, "batches")
    d.node(400, 0, NW, 104, "Rerun Catalog", "cyl")
    d.arrow([(120, 24), (184, 24)])
    d.arrow([(336, 24), (400, 24)])
    d.arrow([(400, 80), (336, 80)])
    return d


def chunk_pipeline() -> Diagram:
    d = Diagram("chunk-pipeline", "A reader feeds a store or a LazyChunkStream, which ends in a terminal call")
    d.node(0, 72, 112, NH, "Reader", "para")
    d.node(184, 0, 104, 48, "Store", "cyl")
    d.node(368, 72, 144, NH, "`LazyChunkStream`")
    d.node(568, 72, 112, NH, "Terminal\ncall", "para")
    d.arrow([(56, 72), (56, 28), (184, 28)], "`.store()`", seg=1)
    d.arrow([(288, 28), (408, 28), (408, 72)], "`.stream()`")
    d.arrow([(112 - SLANT / 2, 92), (368, 92)], "`.stream()`")
    d.arrow([(512, 92), (568 + SLANT / 2, 92)])
    d.arrow([(416, 112), (416, 140), (464, 140), (464, 112)])
    d.caption(440, 158, "`filter`/`lenses`/`map`/…")
    return d


def catalog_object_model() -> Diagram:
    d = Diagram(
        "catalog-object-model", "Dataset segments and layers in the catalog pointing at .rrd files in an object store"
    )
    d.group(0, 0, 368, 304, "Catalog", "cyl")
    d.group(16, 56, 336, 216, "my_dataset")
    d.group(32, 88, 304, 56, "segment_a", label_dy=28)
    d.node(168, 96, 152, NH, 'layer "base"', "para")
    d.group(32, 152, 304, 104, "segment_b", label_dy=28)
    d.node(168, 160, 152, NH, 'layer "base"', "para")
    d.node(168, 208, 152, NH, 'layer "extra"', "para")

    d.group(472, 44, 208, 232, "Object store", "cyl")
    d.node(500, 96, NW, NH, "recording_a.rrd", "page")
    d.node(500, 160, NW, NH, "recording_b.rrd", "page")
    d.node(500, 208, NW, NH, "extra_b.rrd", "page")

    for y in (116, 180, 228):
        d.arrow([(500, y), (320 - SLANT / 2, y)])
    return d


def schema_stack() -> Diagram:
    d = Diagram(
        "schema-stack", "A dataset schema is the union of its segment schemas, which are unions of layer schemas"
    )
    d.node(0, 0, 488, NH, "my_dataset schema")
    d.node(0, 48, 240, NH, "segment_a schema")
    d.node(248, 48, 240, NH, "segment_b schema")
    d.node(0, 96, 116, NH, "base schema")
    d.node(124, 96, 116, NH, "extra schema")
    d.node(248, 96, 240, NH, "base schema")
    return d


def dataframe_query() -> Diagram:
    d = Diagram(
        "dataframe-query", "A Dataset, optionally filtered into a view, read into a DataFrame and collected into rows"
    )
    d.node(0, 0, 96, 48, "Dataset", "cyl")
    d.node(264, 6, 128, NH, "Dataset view")
    d.node(496, 6, 184, NH, "DataFusion DataFrame")
    d.node(504, 120, 176, 48, "Materialized rows\n(Arrow RecordBatch)", "page")
    d.arrow([(96, 26), (264, 26)], "`filter_contents()`\n`filter_segments()`")
    d.arrow([(392, 26), (496, 26)], "`reader()`")
    d.arrow([(48, 48), (48, 84), (536, 84), (536, 46)], "`reader()`", seg=1)
    d.arrow([(608, 46), (608, 120)], "`collect()`")
    return d


DIAGRAMS = [
    overview,
    log_and_ingest,
    stream_to_viewer,
    save_and_load,
    viewer_sources,
    prepare_catalog,
    query_catalog,
    dataloader,
    chunk_pipeline,
    catalog_object_model,
    schema_stack,
    dataframe_query,
]


def main() -> None:
    ap = argparse.ArgumentParser(description="Generate the docs diagrams as light and dark SVGs.")
    ap.add_argument("--out", type=Path, help="Folder to write the SVGs to. Defaults to a temporary folder.")
    ap.add_argument(
        "--upload",
        action="store_true",
        help="Upload the SVGs to static.rerun.io and print the HTML blocks to paste into the docs.",
    )
    args = ap.parse_args()

    out: Path = args.out or Path(tempfile.mkdtemp(prefix="docs_diagrams_"))
    out.mkdir(parents=True, exist_ok=True)

    diagrams = [build() for build in DIAGRAMS]
    warnings: list[str] = []
    for d in diagrams:
        for theme in THEMES:
            (out / f"diagram-{d.name}-{theme}.svg").write_text(d.render(theme), encoding="utf-8")
        w, h = round(d.w) + 1, round(d.h) + 1
        if w > MAX_W + 1:
            d.warnings.append(f"{d.name}: {w} wide, max is {MAX_W + 1}")
        print(f"{d.name:22} {w:4} x {h:<4}", file=sys.stderr)
        warnings += d.warnings
    for msg in warnings:
        print("WARNING", msg, file=sys.stderr)
    print(f"Wrote {2 * len(diagrams)} SVGs to {out}", file=sys.stderr)

    if not args.upload:
        return
    if warnings:
        raise SystemExit("Not uploading: fix the warnings above first.")

    # Imported lazily so generating the SVGs does not need upload_image's
    # dependencies (PIL, google-cloud-storage, …).
    from upload_image import Uploader

    uploader = Uploader()
    for d in diagrams:
        urls = {
            theme: f"https://static.rerun.io/{uploader.upload_file(out / f'diagram-{d.name}-{theme}.svg')}"
            for theme in THEMES
        }
        print(
            f"\n<!-- {d.name} -->\n"
            '<div class="d2-diagram">\n'
            f'  <img class="d2-dark" src="{urls["dark"]}" alt="{escape(d.title)}">\n'
            f'  <img class="d2-light" src="{urls["light"]}" alt="{escape(d.title)}">\n'
            "</div>"
        )


if __name__ == "__main__":
    main()
