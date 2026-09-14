#!/usr/bin/env python3
"""
Generate the crate dependency diagram shown in `ARCHITECTURE.md`.

Reads the real dependency graph from `cargo metadata`, so the diagram cannot
drift from the code. Only Rerun's own workspace crates are included; crates
under `examples/`, `tests/`, and `docs/` are left out.

Each workspace directory (`crates/viewer`, `crates/utils`, …) becomes one
horizontal band, and the bands never overlap vertically. Graphviz cannot do
that on its own, so the layout is done in two passes: each band is laid out
alone, then the bands are stacked and the cross-band edges are routed with
`neato -n2` over the resulting fixed positions.

Edges are transitively reduced: an edge is drawn only if the dependency is not
already implied by a longer path through other Rerun crates. Without this the
graph is unreadable, since nearly every crate depends on the likes of `re_log`.

Must run inside the pixi environment: it uses that environment's Graphviz and,
so that the checked-in SVG is the same on every platform, the fonts pinned
there rather than whatever the host has installed.

Usage:
    pixi run crate-graph

Writes `crate_graph.svg` next to `ARCHITECTURE.md`, and the crate tables into
`ARCHITECTURE.md` itself, between the `crate-tables` markers — descriptions come
from each crate's `description` in its `Cargo.toml`, so they cannot drift either.
Pass `--dot` to also write the composed Graphviz source, useful when tweaking
the layout.
"""

from __future__ import annotations

import argparse
import colorsys
import difflib
import functools
import html
import itertools
import json
import os
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

# The subset of `cargo metadata` output this script reads.
Metadata = dict[str, Any]

SCRIPT_DIR = Path(__file__).absolute().parent
RERUN_ROOT = SCRIPT_DIR.parent
DEFAULT_OUTPUT = RERUN_ROOT / "crate_graph.svg"
ARCHITECTURE = RERUN_ROOT / "ARCHITECTURE.md"

# The generated crate tables replace whatever sits between these two markers.
TABLES_START = "<!-- crate-tables:start -->"
TABLES_END = "<!-- crate-tables:end -->"

# Kept out of the generated tables, for crates listed by hand under a
# `Deprecated crates` heading instead.
DEPRECATED_CRATES: set[str] = set()

# One row per layer, top to bottom, as `(folder, heading, blurb)`.
# `scripts/check_crate_layers.py` enforces the order, so every arrow in the
# diagram points downwards. A row with two folders in it holds siblings: they do
# not depend on each other in either direction — `check_crate_layers.py` enforces
# that too — so they are drawn side by side rather than stacked.
LAYERS = [
    [("crates/tests", "Test support", "only tests depend on these, so they sit at the top of the diagram.")],
    [("crates/top", "SDK / CLI / Wasm", "the entry points for our users, and the viewer app itself.")],
    [
        (
            "crates/views",
            "Views",
            "the visualizations a user can put in the viewport.\nA sibling of `crates/panels`: neither depends on the other.",
        ),
        (
            "crates/panels",
            "Panels",
            "the panels the app is assembled from, and the widgets they are built out of.\nA sibling of `crates/views`: neither depends on the other.",
        ),
    ],
    [("crates/viewer_support", "Viewer support", "the UI and rendering machinery the views are built on.")],
    [("crates/store_app", "Application-level store", "the queryable state a viewer or a server works with.")],
    [("crates/data_flow", "Data flow", "getting data in and out: clients, servers, and file importers.")],
    [("crates/store", "Data model & chunk store", "the data model, and the in-memory store that holds it.")],
    [("crates/build", "Build support", "crates that run at build time.")],
    [("crates/utils", "Utilities", "small crates that depend on nothing outside `crates/utils`.")],
]

# The folders in layer order, ignoring which of them are siblings.
FOLDERS = [entry for row in LAYERS for entry in row]

# Each band gets its own hue, swept from red at the top to violet at the
# bottom, so the reader can tell how deep a crate sits from its color alone.
# The sweep stops short of a full turn, or the bottom band would be red again.
HUE_SWEEP = 0.8
SATURATION = 0.75
CRATE_LIGHTNESS = 0.82  # The crate boxes.
BAND_LIGHTNESS = 0.96  # The band behind them, which must stay far paler.

# Crates that are not part of any workspace directory listed in `LAYERS`.
EXTRA_CRATES = {
    "rerun_c": "crates/top",
    "rerun_py": "crates/top",
}

