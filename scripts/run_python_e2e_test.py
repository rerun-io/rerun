#!/usr/bin/env python3

"""
Run some of our python examples, piping their log stream to the rerun process.

This is an end-to-end test for testing:
* Our Python API
* LogMsg encoding/decoding
* Arrow encoding/decoding
* TCP connection
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

    examples = [
        # Trivial examples that don't require weird dependencies, or downloading data
        ("tests/python/test_api", ["--test", "all"]),
        ("examples/python/minimal_options", []),
        ("examples/python/multithreading", []),
        ("examples/python/plots", []),
    ]

    if not parser.parse_args().no_install:
        print("----------------------------------------------------------")
        print("Installing examples…")
        start_time = time.time()
        args = ["pip", "install", "--quiet"]
        for example, _ in examples:
            # install in editable mode so `__file__` relative paths work
            args.extend(["-e", example])
        subprocess.run(args, check=True)
        elapsed = time.time() - start_time
        print(f"pip install in {elapsed:.1f} seconds")
        print("")

    for example, args in examples:
        print("----------------------------------------------------------")
        print(f"Testing {example}…\n")
        start_time = time.time()
        run_example(Path(example).name, args)
        elapsed = time.time() - start_time
        print(f"{example} done in {elapsed:.1f} seconds")
        print()

    print()
    print("All tests passed successfully!")


def run_example(example: str, extra_args: list[str]) -> None:
    env = os.environ.copy()
    env["RERUN_STRICT"] = "1"
    env["RERUN_PANIC_ON_WARN"] = "1"

    cmd = ["python", "-m", "rerun", "--port", str(PORT), "--test-receive"]
    rerun_process = subprocess.Popen(cmd, env=env)
    time.sleep(0.5)  # Wait for rerun server to start to remove a logged warning

    cmd = ["python", "-m", example, "--connect", "--addr", f"127.0.0.1:{PORT}"] + extra_args
    python_process = subprocess.Popen(cmd, env=env)

    print("Waiting for python process to finish…")
    returncode = python_process.wait(timeout=30)
    assert returncode == 0, f"python process exited with error code {returncode}"

    print("Waiting for rerun process to finish…")
    returncode = rerun_process.wait(timeout=30)
    assert returncode == 0, f"rerun process exited with error code {returncode}"


if __name__ == "__main__":
    main()
