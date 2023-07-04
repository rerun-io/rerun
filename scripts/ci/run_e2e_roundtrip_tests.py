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

    if parser.parse_args().no_build:
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
        rust_output_path = run_roundtrip_rust(arch)
        run_comparison(python_output_path, rust_output_path)


def run_roundtrip_python(arch: str) -> str:
    main_path = f"tests/python/roundtrips/{arch}/main.py"
    output_path = f"tests/python/roundtrips/{arch}/out.rrd"

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    cmd = [python_executable, main_path, "--save", output_path]
    print(cmd)
    roundtrip_process = subprocess.Popen(cmd)
    returncode = roundtrip_process.wait(timeout=600)
    assert returncode == 0, f"python roundtrip process exited with error code {returncode}"

    return output_path


def run_roundtrip_rust(arch: str) -> str:
    project_name = f"roundtrip_{arch}"
    output_path = f"tests/rust/roundtrips/{arch}/out.rrd"

    cmd = ["cargo", "r", "-p", project_name, "--", "--save", output_path]
    print(cmd)
    roundtrip_process = subprocess.Popen(cmd)
    returncode = roundtrip_process.wait(timeout=6000)
    assert returncode == 0, f"rust roundtrip process exited with error code {returncode}"

    return output_path


def run_comparison(python_output_path: str, rust_output_path: str):
    cmd = ["cargo", "r", "-p", "rerun-cli", "--", "compare", python_output_path, rust_output_path]
    print(cmd)
    roundtrip_process = subprocess.Popen(cmd)
    returncode = roundtrip_process.wait(timeout=600)
    assert returncode == 0, f"comparison process exited with error code {returncode}"


if __name__ == "__main__":
    main()
