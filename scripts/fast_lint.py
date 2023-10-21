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


def run_cmd(cmd: list[str], files: list[str] | bool) -> bool:
    start = time.time()
    if not files:
        logging.info(f"SKIP: `{' '.join(cmd)}`")
        return True

    if isinstance(files, bool):
        files = []
    proc = subprocess.run(cmd + files, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if proc.returncode == 0:
        logging.info(f"PASS: `{' '.join(cmd)} {'<FILES>' if files else ''}` in {time.time() - start:.2f}s")
        if proc.stdout:
            logging.debug(f"stdout: {proc.stdout}")
    else:
        logging.info(f"FAIL: `{' '.join(cmd)} {'<FILES>' if files else ''}` in {time.time() - start:.2f}s")
        if proc.stdout:
            logging.info(f"stdout: {proc.stdout}")

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

    logging.debug("Checking:")
    for f in files:
        logging.debug(f"  {f}")

    jobs = [
        (["pixi", "run", "codegen", "--check"], True),
        (["rustfmt", "--check"], filter_ext(files, [".rs"])),
        (["python", "scripts/lint.py"], files),
        (["typos"], files),
        (["taplo", "fmt", "--check"], filter_ext(files, [".toml"])),
        (["pixi", "run", "clang-format", "--dry-run"], filter_ext(files, [".cpp", ".c", ".h", ".hpp"])),
        (["ruff", "check", "--config", "rerun_py/pyproject.toml"], filter_ext(files, [".py"])),
        (["black", "--check", "--config", "rerun_py/pyproject.toml"], filter_ext(files, [".py"])),
        (["blackdoc", "--check"], filter_ext(files, [".py"])),
        (["mypy", "--no-warn-unused-ignore"], filter_ext(files, [".py"])),
    ]

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.num_threads) as executor:
        results = [executor.submit(run_cmd, command, files) for command, files in jobs]

    success = all(result.result() for result in results)

    if success:
        logging.info(f"All lints passed in {time.time() - start:.2f}s")
        sys.exit(0)
    else:
        logging.info(f"Lints failed in {time.time() - start:.2f}s")
        sys.exit(1)


if __name__ == "__main__":
    main()
