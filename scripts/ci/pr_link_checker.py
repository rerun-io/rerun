#!/usr/bin/env python3
"""
Check links only in lines added by a PR.

This script extracts lines added in a PR and runs lychee on them to avoid
checking links in the entire codebase on every PR.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


def eprint(*args: Any, **kwargs: Any) -> None:
    """Prints a message to stderr."""
    print(*args, file=sys.stderr, **kwargs)


def get_added_lines_with_links(base_ref: str = "origin/main") -> dict[str, list[str]]:
    """
    Get lines added in the current branch that contain URLs.

    Returns a dict mapping filenames to lists of lines containing links.
    """
    # Get the diff of added lines (try committed changes first, then staged changes)
    # Disable external diff tools to get standard git diff format
    env = os.environ.copy()
    env["GIT_EXTERNAL_DIFF"] = ""

    try:
        result = subprocess.run(
            ["git", "diff", "--no-ext-diff", "--no-merges", "--diff-filter=AM", f"{base_ref}...HEAD"],  # NOLINT
            capture_output=True,
            text=True,
            check=True,
            env=env,
        )
        # If no committed changes, try staged changes
        if not result.stdout.strip():
            result = subprocess.run(
                ["git", "diff", "--no-ext-diff", "--cached", "--no-merges", "--diff-filter=AM"],
                capture_output=True,
                text=True,
                check=True,
                env=env,
            )
    except subprocess.CalledProcessError as e:
        eprint(f"Error getting git diff: {e}")
        return {}

    lines_by_file: dict[str, list[str]] = {}
    current_file: str | None = None

    for line in result.stdout.split("\n"):
        if line.startswith("+++"):
            # Extract filename from +++ b/path/to/file
            if line.startswith("+++ b/"):
                current_file = line[6:]  # Remove '+++ b/'
                if current_file not in lines_by_file:
                    lines_by_file[current_file] = []
            else:
                current_file = None
        elif line.startswith("+") and not line.startswith("+++") and current_file:
            # This is an added line, check if it contains URLs
            line_content = line[1:]  # Remove the '+' prefix
            if (
                "http://" in line_content
                or "https://" in line_content
                or "ftp://" in line_content
                or "file://" in line_content
                or re.search(r"\[.+\]\(.+\)", line_content)  # Markdown links [text](url)
            ):
                lines_by_file[current_file].append(line_content)

    # Remove empty entries
    return {filename: lines for filename, lines in lines_by_file.items() if lines}


class TempLinkFile:
    def __init__(self, path: str, source_file: str) -> None:
        self.path = path
        self.source_file = source_file


def create_temp_files(lines_by_file: dict[str, list[str]]) -> list[TempLinkFile]:
    """
    Create temporary files with the lines that contain links.

    Returns a list of temporary file paths.
    """
    temp_files = []

    for file, lines in lines_by_file.items():
        if not lines:
            continue

        # Create temp file with appropriate extension
        ext = Path(file).suffix
        fd, temp_path = tempfile.mkstemp(suffix=f"{ext}", prefix="pr_links_")

        try:
            with os.fdopen(fd, "w") as f:
                for line in lines:
                    f.write(line + "\n")

            # TODO(lycheeverse/lychee#972): Windows absolute paths don't work.
            # But looks like UNC paths work!
            if sys.platform == "win32":
                temp_path = "\\\\.\\" + temp_path

            temp_files.append(TempLinkFile(temp_path, file))
        except Exception:
            os.unlink(temp_path)
            raise

    return temp_files


def run_lychee(temp_files: list[TempLinkFile]) -> int:
    """
    Run lychee on the temporary files.

    Returns the exit code from lychee.
    """
    if not temp_files:
        eprint("No files with links found in added lines.")
        return 0

    failed = False

    # Since each temp file may contain relative links, we have to run lychee once per file
    # and set the right base url for each.
    for temp_file in temp_files:
        # Build lychee command
        cmd = [
            "lychee",
            "--verbose",
            "--cache",
            "--max-cache-age",
            "1d",
            "--base-url",
            "file:" + str(Path(temp_file.source_file).parent.resolve()) + "/",
            temp_file.path,
        ]

        eprint(f"Running lychee on new links in {temp_file.source_file}: {' '.join(cmd)}")

        try:
            result = subprocess.run(cmd, check=False)
            if result.returncode != 0:
                failed = True
            eprint()
        except FileNotFoundError:
            eprint("Error: lychee not found. Please install lychee.")
            return 1

    return 1 if failed else 0


def cleanup_temp_files(temp_files: list[TempLinkFile]) -> None:
    """Clean up temporary files."""
    for temp_file in temp_files:
        try:
            os.unlink(temp_file.path)
        except OSError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description="Check links in PR-added lines only")
    parser.add_argument(
        "--base-ref", default="origin/main", help="Base reference to compare against (default: origin/main)"
    )
    parser.add_argument("--dry-run", action="store_true", help="Show what would be checked without running lychee")
    parser.add_argument("--no-cleanup", action="store_true", help="Don't clean up temporary files")

    args = parser.parse_args()

    # Get lines with links from the diff
    lines_by_file = get_added_lines_with_links(args.base_ref)

    if not lines_by_file:
        eprint("No added lines with links found.")
        return 0

    if args.dry_run:
        eprint("Would check the following lines:")
        for file, lines in lines_by_file.items():
            eprint(f"\n{file}:")
            for line in lines:
                eprint(f"  {line}")
        return 0

    # Create temporary files
    temp_files = create_temp_files(lines_by_file)

    try:
        # Run lychee
        exit_code = run_lychee(temp_files)
        return exit_code
    finally:
        # Clean up
        if not args.no_cleanup:
            cleanup_temp_files(temp_files)


if __name__ == "__main__":
    sys.exit(main())
