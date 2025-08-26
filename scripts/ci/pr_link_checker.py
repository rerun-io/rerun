#!/usr/bin/env python3
"""
Check links only in lines added by a PR.

This script extracts lines added in a PR and runs lychee on them to avoid
checking links in the entire codebase on every PR.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def get_added_lines_with_links(base_ref: str = "origin/main") -> dict[str, list[str]]:
    """
    Get lines added in the current branch that contain URLs.

    Returns a dict mapping file extensions to lists of lines containing links.
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
        print(f"Error getting git diff: {e}", file=sys.stderr)
        return {}

    lines_by_ext: dict[str, list[str]] = {}
    current_file: str | None = None
    current_ext: str | None = None

    for line in result.stdout.split("\n"):
        if line.startswith("+++"):
            # Extract filename from +++ b/path/to/file
            if line.startswith("+++ b/"):
                current_file = line[6:]  # Remove '+++ b/'
                current_ext = Path(current_file).suffix.lstrip(".")
                if current_ext not in lines_by_ext:
                    lines_by_ext[current_ext] = []
            else:
                current_file = None
                current_ext = None
        elif line.startswith("+") and not line.startswith("+++") and current_ext:
            # This is an added line, check if it contains URLs
            line_content = line[1:]  # Remove the '+' prefix
            if (
                "http://" in line_content
                or "https://" in line_content
                or "ftp://" in line_content
                or "file://" in line_content
            ):
                lines_by_ext[current_ext].append(line_content)

    # Remove empty entries
    return {ext: lines for ext, lines in lines_by_ext.items() if lines}


def create_temp_files(lines_by_ext: dict[str, list[str]]) -> list[str]:
    """
    Create temporary files with the lines that contain links.

    Returns a list of temporary file paths.
    """
    temp_files = []

    for ext, lines in lines_by_ext.items():
        if not lines:
            continue

        # Create temp file with appropriate extension
        fd, temp_path = tempfile.mkstemp(suffix=f".{ext}", prefix="pr_links_")

        try:
            with os.fdopen(fd, "w") as f:
                for line in lines:
                    f.write(line + "\n")
            temp_files.append(temp_path)
        except Exception:
            os.unlink(temp_path)
            raise

    return temp_files


def run_lychee(temp_files: list[str]) -> int:
    """
    Run lychee on the temporary files.

    Returns the exit code from lychee.
    """
    if not temp_files:
        print("No files with links found in added lines.")
        return 0

    # Build lychee command
    cmd = [
        "lychee",
        "--verbose",
        "--cache",
        "--max-cache-age",
        "1d",
        "--base-url",
        ".",
    ]

    # Add all temp files
    cmd.extend(temp_files)

    print(f"Running lychee on {len(temp_files)} temporary files containing added lines with links…")

    try:
        result = subprocess.run(cmd, check=False)
        return result.returncode
    except FileNotFoundError:
        print("Error: lychee not found. Please install lychee.", file=sys.stderr)
        return 1


def cleanup_temp_files(temp_files: list[str]) -> None:
    """Clean up temporary files."""
    for temp_file in temp_files:
        try:
            os.unlink(temp_file)
        except OSError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description="Check links in PR-added lines only")
    parser.add_argument(
        "--base-ref", default="origin/main", help="Base reference to compare against (default: origin/main)"
    )
    parser.add_argument("--dry-run", action="store_true", help="Show what would be checked without running lychee")

    args = parser.parse_args()

    # Get lines with links from the diff
    lines_by_ext = get_added_lines_with_links(args.base_ref)

    if not lines_by_ext:
        print("No added lines with links found.")
        return 0

    if args.dry_run:
        print("Would check the following lines:")
        for ext, lines in lines_by_ext.items():
            print(f"\n{ext} files:")
            for line in lines:
                print(f"  {line}")
        return 0

    # Create temporary files
    temp_files = create_temp_files(lines_by_ext)

    try:
        # Run lychee
        exit_code = run_lychee(temp_files)
        return exit_code
    finally:
        # Clean up
        cleanup_temp_files(temp_files)


if __name__ == "__main__":
    sys.exit(main())
