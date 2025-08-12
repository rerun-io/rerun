#!/usr/bin/env python3
"""Check if CLI documentation is up-to-date."""

from __future__ import annotations

import argparse
import difflib
import subprocess
import sys
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description="Check if CLI documentation is up-to-date")
    parser.add_argument(
        "command",
        nargs="*",
        default=["pixi", "run", "rerun", "man"],
        help="Command to run (default: pixi run rerun man)",
    )

    args = parser.parse_args()

    # Handle command as list
    command: list[str] = args.command if isinstance(args.command, list) else [args.command]
    if not command or command == []:
        command = ["pixi", "run", "rerun", "man"]

    expected_file: Path = Path("docs/content/reference/cli.md")

    # Generate current output
    try:
        result = subprocess.run(command, capture_output=True, text=True, check=True)
        current: str = result.stdout
    except subprocess.CalledProcessError as e:
        print(f"Error running command: {e}", file=sys.stderr)
        sys.exit(2)
    except FileNotFoundError:
        print(f"Command not found: {' '.join(command)}", file=sys.stderr)
        sys.exit(2)

    # Read expected
    try:
        expected: str = expected_file.read_text()
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

    print(f"\nUpdate with: {' '.join(command)} > {expected_file}", file=sys.stderr)
    sys.exit(1)


if __name__ == "__main__":
    main()