# `re_type_definitions` is never linked; it only exists as codegen input, so an
# edge to it would suggest a build dependency that does not exist.
SKIPPED_CRATES = {"re_type_definitions"}

POINTS_PER_INCH = 72.0
BAND_PADDING = 14.0  # Between a band's crates and the edge of its box, in points.
BAND_LABEL_HEIGHT = 34.0  # Room for the band label above its crates, in points.
BAND_GAP = 34.0  # Between two band boxes, in points.
BAND_LABEL_FONT_SIZE = 19.0
ARROW_HEAD = 9.0  # The layering arrow drawn in the gap between two rows of bands.

# Shipped by the `fonts-conda-ecosystem` package in `pixi.toml`, so every
# platform measures the label widths against the exact same TTF.
FONT = "DejaVu Sans"
FONT_FILE = "fonts/DejaVuSans.ttf"  # Relative to the pixi environment.
NODE_FONT_SIZE = 11.0
NODE_MARGIN = 0.09  # Inches on each side of a label, matching `NODE_DEFAULTS`.
MIN_NODE_WIDTH = 0.75  # Inches. Graphviz's own default, kept so narrow names still line up.

# `fixedsize` is what keeps the diagram identical everywhere: every node carries a
# width this script measured, so graphviz never measures a label itself. It would
# otherwise use pango on Linux and its own built-in estimates on macOS, and the two
# lay the same graph out over 100 points apart.
NODE_DEFAULTS = (
    f'node [fontname="{FONT}", fontsize={NODE_FONT_SIZE:.0f}, shape=box, style="rounded,filled", '
    'color="#00000033", margin="0.09,0.04", height=0.28, fixedsize=true]'
)
EDGE_DEFAULTS = 'edge [color="#00000055", arrowsize=0.6, penwidth=0.8]'
CORNER_ATTRS = 'style=invis, shape=point, width=0.01, label=""'

# Everything graphviz draws lives in this group, and its first child is the
# white background polygon. The band boxes go right after that: behind the
# crates, in front of the background.
SVG_BACKGROUND = re.compile(r'(<g id="graph0".*?<polygon[^>]*>)', re.DOTALL)

NUMBER = re.compile(r"-?\d+(?:\.\d+)?")

# A crate's box and the label graphviz centered in it: the `d` of the box holds
# alternating x and y, so its own extent says where the middle of the box is.
NODE_LABEL = re.compile(r'(<path[^>]*\bd="([^"]+)"[^>]*/>\s*<text[^>]*\by=")(-?\d+(?:\.\d+)?)(")')

# How far a number in the SVG may drift before `--check` calls the file stale,
# in points. The check compares the numbers with this tolerance rather than the
# file byte for byte; everything around them still has to match exactly, so a
# crate that appears, moves band, or changes name still fails.
#
# Two things drift. Graphviz rounds the corners of a node box with a little
# trigonometry, and the last digit it prints depends on the CPU: aarch64 and
# x86-64 disagree by 0.01 on a handful of control points. Larger, `dot` lays a
# band out up to a point wider on one platform than the other even when every
# node carries a width this script measured, and the widest band sets the width
# of the canvas, so that point reaches every coordinate in the file.
SVG_TOLERANCE = 1.5


def cargo_metadata() -> Metadata:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version=1", "--all-features"],
        cwd=RERUN_ROOT,
        text=True,
        capture_output=True,
    )
    if out.returncode != 0:
        sys.stderr.write(out.stderr)
        raise SystemExit(f"cargo metadata failed with exit code {out.returncode}")
    metadata: Metadata = json.loads(out.stdout)
    return metadata


def layer_of(manifest_path: Path, workspace_root: Path, name: str) -> str | None:
    directory = str(manifest_path.parent.relative_to(workspace_root).parent)
    if any(directory == layer for layer, _, _ in FOLDERS):
        return directory
    return EXTRA_CRATES.get(name)


