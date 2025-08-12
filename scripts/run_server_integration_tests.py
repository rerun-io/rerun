#!/usr/bin/env python3

"""
Run some of our python examples, piping their log stream to the rerun process.

This is an end-to-end test for testing:
* Our Python API
* LogMsg encoding/decoding
* Arrow encoding/decoding
* gRPC connection
* Data store ingestion
"""

from __future__ import annotations

import argparse
import os
import subprocess
import time
from pathlib import Path

PORT = 9752


def main() -> None:
    parser = argparse.ArgumentParser(description="Runs end-to-end tests of select python example.")
    parser.add_argument("--no-build", action="store_true", help="Skip building rerun-sdk")
    parser.add_argument("--no-install", action="store_true", help="Skip installing the examples")

    if parser.parse_args().no_build:
        print("Skipping building rerun-sdk - assuming it is already built and up-to-date!")
    else:
        build_env = os.environ.copy()
        if "RUST_LOG" in build_env:
            del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

        print("----------------------------------------------------------")
        print("Building rerun-sdk…")
        start_time = time.time()
        subprocess.Popen(["pixi", "run", "py-build", "--quiet"], env=build_env).wait()
        elapsed = time.time() - start_time
        print(f"rerun-sdk built in {elapsed:.1f} seconds")
        print("")

    tests = [
        ("tests/python/server_integration", ["--test", "all"]),
    ]

    if not parser.parse_args().no_install:
        print("----------------------------------------------------------")
        print("Installing examples…")
        start_time = time.time()
        args = ["pip", "install", "--quiet"]
        for test, _ in tests:
            # install in editable mode so `__file__` relative paths work
            args.extend(["-e", test])
        subprocess.run(args, check=True)
        elapsed = time.time() - start_time
        print(f"pip install in {elapsed:.1f} seconds")
        print("")

    for test, args in tests:
        print("----------------------------------------------------------")
        print(f"Testing {test}…\n")
        start_time = time.time()
        run_example(Path(test).name, args)
        elapsed = time.time() - start_time
        print(f"{test} done in {elapsed:.1f} seconds")
        print()

    print()
    print("All tests passed successfully!")


def run_example(example: str, extra_args: list[str]) -> None:
    env = os.environ.copy()

    # raise exception on warnings, e.g. when using a @deprecated function:
    env["PYTHONWARNINGS"] = "error"

    env["RERUN_STRICT"] = "1"
    env["RERUN_PANIC_ON_WARN"] = "1"

    server_env = env.copy()
    if "RUST_LOG" not in server_env:
        # Server can be noisy by default
        server_env["RUST_LOG"] = "warning"
    server_env["RUST_LOG"] = "info"

    cmd = ["cargo", "run", "--bin", "rerun-server", "--", "--dataset", "tests/assets/rrd/dataset"]
    server_process = subprocess.Popen(cmd, env=server_env)
    time.sleep(10.5)  # Wait for rerun server to start to remove a logged warning

    cmd = ["python", "-m", example] + extra_args
    python_process = subprocess.Popen(cmd, env=env)

    print("Waiting for python process to finish…")
    returncode = python_process.wait(timeout=30)

    # Be certain to terminate server process before checking the assert below
    server_process.terminate()

    assert returncode == 0, f"python process exited with error code {returncode}"

    print("Waiting for server process to finish…")
    returncode = server_process.wait(timeout=30)
    assert returncode == 0, f"server process exited with error code {returncode}"


if __name__ == "__main__":
    main()
