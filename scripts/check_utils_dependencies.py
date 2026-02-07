#!/usr/bin/env python3

"""
Check that crates under crates/utils don't depend on disallowed workspace crates.

Ensures that utility crates remain independent from higher-level crates like store, viewer, and top.
Dependencies on crates/build and other crates/utils crates are allowed.
"""

from __future__ import annotations

import sys
from pathlib import Path

import tomli


def main() -> int:
    """Check utils crate dependencies and return exit code."""
    repo_root = Path(__file__).parent.parent
    utils_dir = repo_root / "crates" / "utils"
    workspace_toml = repo_root / "Cargo.toml"

    # Parse workspace Cargo.toml to get all workspace crates and their locations
    with open(workspace_toml, "rb") as f:
        workspace_data = tomli.load(f)

    # Get all workspace crates
    workspace_deps = workspace_data.get("workspace", {}).get("dependencies", {})

    # Identify disallowed workspace crates (from store, top, viewer)
    # Allowed: crates/utils and crates/build
    disallowed_workspace_crates = set()

    for crate_name, crate_info in workspace_deps.items():
        if not isinstance(crate_info, dict) or "path" not in crate_info:
            continue

        crate_path = crate_info["path"]
        # Disallow dependencies on store, top, and viewer crates
        if crate_path.startswith(("crates/store/", "crates/top/", "crates/viewer/")):
            disallowed_workspace_crates.add(crate_name)

    # Check each Cargo.toml in crates/utils
    errors = []
    for cargo_toml in sorted(utils_dir.glob("*/Cargo.toml")):
        crate_dir = cargo_toml.parent
        crate_name = crate_dir.name

        with open(cargo_toml, "rb") as f:
            crate_data = tomli.load(f)

        # Check all dependency sections
        dep_sections = [
            ("dependencies", crate_data.get("dependencies", {})),
            ("dev-dependencies", crate_data.get("dev-dependencies", {})),
            ("build-dependencies", crate_data.get("build-dependencies", {})),
        ]

        # Also check target-specific dependencies
        for target_name, target_data in crate_data.get("target", {}).items():
            if isinstance(target_data, dict):
                dep_sections.append((f"target.{target_name}.dependencies", target_data.get("dependencies", {})))
                dep_sections.append((f"target.{target_name}.dev-dependencies", target_data.get("dev-dependencies", {})))
                dep_sections.append((
                    f"target.{target_name}.build-dependencies",
                    target_data.get("build-dependencies", {}),
                ))

        for section_name, deps in dep_sections:
            if not isinstance(deps, dict):
                continue

            for dep_name in deps:
                if dep_name in disallowed_workspace_crates:
                    errors.append(
                        f"ERROR: {crate_name} ({section_name}) depends on {dep_name}, "
                        f"which is a disallowed workspace crate"
                    )

    if errors:
        print("Found disallowed dependencies from crates/utils:\n")
        for error in errors:
            print(error)
        print(
            "\nCrates under crates/utils should only depend on:\n"
            "  - Other crates under crates/utils\n"
            "  - Crates under crates/build\n"
            "  - External crates (from crates.io)\n"
            "\nThey should NOT depend on workspace crates from:\n"
            "  - crates/store\n"
            "  - crates/top\n"
            "  - crates/viewer"
        )
        return 1

    print("✓ All crates under crates/utils have valid dependencies")
    return 0


if __name__ == "__main__":
    sys.exit(main())