def collect_graph(metadata: Metadata) -> tuple[dict[str, str], dict[str, set[str]]]:
    """Return (crate name -> layer, crate name -> names of its Rerun dependencies)."""
    workspace_root = Path(metadata["workspace_root"])
    members = set(metadata["workspace_members"])
    packages = {pkg["id"]: pkg for pkg in metadata["packages"]}

    layers: dict[str, str] = {}
    for package_id in members:
        package = packages[package_id]
        name = package["name"]
        if name in SKIPPED_CRATES:
            continue
        layer = layer_of(Path(package["manifest_path"]), workspace_root, name)
        if layer is not None:
            layers[name] = layer

    deps: dict[str, set[str]] = {name: set() for name in layers}
    for package_id in members:
        package = packages[package_id]
        if package["name"] not in layers:
            continue
        for dep in package["dependencies"]:
            if dep["kind"] == "dev":
                continue
            if dep["name"] in layers and dep["name"] != package["name"]:
                deps[package["name"]].add(dep["name"])

    return layers, deps


def transitive_reduction(deps: dict[str, set[str]]) -> dict[str, set[str]]:
    """Drop every edge that is already implied by a path of two or more edges."""
    reachable: dict[str, set[str]] = {}

    def reach(name: str) -> set[str]:
        # The dependency graph is acyclic, so plain memoized recursion terminates.
        if name not in reachable:
            reachable[name] = set()  # Guard against a cycle sneaking in.
            out: set[str] = set()
            for dep in deps[name]:
                out.add(dep)
                out |= reach(dep)
            reachable[name] = out
        return reachable[name]

    reduced: dict[str, set[str]] = {}
    for name, direct in deps.items():
        indirect: set[str] = set()
        for dep in direct:
            indirect |= reach(dep)
        reduced[name] = direct - indirect
    return reduced


class Node:
    """A laid-out crate box. Coordinates are in points, y growing upwards."""

    def __init__(self, x: float, y: float, width: float, height: float) -> None:
        self.x = x
        self.y = y
        self.width = width
        self.height = height


class Band:
    """A laid-out band: one workspace directory and the crates it contains."""

    def __init__(self, label: str, colors: tuple[str, str], nodes: dict[str, Node]) -> None:
        self.label = label
        self.crate_color, self.color = colors
        self.nodes = nodes
        self.left = 0.0
        self.bottom = 0.0
        self.width = 0.0
        self.height = 0.0


def hues(index: int) -> tuple[str, str]:
    """The (crate, band) colors of the band at `index`, as `#rrggbb`."""
    hue = HUE_SWEEP * index / max(len(FOLDERS) - 1, 1)

    def hex_color(lightness: float) -> str:
        channels = colorsys.hls_to_rgb(hue, lightness, SATURATION)
        return "#" + "".join(f"{round(channel * 255):02x}" for channel in channels)

    return hex_color(CRATE_LIGHTNESS), hex_color(BAND_LIGHTNESS)


@functools.lru_cache(maxsize=1)
def graphviz_env() -> dict[str, str]:
    """The environment `dot` and `neato` must run under to lay out reproducibly.

    Graphviz sizes every node from the measured width of its label, so the SVG
    is only stable if the same font file is measured the same way everywhere:

    * `FONTCONFIG_FILE` points at a config that lists the fonts shipped in the
      pixi environment and nothing else. Without it the host's own fonts are
      searched first, and a name resolves to a different face on each platform.
    * `PANGOCAIRO_BACKEND=fc` keeps pango on fontconfig. It is already the only
      backend on Linux, but on macOS pango defaults to CoreText, which ignores
      fontconfig and measures differently.
    """
    prefix = os.environ.get("CONDA_PREFIX")
    fonts = Path(prefix, "fonts") if prefix else None
    if fonts is None or not fonts.is_dir():
        raise SystemExit("no fonts in the pixi environment; run this under `pixi run`")

    config_dir = Path(tempfile.mkdtemp(prefix="crate-graph-fonts-"))
    config = config_dir / "fonts.conf"
    config.write_text(
        '<?xml version="1.0"?>\n'
        '<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">\n'
        f"<fontconfig><dir>{fonts}</dir><cachedir>{config_dir}</cachedir></fontconfig>\n"
    )

    return os.environ | {"FONTCONFIG_FILE": str(config), "PANGOCAIRO_BACKEND": "fc"}


def font_tables(data: bytes) -> dict[bytes, tuple[int, int]]:
    """Map each TTF table tag to its `(offset, length)`."""
    (num_tables,) = struct.unpack_from(">H", data, 4)
    tables = {}
    for i in range(num_tables):
        tag, _checksum, offset, length = struct.unpack_from(">4sLLL", data, 12 + 16 * i)
        tables[tag] = (offset, length)
    return tables


