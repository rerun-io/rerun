#!/usr/bin/env python3

"""
Check that certain workspace crates don't depend on forbidden external crates.

Uses `cargo tree` to resolve the full dependency tree for each entry,
then verifies that none of the forbidden crates appear.

Each entry in FORBIDDEN_DEPENDENCIES is a tuple of:
  - A crate spec: the workspace crate name, optionally followed by
    `--all-features` or `--no-default-features`
  - A set of crate names that must NOT appear in the dependency tree
"""

from __future__ import annotations

import subprocess
import sys

# (crate_spec, forbidden_deps)
#
# crate_spec is the workspace crate name, optionally followed by
# `--all-features` or `--no-default-features`.
FORBIDDEN_DEPENDENCIES: list[tuple[str, set[str]]] = [
    ("re_sdk", {"datafusion", "egui"}),
    ("re_sdk --all-features", {"datafusion", "egui"}),
    ("rerun-cli --no-default-features", {"datafusion", "egui"}),
]


def get_dependencies(crate_spec: str) -> set[str]:
    """Get all dependencies of a crate using `cargo tree`."""
    parts = crate_spec.split()
    crate_name = parts[0]
    extra_args = parts[1:]

    cmd = [
        "cargo",
        "tree",
        "--package",
        crate_name,
        "--prefix",
        "none",
    ] + extra_args

    result = subprocess.run(cmd, capture_output=True, text=True)

    if result.returncode != 0:
        print(f"ERROR: `{' '.join(cmd)}` failed:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)

    deps: set[str] = set()
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        # Each line looks like: `crate_name vX.Y.Z [optional extra]`
        dep_name = line.split()[0]
        deps.add(dep_name)

    return deps


def get_dependency_path(crate_spec: str, forbidden_dep: str) -> str:
    """Find a shortest dependency path from a crate to a forbidden dependency.

    Runs `cargo tree` with the default tree-prefix output and finds the first
    line matching the forbidden dep. Because `cargo tree` prints depth-first,
    the first match is one of the shortest paths. We then walk backwards
    through the output to reconstruct the chain from root to the forbidden dep.
    """
    parts = crate_spec.split()
    crate_name = parts[0]
    extra_args = parts[1:]

    cmd = [
        "cargo",
        "tree",
        "--package",
        crate_name,
        "--edges",
        "normal",
    ] + extra_args

    result = subprocess.run(cmd, capture_output=True, text=True)

    if result.returncode != 0:
        return f"  (failed to get dependency path: {result.stderr.strip()})"

    lines = result.stdout.splitlines()

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


def main() -> int:
    errors: list[str] = []

    for crate_spec, forbidden in FORBIDDEN_DEPENDENCIES:
        print(f"Checking {crate_spec}…")
        deps = get_dependencies(crate_spec)

        for forbidden_dep in sorted(forbidden):
            if forbidden_dep in deps:
                path = get_dependency_path(crate_spec, forbidden_dep)
                errors.append(f"{crate_spec} depends on `{forbidden_dep}`, which is forbidden:\n{path}")

    if errors:
        print("\nForbidden dependencies found:\n")
        for error in errors:
            print(error)
            print()
        return 1

    print("\n✓ No forbidden dependencies found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
