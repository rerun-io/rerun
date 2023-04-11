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

import os
import subprocess
import sys
import time


def main() -> None:
    build_env = os.environ.copy()
    if "RUST_LOG" in build_env:
        del build_env["RUST_LOG"] # The user likely only meant it for the actual tests; not the setup

    print("----------------------------------------------------------")
    print("Building rerun-sdk…")
    start_time = time.time()
    subprocess.Popen(["just", "py-build", "--quiet"], env=build_env).wait()
    elapsed = time.time() - start_time
    print(f"rerun-sdk built in {elapsed:.1f} seconds")
    print("")

    examples = [
        "examples/python/api_demo/main.py",
        "examples/python/car/main.py",
        "examples/python/multithreading/main.py",
        "examples/python/plots/main.py",
        "examples/python/text_logging/main.py",
    ]
    for example in examples:
        print("----------------------------------------------------------")
        print(f"Testing {example}…\n")
        start_time = time.time()
        run_example(example)
        elapsed = time.time() - start_time
        print(f"{example} done in {elapsed:.1f} seconds")
        print()

    print()
    print("All tests passed successfully!")


def run_example(example: str) -> None:
    port = 9752

    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    rerun_process = subprocess.Popen(
        [python_executable, "-m", "rerun", "--port", str(port), "--strict", "--test-receive"]
    )
    time.sleep(0.3)  # Wait for rerun server to start to remove a logged warning

    python_process = subprocess.Popen([python_executable, example, "--connect", "--addr", f"127.0.0.1:{port}"])

    print("Waiting for python process to finish…")
    python_process.wait(timeout=30)

    print("Waiting for rerun process to finish…")
    rerun_process.wait(timeout=30)


if __name__ == "__main__":
    main()
