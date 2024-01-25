#!/usr/bin/env python3

"""Run various rust checks for CI."""

from __future__ import annotations

import os
import re
import subprocess
import sys


def run_cargo(args: str) -> None:
    print(f"Running `cargo {args}`")
    result = subprocess.call(f"cargo {args}", shell=True)
    if result != 0:
        sys.exit(result)


def main() -> None:
    # First check with --locked to make sure Cargo.lock is up to date.
    run_cargo("check --locked --all-features")

    run_cargo("fmt --all -- --check")
    run_cargo("cranky --all-targets --all-features -- --deny warnings")

    # Since features are additive, check samples individually.
    for path, dirnames, filenames in os.walk("examples/rust"):
        if "Cargo.toml" in filenames:
            # Read the package name from the Cargo.toml file. Crude but effective.
            with open(f"{path}/Cargo.toml") as file:
                cargo_toml_contents = file.read()
            package_name_result = re.search(r'name\s+=\s"([\w\-_]+)"', cargo_toml_contents)
            if package_name_result is None:
                raise Exception(f"Failed to find package name in {path}/Cargo.toml")
            package_name = package_name_result.group(1)

            run_cargo(f"check --no-default-features -p {package_name}")
            run_cargo(f"check --all-features -p {package_name}")

    # Check a few important permutations of the feature flags for our `rerun` library:
    run_cargo("cranky -p rerun --no-default-features")
    run_cargo("cranky -p rerun --no-default-features --features sdk")

    # Doc tests
    run_cargo("doc --all-features")
    run_cargo("doc --no-deps --all-features --workspace")
    run_cargo("doc --document-private-items --no-deps --all-features --workspace")

    # Just a normal `cargo test` should always work:
    run_cargo("test --all-targets")

    # Full test of everything:
    run_cargo("test --all-targets --all-features")


if __name__ == "__main__":
    main()
