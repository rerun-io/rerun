#!/usr/bin/env python3

"""
Check that the folders under `crates/` form layers, each depending only on the ones below it.

The layers, top to bottom, are `LAYERS` below. A crate may depend on crates in its own folder,
or in any folder further down. `dev-dependencies` are exempt: a test may reach anywhere.

The same layering is what `ARCHITECTURE.md` draws, one band per folder, so a violation here is
an arrow pointing the wrong way in that diagram.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

# Top to bottom. Each folder may only depend on itself and the ones after it.
LAYERS = [
    "crates/tests",
    "crates/top",
    "crates/viewer",
    "crates/store",
    "crates/build",
    "crates/utils",
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


def main() -> int:
    metadata = cargo_metadata()
    workspace_root = Path(metadata["workspace_root"])

    # Crates outside `crates/` (examples, `tests/rust`, …) are leaves that nothing depends on,
    # so they have no layer to violate.
    layer_of_crate = {}
    for package in metadata["packages"]:
        folder = str(Path(package["manifest_path"]).parent.relative_to(workspace_root).parent)
        if folder in LAYERS:
            layer_of_crate[package["name"]] = folder

    errors = set()  # Target-specific sections can repeat a dependency.
    for package in metadata["packages"]:
        layer = layer_of_crate.get(package["name"])
        if layer is None:
            continue
        allowed = LAYERS[LAYERS.index(layer) :]
        for dep in package["dependencies"]:
            if dep["kind"] == "dev":
                continue
            dep_layer = layer_of_crate.get(dep["name"])
            if dep_layer is not None and dep_layer not in allowed:
                errors.add(f"{layer}/{package['name']} depends on {dep['name']} in {dep_layer}, which is above it")

    if errors:
        print("The layering of the folders under crates/ is violated:\n")
        for error in sorted(errors):
            print(f"  {error}")
        print("\nEither move a crate to a lower folder, or make the dependency a dev-dependency.")
        print(f"Layers, top to bottom: {', '.join(LAYERS)}")
        return 1

    print(f"All {len(layer_of_crate)} crates under crates/ respect the layering.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
