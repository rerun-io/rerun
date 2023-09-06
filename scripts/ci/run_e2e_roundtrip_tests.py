#!/usr/bin/env python3

"""
Run our end-to-end cross-language roundtrip tests for all SDKs.

The list of archetypes is read directly from `crates/re_types/definitions/rerun/archetypes`.
If you create a new archetype definition without end-to-end tests, this will fail.
"""

from __future__ import annotations

import argparse
import multiprocessing
import os
import subprocess
import sys
import time
from os import listdir
from os.path import isfile, join

ARCHETYPES_PATH = "crates/re_types/definitions/rerun/archetypes"

opt_out = {
    "depth_image": ["cpp"],
    "line_strips2d": ["cpp"],  # TODO(#2786): Needs rect
    "points2d": ["cpp"],  # TODO(#2786): Needs rect
    "image": ["cpp"],
    "tensor": ["cpp"],
}


def main() -> None:
    parser = argparse.ArgumentParser(description="Run our end-to-end cross-language roundtrip tests for all SDK")
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
    parser.add_argument("archetype", nargs="*", type=str, default=None, help="Run only the specified archetypes")

    args = parser.parse_args()

    build_env = os.environ.copy()
    if "RUST_LOG" in build_env:
        del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

    if args.no_py_build:
        print("Skipping building python rerun-sdk - assuming it is already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Building rerun-sdk for Python…")
        start_time = time.time()
        returncode = subprocess.Popen(["just", "py-build", "--quiet"], env=build_env).wait()
        assert returncode == 0, f"Python rerun-sdk build failed with exit code {returncode}"
        elapsed = time.time() - start_time
        print(f"rerun-sdk for Python built in {elapsed:.1f} seconds")
        print("")

    if args.no_cpp_build:
        print("Skipping cmake configure & build for rerun_c & rerun_cpp - assuming it is already built and up-to-date!")
    else:
        print("----------------------------------------------------------")
        print("Build rerun_c & rerun_cpp…")
        start_time = time.time()
        os.makedirs("build", exist_ok=True)
        build_type = "Debug"
        if args.release:
            build_type = "Release"
        configure_args = ["cmake", f"-DCMAKE_BUILD_TYPE={build_type}", "-DCMAKE_COMPILE_WARNING_AS_ERROR=ON", ".."]
        print("> ${subprocess.list2cmdline(configure_args)}")
        returncode = subprocess.Popen(
            configure_args,
            env=build_env,
            cwd="build",
        ).wait()
        assert returncode == 0, f"configuring cmake failed with exit code {returncode}"
        cmake_build("rerun_sdk", args.release)
        elapsed = time.time() - start_time
        print(f"rerun-sdk for C++ built in {elapsed:.1f} seconds")
        print("")

    files = [f for f in listdir(ARCHETYPES_PATH) if isfile(join(ARCHETYPES_PATH, f))]

    if len(args.archetype) > 0:
        archetypes = args.archetype
    else:
        archetypes = [
            filename for filename, extension in [os.path.splitext(file) for file in files] if extension == ".fbs"
        ]

    for arch in archetypes:
        arch_opt_out = opt_out.get(arch, [])

        if "rust" not in arch_opt_out:
            rust_output_path = run_roundtrip_rust(arch, args.release, args.target, args.target_dir)

            if "py" not in arch_opt_out:
                python_output_path = run_roundtrip_python(arch)
                run_comparison(python_output_path, rust_output_path, args.full_dump)

            if "cpp" not in arch_opt_out:
                cpp_output_path = run_roundtrip_cpp(arch, args.release)
                run_comparison(rust_output_path, cpp_output_path, args.full_dump)


def roundtrip_env() -> dict[str, str]:
    # NOTE: Make sure to disable batching, otherwise the Arrow concatenation logic within
    # the batcher will happily insert uninitialized padding bytes as needed!
    env = os.environ.copy()
    env["RERUN_FLUSH_NUM_ROWS"] = "0"
    return env


def run_roundtrip_python(arch: str) -> str:
    main_path = f"tests/python/roundtrips/{arch}/main.py"
    output_path = f"tests/python/roundtrips/{arch}/out.rrd"

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    cmd = [python_executable, main_path, "--save", output_path]

    print(f"\n> {subprocess.list2cmdline(cmd)}")
    roundtrip_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = roundtrip_process.wait(timeout=30)
    assert returncode == 0, f"python roundtrip process exited with error code {returncode}"

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

    print(f"\n> {subprocess.list2cmdline(cmd)}")
    roundtrip_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = roundtrip_process.wait(timeout=12000)
    assert returncode == 0, f"rust roundtrip process exited with error code {returncode}"

    return output_path


def run_roundtrip_cpp(arch: str, release: bool) -> str:
    target_name = f"roundtrip_{arch}"
    output_path = f"tests/cpp/roundtrips/{arch}/out.rrd"

    cmake_build(target_name, release)

    cmd = [f"./build/tests/cpp/roundtrips/{target_name}", output_path]
    print(f"\n> {subprocess.list2cmdline(cmd)}")
    roundtrip_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = roundtrip_process.wait(timeout=12000)
    assert returncode == 0, f"cpp roundtrip process exited with error code {returncode}"

    return output_path


def cmake_build(target: str, release: bool) -> None:
    config = "Debug"
    if release:
        config = "Release"

    build_process_args = [
        "cmake",
        "--build",
        ".",
        "--config",
        config,
        "--target",
        target,
        "--parallel",
        str(multiprocessing.cpu_count()),
    ]
    print(f"\n> {subprocess.list2cmdline(build_process_args)}")
    result = subprocess.run(build_process_args, cwd="build")

    assert result.returncode == 0, f"cmake build of {target} exited with error code {result.returncode}"


def run_comparison(rrd0_path: str, rrd1_path: str, full_dump: bool) -> None:
    cmd = ["rerun", "compare"]
    if full_dump:
        cmd += ["--full-dump"]
    cmd += [rrd0_path, rrd1_path]

    print(f"\n> {subprocess.list2cmdline(cmd)}")
    comparison_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = comparison_process.wait(timeout=30)
    assert returncode == 0, f"comparison process exited with error code {returncode}"


if __name__ == "__main__":
    main()
