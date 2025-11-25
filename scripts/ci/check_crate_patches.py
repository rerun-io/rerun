#!/usr/bin/env python3
"""Check for patch.crates-io sections in Cargo.toml files."""

from __future__ import annotations

import sys
from pathlib import Path

import tomllib  # Python 3.11+, or use tomli for older versions


def check_cargo_toml(path: Path) -> bool:
    """
    Check if a Cargo.toml file has [patch.crates-io] section.

    Returns True if patches found, False otherwise.
    """
    try:
        with open(path, "rb") as f:
            cargo_data = tomllib.load(f)

        if "patch" in cargo_data and "crates-io" in cargo_data["patch"]:
            patches = cargo_data["patch"]["crates-io"]
            if patches:  # Non-empty patch section
                return True
    except Exception as e:
        print(f"Error reading {path}: {e}", file=sys.stderr)
        return False

    return False


def main() -> None:
    """Find all Cargo.toml files and check for patches."""
    found_patches = []

    # Find all Cargo.toml files in the repository
    for cargo_toml in Path(".").rglob("Cargo.toml"):
        # Skip target directories
        if "target" in cargo_toml.parts:
            continue

        if check_cargo_toml(cargo_toml):
            found_patches.append(cargo_toml)

    if found_patches:
        print("⚠️  WARNING: patch.crates-io sections found in:")
        for path in found_patches:
            print(f"  - {path}")
        print("\nThese should be removed before release")
        sys.exit(1)
    else:
        print("✅ No crates.io patches found")
        sys.exit(0)


if __name__ == "__main__":
    main()
