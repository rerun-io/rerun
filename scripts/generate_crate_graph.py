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
import difflib
import functools
import html
import itertools
import json
import os
import re
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

# Listed under `Deprecated crates`, which is hand-written, so keep them out of
# the generated tables.
DEPRECATED_CRATES = {"re_types"}

# One band each, top to bottom. `scripts/check_crate_layers.py` enforces this
# order, so every arrow in the diagram points downwards.
LAYERS = [
    ("crates/tests", "Test support", "#d9cdea"),
    ("crates/top", "SDK / CLI / Wasm", "#f7c9a1"),
    ("crates/viewer", "Viewer", "#b9d5f0"),
    ("crates/store", "Store & data flow", "#bfe3bf"),
    ("crates/build", "Build support", "#eec4dd"),
    ("crates/utils", "Utilities", "#d8d8e4"),
]

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

# Shipped by the `fonts-conda-ecosystem` package in `pixi.toml`, so every
# platform measures the label widths against the exact same TTF.
FONT = "DejaVu Sans"

NODE_DEFAULTS = (
    f'node [fontname="{FONT}", fontsize=11, shape=box, style="rounded,filled", '
    'color="#00000033", margin="0.09,0.04", height=0.28]'
)
EDGE_DEFAULTS = 'edge [color="#00000055", arrowsize=0.6, penwidth=0.8]'
CORNER_ATTRS = 'style=invis, shape=point, width=0.01, label=""'

# Everything graphviz draws lives in this group, and its first child is the
# white background polygon. The band boxes go right after that: behind the
# crates, in front of the background.
SVG_BACKGROUND = re.compile(r'(<g id="graph0".*?<polygon[^>]*>)', re.DOTALL)

NUMBER = re.compile(r"-?\d+(?:\.\d+)?")

# Graphviz rounds the corners of a node box with a little trigonometry, and the
# last digit it prints depends on the CPU: aarch64 and x86-64 disagree by 0.01
# on a handful of control points. `--check` therefore compares the SVG's numbers
# with this tolerance, in points, rather than byte for byte. Everything else in
# the file — every position, spline, and label — has to match exactly.
SVG_TOLERANCE = 0.05


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
    if any(directory == layer for layer, _, _ in LAYERS):
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

    def __init__(self, label: str, color: str, nodes: dict[str, Node]) -> None:
        self.label = label
        self.color = color
        self.nodes = nodes
        self.left = 0.0
        self.bottom = 0.0
        self.width = 0.0
        self.height = 0.0


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


def run_graphviz(engine: str, args: list[str], source: str) -> str:
    out = subprocess.run([engine, *args], input=source, text=True, capture_output=True, env=graphviz_env())
    if out.returncode != 0:
        sys.stderr.write(out.stderr)
        raise SystemExit(f"{engine} failed with exit code {out.returncode}")
    return out.stdout


def layout_band(crates: list[str], deps: dict[str, set[str]]) -> dict[str, Node]:
    """Lay out one band on its own, using only the dependencies internal to it."""
    within = set(crates)
    lines = [
        "digraph band {",
        "  rankdir=TB",
        "  ranksep=0.4",
        "  nodesep=0.18",
        f"  {NODE_DEFAULTS}",
    ]
    lines += [f'  "{name}"' for name in crates]
    lines += [f'  "{name}" -> "{dep}"' for name in crates for dep in sorted(deps[name] & within)]
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
    """Lay out every band, then stack them bottom-up into non-overlapping rows."""
    bands = []
    for layer, label, color in LAYERS:
        crates = sorted(name for name, crate_layer in layers.items() if crate_layer == layer)
        bands.append(Band(label, color, layout_band(crates, deps)))

    # All bands are given the same width so that they read as rows.
    width = max(max(node.x + node.width / 2 for node in band.nodes.values()) for band in bands) + 2 * BAND_PADDING

    bottom = 0.0
    for band in reversed(bands):  # Graphviz y grows upwards, so build bottom-up.
        height = max(node.y + node.height / 2 for node in band.nodes.values())
        band_width = max(node.x + node.width / 2 for node in band.nodes.values())
        indent = (width - 2 * BAND_PADDING - band_width) / 2
        for node in band.nodes.values():
            node.x += indent + BAND_PADDING
            node.y += bottom + BAND_PADDING
        band.left = 0.0
        band.bottom = bottom
        band.width = width
        band.height = height + 2 * BAND_PADDING + BAND_LABEL_HEIGHT
        bottom += band.height + BAND_GAP

    return bands


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
                f'fillcolor="{band.color}"]'
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

    match = SVG_BACKGROUND.search(svg)
    if match is None:
        raise SystemExit("could not find the background of the SVG that neato produced")
    return svg.replace(match.group(1), match.group(1) + band_boxes(bands), 1)


def band_boxes(bands: list[Band]) -> str:
    """Rounded background boxes with a label, one per band.

    The group these are emitted into already flips y, hence the negated
    coordinates.
    """
    out = []
    for band in bands:
        top = -(band.bottom + band.height)
        out.append(
            f'\n<rect x="{band.left:.1f}" y="{top:.1f}" '
            f'width="{band.width:.1f}" height="{band.height:.1f}" '
            f'rx="8" ry="8" fill="{band.color}44" />'
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

    described: dict[str, tuple[str, str, str]] = {}
    for package in metadata["packages"]:
        if package["id"] not in members or package["name"] in DEPRECATED_CRATES:
            continue
        folder = str(Path(package["manifest_path"]).parent.relative_to(workspace_root).parent)
        folder = folder if folder in {layer for layer, _, _ in LAYERS} else EXTRA_CRATES.get(package["name"], "")
        if folder:
            described[package["name"]] = (folder, package["name"], (package.get("description") or "").strip())

    blocks = []
    for layer, label, _ in LAYERS:
        rows = sorted((name, desc) for _, name, desc in described.values() if described[name][0] == layer)
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
        blocks.append(f"### {label}\n\n[`{layer}`](./{layer})\n\n" + "\n".join(table) + "\n")

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


def architecture_with_tables(text: str, tables: str) -> str:
    """`ARCHITECTURE.md` with everything between the markers replaced by `tables`."""
    try:
        start = text.index(TABLES_START) + len(TABLES_START)
        end = text.index(TABLES_END)
    except ValueError:
        raise SystemExit(f"{ARCHITECTURE} has no {TABLES_START} / {TABLES_END} markers") from None
    return text[:start] + "\n\n" + tables + "\n" + text[end:]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT, help="where to write the SVG")
    parser.add_argument("--dot", type=Path, help="also write the composed Graphviz source here")
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
