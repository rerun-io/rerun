#!/usr/bin/env python3

"""
Run our end-to-end cross-language roundtrip tests for all SDKs.

The list of archetypes is read directly from `crates/store/re_types/definitions/rerun/archetypes`.
If you create a new archetype definition without end-to-end tests, this will fail.
"""

from __future__ import annotations

import argparse
import multiprocessing
import os
import sys
import time
from os import listdir
from os.path import isfile, join

sys.path.append(os.path.dirname(os.path.realpath(__file__)) + "/../scripts/")
from roundtrip_utils import cpp_build_dir, roundtrip_env, run, run_comparison  # noqa

ARCHETYPES_PATHS = [
    "crates/store/re_types/definitions/rerun/archetypes",
    "crates/store/re_types/definitions/rerun/blueprint/archetypes",
]

opt_out = {}


def main() -> None:
    parser = argparse.ArgumentParser(description="Run our end-to-end cross-language roundtrip tests for all SDK")
    parser.add_argument(
        "--no-run",
        action="store_true",
        help="Do not build or run anything. Only check that the roundtrip tests exists.",
    )
    parser.add_argument("--no-py-build", action="store_true", help="Skip building rerun-sdk for Python")
    parser.add_argument(
        "--no-cpp-build",
        action="store_true",
        help="Skip cmake configure and ahead of time build for rerun_c & rerun_cpp",
    )
    parser.add_argument("--full-dump", action="store_true", help="Dump both rrd files as tables")
    parser.add_argument("--release", action="store_true", help="Run cargo invocations with --release")
    parser.add_argument("--target", type=str, default=None, help="Target used for cargo invocations")
    parser.add_argument("--target-dir", type=str, default=None, help="Target directory used for cargo invocations")
    parser.add_argument("archetype", nargs="*", type=str, default=None, help="Run only the specified archetypes")

    args = parser.parse_args()

    # Which archetypes to run?
    if len(args.archetype) > 0:
        archetypes = args.archetype
    else:
        files = [
            f for archetype_path in ARCHETYPES_PATHS for f in listdir(archetype_path) if isfile(join(archetype_path, f))
        ]
        archetypes = [
            filename for filename, extension in [os.path.splitext(file) for file in files] if extension == ".fbs"
        ]
        assert len(archetypes) > 0, "No archetypes found!"

    # Opt out of archetypes for which there's no test.
    for arch in archetypes:
        for lang in ["cpp", "python", "rust"]:
            if lang not in opt_out.get(arch, []):
                dir_path = f"tests/{lang}/roundtrips/{arch}"
                if not os.path.exists(dir_path):
                    if arch in opt_out:
                        opt_out[arch].append(lang)
                    else:
                        opt_out[arch] = [lang]

    if args.no_run:
        print("All archetypes have roundtrip tests.")
        sys.exit(0)

    build_env = os.environ.copy()
    if "RUST_LOG" in build_env:
        del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

    if args.no_py_build:
        print("Skipping building python rerun-sdk - assuming it is already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Building rerun-sdk for Python…")
        start_time = time.time()
        run(["pixi", "run", "-e", "py", "py-build", "--quiet"], env=build_env)
        elapsed = time.time() - start_time
        print(f"rerun-sdk for Python built in {elapsed:.1f} seconds")
        print("")

    if args.no_cpp_build:
        print("Skipping cmake configure & build - assuming all tests are already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Build rerun_c & roundtrips for C++…")
        start_time = time.time()
        run(["pixi", "run", "-e", "cpp", "cpp-build-roundtrips"])
        elapsed = time.time() - start_time
        print(f"rerun-sdk for C++ built in {elapsed:.1f} seconds")
        print("")

    print("----------------------------------------------------------")
    print(f"Building {len(archetypes)} archetypes…")

    with multiprocessing.Pool() as pool:
        start_time = time.time()
        jobs = []
        for arch in archetypes:
            arch_opt_out = opt_out.get(arch, [])
            for language in ["python", "rust", "cpp"]:
                if language in arch_opt_out:
                    continue
                job = pool.apply_async(run_roundtrips, (arch, language, args))
                jobs.append(job)
        print(f"Waiting for {len(jobs)} build jobs to finish…")
        for job in jobs:
            job.get()
        elapsed = time.time() - start_time
        print(f"C++, Python and Rust examples ran in {elapsed:.1f} seconds")

    print("----------------------------------------------------------")
    print(f"Comparing recordings for {len(archetypes)} archetypes…")
    start_time = time.time()

    for arch in archetypes:
        print()
        print("----------------------------------------------------------")
        print(f"Comparing archetype '{arch}'…")

        arch_opt_out = opt_out.get(arch, [])

        if "rust" not in arch_opt_out:
            cpp_output_path = f"tests/cpp/roundtrips/{arch}/out.rrd"
            python_output_path = f"tests/python/roundtrips/{arch}/out.rrd"
            rust_output_path = f"tests/rust/roundtrips/{arch}/out.rrd"

            if "python" not in arch_opt_out:
                run_comparison(python_output_path, rust_output_path, args.full_dump)

            if "cpp" not in arch_opt_out:
                run_comparison(cpp_output_path, rust_output_path, args.full_dump)

    print()
    elapsed = time.time() - start_time
    print(f"Comparisons ran in {elapsed:.1f} seconds")
    print()
    print("----------------------------------------------------------")
    print("All tests passed!")


def run_roundtrips(arch: str, language: str, args: argparse.Namespace) -> None:
    if language == "cpp":
        run_roundtrip_cpp(arch)
    elif language == "python":
        run_roundtrip_python(arch)
    elif language == "rust":
        run_roundtrip_rust(arch, args.release, args.target, args.target_dir)
    else:
        assert False, f"Unknown language: {language}"


def run_roundtrip_python(arch: str) -> str:
    main_path = f"tests/python/roundtrips/{arch}/main.py"
    output_path = f"tests/python/roundtrips/{arch}/out.rrd"

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    cmd = [python_executable, main_path, "--save", output_path]

    run(cmd, env=roundtrip_env(), timeout=30)

    return output_path


def run_roundtrip_rust(arch: str, release: bool, target: str | None, target_dir: str | None) -> str:
    project_name = f"roundtrip_{arch}"
    output_path = f"tests/rust/roundtrips/{arch}/out.rrd"

    cmd = ["cargo", "run", "--quiet", "-p", project_name]

    if target is not None:
        cmd += ["--target", target]

    if target_dir is not None:
        cmd += ["--target-dir", target_dir]

    if release:
        cmd += ["--release"]

    cmd += ["--", "--save", output_path]

    run(cmd, env=roundtrip_env(), timeout=12000)

    return output_path


def run_roundtrip_cpp(arch: str) -> str:
    target_name = f"roundtrip_{arch}"
    output_path = f"tests/cpp/roundtrips/{arch}/out.rrd"

    extension = ".exe" if os.name == "nt" else ""
    cmd = [f"./build/debug/tests/cpp/roundtrips/{target_name}{extension}", output_path]
    run(cmd, env=roundtrip_env(), timeout=12000)

    return output_path


if __name__ == "__main__":
    main()
