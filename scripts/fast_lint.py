#!/usr/bin/env python3

"""Runs the common linters on any files that have changed relative to the main branch."""
from __future__ import annotations

import argparse
import concurrent.futures
import logging
import os
import subprocess
import sys
import time

from git import Repo


def changed_files() -> list[str]:
    repo = Repo(os.getcwd())

    current_branch = repo.active_branch
    common_ancestor = repo.merge_base(current_branch, "main")[0]

    return [item.b_path for item in repo.index.diff(common_ancestor)]


def run_cmd(cmd: str, files: list[str] | None = None) -> bool:
    start = time.time()

    if files is not None and len(files) == 0:
        logging.info(f"SKIP: {cmd}")
        return True

    if files is None:
        files = []

    cmd_arr = ["pixi", "run", cmd]

    cmd_preview = " ".join(cmd_arr) + " <FILES>" if files else ""

    proc = subprocess.run(cmd_arr + files, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if proc.returncode == 0:
        logging.info(f"PASS: {cmd} in {time.time() - start:.2f}s")
        logging.debug(f"----------\n{cmd_preview}\n{proc.stdout}\n----------")
    else:
        logging.info(
            f"FAIL: {cmd} in {time.time() - start:.2f}s \n----------\n{cmd_preview}\n{proc.stdout}\n----------"
        )

    return proc.returncode == 0


def filter_ext(files: list[str], ext: list[str]) -> list[str]:
    return [f for f in files if any(f.endswith(e) for e in ext)]


def filter_cpp(files: list[str]) -> list[str]:
    return [f for f in files if f.endswith(".h") or f.endswith(".hpp") or f.endswith(".c") or f.endswith(".cpp")]


def main() -> None:
    start = time.time()
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--log-level",
        dest="log_level",
        default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"],
        help="Set the logging level (default: INFO)",
    )
    parser.add_argument("--num-threads", type=int, default=8, help="Number of threads to use (default: 4)")
    parser.add_argument("--skip", type=str, default="", help="Comma-separated list of tasks to skip")
    parser.add_argument(
        "files",
        metavar="file",
        type=str,
        nargs="*",
        help="File paths. Empty = all files, recursively.",
    )

    args = parser.parse_args()
    logging.basicConfig(level=args.log_level)

    script_dirpath = os.path.dirname(os.path.realpath(__file__))
    root_dirpath = os.path.abspath(f"{script_dirpath}/..")
    os.chdir(root_dirpath)

    if args.files:
        files = args.files
    else:
        files = changed_files()

    skip = args.skip.split(",")

    logging.debug("Checking:")
    for f in files:
        logging.debug(f"  {f}")

    jobs = [
        ("lint-codegen", None),
        ("lint-cpp", filter_ext(files, [".cpp", ".c", ".h", ".hpp"])),
        ("lint-rerun", files),
        ("lint-rs", filter_ext(files, [".rs"])),
        ("lint-py-black", filter_ext(files, [".py"])),
        ("lint-py-blackdoc", filter_ext(files, [".py"])),
        ("lint-py-mypy", filter_ext(files, [".py"])),
        ("lint-py-ruff", filter_ext(files, [".py"])),
        ("lint-taplo", filter_ext(files, [".toml"])),
        ("lint-typos", files),
    ]

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.num_threads) as executor:
        results = [executor.submit(run_cmd, command, files) for command, files in jobs if command not in skip]

    success = all(result.result() for result in results)

    if success:
        logging.info(f"All lints passed in {time.time() - start:.2f}s")
        sys.exit(0)
    else:
        logging.info(f"Lints failed in {time.time() - start:.2f}s")
        sys.exit(1)


if __name__ == "__main__":
    main()