def character_glyphs(data: bytes, cmap_offset: int) -> dict[int, int]:
    """Read the Unicode BMP (format 4) subtable of `cmap` into character -> glyph."""
    (num_subtables,) = struct.unpack_from(">H", data, cmap_offset + 2)
    subtable = None
    for i in range(num_subtables):
        platform, encoding, offset = struct.unpack_from(">HHL", data, cmap_offset + 4 + 8 * i)
        if (platform, encoding) in {(3, 1), (0, 3), (0, 4)}:
            subtable = cmap_offset + offset
            break
    if subtable is None:
        raise SystemExit(f"{FONT_FILE} has no Unicode cmap subtable")

    (fmt, _length, _language, segment_count_x2) = struct.unpack_from(">HHHH", data, subtable)
    if fmt != 4:
        raise SystemExit(f"{FONT_FILE} uses cmap format {fmt}, which this script cannot read")
    segments = segment_count_x2 // 2

    ends_at = subtable + 14
    starts_at = ends_at + segment_count_x2 + 2
    deltas_at = starts_at + segment_count_x2
    ranges_at = deltas_at + segment_count_x2
    ends = struct.unpack_from(f">{segments}H", data, ends_at)
    starts = struct.unpack_from(f">{segments}H", data, starts_at)
    deltas = struct.unpack_from(f">{segments}h", data, deltas_at)
    range_offsets = struct.unpack_from(f">{segments}H", data, ranges_at)

    glyphs: dict[int, int] = {}
    for i in range(segments):
        for code in range(starts[i], min(ends[i], 0xFFFF) + 1):
            if range_offsets[i] == 0:
                glyph = (code + deltas[i]) & 0xFFFF
            else:
                at = ranges_at + 2 * i + range_offsets[i] + 2 * (code - starts[i])
                (glyph,) = struct.unpack_from(">H", data, at)
                if glyph != 0:
                    glyph = (glyph + deltas[i]) & 0xFFFF
            if glyph != 0:
                glyphs[code] = glyph
    return glyphs


@functools.cache
def font() -> tuple[bytes, dict[bytes, tuple[int, int]], int]:
    """`(the pinned TTF, its tables, its units per em)`."""
    prefix = os.environ.get("CONDA_PREFIX")
    if prefix is None:
        raise SystemExit("Run inside the pixi environment: `pixi run crate-graph`")
    path = Path(prefix) / FONT_FILE
    if not path.exists():
        raise SystemExit(f"The pinned font is missing. Expected it at: {path}")
    data = path.read_bytes()
    tables = font_tables(data)
    (units_per_em,) = struct.unpack_from(">H", data, tables[b"head"][0] + 18)
    return data, tables, units_per_em


@functools.cache
def cap_height() -> float:
    """The height of a capital `H`, in points at the node font size."""
    data, tables, units_per_em = font()
    glyph = character_glyphs(data, tables[b"cmap"][0])[ord("H")]
    (long_offsets,) = struct.unpack_from(">h", data, tables[b"head"][0] + 50)
    if long_offsets:
        (at,) = struct.unpack_from(">L", data, tables[b"loca"][0] + 4 * glyph)
    else:
        (short,) = struct.unpack_from(">H", data, tables[b"loca"][0] + 2 * glyph)
        at = short * 2
    (top,) = struct.unpack_from(">h", data, tables[b"glyf"][0] + at + 8)
    return float(top) * NODE_FONT_SIZE / units_per_em


@functools.cache
def font_metrics() -> tuple[int, dict[int, int]]:
    """`(units per em, advance width per character)` of the pinned TTF."""
    data, tables, units_per_em = font()
    (metric_count,) = struct.unpack_from(">H", data, tables[b"hhea"][0] + 34)
    hmtx = tables[b"hmtx"][0]
    advances = struct.unpack_from(f">{metric_count * 2}H", data, hmtx)[::2]

    glyphs = character_glyphs(data, tables[b"cmap"][0])
    # Glyphs past the last entry of `hmtx` all share its advance, by the spec.
    width_of = {code: advances[min(glyph, metric_count - 1)] for code, glyph in glyphs.items()}
    return units_per_em, width_of


def node_width(label: str) -> float:
    """The width in inches graphviz should give `label`'s box."""
    units_per_em, width_of = font_metrics()
    missing = [character for character in label if ord(character) not in width_of]
    if missing:
        raise SystemExit(f"{FONT} has no glyph for {missing!r} in the label: {label}")
    em_widths = sum(width_of[ord(character)] for character in label) / units_per_em
    return max(MIN_NODE_WIDTH, em_widths * NODE_FONT_SIZE / POINTS_PER_INCH + 2 * NODE_MARGIN)


