#!/usr/bin/env python3

"""Runs all our code-examples, for all our languages, and compares the .rrd they output."""

from __future__ import annotations

import argparse
import multiprocessing
import os
import sys
import time
from os import listdir
from os.path import isfile, join
from pathlib import Path

import tomlkit

sys.path.append(os.path.dirname(os.path.realpath(__file__)) + "/../../scripts/")
from roundtrip_utils import roundtrip_env, run, run_comparison  # noqa

config_path = Path(__file__).parent / "snippets.toml"
config = tomlkit.loads(config_path.read_text())
OPT_OUT_RUN = config.opt_out.run
OPT_OUT_COMPARE = config.opt_out.compare
EXTRA_ARGS = {
    name: [arg.replace("$config_dir", str(Path(__file__).parent.absolute())) for arg in args]
    for name, args in config.extra_args.items()
}

EXTRA_ARGS = {
    "asset3d_simple": [f"{os.path.dirname(__file__)}/../assets/cube.glb"],
    "asset3d_out_of_tree": [f"{os.path.dirname(__file__)}/../assets/cube.glb"],
}


def main() -> None:
    parser = argparse.ArgumentParser(description="Run end-to-end cross-language roundtrip tests for all API examples")
    parser.add_argument("--no-py", action="store_true", help="Skip Python tests")
    parser.add_argument("--no-cpp", action="store_true", help="Skip C++ tests")
    # We don't allow skipping Rust - it is what we compate to at the moment
    parser.add_argument("--no-py-build", action="store_true", help="Skip building rerun-sdk for Python")
    parser.add_argument(
        "--no-cpp-build",
        action="store_true",
        help="Skip cmake configure and ahead of time build for rerun_c & rerun_cpp",
    )
    parser.add_argument("--full-dump", action="store_true", help="Dump both rrd files as tables")
    parser.add_argument(
        "--release",
        action="store_true",
        help="Run cargo invocations with --release and CMake with `-DCMAKE_BUILD_TYPE=Release` & `--config Release`",
    )
    parser.add_argument("--target", type=str, default=None, help="Target used for cargo invocations")
    parser.add_argument("--target-dir", type=str, default=None, help="Target directory used for cargo invocations")
    parser.add_argument("example", nargs="*", type=str, default=None, help="Run only the specified examples")

    args = parser.parse_args()

    build_env = os.environ.copy()
    if "RUST_LOG" in build_env:
        del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

    if args.no_py:
        pass  # No need to build the Python SDK
    elif args.no_py_build:
        print("Skipping building python rerun-sdk - assuming it is already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Building rerun-sdk for Python…")
        start_time = time.time()
        run(["just", "py-build", "--quiet"], env=build_env)
        elapsed = time.time() - start_time
        print(f"rerun-sdk for Python built in {elapsed:.1f} seconds")
        print("")

    if args.no_cpp:
        pass  # No need to build the C++ SDK
    elif args.no_cpp_build:
        print("Skipping cmake configure & build for rerun_c & rerun_cpp - assuming it is already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Build rerun_c & rerun_cpp…")
        start_time = time.time()
        run(["pixi", "run", "cpp-build-doc-examples"])
        elapsed = time.time() - start_time
        print(f"rerun-sdk for C++ built in {elapsed:.1f} seconds")
        print("")

    if len(args.example) > 0:
        examples = args.example
    else:
        dir = os.path.join(os.path.dirname(__file__), "all")
        files = [f for f in listdir(dir) if isfile(join(dir, f))]
        examples = [
            filename
            for filename, extension in [os.path.splitext(file) for file in files]
            if extension == ".cpp" and not args.no_cpp or extension == ".py" and not args.no_py or extension == ".rs"
        ]

    examples = list(set(examples))
    examples.sort()

    print("----------------------------------------------------------")

    active_languages = ["rust"]
    if not args.no_cpp:
        active_languages.append("cpp")
    if not args.no_py:
        active_languages.append("py")

    # Running CMake in parallel causes failures during rerun_sdk & arrow build.
    if not args.no_cpp_build:
        print(f"Running {len(examples)} C++ examples…")
        for example in examples:
            example_opt_out_entirely = OPT_OUT_RUN.get(example, [])
            if "cpp" in example_opt_out_entirely:
                continue
            run_example(example, "cpp", args)

    print(f"Running {len(examples)} Rust and Python examples…")
    with multiprocessing.Pool() as pool:
        jobs = []
        for example in examples:
            example_opt_out_entirely = OPT_OUT_RUN.get(example, [])
            for language in active_languages:
                if language in example_opt_out_entirely or language == "cpp":  # cpp already processed in series.
                    continue
                job = pool.apply_async(run_example, (example, language, args))
                jobs.append(job)
        print(f"Waiting for {len(jobs)} runs to finish…")
        for job in jobs:
            job.get()

    print("----------------------------------------------------------")
    print(f"Comparing {len(examples)} examples…")

    for example in examples:
        print()
        print("----------------------------------------------------------")
        print(f"Comparing example '{example}'…")

        example_opt_out_entirely = OPT_OUT_RUN.get(example, [])
        example_opt_out_compare = OPT_OUT_COMPARE.get(example, [])

        if "rust" in example_opt_out_entirely:
            continue  # No baseline to compare against

        cpp_output_path = f"docs/code-examples/all/{example}_cpp.rrd"
        python_output_path = f"docs/code-examples/all/{example}_py.rrd"
        rust_output_path = f"docs/code-examples/all/{example}_rust.rrd"

        if "cpp" in active_languages and "cpp" not in example_opt_out_entirely and "cpp" not in example_opt_out_compare:
            run_comparison(cpp_output_path, rust_output_path, args.full_dump)

        if "py" in active_languages and "py" not in example_opt_out_entirely and "py" not in example_opt_out_compare:
            run_comparison(python_output_path, rust_output_path, args.full_dump)

    print()
    print("----------------------------------------------------------")
    print("All tests passed!")


