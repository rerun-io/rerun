#!/usr/bin/env python3

"""
Run various rust checks for CI.

You can run the script via pixi:
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
from functools import partial
from glob import glob
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Callable


class Result:
    """The result of running one test."""

    def __init__(self, command: str, success: bool, duration: float) -> None:
        self.command = command
        self.success = success
        self.duration = duration


def run_cargo(
    cargo_cmd: str,
    cargo_args: str,
    clippy_conf: str | None = None,
    deny_warnings: bool = True,
    output_checks: Callable[[str], str | None] | None = None,
) -> Result:
    args = ["cargo", cargo_cmd]
    args += cargo_args.split(" ")

    cmd_str = subprocess.list2cmdline(args)
    print(f"> {cmd_str} ", end="", flush=True)
    start_time = time.time()

    additional_env_vars = {}

    extra_cfgs = ""
    if "wasm" in cargo_args:
        extra_cfgs = '--cfg=web_sys_unstable_apis --cfg=getrandom_backend="wasm_js"'

    additional_env_vars["RUSTFLAGS"] = f"{extra_cfgs} {'--deny warnings' if deny_warnings else ''}"
    additional_env_vars["RUSTDOCFLAGS"] = f"{extra_cfgs} {'--deny warnings' if deny_warnings else ''}"

    # We shouldn't require the web viewer .wasm to exist before running clippy, unit tests, etc:
    additional_env_vars["RERUN_DISABLE_WEB_VIEWER_SERVER"] = "1"

    # Disable TRACY to avoid macOS failure on CI, that looks like this:
    # > Tracy Profiler initialization failure: CPU doesn't support invariant TSC.
    # > Define TRACY_NO_INVARIANT_CHECK=1 to ignore this error, *if you know what you are doing*.
    # > Alternatively you may rebuild the application with the TRACY_TIMER_FALLBACK define to use lower resolution timer.
    additional_env_vars["TRACY_ENABLED"] = "0"
    additional_env_vars["TRACY_NO_INVARIANT_CHECK"] = "1"

    if clippy_conf is not None:
        additional_env_vars["CLIPPY_CONF_DIR"] = (
            # Clippy has issues finding this directory on CI when we're not using an absolute path here.
            f"{os.getcwd()}/{clippy_conf}"
        )

    capture = output_checks is not None

    env = os.environ.copy()
    env.update(additional_env_vars)

    # Use encoding='utf-8' with errors='replace' to handle binary data in cargo/linker output on Windows
    result = subprocess.run(
        args, env=env, check=False, capture_output=capture, text=True, encoding="utf-8", errors="replace"
    )
    success = result.returncode == 0

    if success:
        output_check_error = None
        if output_checks is not None:
            output_check_error = output_checks(result.stdout)

        if output_check_error is not None:
            print("❌")
            print(output_check_error)
            success = False
        else:
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

    # On CI, we split these checks into groups to reduce the time it takes to run all of this.
    # Make sure the `reusable_checks_rust` workflow stays up-to-date with this list.
    checks = [
        ("base_checks", base_checks),
        ("sdk_variations", sdk_variations),
        ("cargo_deny", cargo_deny),
        ("denied_sdk_deps", denied_sdk_deps),
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


deny_targets = [
    "aarch64-apple-darwin",
    "wasm32-unknown-unknown",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
]


def cargo_deny(results: list[Result]) -> None:
    # Note: running just `cargo deny check` without a `--target` can result in
    # false positives due to https://github.com/EmbarkStudios/cargo-deny/issues/324
    # Installing is quite quick if it's already installed.
    results.append(run_cargo("install", "--locked cargo-deny@^0.18"))

    for target in deny_targets:
        results.append(
            run_cargo("deny", f"--all-features --exclude-dev --log-level error --target {target} check"),
        )


def denied_sdk_deps(results: list[Result]) -> None:
    """
    Check for disallowed SDK dependencies.

    This protects against leaking UI dependencies into the SDK.
    """

    # Installing is quite quick if it's already installed.
    results.append(run_cargo("install", "--locked cargo-tree@^0.29.0"))

    # Sampling of dependencies that should never show up in the SDK, unless the viewer is enabled.
    # They are ordered from "big to small" to make sure the bigger leaks are caught & reported first.
    # (e.g. `re_viewer` depends on `rfd` which is also disallowed, but if re_viewer is leaking, only report `re_viewer`)
    disallowed_dependencies = [
        "eframe",
        "re_viewer",
        "wgpu",
        "egui",
        "winit",
        "rfd",  # File dialog library.
        "objc2-ui-kit",  # MacOS system ui libraries.
        "cocoa",  # Legacy MacOS system ui libraries.
        "wayland-sys",  # Linux windowing.
    ]

    def check_sdk_tree_with_default_features(tree_output: str, features: str) -> str | None:
        for disallowed_dependency in disallowed_dependencies:
            if disallowed_dependency in tree_output:
                return (
                    f"{disallowed_dependency} showed up in the SDK's dependency tree when building with features={features}"
                    "This dependency should only ever show up if the `native_viewer` feature is enabled. "
                    f"Full dependency tree:\n{tree_output}"
                )

        return None

    for features in ["default", "default,auth,oss_server,perf_telemetry,web_viewer"]:
        for target in deny_targets:
            result = run_cargo(
                "tree",
                # -f '{lib}' is used here because otherwise cargo tree would print links to repositories of patched crates
                # which would cause false positives e.g. when checking for egui.
                f"-p rerun --target {target} -f '{{lib}}' -F {features}",
                output_checks=partial(check_sdk_tree_with_default_features, features=features),
            )
            result.command = f"Check dependencies in `{result.command}`"
            results.append(result)


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


test_failure_message = 'See the "Upload test results" step for a link to the snapshot test artifact.'


def tests(results: list[Result]) -> None:
    # We first use `--no-run` to measure the time of compiling vs actually running
    results.append(run_cargo("nextest", "run --all-targets --all-features --no-run", deny_warnings=False))
    results.append(run_cargo("nextest", "run --all-targets --all-features --no-fail-fast", deny_warnings=False))

    if not results[-1].success:
        print(test_failure_message)

    # Cargo nextest doesn't support doc tests yet, run those separately.
    results.append(run_cargo("test", "--all-features --doc", deny_warnings=False))


def tests_without_all_features(results: list[Result]) -> None:
    # We first use `--no-run` to measure the time of compiling vs actually running
    results.append(run_cargo("test", "--all-targets --no-run", deny_warnings=False))
    results.append(run_cargo("nextest", "run --all-targets --no-fail-fast", deny_warnings=False))

    if not results[-1].success:
        print(test_failure_message)


if __name__ == "__main__":
    main()
