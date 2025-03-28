#!/usr/bin/env python3

"""Runs all our snippets, for all our languages, and compares the .rrd they output."""

from __future__ import annotations

import argparse
import glob
import multiprocessing
import os
import sys
import time
from pathlib import Path
from typing import Any, cast

import tomlkit
from tomlkit.container import Container

sys.path.append(os.path.dirname(os.path.realpath(__file__)) + "/../../scripts/")
from roundtrip_utils import roundtrip_env, run, run_comparison  # noqa

config_path = Path(__file__).parent / "snippets.toml"
config = tomlkit.loads(config_path.read_text())

OPT_OUT: dict[str, Any] = cast(Container, config["opt_out"])
OPT_OUT_ENTIRELY: dict[str, Any] = OPT_OUT["run"].value
OPT_OUT_COMPARE = OPT_OUT["compare"].value
EXTRA_ARGS = config["extra_args"].value


class Example:
    def __init__(self, subdir: str, name: str) -> None:
        self.subdir = subdir
        self.name = name

    def opt_out_entirely(self) -> list[str]:
        for key in [self.subdir, self.subdir + "/" + self.name]:
            if key in OPT_OUT_ENTIRELY:
                return list(OPT_OUT_ENTIRELY[key])
        return []

    def opt_out_compare(self) -> list[str]:
        for key in [self.subdir, self.subdir + "/" + self.name]:
            if key in OPT_OUT_COMPARE:
                return list(OPT_OUT_COMPARE[key])
        return []

    def extra_args(self) -> list[str]:
        for key in [self.subdir, self.subdir + "/" + self.name]:
            if key in EXTRA_ARGS:
                return [
                    arg.replace("$config_dir", str(Path(__file__).parent.absolute())) for arg in EXTRA_ARGS.get(key, [])
                ]
        return []

    def output_path(self, language: str) -> str:
        return f"docs/snippets/all/{self.subdir}/{self.name}_{language}.rrd"

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Example):
            return NotImplemented
        return self.subdir == other.subdir and self.name == other.name

    def __hash__(self) -> int:
        return hash((self.subdir, self.name))

    def __lt__(self, other: Example) -> bool:
        return self.name < other.name

    def __repr__(self) -> str:
        return f"Example(subdir={self.subdir}, name={self.name})"

    def __str__(self) -> str:
        return f"{self.subdir}/{self.name}"


def main() -> None:
    parser = argparse.ArgumentParser(description="Run end-to-end cross-language roundtrip tests for all API examples")
    parser.add_argument("--no-py", action="store_true", help="Skip Python tests")
    parser.add_argument("--no-cpp", action="store_true", help="Skip C++ tests")
    # We don't allow skipping Rust - it is what we compare to at the moment.
    parser.add_argument("--no-py-build", action="store_true", help="Skip building rerun-sdk for Python")
    parser.add_argument(
        "--no-cpp-build",
        action="store_true",
        help="Skip cmake configure and ahead of time build for rerun_c & rerun_prebuilt_cpp",
    )
    parser.add_argument("--full-dump", action="store_true", help="Dump both rrd files as tables")
    parser.add_argument("--release", action="store_true", help="Run cargo invocations with --release")
    parser.add_argument("--target", type=str, default=None, help="Target used for cargo invocations")
    parser.add_argument("--target-dir", type=str, default=None, help="Target directory used for cargo invocations")
    parser.add_argument("example", nargs="*", type=str, default=None, help="Run only the specified example(s)")

    args = parser.parse_args()

    build_env = os.environ.copy()
    if "RUST_LOG" in build_env:
        del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

    if args.no_py:
        pass  # No need to build the Python SDK
    elif args.no_py_build:
        print("Skipping building python rerun-sdk - assuming it is already built and up-to-date!")
    else:
        build_python_sdk(build_env)

    if args.no_cpp:
        pass  # No need to build the C++ SDK
    elif args.no_cpp_build:
        print(
            "Skipping cmake configure & build for rerun_c & rerun_prebuilt_cpp - assuming it is already built and up-to-date!",
        )
    else:
        build_cpp_snippets()

    # Always build rust since we use it as the baseline for comparison.
    build_rust_snippets(build_env, args.release, args.target, args.target_dir)

    examples = []
    if len(args.example) > 0:
        for example in args.example:
            example = example.replace("\\", "/")
            parts = example.split("/")
            examples.append(Example("/".join(parts[0:-1]), parts[-1]))
    else:
        dir = os.path.join(os.path.dirname(__file__), "all")
        for file in glob.glob(dir + "/**", recursive=True):
            name = os.path.basename(file)
            if name == "__init__.py":
                continue
            name, extension = os.path.splitext(name)
            if extension == ".cpp" and not args.no_cpp or extension == ".py" and not args.no_py or extension == ".rs":
                subdir = os.path.relpath(os.path.dirname(file), dir)
                examples += [Example(subdir.replace("\\", "/"), name)]

    examples = list(set(examples))

    examples.sort()

    print("----------------------------------------------------------")

    active_languages = ["rust"]
    if not args.no_cpp:
        active_languages.append("cpp")
    if not args.no_py:
        active_languages.append("py")

    print(f"Running {len(examples)} C++, Rust and Python examples…")
    with multiprocessing.Pool() as pool:
        jobs = []
        for example in examples:
            example_opt_out_entirely = example.opt_out_entirely()
            for language in active_languages:
                if language in example_opt_out_entirely:
                    continue
                job = pool.apply_async(run_example, (example, language, args))
                jobs.append(job)
        print(f"Waiting for {len(jobs)} runs to finish…")
        for job in jobs:
            job.get()

    print("----------------------------------------------------------")
    print(f"Active languages: {active_languages}")
    print(f"Comparing {len(examples)} examples…")

    errors = []

    for example in examples:
        print()
        print(f"Comparing '{example}'…")

        example_opt_out_entirely = example.opt_out_entirely()
        example_opt_out_compare = example.opt_out_compare()

        if "rust" in example_opt_out_entirely:
            print("SKIPPED: Missing Rust baseline to compare against")
            continue

        cpp_output_path = example.output_path("cpp")
        python_output_path = example.output_path("python")
        rust_output_path = example.output_path("rust")

        if "cpp" in active_languages:
            if "cpp" in example_opt_out_entirely:
                print("Skipping cpp completely")
            elif "cpp" in example_opt_out_compare:
                print("Skipping cpp compare")
            else:
                try:
                    run_comparison(cpp_output_path, rust_output_path, args.full_dump)
                except Exception as e:
                    errors.append((example, e))

        if "py" in active_languages:
            if "py" in example_opt_out_entirely:
                print("Skipping py completely")
            elif "py" in example_opt_out_compare:
                print("Skipping py compare")
            else:
                try:
                    run_comparison(python_output_path, rust_output_path, args.full_dump)
                except Exception as e:
                    errors.append((example, e))

    if len(errors) == 0:
        print("All tests passed!")
    else:
        print(f"{len(errors)} errors found:")

        for example, _error in errors:
            print(f"❌ {example}")

        for _example, error in errors:
            print()
            print(error)
            print("--------------------------------------")

        print()
        print("----------------------------------------------------------")
        print()

        for example, _error in errors:
            print(f"❌ {example}")

        sys.exit(1)