def run_example(example: str, language: str, args: argparse.Namespace) -> None:
    if language == "cpp":
        cpp_output_path = run_roundtrip_cpp(example, args.release)
        check_non_empty_rrd(cpp_output_path)
    elif language == "py":
        python_output_path = run_roundtrip_python(example)
        check_non_empty_rrd(python_output_path)
    elif language == "rust":
        rust_output_path = run_roundtrip_rust(example, args.release, args.target, args.target_dir)
        check_non_empty_rrd(rust_output_path)
    else:
        assert False, f"Unknown language: {language}"


def run_roundtrip_python(example: str) -> str:
    main_path = f"docs/code-examples/all/{example}.py"
    output_path = f"docs/code-examples/all/{example}_py.rrd"

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    cmd = [python_executable, main_path] + (EXTRA_ARGS.get(example) or [])

    env = roundtrip_env(save_path=output_path)
    run(cmd, env=env, timeout=30)

    return output_path


def run_roundtrip_rust(example: str, release: bool, target: str | None, target_dir: str | None) -> str:
    output_path = f"docs/code-examples/all/{example}_rust.rrd"

    cmd = ["cargo", "run", "--quiet", "-p", "code_examples"]

    if target is not None:
        cmd += ["--target", target]

    if target_dir is not None:
        cmd += ["--target-dir", target_dir]

    if release:
        cmd += ["--release"]

    cmd += ["--", example]

    if EXTRA_ARGS.get(example):
        cmd += EXTRA_ARGS[example]

    env = roundtrip_env(save_path=output_path)
    run(cmd, env=env, timeout=12000)

    return output_path


def run_roundtrip_cpp(example: str, release: bool) -> str:
    output_path = f"docs/code-examples/all/{example}_cpp.rrd"

    cmd = [f"./build/debug/docs/code-examples/{example}"] + (EXTRA_ARGS.get(example) or [])
    env = roundtrip_env(save_path=output_path)
    run(cmd, env=env, timeout=12000)

    return output_path


def check_non_empty_rrd(path: str) -> None:
    from pathlib import Path

    assert Path(path).stat().st_size > 0


if __name__ == "__main__":
    main()