def run_graphviz(engine: str, args: list[str], source: str) -> str:
    out = subprocess.run([engine, *args], input=source, text=True, capture_output=True, env=graphviz_env())
    if out.returncode != 0:
        sys.stderr.write(out.stderr)
        raise SystemExit(f"{engine} failed with exit code {out.returncode}")
    return out.stdout


def content_size(nodes: dict[str, Node]) -> tuple[float, float]:
    """The width and height the nodes cover, in points."""
    return (
        max(node.x + node.width / 2 for node in nodes.values()),
        max(node.y + node.height / 2 for node in nodes.values()),
    )


def layout_band(crates: list[str], deps: dict[str, set[str]]) -> dict[str, Node]:
    """Lay out one band on its own, using only the dependencies internal to it."""
    within = set(crates)
    edges = [(name, dep) for name in crates for dep in sorted(deps[name] & within)]
    connected = {name for edge in edges for name in edge}
    # Crates with no dependency inside their band would all land on one rank, which
    # makes a band like `crates/utils` a single very wide row. Wrapping them over
    # several ranks fixes that, but how many columns to wrap at cannot be read off
    # their count alone: the crates that do depend on each other are laid out beside
    # the wrapped ones and take columns of their own. So every width is tried, and
    # the narrowest band that is still no taller than it is wide wins — the bands read
    # as horizontal layers, and one on end stops looking like a layer at all.
    loose = [name for name in crates if name not in connected]

    def upright_then_narrow(nodes: dict[str, Node]) -> tuple[bool, float]:
        width, height = content_size(nodes)
        return (width < height, width)

    return min(
        (wrapped_band(crates, edges, loose, columns) for columns in range(1, max(len(loose), 1) + 1)),
        key=upright_then_narrow,
    )


def wrapped_band(
    crates: list[str],
    edges: list[tuple[str, str]],
    loose: list[str],
    columns: int,
) -> dict[str, Node]:
    """Lay the band out with its dependency-free crates wrapped into `columns` columns."""
    lines = [
        "digraph band {",
        "  rankdir=TB",
        "  ranksep=0.4",
        "  nodesep=0.18",
        f"  {NODE_DEFAULTS}",
    ]
    lines += [f'  "{name}" [width={node_width(name):.3f}]' for name in crates]
    lines += [f'  "{name}" -> "{dep}"' for name, dep in edges]

    rows = [loose[i : i + columns] for i in range(0, len(loose), columns)]
    for row in rows:
        lines.append("  { rank=same; " + " ".join(f'"{name}"' for name in row) + " }")
    # Invisible edges keep the wrapped rows in order, one under the next.
    for upper, lower in itertools.pairwise(rows):
        lines.append(f'  "{upper[0]}" -> "{lower[0]}" [style=invis]')

    lines.append("}")

    nodes: dict[str, Node] = {}
    for line in run_graphviz("dot", ["-Tplain"], "\n".join(lines)).splitlines():
        fields = line.split()
        if fields[0] == "node":
            name, x, y, width, height = fields[1], *(float(f) for f in fields[2:6])
            nodes[name.strip('"')] = Node(
                x * POINTS_PER_INCH,
                y * POINTS_PER_INCH,
                width * POINTS_PER_INCH,
                height * POINTS_PER_INCH,
            )
    return nodes


