#!/usr/bin/env python3

"""Runs the common linters on any files that have changed relative to the main branch."""

from __future__ import annotations

import argparse
import concurrent.futures
import logging
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field

from git import Repo


def changed_files() -> list[str]:
    try:
        repo = Repo(os.getcwd())
    except Exception as e:
        logging.error(f"Failed to open git repo at {os.getcwd()}: {e}")
        repo = Repo(os.path.dirname(os.getcwd()))

    current_branch = repo.active_branch
    common_ancestor = repo.merge_base(current_branch, "main")[0]

    changed_files = [
        item.b_path
        for item in repo.index.diff(common_ancestor)
        if item.b_path is not None and os.path.exists(item.b_path)
    ]
    return changed_files


@dataclass
class LintJob:
    command: str | list[str]
    extensions: list[str] | None = None
    accepts_files: bool = True
    no_filter_args: list[str] = field(default_factory=list)
    no_filter_cmd: str | None = None
    allow_no_filter: bool = True
    filter_files: str | None = None
    _commands: list[str] = field(init=False, repr=False)

    def __post_init__(self) -> None:
        if isinstance(self.command, str):
            self._commands = self.command.split()
        else:
            self._commands = self.command

    def run_cmd(self, files: list[str], skip_list: list[str], no_change_filter: bool) -> bool:
        start = time.time()

        cmd = self._commands

        if self.extensions is not None:
            files = [f for f in files if any(f.endswith(e) for e in self.extensions)]

        if self.command in skip_list:
            logging.info(f"SKIP: {self.command} (skipped manually)")
            return True
        if self.accepts_files and not no_change_filter and not files:
            logging.info(f"SKIP: {self.command} (no modified files)")
            return True

        if not self.accepts_files:
            files = []

        if len(files) == 0:
            if not self.allow_no_filter:
                logging.info(f"SKIP: {self.command} (no-change-filter not supported)")
                return True
            files = self.no_filter_args
            if self.no_filter_cmd is not None:
                cmd = self.no_filter_cmd.split()
        # Apply regex filtering if filter_files is specified
        elif self.filter_files is not None:
            pattern = re.compile(self.filter_files)
            files = [f for f in files if not pattern.match(f)]

        cmd_arr = ["pixi", "run", *cmd]

        cmd_preview = subprocess.list2cmdline([*cmd_arr, "<FILES>"]) if files else subprocess.list2cmdline(cmd_arr)

        full_cmd = cmd_arr + files
        proc = subprocess.run(full_cmd, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        if proc.returncode == 0:
            logging.info(f"PASS: {cmd} in {time.time() - start:.2f}s")
            logging.debug(f"----------\n{cmd_preview}\n{proc.stdout}\n----------")
        else:
            logging.info(
                f"FAIL: {cmd} in {time.time() - start:.2f}s \n----------\n{subprocess.list2cmdline(full_cmd)}\n{proc.stdout}\n----------",
            )

        return proc.returncode == 0


PY_FOLDERS = ["."]


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
    parser.add_argument(
        "--num-threads",
        type=int,
        default=8,
        help="Number of threads to use (default: 8).",
    )
    parser.add_argument(
        "--skip",
        type=str,
        default=os.environ.get("RERUN_LINT_SKIP", ""),
        help="Comma-separated list of tasks to skip.",
    )
    parser.add_argument(
        "--no-change-filter",
        action="store_true",
        help="Run lints without filtering based on changes.",
    )
    parser.add_argument(
        "files",
        metavar="file",
        type=str,
        nargs="*",
        help="File paths. Empty = all files, recursively.",
    )

    args = parser.parse_args()
    logging.basicConfig(level=args.log_level, format="%(name)s(%(levelname)s): %(message)s")
    root_logger = logging.getLogger()
    root_logger.name = "fast-lint"

    script_dirpath = os.path.dirname(os.path.realpath(__file__))
    root_dirpath = os.path.abspath(f"{script_dirpath}/..")
    os.chdir(root_dirpath)

    if args.files:
        files = args.files
    elif args.no_change_filter:
        files = []
    else:
        files = changed_files()

    skip = [s for s in args.skip.split(",") if s != ""]

    logging.debug("Checking:")
    for f in files:
        logging.debug(f"  {f}")

    jobs = [
        LintJob("lint-codegen", accepts_files=False),
        # TODO(jleibs): implement lint-cpp-files
        # LintJob(
        #     "lint-cpp-files",
        #     extensions=[".cpp", ".c", ".h", ".hpp"],
        #     allow_no_filter=False,
        # ),
        LintJob("lint-rerun"),
        LintJob(
            "lint-rs-files",
            extensions=[".rs"],
            no_filter_cmd="lint-rs-all",
            filter_files=r"^crates/store/re_sdk_types(_core)?/",
        ),
        LintJob("py-fmt-check", extensions=[".py"], no_filter_args=PY_FOLDERS),
        # Even though mypy will accept a list of files, the results it generates are inconsistent
        # with running on the full project.
        LintJob("py-lint", extensions=[".py"], accepts_files=False),
        LintJob("toml-fmt-check", extensions=[".toml"]),
        LintJob("lint-typos --force-exclude"),
        LintJob("check-large-files"),
        LintJob("nb-strip-check", accepts_files=False),
    ]

    for command in skip:
        if command not in [j.command for j in jobs]:
            logging.error(f"Unknown command '{command}' in 'skip', expected one of {[j.command for j in jobs]}")
            sys.exit(1)

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.num_threads) as executor:
        results = [executor.submit(job.run_cmd, files, skip, args.no_change_filter) for job in jobs]

    success = all(result.result() for result in results)

    if success:
        logging.info(f"All lints passed in {time.time() - start:.2f}s")
        sys.exit(0)
    else:
        logging.info(f"Lints failed in {time.time() - start:.2f}s")
        sys.exit(1)


if __name__ == "__main__":
    main()
