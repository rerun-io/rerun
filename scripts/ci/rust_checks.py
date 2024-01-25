#!/usr/bin/env python3

"""Run various rust checks for CI."""

from __future__ import annotations

import os
import re
import subprocess
import sys
import time


class Timing:
    def __init__(self, command: str, start_time: float) -> None:
        self.command = command
        self.duration = time.time() - start_time


def run_cargo(args: str) -> Timing:
    command = f"cargo {args}"
    print(f"Running '{command}'")
    start = time.time()
    result = subprocess.call(command, shell=True)

    if result != 0:
        sys.exit(result)

    return Timing(command, start)


def main() -> None:
    timings = []

    # First check with --locked to make sure Cargo.lock is up to date.
    timings.append(run_cargo("check --locked --all-features"))

    timings.append(run_cargo("fmt --all -- --check"))
    timings.append(run_cargo("cranky --all-targets --all-features -- --deny warnings"))

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

            timings.append(run_cargo(f"check --no-default-features -p {package_name}"))
            timings.append(run_cargo(f"check --all-features -p {package_name}"))

    # Check a few important permutations of the feature flags for our `rerun` library:
    timings.append(run_cargo("cranky -p rerun --no-default-features"))
    timings.append(run_cargo("cranky -p rerun --no-default-features --features sdk"))

    # Doc tests
    timings.append(run_cargo("doc --all-features"))
    timings.append(run_cargo("doc --no-deps --all-features --workspace"))
    timings.append(run_cargo("doc --document-private-items --no-deps --all-features --workspace"))

    # Just a normal `cargo test` should always work:
    timings.append(run_cargo("test --all-targets"))

    # Full test of everything:
    timings.append(run_cargo("test --all-targets --all-features"))

    # Print timings overview
    print("-----------------")
    print("Timings:")
    for timing in timings:
        print(f"{timing.duration:.2f}s \t {timing.command}")


if __name__ == "__main__":
    main()
