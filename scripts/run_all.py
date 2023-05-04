#!/usr/bin/env python3

"""Run all examples."""

import argparse
import os
import subprocess
import time
from glob import glob


def run_py_example(path: str, args: list[str] = []) -> None:
    process = subprocess.Popen(
        ["python3", "main.py", "--num-frames=30", "--steps=200"] + args,
        cwd=path,
    )
    returncode = process.wait()
    print(f"process exited with error code {returncode}")


def run_saved_example(path: str, args: list[str] = []) -> None:
    process = subprocess.Popen(
        ["cargo", "run", "-p", "rerun", "--all-features", "--", "out.rrd"] + args,
        cwd=path,
    )
    returncode = process.wait()
    print(f"process exited with error code {returncode}")


def collect_examples() -> list[str]:
    return [os.path.dirname(entry) for entry in glob("examples/python/**/main.py")]


def start_viewer(args: list[str] = []) -> subprocess.Popen[bytes]:
    process = subprocess.Popen(
        ["cargo", "run", "-p", "rerun", "--all-features", "--"] + args,
        stdout=subprocess.PIPE,
    )
    time.sleep(1)  # give it a moment to start
    return process


def run_build() -> None:
    process = subprocess.Popen(
        ["maturin", "develop", "--manifest-path", "rerun_py/Cargo.toml", '--extras="tests"'],
    )
    returncode = process.wait()
    assert returncode == 0, f"process exited with error code {returncode}"


def main() -> None:
    parser = argparse.ArgumentParser(description="Runs all examples.")
    parser.add_argument("--skip-build", action="store_true", help="Skip building the Python SDK")
    subparsers = parser.add_subparsers(dest="command")
    subparsers.add_parser("web", help="Run all examples in a web viewer.")
    subparsers.add_parser("save", help="Run all examples, save them to disk as rrd, then view them natively.")

    args = parser.parse_args()

    examples = collect_examples()

    if not args.skip_build:
        run_build()

    if args.command == "web":
        viewer = start_viewer(["--web-viewer"])
        for example in examples:
            run_py_example(example, ["--connect"])
        viewer.kill()
    elif args.command == "save":
        viewer = start_viewer()
        for example in examples:
            run_py_example(example, ["--save", "out.rrd"])
        viewer.kill()

        for example in examples:
            run_saved_example(example)
    else:
        viewer = start_viewer()
        for example in examples:
            run_py_example(example)
        viewer.kill()


if __name__ == "__main__":
    main()
