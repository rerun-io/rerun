#!/usr/bin/env python3

"""Lints as quickly as possible."""
from __future__ import annotations

import argparse
import logging
import os
import subprocess

from git import Repo


def changed_files():
    repo = Repo(os.getcwd())

    current_branch = repo.active_branch
    common_ancestor = repo.merge_base(current_branch, "main")[0]

    return [item.b_path for item in repo.index.diff(common_ancestor)]


def rerun_lints(files):
    cmd = ["python", "scripts/lint.py"]
    logging.debug(f"Running: `{' '.join(cmd)} <FILES>`")
    proc = subprocess.run(cmd + files)
    logging.debug(f"Returned: {proc.returncode}")
    return proc.returncode


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--log-level",
        dest="log_level",
        default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"],
        help="Set the logging level (default: INFO)",
    )
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

    rerun_lints(files)


if __name__ == "__main__":
    main()
