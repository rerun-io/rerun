#!/usr/bin/env python3
"""Check if CLI documentation is up-to-date."""

from __future__ import annotations

import difflib
import subprocess
import sys
from pathlib import Path


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description="Check if CLI documentation is up-to-date.")
    parser.add_argument(
        "--rerun-exe",
        type=str,
        default=None,
        help="Path to rerun executable to use instead of building with cargo.",
    )
    args = parser.parse_args()

    expected_file: Path = Path("docs/content/reference/cli.md")

    if args.rerun_exe:
        command = [args.rerun_exe, "man"]
    else:
        command = ["cargo", "run", "--package", "rerun-cli", "--all-features", "--", "man"]

    # Generate current output
    try:
        print(f"Running command: {' '.join(command)}")
        result = subprocess.run(command, capture_output=True, text=True, check=True)
        current: str = result.stdout
    except subprocess.CalledProcessError as e:
        print(f"Error running command: {e}", file=sys.stderr)
        print(f"Exit code: {e.returncode}", file=sys.stderr)
        print(f"Stdout: {e.stdout}", file=sys.stderr)
        print(f"Stderr: {e.stderr}", file=sys.stderr)
        sys.exit(2)
    except FileNotFoundError:
        print(f"Command not found: {' '.join(command)}", file=sys.stderr)
        sys.exit(2)

    # Read expected
    try:
        expected: str = expected_file.read_text("utf-8")
    except FileNotFoundError:
        print(f"Expected file not found: {expected_file}", file=sys.stderr)
        sys.exit(2)

    # Compare
    if current == expected:
        print("✓ CLI documentation is up-to-date")
        sys.exit(0)

    # Show diff
    print("✗ CLI documentation is outdated", file=sys.stderr)
    print("\nDiff (- expected, + actual):\n", file=sys.stderr)

    # Split into lines for diff
    expected_lines: list[str] = expected.splitlines(keepends=True)
    current_lines: list[str] = current.splitlines(keepends=True)

    diff = difflib.unified_diff(
        expected_lines, current_lines, fromfile=str(expected_file), tofile=" ".join(command), lineterm=""
    )

    diff_output: str = "".join(diff)
    sys.stderr.write(diff_output)

    print("\nUpdate with: pixi run man", file=sys.stderr)
    sys.exit(1)


if __name__ == "__main__":
    main()