def stack_bands(layers: dict[str, str], deps: dict[str, set[str]]) -> list[Band]:
    """Lay out every band, then stack the rows bottom-up so none of them overlap.

    Siblings share a row: they are placed next to each other, and the row is as
    tall as the taller of the two.
    """
    index = 0
    rows: list[list[Band]] = []
    for row in LAYERS:
        laid_out = []
        for layer, label, _ in row:
            crates = sorted(name for name, crate_layer in layers.items() if crate_layer == layer)
            laid_out.append(Band(label, hues(index), layout_band(crates, deps)))
            index += 1
        rows.append(laid_out)

    # Every row spans the full width, so the rows read as layers.
    width = max(
        sum(content_size(band.nodes)[0] + 2 * BAND_PADDING for band in row) + BAND_GAP * (len(row) - 1) for row in rows
    )

    bottom = 0.0
    for band_row in reversed(rows):  # Graphviz y grows upwards, so build bottom-up.
        sizes = [content_size(band.nodes) for band in band_row]
        # Share the slack between the bands of the row, in proportion to their content.
        slack = width - sum(w + 2 * BAND_PADDING for w, _ in sizes) - BAND_GAP * (len(band_row) - 1)
        total_content = sum(w for w, _ in sizes) or 1.0
        height = max(h for _, h in sizes) + 2 * BAND_PADDING + BAND_LABEL_HEIGHT

        tallest = max(h for _, h in sizes)
        left = 0.0
        for band, (band_width, band_height) in zip(band_row, sizes):
            box_width = band_width + 2 * BAND_PADDING + slack * band_width / total_content
            indent = (box_width - band_width) / 2
            # Hang a short band from the top of the row, under its own label,
            # instead of leaving a gap between the label and the crates.
            drop = tallest - band_height
            for node in band.nodes.values():
                node.x += left + indent
                node.y += bottom + BAND_PADDING + drop
            band.left = left
            band.bottom = bottom
            band.width = box_width
            band.height = height
            left += box_width + BAND_GAP

        bottom += height + BAND_GAP

    return [band for row in rows for band in row]


def compose(bands: list[Band], deps: dict[str, set[str]]) -> str:
    """A graph with every crate pinned to the position `stack_bands` gave it."""
    top = max(band.bottom + band.height for band in bands)
    lines = [
        "digraph rerun {",
        "  bgcolor=white",
        f"  {NODE_DEFAULTS}",
        f"  {EDGE_DEFAULTS}",
        "",
        # The band boxes are drawn into the SVG afterwards, so `neato` does not
        # know about them. These two pinned corners keep them inside the canvas.
        f'  "canvas_min" [pos="0,0!", {CORNER_ATTRS}]',
        f'  "canvas_max" [pos="{max(band.width for band in bands)},{top}!", {CORNER_ATTRS}]',
        "",
    ]

    for band in bands:
        for name, node in sorted(band.nodes.items()):
            lines.append(
                f'  "{name}" [pos="{node.x:.1f},{node.y:.1f}!", '
                f"width={node.width / POINTS_PER_INCH:.3f}, "
                f"height={node.height / POINTS_PER_INCH:.3f}, "
                f'fillcolor="{band.crate_color}"]'
            )
    lines.append("")

    for name in sorted(deps):
        for dep in sorted(deps[name]):
            lines.append(f'  "{name}" -> "{dep}"')

    lines.append("}")
    return "\n".join(lines) + "\n"


def render_svg(composed: str, bands: list[Band]) -> str:
    # `-n2` takes the pinned node positions as given and only routes the edges.
    svg = run_graphviz("neato", ["-n2", "-Gsplines=true", "-Tsvg"], composed)

    # Graphviz stamps its own version into a comment; dropping it keeps the
    # checked-in file from changing just because a contributor has another dot.
    svg = re.sub(r"<!-- Generated by graphviz.*?-->\n", "", svg, flags=re.DOTALL)

    # Graphviz names the font it measured and nothing else. Readers rarely have
    # it installed, and the labels are sized for it, so fall back to whatever
    # sans-serif the reader does have.
    svg = svg.replace(f'font-family="{FONT}"', f'font-family="{FONT},sans-serif"')

    svg = centered_labels(svg)

    match = SVG_BACKGROUND.search(svg)
    if match is None:
        raise SystemExit("could not find the background of the SVG that neato produced")
    return svg.replace(match.group(1), match.group(1) + band_boxes(bands), 1)


def centered_labels(svg: str) -> str:
    """Put every crate label's baseline where the pinned font says it belongs.

    Node widths are this script's to give, but the baseline inside the box stays
    graphviz's, and it derives that from the font's vertical metrics — pango's on
    Linux, its own estimates on macOS, three quarters of a point apart. Centering
    the capitals is a rule the TTF answers on its own, so both platforms agree.
    """

    def recenter(match: re.Match[str]) -> str:
        coordinates = [float(number) for number in NUMBER.findall(match.group(2))]
        vertical = coordinates[1::2]
        middle = (min(vertical) + max(vertical)) / 2
        return f"{match.group(1)}{middle + cap_height() / 2:.2f}{match.group(4)}"

    return NODE_LABEL.sub(recenter, svg)


