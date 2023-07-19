#!/usr/bin/env python3

"""
Run our end-to-end cross-language roundtrip tests for all SDKs.

The list of archetypes is read directly from `crates/re_types/definitions/rerun/archetypes`.
If you create a new archetype definition without end-to-end tests, this will fail.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from os import listdir
from os.path import isfile, join

ARCHETYPES_PATH = "crates/re_types/definitions/rerun/archetypes"


def main() -> None:
    parser = argparse.ArgumentParser(description="Run our end-to-end cross-language roundtrip tests for all SDK")
    parser.add_argument("--no-build", action="store_true", help="Skip building rerun-sdk")
    parser.add_argument("--full-dump", action="store_true", help="Dump both rrd files as tables")
    parser.add_argument("--release", action="store_true", help="Run cargo invocations with --release")
    parser.add_argument("--target", type=str, default=None, help="Target used for cargo invocations")
    parser.add_argument("--target-dir", type=str, default=None, help="Target directory used for cargo invocations")

    args = parser.parse_args()

    if args.no_build:
        print("Skipping building rerun-sdk - assuming it is already built and up-to-date!")
    else:
        build_env = os.environ.copy()
        if "RUST_LOG" in build_env:
            del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

        print("----------------------------------------------------------")
        print("Building rerun-sdk…")
        start_time = time.time()
        subprocess.Popen(["just", "py-build"], env=build_env).wait()
        elapsed = time.time() - start_time
        print(f"rerun-sdk built in {elapsed:.1f} seconds")
        print("")

    files = [f for f in listdir(ARCHETYPES_PATH) if isfile(join(ARCHETYPES_PATH, f))]
    archetypes = [filename for filename, extension in [os.path.splitext(file) for file in files] if extension == ".fbs"]

    for arch in archetypes:
        python_output_path = run_roundtrip_python(arch)
        rust_output_path = run_roundtrip_rust(arch, args.release, args.target, args.target_dir)
        run_comparison(python_output_path, rust_output_path, args.full_dump)


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

    print(cmd)
    roundtrip_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = roundtrip_process.wait(timeout=30)
    assert returncode == 0, f"python roundtrip process exited with error code {returncode}"

    return output_path


def run_roundtrip_rust(arch: str, release: bool, target: str | None, target_dir: str | None) -> str:
    project_name = f"roundtrip_{arch}"
    output_path = f"tests/rust/roundtrips/{arch}/out.rrd"

    cmd = ["cargo", "r", "-p", project_name]

    if target is not None:
        cmd += ["--target", target]

    if target_dir is not None:
        cmd += ["--target-dir", target_dir]

    if release:
        cmd += ["--release"]

    cmd += ["--", "--save", output_path]

    print(cmd)
    roundtrip_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = roundtrip_process.wait(timeout=12000)
    assert returncode == 0, f"rust roundtrip process exited with error code {returncode}"

    return output_path


def run_comparison(python_output_path: str, rust_output_path: str, full_dump: bool):
    cmd = ["rerun", "compare"]
    if full_dump:
        cmd += ["--full-dump"]
    cmd += [python_output_path, rust_output_path]

    print(cmd)
    comparison_process = subprocess.Popen(cmd, env=roundtrip_env())
    returncode = comparison_process.wait(timeout=30)
    assert returncode == 0, f"comparison process exited with error code {returncode}"


if __name__ == "__main__":
    main()
