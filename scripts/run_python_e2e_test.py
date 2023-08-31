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
from __future__ import annotations

import argparse
import fileinput
import os
import shutil
import subprocess
import sys
import tempfile
import time
from typing import Iterable

PORT = 9752


def main() -> None:
    parser = argparse.ArgumentParser(description="Runs end-to-end tests of select python example.")
    parser.add_argument("--no-build", action="store_true", help="Skip building rerun-sdk")
    parser.add_argument("--no-pip-reqs", action="store_true", help="Skip installing pip requirements")

    if parser.parse_args().no_build:
        print("Skipping building rerun-sdk - assuming it is already built and up-to-date!")
    else:
        build_env = os.environ.copy()
        if "RUST_LOG" in build_env:
            del build_env["RUST_LOG"]  # The user likely only meant it for the actual tests; not the setup

        print("----------------------------------------------------------")
        print("Building rerun-sdk…")
        start_time = time.time()
        subprocess.Popen(["just", "py-build", "--quiet"], env=build_env).wait()
        elapsed = time.time() - start_time
        print(f"rerun-sdk built in {elapsed:.1f} seconds")
        print("")

    if not parser.parse_args().no_pip_reqs:
        requirements = [
            "tests/python/test_api/requirements.txt",
            "examples/python/car/requirements.txt",
            "examples/python/minimal_options/requirements.txt",
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
        ("tests/python/test_api/main.py", ["--test", "all"]),
        ("examples/python/car/main.py", []),
        ("examples/python/minimal_options/main.py", []),
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

    # NOTE: Doc-examples don't take any parameters and we want to keep it that way.
    # For that reason, we copy them to a temporary directory and monkey patch them so
    # that they connect to a remote Rerun viewer rather than spawn()ing.
    DOC_EXAMPLES_DIR_PATH = "docs/code-examples/"
    old_str = ", spawn=True)"
    new_str = f'); rr.connect(addr="127.0.0.1:{PORT}");'
    for original, example in copy_and_patch(DOC_EXAMPLES_DIR_PATH, old_str, new_str):
        print("----------------------------------------------------------")
        print(f"Testing {original}…\n")
        start_time = time.time()
        run_example(example, [])
        elapsed = time.time() - start_time
        print(f"{original} done in {elapsed:.1f} seconds")
        print()

    print()
    print("All tests passed successfully!")


def run_example(example: str, args: list[str]) -> None:
    # sys.executable: the absolute path of the executable binary for the Python interpreter
    python_executable = sys.executable
    if python_executable is None:
        python_executable = "python3"

    rerun_process = subprocess.Popen(
        [python_executable, "-m", "rerun", "--port", str(PORT), "--strict", "--test-receive"]
    )
    time.sleep(0.3)  # Wait for rerun server to start to remove a logged warning

    python_process = subprocess.Popen([python_executable, example, "--connect", "--addr", f"127.0.0.1:{PORT}"] + args)

    print("Waiting for python process to finish…")
    returncode = python_process.wait(timeout=30)
    assert returncode == 0, f"python process exited with error code {returncode}"

    print("Waiting for rerun process to finish…")
    returncode = rerun_process.wait(timeout=30)
    assert returncode == 0, f"rerun process exited with error code {returncode}"


# Copies all files in a directory to a tempdir and replaces `old_str` with `new_str` in-place.
#
# Yields the patched filenames as it goes.
# When all file have been yielded, the temporary directory is destroyed.
def copy_and_patch(src_dir: str, old_str: str, new_str: str) -> Iterable[tuple[str, str]]:
    with tempfile.TemporaryDirectory() as tmp_dir:
        for root, _, files in os.walk(src_dir):
            for file in [f for f in files if f.endswith(".py")]:
                src_path = os.path.join(root, file)
                dest_path = os.path.join(tmp_dir, file)
                shutil.copy(src_path, dest_path)

                with fileinput.FileInput(dest_path, inplace=True) as f:
                    for line in f:
                        print(line.replace(old_str, new_str), end="")

                print(src_path)
                yield (src_path, dest_path)

            break  # NOTE: Do _not_ recurse into sub-dirs, only weird non-runnable examples live there.


if __name__ == "__main__":
    main()
