#!/usr/bin/env python3

"""
Run some of our python exeamples, piping their log stream to the rerun process.

This is an end-to-end test for testing:
* Our Python API
* LogMsg encoding/decoding
* Arrow encoding/decoding
* TCP connection
* Data store ingestion
"""

import argparse
import os
import subprocess
import sys
import time
from typing import List


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs Objectron data using the Rerun SDK.")
    parser.add_argument("--no-build", action="store_true", help="Skip building depthai-viewer")
    parser.add_argument("--no-pip-reqs", action="store_true", help="Skip installing pip requirements")

    if parser.parse_args().no_build:
        print("Skipping building depthai-viewer - assuming it is already built and up-to-date!")
    else:
        build_env = os.environ.copy()
        if "RUST_LOG" in build_env:
            del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

        print("----------------------------------------------------------")
        print("Building depthai-viewer…")
        start_time = time.time()
        subprocess.Popen(["just", "py-build", "--quiet"], env=build_env).wait()
        elapsed = time.time() - start_time
        print(f"depthai-viewer built in {elapsed:.1f} seconds")
        print("")

    if not parser.parse_args().no_pip_reqs:
        requirements = [
            "examples/python/api_demo/requirements.txt",
            "examples/python/car/requirements.txt",
            "examples/python/multithreading/requirements.txt",
            "examples/python/plots/requirements.txt",
            "examples/python/text_logging/requirements.txt",
        ]

        print("----------------------------------------------------------")
        print("Installing pip dependencies…")
        start_time = time.time()
        for requirement in requirements:
            subprocess.run(["pip", "install", "--quiet", "-r", requirement], check=True)
        elapsed = time.time() - start_time
        print(f"pip install in {elapsed:.1f} seconds")
        print("")

    examples = [
        # Trivial examples that don't require weird dependencies, or downloading data
        ("examples/python/api_demo/main.py", ["--demo", "all"]),
        ("examples/python/car/main.py", []),
        ("examples/python/multithreading/main.py", []),
        ("examples/python/plots/main.py", []),
        ("examples/python/text_logging/main.py", []),
    ]
    for example, args in examples:
        print("----------------------------------------------------------")
        print(f"Testing {example}…\n")
        start_time = time.time()
        run_example(example, args)
        elapsed = time.time() - start_time
        print(f"{example} done in {elapsed:.1f} seconds")
        print()

    print()
    print("All tests passed successfully!")


def run_example(example: str, args: List[str]) -> None:
    port = 9752

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    rerun_process = subprocess.Popen(
        [python_executable, "-m", "depthai_viewer", "--port", str(port), "--strict", "--test-receive"]
    )
    time.sleep(0.3)  # Wait for rerun server to start to remove a logged warning

    python_process = subprocess.Popen([python_executable, example, "--connect", "--addr", f"127.0.0.1:{port}"] + args)

    print("Waiting for python process to finish…")
    returncode = python_process.wait(timeout=30)
    assert returncode == 0, f"python process exited with error code {returncode}"

    print("Waiting for rerun process to finish…")
    returncode = rerun_process.wait(timeout=30)
    assert returncode == 0, f"rerun process exited with error code {returncode}"


if __name__ == "__main__":
    main()
