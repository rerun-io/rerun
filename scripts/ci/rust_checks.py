#!/usr/bin/env python3

"""Run various rust checks for CI."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time
from glob import glob


class Timing:
    def __init__(self, command: str, duration: float) -> None:
        self.command = command
        self.duration = duration


def run_cargo(cargo_cmd: str, cargo_args: str, clippy_conf: str | None = None) -> Timing:
    args = ["cargo", cargo_cmd]
    if cargo_cmd != "deny":
        args.append("--quiet")
    args += cargo_args.split(" ")

    cmd_str = subprocess.list2cmdline(args)
    print(f"> {cmd_str}")
    start_time = time.time()

    env = os.environ.copy()
    env["RUSTFLAGS"] = "--deny warnings"
    env["RUSTDOCFLAGS"] = "--deny warnings"
    if clippy_conf is not None:
        env["CLIPPY_CONF_DIR"] = (
            f"{os.getcwd()}/{clippy_conf}"  # Clippy has issues finding this directory on CI when we're not using an absolute path here.
        )

    result = subprocess.run(args, env=env, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"'{cmd_str}' failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}")
        sys.exit(result.returncode)

    return Timing(cmd_str, time.time() - start_time)


def package_name_from_cargo_toml(cargo_toml_path: str) -> str:
    with open(cargo_toml_path, encoding="utf8") as file:
        cargo_toml_contents = file.read()
    package_name_result = re.search(r'name\s+=\s"([\w\-_]+)"', cargo_toml_contents)
    if package_name_result is None:
        raise Exception(f"Failed to find package name in '{cargo_toml_path}'")

    return package_name_result.group(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Run Rust checks and tests")
    parser.add_argument(
        "--skip-check-individual-crates",
        help="If true, don't check individual crates in /crates/.",
        action="store_true",
    )
    parser.add_argument(
        "--skip-check-individual-examples",
        help="If true, don't check individual examples in /examples/rust/.",
        action="store_true",
    )
    parser.add_argument("--skip-wasm-checks", help="If true, don't run explicit wasm32 checks.", action="store_true")
    parser.add_argument("--skip-docs", help="If true, don't run doc generation.", action="store_true")
    parser.add_argument("--skip-tests", help="If true, don't run tests.", action="store_true")
    parser.add_argument("--skip-cargo-deny", help="If true, don't run cargo deny.", action="store_true")
    args = parser.parse_args()

    # ----------------------

    # NOTE: a lot of these jobs use very little CPU, but we csannot parallelize them because they all take a lock on the `target` directory.

    timings = []

    # First check with --locked to make sure Cargo.lock is up to date.
    timings.append(run_cargo("check", "--locked --all-features"))

    timings.append(run_cargo("fmt", "--all -- --check"))

    timings.append(run_cargo("clippy", "--all-targets --all-features -- --deny warnings"))

    # Check a few important permutations of the feature flags for our `rerun` library:
    timings.append(run_cargo("check", "-p rerun --no-default-features"))
    timings.append(run_cargo("check", "-p rerun --no-default-features --features sdk"))

    # Cargo deny
    if not args.skip_cargo_deny:
        # Note: running just `cargo deny check` without a `--target` can result in
        # false positives due to https://github.com/EmbarkStudios/cargo-deny/issues/324
        # Installing is quite quick if it's already installed.
        timings.append(run_cargo("install", "--locked cargo-deny"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target aarch64-apple-darwin check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target i686-pc-windows-gnu check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target i686-pc-windows-msvc check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target i686-unknown-linux-gnu check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target wasm32-unknown-unknown check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target x86_64-apple-darwin check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target x86_64-pc-windows-gnu check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target x86_64-pc-windows-msvc check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target x86_64-unknown-linux-gnu check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target x86_64-unknown-linux-musl check"))
        timings.append(run_cargo("deny", "--all-features --log-level error --target x86_64-unknown-redox check"))

    if not args.skip_wasm_checks:
        # Check viewer for wasm32
        timings.append(
            run_cargo(
                "clippy",
                "--all-features --target wasm32-unknown-unknown --target-dir target_wasm -p re_viewer -- --deny warnings",
                clippy_conf="scripts/clippy_wasm",  # Use ./scripts/clippy_wasm/clippy.toml
            )
        )
        # Check re_renderer examples for wasm32.
        timings.append(
            run_cargo("check", "--target wasm32-unknown-unknown --target-dir target_wasm -p re_renderer --examples")
        )

    # Since features are additive, check examples & crates individually unless opted out.
    if not args.skip_check_individual_examples:
        for cargo_toml_path in glob("./examples/rust/**/Cargo.toml", recursive=True):
            package_name = package_name_from_cargo_toml(cargo_toml_path)
            timings.append(run_cargo("check", f"--no-default-features -p {package_name}"))
            timings.append(run_cargo("check", f"--all-features -p {package_name}"))

    if not args.skip_check_individual_crates:
        for cargo_toml_path in glob("./crates/**/Cargo.toml", recursive=True):
            package_name = package_name_from_cargo_toml(cargo_toml_path)
            timings.append(run_cargo("check", f"--no-default-features -p {package_name}"))
            timings.append(run_cargo("check", f"--all-features -p {package_name}"))

    # Doc tests
    if not args.skip_docs:
        # Full doc build takes prohibitively long (over 17min as of writing), so we skip it:
        # timings.append(run_cargo("doc", "--all-features"))

        # These take around 3m40s each on CI, but very useful for catching broken doclinks:
        timings.append(run_cargo("doc", "--no-deps --all-features --workspace"))
        timings.append(run_cargo("doc", "--document-private-items --no-deps --all-features --workspace"))

    if not args.skip_tests:
        # We first use `--no-run` to measure the time of compiling vs actually running

        # Just a normal `cargo test` should always work:
        timings.append(run_cargo("test", "--all-targets --no-run"))
        timings.append(run_cargo("test", "--all-targets"))

        # Full test of everything:
        timings.append(run_cargo("test", "--all-targets --all-features --no-run"))
        timings.append(run_cargo("test", "--all-targets --all-features"))

    # Print timings overview
    print("-----------------")
    print("Timings:")
    timings.sort(key=lambda timing: timing.duration, reverse=True)
    for timing in timings:
        print(f"{timing.duration:.2f}s \t {timing.command}")


if __name__ == "__main__":
    main()
