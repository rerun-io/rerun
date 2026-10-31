#!/usr/bin/env python3

"""
Check that the folders under `crates/` form layers, each depending only on the ones below it.

The layers, top to bottom, are `LAYERS` in `crate_layers.py`. A crate may depend on crates in its own folder,
or in any folder in a lower layer. `dev-dependencies` are exempt: a test may reach anywhere.

A layer can hold more than one column, and those are siblings: independent of each other, so
neither may depend on the other in either direction. That is what lets `ARCHITECTURE.md` draw
them side by side. A column can in turn stack several folders, each depending only on itself
and the ones under it.

The same layering is what `ARCHITECTURE.md` draws, one band per folder, so a violation here is
an arrow pointing the wrong way in that diagram.

`FORBIDDEN_DEPENDENCIES` lists `(crate, dependency)` pairs where `crate` must not depend on
`dependency`, directly or through other crates, even when they share a folder.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

from crate_layers import FOLDERS, LAYERS

FORBIDDEN_DEPENDENCIES = [
    # re_agent_ui is a generic chat UI for coding agents; everything that ties it to the
    # Rerun Viewer belongs in re_agent_panel. See the README of either crate.
    ("re_agent_ui", "re_viewer_context"),
    ("re_agent_ui", "re_viewer_control_tools"),
    # The generated types describe how we encode data as Arrow;
    # it should not need to understand our Sorbet spec.
    ("re_sdk_types", "re_sorbet"),
    # re_sorbet defines how we name our column names and Arrow metadata;
    # it should never know about the higher level types in re_sdk_types.
    ("re_sorbet", "re_sdk_types"),
]

RERUN_ROOT = Path(__file__).absolute().parent.parent


def cargo_metadata() -> dict[str, Any]:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version=1", "--no-deps", "--all-features"],
        cwd=RERUN_ROOT,
        text=True,
        capture_output=True,
    )
    if out.returncode != 0:
        sys.stderr.write(out.stderr)
        raise SystemExit(f"cargo metadata failed with exit code {out.returncode}")
    metadata: dict[str, Any] = json.loads(out.stdout)
    return metadata


def allowed_and_siblings(folder: str) -> tuple[set[str], set[str]]:
    """The folders `folder` may depend on, and the sibling folders it must not."""
    for index, layer in enumerate(LAYERS):
        for column in layer:
            if folder in column:
                below = {f for lower in LAYERS[index + 1 :] for col in lower for f in col}
                siblings = {f for other in layer if other is not column for f in other}
                return set(column[column.index(folder) :]) | below, siblings
    raise ValueError(f"{folder} is not in LAYERS")


def dependency_chain(normal_deps: dict[str, set[str]], start: str, target: str) -> list[str] | None:
    """The shortest chain of non-dev dependencies from `start` to `target`, if there is one."""
    parent: dict[str, str] = {}
    queue = [start]
    seen = {start}
    while queue:
        crate = queue.pop(0)
        if crate == target:
            chain = [crate]
            while chain[-1] != start:
                chain.append(parent[chain[-1]])
            return chain[::-1]
        for dep in sorted(normal_deps.get(crate, ())):
            if dep not in seen:
                seen.add(dep)
                parent[dep] = crate
                queue.append(dep)
    return None


def main() -> int:
    metadata = cargo_metadata()
    workspace_root = Path(metadata["workspace_root"])

    # Crates outside `crates/` (examples, `tests/rust`, …) are leaves that nothing depends on,
    # so they have no layer to violate.
    layer_of_crate = {}
    for package in metadata["packages"]:
        folder = str(Path(package["manifest_path"]).parent.relative_to(workspace_root).parent)
        if folder in FOLDERS:
            layer_of_crate[package["name"]] = folder

    errors = set()  # Target-specific sections can repeat a dependency.
    for package in metadata["packages"]:
        layer = layer_of_crate.get(package["name"])
        if layer is None:
            continue
        allowed, siblings = allowed_and_siblings(layer)
        for dep in package["dependencies"]:
            if dep["kind"] == "dev":
                continue
            dep_layer = layer_of_crate.get(dep["name"])
            if dep_layer is None or dep_layer in allowed:
                continue
            where = "a sibling of it" if dep_layer in siblings else "above it"
            errors.add(f"{layer}/{package['name']} depends on {dep['name']} in {dep_layer}, which is {where}")

    workspace_crates = {package["name"] for package in metadata["packages"]}
    normal_deps = {
        package["name"]: {
            dep["name"] for dep in package["dependencies"] if dep["kind"] != "dev" and dep["name"] in workspace_crates
        }
        for package in metadata["packages"]
    }
    for crate, dependency in FORBIDDEN_DEPENDENCIES:
        chain = dependency_chain(normal_deps, crate, dependency)
        if chain is not None:
            errors.add(f"{crate} must not depend on {dependency}, but does: {' → '.join(chain)}")

    if errors:
        print("The crate layering is violated:\n")
        for error in sorted(errors):
            print(f"  {error}")
        print("\nEither move a crate to a lower folder, or make the dependency a dev-dependency.")
        print("Sibling columns, listed together below, must not depend on each other at all.")
        print("Layers, top to bottom:")
        for columns in LAYERS:
            print(f"  {' + '.join(' > '.join(column) for column in columns)}")
        print("Forbidden dependencies:")
        for crate, dependency in FORBIDDEN_DEPENDENCIES:
            print(f"  {crate} → {dependency}")
        return 1

    print(f"All {len(layer_of_crate)} crates under crates/ respect the layering.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
