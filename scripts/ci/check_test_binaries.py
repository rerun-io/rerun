#!/usr/bin/env python3

"""
Check that every workspace crate has at most one integration test binary.

Each file directly under `tests/` is linked into its own binary, so a crate with many of them
pays the link step many times over, on every platform CI builds tests for. Keep one binary per
crate: a `tests/<name>/main.rs` that declares the individual test files as modules.

A crate that really needs several binaries must list them all in its `Cargo.toml`:

    [package.metadata.rerun]
    test-binaries = ["integration", "inspection"]

The list must match the crate's test targets exactly, so it cannot go stale.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

RERUN_ROOT = Path(__file__).absolute().parent.parent.parent

METADATA_KEY = "package.metadata.rerun.test-binaries"


def cargo_metadata() -> dict[str, Any]:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version=1", "--no-deps"],
        cwd=RERUN_ROOT,
        text=True,
        capture_output=True,
    )
    if out.returncode != 0:
        sys.stderr.write(out.stderr)
        raise SystemExit(f"cargo metadata failed with exit code {out.returncode}")
    metadata: dict[str, Any] = json.loads(out.stdout)
    return metadata


def allowed_test_binaries(package: dict[str, Any]) -> list[str] | None:
    metadata = package.get("metadata") or {}
    rerun = metadata.get("rerun") or {}
    allowed = rerun.get("test-binaries")
    if allowed is None:
        return None
    if not isinstance(allowed, list) or not all(isinstance(name, str) for name in allowed):
        raise ValueError(f"`{METADATA_KEY}` must be a list of strings")
    return sorted(allowed)


def main() -> int:
    metadata = cargo_metadata()
    workspace_root = Path(metadata["workspace_root"])

    errors: list[str] = []
    for package in metadata["packages"]:
        manifest = Path(package["manifest_path"]).relative_to(workspace_root)
        actual = sorted(target["name"] for target in package["targets"] if "test" in target["kind"])

        try:
            allowed = allowed_test_binaries(package)
        except ValueError as err:
            errors.append(f"{manifest}: {err}")
            continue

        if allowed is None:
            if len(actual) > 1:
                errors.append(f"{manifest}: {len(actual)} integration test binaries: {', '.join(actual)}")
        elif allowed != actual:
            errors.append(f"{manifest}: `{METADATA_KEY}` lists {allowed}, but the crate's test binaries are {actual}")

    if errors:
        print("Integration test binary check failed:\n")
        for error in errors:
            print(f"  {error}")
        print()
        print("Each file directly under `tests/` is linked into its own binary.")
        print("Merge them into one `tests/<name>/main.rs` that declares the files as modules,")
        print("e.g. `crates/store/re_chunk/tests/integration/main.rs`.")
        print("If a crate really needs several binaries, list them all in its `Cargo.toml`:")
        print()
        print("  [package.metadata.rerun]")
        print('  test-binaries = ["integration", "inspection"]')
        return 1

    print(f"All {len(metadata['packages'])} workspace crates have at most one integration test binary.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
