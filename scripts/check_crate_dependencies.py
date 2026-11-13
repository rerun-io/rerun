#!/usr/bin/env python3

"""
Check which crates our crates may depend on.

Two kinds of rules:

* The folders under `crates/` form layers, each depending only on the ones below it.
  The layers, top to bottom, are `LAYERS` in `crate_layers.py`. A crate may depend on crates in
  its own folder, or in any folder in a lower layer. `dev-dependencies` are exempt: a test may
  reach anywhere.

  A layer can hold more than one column, and those are siblings: independent of each other, so
  neither may depend on the other in either direction. That is what lets `ARCHITECTURE.md` draw
  them side by side. A column can in turn stack several folders, each depending only on itself
  and the ones under it.

  The same layering is what `ARCHITECTURE.md` draws, one band per folder, so a violation here is
  an arrow pointing the wrong way in that diagram.

* `FORBIDDEN_DEPENDENCIES` lists crates that a crate must not depend on, directly or through
  other crates, even when the layering allows it.
  Rules between our own crates are checked on the declared dependencies, so they cover every
  target, build-dependencies, and optional dependencies. Rules on external crates use
  `cargo tree`, which resolves features but only sees the host target.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

from crate_layers import FOLDERS, LAYERS

# (crate_spec, forbidden_deps)
#
# crate_spec is the workspace crate name, optionally followed by extra `cargo tree` arguments,
# such as `--all-features`, `--no-default-features`, or `--edges normal` (to ignore dev-dependencies).
FORBIDDEN_DEPENDENCIES: list[tuple[str, set[str]]] = [
    ("re_sdk", {"datafusion", "egui", "lance"}),
    ("re_sdk --all-features", {"datafusion", "egui", "lance"}),
    ("rerun-cli --no-default-features", {"datafusion", "egui", "lance"}),
    ("rerun --no-default-features --features sdk,native_viewer,map_view,crypto-aws-lc-rs", {"ring"}),
    ("re_sdk --no-default-features --features crypto-aws-lc-rs", {"ring"}),
    ("rerun_py --no-default-features --features extension-module,crypto-aws-lc-rs", {"ring"}),
    ("rerun --no-default-features --features sdk,native_viewer,map_view,crypto-ring", {"aws-lc-rs"}),
]

# (crate, forbidden_dep), both workspace crates. `dev-dependencies` are exempt.
FORBIDDEN_WORKSPACE_DEPENDENCIES: list[tuple[str, str]] = [
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

# Setting `RUSTFLAGS` disables `re_workspace_hack`, which depends on everything.
CARGO_ENV = {**os.environ, "RUSTFLAGS": os.environ.get("RUSTFLAGS", "")}


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


def layering_errors(metadata: dict[str, Any]) -> list[str]:
    """Every dependency that points up the layers, or across to a sibling."""
    workspace_root = Path(metadata["workspace_root"])

    # Crates outside `crates/` (examples, `tests/rust`, …) are leaves that nothing depends on,
    # so they have no layer to violate.
    layer_of_crate = {}
    for package in metadata["packages"]:
        folder = Path(package["manifest_path"]).parent.relative_to(workspace_root).parent.as_posix()
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

    print(f"Checked the layering of {len(layer_of_crate)} crates under crates/")
    return sorted(errors)


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


def forbidden_workspace_dependency_errors(metadata: dict[str, Any]) -> list[str]:
    """Every dependency listed in `FORBIDDEN_WORKSPACE_DEPENDENCIES` that a crate has anyway."""
    workspace_crates = {package["name"] for package in metadata["packages"]}
    normal_deps = {
        package["name"]: {
            dep["name"] for dep in package["dependencies"] if dep["kind"] != "dev" and dep["name"] in workspace_crates
        }
        for package in metadata["packages"]
    }
    errors = []
    for crate, forbidden_dep in FORBIDDEN_WORKSPACE_DEPENDENCIES:
        chain = dependency_chain(normal_deps, crate, forbidden_dep)
        if chain is not None:
            errors.append(f"{crate} depends on `{forbidden_dep}`, which is forbidden:\n  {' → '.join(chain)}")
    return errors


def cargo_tree(crate_spec: str, *args: str) -> str:
    crate_name, *extra_args = crate_spec.split()
    cmd = ["cargo", "tree", "--package", crate_name, *args, *extra_args]
    result = subprocess.run(cmd, cwd=RERUN_ROOT, capture_output=True, text=True, env=CARGO_ENV)
    if result.returncode != 0:
        raise SystemExit(f"`{' '.join(cmd)}` failed:\n{result.stderr}")
    return result.stdout


def get_dependencies(crate_spec: str) -> set[str]:
    """Get all dependencies of a crate using `cargo tree`."""
    # Each line looks like: `crate_name vX.Y.Z [optional extra]`
    return {line.split()[0] for line in cargo_tree(crate_spec, "--prefix", "none").splitlines() if line.strip()}


def get_dependency_path(crate_spec: str, forbidden_dep: str) -> str:
    """Find a shortest dependency path from a crate to a forbidden dependency.

    Runs `cargo tree` with the default tree-prefix output and finds the first
    line matching the forbidden dep. Because `cargo tree` prints depth-first,
    the first match is one of the shortest paths. We then walk backwards
    through the output to reconstruct the chain from root to the forbidden dep.
    """
    lines = cargo_tree(crate_spec, "--edges", "normal").splitlines()

    # Find the first line that references the forbidden dep.
    target_idx = None
    for i, line in enumerate(lines):
        # Extract the crate name from the tree line (strip tree-drawing characters)
        stripped = line.lstrip("│├└─ ─\t")
        if stripped.split()[0] == forbidden_dep:
            target_idx = i
            break

    if target_idx is None:
        return f"  (could not find {forbidden_dep} in cargo tree output)"

    def indent_level(line: str) -> int:
        """Count the indentation level from the tree-drawing prefix."""
        # Each level of depth adds 4 characters of prefix (e.g. "│   " or "├── ")
        content_start = 0
        for j, ch in enumerate(line):
            if ch not in "│├└─ ─\t":
                content_start = j
                break
        return content_start

    # Walk backwards to reconstruct the path from root to forbidden dep
    path_lines: list[str] = [lines[target_idx]]
    current_indent = indent_level(lines[target_idx])
    for i in range(target_idx - 1, -1, -1):
        line_indent = indent_level(lines[i])
        if line_indent < current_indent:
            path_lines.append(lines[i])
            current_indent = line_indent
            if line_indent == 0:
                break

    path_lines.reverse()
    return "\n".join(path_lines)


def forbidden_dependency_errors() -> list[str]:
    """Every dependency listed in `FORBIDDEN_DEPENDENCIES` that a crate has anyway."""
    errors = []
    for crate_spec, forbidden in FORBIDDEN_DEPENDENCIES:
        print(f"Checking {crate_spec}…")
        deps = get_dependencies(crate_spec)
        for forbidden_dep in sorted(forbidden & deps):
            path = get_dependency_path(crate_spec, forbidden_dep)
            errors.append(f"{crate_spec} depends on `{forbidden_dep}`, which is forbidden:\n{path}")
    return errors


def main() -> int:
    metadata = cargo_metadata()
    layering = layering_errors(metadata)
    forbidden = forbidden_workspace_dependency_errors(metadata) + forbidden_dependency_errors()

    if layering:
        print("\nThe crate layering is violated:\n")
        for error in layering:
            print(f"  {error}")
        print("\nEither move a crate to a lower folder, or make the dependency a dev-dependency.")
        print("Sibling columns, listed together below, must not depend on each other at all.")
        print("Layers, top to bottom:")
        for columns in LAYERS:
            print(f"  {' + '.join(' > '.join(column) for column in columns)}")

    if forbidden:
        print("\nForbidden dependencies found:\n")
        for error in forbidden:
            print(error)
            print()

    if layering or forbidden:
        return 1

    print("\n✓ The crate layering holds, and no forbidden dependencies were found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
