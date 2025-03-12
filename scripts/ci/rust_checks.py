#!/usr/bin/env python3

"""
Run various rust checks for CI.

You can run the script via pixi which will make sure that the web build is around and up-to-date:
    pixi run rs-check

Alternatively you can also run it directly via python:
    python3 scripts/ci/rust_checks.py

To run only a specific test you can use the `--only` argument:
    pixi run rs-check --only wasm

To run all tests except a few specific ones you can use the `--skip` argument:
    pixi run rs-check --skip wasm docs docs_slow

To see a list of all available tests you can use the `--help` argument:
    pixi run rs-check --help

"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time
from glob import glob


class Result:
    """The result of running one test."""

    def __init__(self, command: str, success: bool, duration: float) -> None:
        self.command = command
        self.success = success
        self.duration = duration


def run_cargo(cargo_cmd: str, cargo_args: str, clippy_conf: str | None = None) -> Result:
    args = ["cargo", cargo_cmd]
    if cargo_cmd not in ["deny", "fmt", "format", "nextest"]:
        args.append("--quiet")
    args += cargo_args.split(" ")

    if cargo_cmd == "nextest":
        # Needs to go after `run`, so append it last.
        args.append("--cargo-quiet")

    cmd_str = subprocess.list2cmdline(args)
    print(f"> {cmd_str} ", end="", flush=True)
    start_time = time.time()

    additional_env_vars = {}
    # Compilation will fail we don't manually set `--cfg=web_sys_unstable_apis`,
    # because env vars are not propagated from CI.
    additional_env_vars["RUSTFLAGS"] = "--cfg=web_sys_unstable_apis --deny warnings"
    additional_env_vars["RUSTDOCFLAGS"] = "--cfg=web_sys_unstable_apis --deny warnings"
    if clippy_conf is not None:
        additional_env_vars["CLIPPY_CONF_DIR"] = (
            # Clippy has issues finding this directory on CI when we're not using an absolute path here.
            f"{os.getcwd()}/{clippy_conf}"
        )

    env = os.environ.copy()
    env.update(additional_env_vars)

    result = subprocess.run(args, env=env, check=False, capture_output=True, text=True)
    success = result.returncode == 0

    if success:
        print("✅")
    else:
        print("❌")
        # Print output right away, so the user can start fixing it while waiting for the rest of the checks to run:
        env_var_string = " ".join([f'{env_var}="{value}"' for env_var, value in additional_env_vars.items()])
        print(
            f"'{env_var_string} {cmd_str}' failed with exit-code {result.returncode}. Output:\n{result.stdout}\n{result.stderr}",
        )

    duration = time.time() - start_time
    return Result(cmd_str, success, duration)


def package_name_from_cargo_toml(cargo_toml_path: str) -> str:
    with open(cargo_toml_path, encoding="utf8") as file:
        cargo_toml_contents = file.read()
    package_name_result = re.search(r'name\s+=\s"([\w\-_]+)"', cargo_toml_contents)
    if package_name_result is None:
        raise Exception(f"Failed to find package name in '{cargo_toml_path}'")

    return package_name_result.group(1)


def main() -> None:
    # Ensure we can print unicode characters. Has been historically an issue on Windows CI.
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    checks = [
        ("base_checks", base_checks),
        ("sdk_variations", sdk_variations),
        ("cargo_deny", cargo_deny),
        ("wasm", wasm),
        ("individual_examples", individual_examples),
        ("individual_crates", individual_crates),
        ("docs", docs),
        ("docs_slow", docs_slow),
        ("tests", tests),
        ("tests_without_all_features", tests_without_all_features),
    ]
    check_names = [check[0] for check in checks]

    parser = argparse.ArgumentParser(description="Run Rust checks and tests.")
    parser.add_argument(
        "--skip",
        help="Skip all specified given checks but runs everything else.",
        nargs="+",
        type=str,
        choices=check_names,
    )
    parser.add_argument(
        "--only",
        help="Runs only the specified checks (ignores --skip argument).",
        nargs="+",
        type=str,
        choices=check_names,
    )
    args = parser.parse_args()

    enabled_check_names = []
    if args.only is not None:
        enabled_check_names = args.only
    else:
        enabled_check_names = [check[0] for check in checks if check[0] not in (args.skip or [])]
    print("Enabled checks:")
    for check in enabled_check_names:
        print(f" - {check}")
    print()

    # ----------------------

    # NOTE: a lot of these jobs use very little CPU, but we cannot parallelize them because they all take a lock on the `target` directory.

    results: list[Result] = []
    start_time = time.time()

    for enabled_check_name in enabled_check_names:
        checks[check_names.index(enabled_check_name)][1](results)

    total_duration = time.time() - start_time

    # ----------------------

    # Print timings overview
    print("-----------------")
    print(f"Ran {len(results)} checks in {total_duration:.0f}s")
    print("Individual timings, slowest first:")
    results.sort(key=lambda result: result.duration, reverse=True)
    for result in results:
        print(f"{result.duration:.2f}s \t {result.command}")

    # ----------------------

    # Count failures
    num_failures = sum(1 for result in results if not result.success)

    if num_failures == 0:
        print()
        print("✅ All checks passed!")
        sys.exit(0)
    else:
        print()
        print(f"❌ {num_failures} checks / {len(results)} failed:")
        for result in results:
            if not result.success:
                print(f"  ❌ {result.command}")
        sys.exit(1)


def base_checks(results: list[Result]) -> None:
    # First check with --locked to make sure Cargo.lock is up to date.
    results.append(run_cargo("check", "--locked --all-features"))
    results.append(run_cargo("fmt", "--all -- --check"))
    results.append(run_cargo("clippy", "--all-targets --all-features -- --deny warnings"))


def sdk_variations(results: list[Result]) -> None:
    # Check a few important permutations of the feature flags for our `rerun` library:
    results.append(run_cargo("check", "-p rerun --no-default-features"))
    results.append(run_cargo("check", "-p rerun --no-default-features --features sdk"))


def cargo_deny(results: list[Result]) -> None:
    # Note: running just `cargo deny check` without a `--target` can result in
    # false positives due to https://github.com/EmbarkStudios/cargo-deny/issues/324
    # Installing is quite quick if it's already installed.
    results.append(run_cargo("install", "--locked cargo-deny@^0.17"))
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target aarch64-apple-darwin check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target i686-pc-windows-gnu check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target i686-pc-windows-msvc check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target i686-unknown-linux-gnu check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target wasm32-unknown-unknown check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target x86_64-apple-darwin check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target x86_64-pc-windows-gnu check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target x86_64-pc-windows-msvc check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target x86_64-unknown-linux-gnu check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target x86_64-unknown-linux-musl check"),
    )
    results.append(
        run_cargo("deny", "--all-features --exclude-dev --log-level error --target x86_64-unknown-redox check"),
    )


def wasm(results: list[Result]) -> None:
    # Check viewer for wasm32
    results.append(
        run_cargo(
            "clippy",
            "--all-features --target wasm32-unknown-unknown --target-dir target_wasm -p re_viewer -- --deny warnings",
            clippy_conf="scripts/clippy_wasm",  # Use ./scripts/clippy_wasm/clippy.toml
        ),
    )
    # Check re_renderer examples for wasm32.
    results.append(
        run_cargo("check", "--target wasm32-unknown-unknown --target-dir target_wasm -p re_renderer --examples"),
    )


def individual_examples(results: list[Result]) -> None:
    for cargo_toml_path in glob("./examples/rust/**/Cargo.toml", recursive=True):
        package_name = package_name_from_cargo_toml(cargo_toml_path)
        results.append(run_cargo("check", f"--no-default-features -p {package_name}"))
        results.append(run_cargo("check", f"--all-features -p {package_name}"))


def individual_crates(results: list[Result]) -> None:
    for cargo_toml_path in glob("./crates/**/Cargo.toml", recursive=True):
        package_name = package_name_from_cargo_toml(cargo_toml_path)
        results.append(run_cargo("check", f"--no-default-features -p {package_name}"))
        results.append(run_cargo("check", f"--all-features -p {package_name}"))


def docs(results: list[Result]) -> None:
    # ⚠️ This version skips the `rerun` crate itself
    # Presumably due to https://github.com/rust-lang/rust/issues/114891, checking the `rerun` crate
    # takes about 20minutes on CI (per command).
    # Since this crate mostly combines & exposes other crates, it's not as important for iterating on the code.
    #
    # For details see https://github.com/rerun-io/rerun/issues/7387

    # These take a few minutes each on CI, but very useful for catching broken doclinks.
    results.append(run_cargo("doc", "--no-deps --all-features --workspace --exclude rerun"))
    results.append(run_cargo("doc", "--document-private-items --no-deps --all-features --workspace --exclude rerun"))


def docs_slow(results: list[Result]) -> None:
    # See `docs` above, this may take 20min each due to issues in cargo doc.
    results.append(run_cargo("doc", "--no-deps --all-features -p rerun"))
    results.append(run_cargo("doc", "--document-private-items --no-deps --all-features -p rerun"))


def tests(results: list[Result]) -> None:
    # We first use `--no-run` to measure the time of compiling vs actually running
    results.append(run_cargo("test", "--all-targets --all-features --no-run"))
    results.append(run_cargo("nextest", "run --all-targets --all-features"))

    # Cargo nextest doesn't support doc tests yet, run those separately.
    results.append(run_cargo("test", "--all-features --doc"))


def tests_without_all_features(results: list[Result]) -> None:
    # We first use `--no-run` to measure the time of compiling vs actually running
    results.append(run_cargo("test", "--all-targets --no-run"))
    results.append(run_cargo("nextest", "run --all-targets"))


if __name__ == "__main__":
    main()