def layer_arrows(bands: list[Band]) -> str:
    """A downward arrow in every gap between two rows of bands.

    With the cross-band arrows gone, nothing else shows which way a dependency
    is allowed to run, so each gap gets arrows saying "everything above may
    depend on everything below". They follow whichever of the two rows has more
    boxes, so an arrow always starts and lands inside a box rather than in the
    gap between two siblings.
    """
    rows = sorted({band.bottom for band in bands}, reverse=True)  # Top row first.

    out = []
    for upper, lower in itertools.pairwise(rows):
        above = [band for band in bands if band.bottom == upper]
        below = [band for band in bands if band.bottom == lower]
        lower_top = max(band.bottom + band.height for band in below)
        # y is flipped in the group these are emitted into.
        start, end = -upper, -lower_top

        for band in below if len(below) > len(above) else above:
            x = band.left + band.width / 2
            out.append(
                f'\n<path d="M {x:.1f},{start:.1f} L {x:.1f},{end - ARROW_HEAD:.1f}" '
                f'stroke="#00000044" stroke-width="3" fill="none" />'
            )
            out.append(
                f'\n<path d="M {x - ARROW_HEAD * 0.6:.1f},{end - ARROW_HEAD:.1f} '
                f"L {x + ARROW_HEAD * 0.6:.1f},{end - ARROW_HEAD:.1f} "
                f'L {x:.1f},{end:.1f} Z" fill="#00000044" />'
            )
    return "".join(out)


def band_boxes(bands: list[Band]) -> str:
    """Rounded background boxes with a label, one per band.

    The group these are emitted into already flips y, hence the negated
    coordinates.
    """
    out = [layer_arrows(bands)]
    for band in bands:
        top = -(band.bottom + band.height)
        out.append(
            f'\n<rect x="{band.left:.1f}" y="{top:.1f}" '
            f'width="{band.width:.1f}" height="{band.height:.1f}" '
            f'rx="8" ry="8" fill="{band.color}" />'
        )
        out.append(
            f'\n<text x="{band.left + BAND_PADDING:.1f}" '
            f'y="{top + BAND_LABEL_FONT_SIZE + 4:.1f}" '
            f'font-family="{FONT},sans-serif" font-size="{BAND_LABEL_FONT_SIZE:.0f}" '
            f'fill="#00000099">{html.escape(band.label)}</text>'
        )
    return "".join(out)


def crate_tables(metadata: Metadata) -> str:
    """One markdown table per folder under `crates/`, in layer order.

    The description of a crate is its `description` field in `Cargo.toml`, so
    there is only one place to write it.
    """
    workspace_root = Path(metadata["workspace_root"])
    members = set(metadata["workspace_members"])
    folders = {layer for layer, _, _ in FOLDERS}

    described: dict[str, tuple[str, str]] = {}
    for package in metadata["packages"]:
        if package["id"] not in members or package["name"] in DEPRECATED_CRATES:
            continue
        directory = str(Path(package["manifest_path"]).parent.relative_to(workspace_root).parent)
        folder = directory if directory in folders else EXTRA_CRATES.get(package["name"], "")
        if folder:
            described[package["name"]] = (folder, (package.get("description") or "").strip())

    blocks = []
    for layer, label, blurb in FOLDERS:
        rows = sorted((name, desc) for name, (folder, desc) in described.items() if folder == layer)
        if not rows:
            continue
        widths = [
            max(len("Crate"), *(len(name) for name, _ in rows)),
            max(len("Description"), *(len(desc) for _, desc in rows)),
        ]
        table = [
            f"| {'Crate'.ljust(widths[0])} | {'Description'.ljust(widths[1])} |",
            f"| {'-' * widths[0]} | {'-' * widths[1]} |",
        ]
        table += [f"| {name.ljust(widths[0])} | {desc.ljust(widths[1])} |" for name, desc in rows]
        blocks.append(f"### {label}\n\n[`{layer}`](./{layer}) — {blurb}\n\n" + "\n".join(table) + "\n")

    return "\n".join(blocks)


def matches(path: Path, wanted: str) -> bool:
    """Is the file on disk what this script would write?

    The SVG is compared with `SVG_TOLERANCE` applied to every number in it, so
    that a checkout does not go stale over the last digit of a coordinate.
    Everything else is compared exactly.
    """
    if not path.exists():
        return False
    found = path.read_text()
    if found == wanted:
        return True
    if path.suffix != ".svg":
        return False

    if NUMBER.split(found) != NUMBER.split(wanted):
        return False
    return all(
        abs(float(a) - float(b)) <= SVG_TOLERANCE
        for a, b in zip(NUMBER.findall(found), NUMBER.findall(wanted), strict=True)
    )


