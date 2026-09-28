#!/usr/bin/env python3

"""
Check that the folders under `crates/` form layers, each depending only on the ones below it.

The layers, top to bottom, are `LAYERS` below. A crate may depend on crates in its own folder,
or in any folder in a lower layer. `dev-dependencies` are exempt: a test may reach anywhere.

A layer can hold more than one folder, and those are siblings: independent of each other, so
neither may depend on the other in either direction. That is what lets `ARCHITECTURE.md` draw
them side by side.

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

# Top to bottom. Each folder may only depend on itself and the ones after it.
LAYERS = [
    ["crates/tests"],
    ["crates/top"],
    ["crates/views", "crates/panels"],  # Siblings: a view is not a panel, and neither uses the other.
    ["crates/viewer_support"],
    ["crates/store_app"],
    ["crates/data_flow"],
    ["crates/store"],
    ["crates/build"],
    ["crates/utils"],
]

FORBIDDEN_DEPENDENCIES = [
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
        if folder in {f for layer in LAYERS for f in layer}:
            layer_of_crate[package["name"]] = folder

    errors = set()  # Target-specific sections can repeat a dependency.
    for package in metadata["packages"]:
        layer = layer_of_crate.get(package["name"])
        if layer is None:
            continue
        index = next(i for i, folders in enumerate(LAYERS) if layer in folders)
        # Its own folder, plus everything in a lower layer. Never a sibling.
        allowed = {layer} | {folder for folders in LAYERS[index + 1 :] for folder in folders}
        siblings = set(LAYERS[index]) - {layer}
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
        print("Sibling folders, listed together below, must not depend on each other at all.")
        print("Layers, top to bottom:")
        for folders in LAYERS:
            print(f"  {' + '.join(folders)}")
        print("Forbidden dependencies:")
        for crate, dependency in FORBIDDEN_DEPENDENCIES:
            print(f"  {crate} → {dependency}")
        return 1

    print(f"All {len(layer_of_crate)} crates under crates/ respect the layering.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