def run_example(example: Example, language: str, args: argparse.Namespace) -> None:
    if language == "cpp":
        cpp_output_path = run_prebuilt_cpp(example)
        check_non_empty_rrd(cpp_output_path)
    elif language == "py":
        python_output_path = run_python(example)
        check_non_empty_rrd(python_output_path)
    elif language == "rust":
        rust_output_path = run_prebuilt_rust(example, args.release, args.target, args.target_dir)
        check_non_empty_rrd(rust_output_path)
    else:
        raise AssertionError(f"Unknown language: {language}")


def build_rust_snippets(build_env: dict[str, str], release: bool, target: str | None, target_dir: str | None) -> None:
    print("----------------------------------------------------------")
    print("Building snippets for Rust…")

    cmd = ["cargo", "build", "--quiet", "-p", "snippets"]
    if target is not None:
        cmd += ["--target", target]
    if target_dir is not None:
        cmd += ["--target-dir", target_dir]
    if release:
        cmd += ["--release"]

    start_time = time.time()
    run(cmd, env=build_env, timeout=12000)
    elapsed = time.time() - start_time
    print(f"Snippets built in {elapsed:.1f} seconds")
    print("")


def build_python_sdk(build_env: dict[str, str]) -> None:
    print("----------------------------------------------------------")
    print("Building rerun-sdk for Python…")
    start_time = time.time()
    run(["pixi", "run", "py-build", "--quiet"], env=build_env, timeout=12000)
    elapsed = time.time() - start_time
    print(f"rerun-sdk for Python built in {elapsed:.1f} seconds")
    print("")


def build_cpp_snippets() -> None:
    print("----------------------------------------------------------")
    print("Build rerun_c & rerun_prebuilt_cpp…")
    start_time = time.time()
    run(["pixi", "run", "-e", "cpp", "cpp-build-snippets"], timeout=12000)
    elapsed = time.time() - start_time
    print(f"rerun-sdk for C++ built in {elapsed:.1f} seconds")
    print("")


def run_python(example: Example) -> str:
    main_path = f"docs/snippets/all/{example.subdir}/{example.name}.py"
    output_path = example.output_path("python")

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    cmd = [python_executable, main_path] + example.extra_args()

    env = roundtrip_env(save_path=output_path)
    run(cmd, env=env, timeout=30)

    return output_path


def run_prebuilt_rust(example: Example, release: bool, target: str | None, target_dir: str | None) -> str:
    output_path = example.output_path("rust")

    extension = ".exe" if os.name == "nt" else ""

    if target_dir is None:
        mode = "release" if release else "debug"
        if target is not None:
            target_dir = f"./target/{target}/{mode}/snippets"
        else:
            target_dir = f"./target/{mode}/snippets"

    cmd = [f"{target_dir}{extension}"]
    cmd += [example.name]
    cmd += example.extra_args()

    env = roundtrip_env(save_path=output_path)
    run(cmd, env=env, timeout=30)

    return output_path


def run_prebuilt_cpp(example: Example) -> str:
    output_path = example.output_path("cpp")

    extension = ".exe" if os.name == "nt" else ""
    cmd = [f"./build/debug/docs/snippets/{example.name}{extension}"] + example.extra_args()
    env = roundtrip_env(save_path=output_path)
    run(cmd, env=env, timeout=30)

    return output_path


def check_non_empty_rrd(path: str) -> None:
    from pathlib import Path

    assert Path(path).stat().st_size > 0
    # print(f"Confirmed output written to {Path(path).absolute()}")


if __name__ == "__main__":
    main()