def drift_report(path: Path, wanted: str) -> str:
    """How far the numbers in `path` have drifted from `wanted`, worst first.

    The unified diff below it shows only the first differing lines, which says
    nothing about the largest drift in the file — the number `SVG_TOLERANCE`
    has to clear.
    """
    if path.suffix != ".svg" or not path.exists():
        return ""
    found = NUMBER.findall(path.read_text())
    generated = NUMBER.findall(wanted)
    if len(found) != len(generated):
        return f"The file holds {len(found)} numbers, the generated one {len(generated)}.\n"

    drifts = sorted((abs(float(a) - float(b)), a, b) for a, b in zip(found, generated, strict=True) if a != b)
    if not drifts:
        return "Every number matches; the difference is elsewhere in the file.\n"
    worst = "".join(f"  {found} -> {generated} ({drift:.2f})\n" for drift, found, generated in drifts[-5:])
    return (
        f"{len(drifts)} of {len(found)} numbers differ, by at most "
        f"{drifts[-1][0]:.2f} (tolerance {SVG_TOLERANCE}). Widest:\n{worst}"
    )


def architecture_with_tables(text: str, tables: str) -> str:
    """`ARCHITECTURE.md` with everything between the markers replaced by `tables`."""
    try:
        start = text.index(TABLES_START) + len(TABLES_START)
        end = text.index(TABLES_END)
    except ValueError:
        raise SystemExit(f"{ARCHITECTURE} has no {TABLES_START} / {TABLES_END} markers") from None
    return text[:start] + "\n\n" + tables + "\n" + text[end:]


def within_bands(layers: dict[str, str], deps: dict[str, set[str]]) -> dict[str, set[str]]:
    """`deps` with every cross-band edge dropped."""
    return {name: {dep for dep in dependencies if layers[dep] == layers[name]} for name, dependencies in deps.items()}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT, help="where to write the SVG")
    parser.add_argument("--dot", type=Path, help="also write the composed Graphviz source here")
    parser.add_argument(
        "--edges",
        choices=["all", "within-bands"],
        default="within-bands",
        help=(
            "which arrows to draw. `within-bands` (default) draws only the dependencies inside a band: "
            "the layering already says a band may only depend downwards, and the hundreds of cross-band "
            "arrows overlap into noise. `all` draws every dependency, which is worth a look when you "
            "want to see exactly what a crate pulls in — give `--output` a different path to keep both"
        ),
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the files on disk are not what this script would write, instead of writing them",
    )
    args = parser.parse_args()

    metadata = cargo_metadata()
    layers, deps = collect_graph(metadata)
    deps = transitive_reduction(deps)
    bands = stack_bands(layers, deps)

    if args.edges == "within-bands":
        deps = within_bands(layers, deps)

    composed = compose(bands, deps)
    if args.dot:
        args.dot.write_text(composed)

    svg = render_svg(composed, bands)
    architecture = architecture_with_tables(ARCHITECTURE.read_text(), crate_tables(metadata))

    wanted = {args.output: svg, ARCHITECTURE: architecture}

    if args.check:
        stale = [path for path, content in wanted.items() if not matches(path, content)]
        if stale:
            names = ", ".join(path.name for path in stale)
            sys.stderr.write(f"{names} out of date.\nRegenerate with `pixi run crate-graph`, and commit the result.\n")
            for path in stale:
                sys.stderr.write(f"\nFirst differences in {path.name}:\n")
                sys.stderr.write(drift_report(path, wanted[path]))
                diff = difflib.unified_diff(
                    path.read_text().splitlines() if path.exists() else [],
                    wanted[path].splitlines(),
                    fromfile="committed",
                    tofile="generated",
                    lineterm="",
                    n=0,
                )
                sys.stderr.writelines(f"{line}\n" for line in itertools.islice(diff, 30))
            raise SystemExit(1)
        print(f"{args.output.name} and {ARCHITECTURE.name} are up to date ({len(layers)} crates)")
        return

    for path, content in wanted.items():
        path.write_text(content)
    print(f"Wrote {args.output.name} and the crate tables in {ARCHITECTURE.name} ({len(layers)} crates)")


if __name__ == "__main__":
    main()
