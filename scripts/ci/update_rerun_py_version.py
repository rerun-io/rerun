#!/usr/bin/env python3

"""
Updates the version information in rerun_sdk.

This includes:
- `rerun.__version__`
- `rerun.__version_info__`
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import semver


def update(line, version_line, version_info_line):
    if line.startswith("__version__"):
        if line != version_line:
            return version_line
    if line.startswith("__version_info__"):
        if line != version_info_line:
            return version_info_line
    return line


def set_rerun_py_version(init_path: Path, version: str) -> None:
    sem_version = semver.VersionInfo.parse(version)

    version_line = f'__version__ = "{version}"\n'
    version_info_line = f'__version_info__ = ({sem_version.major}, {sem_version.minor}, {sem_version.patch}, "{sem_version.prerelease}")\n'

    with init_path.open() as f:
        lines = f.readlines()

    new_lines = [update(line, version_line, version_info_line) for line in lines]

    if new_lines != lines:
        with init_path.open("w") as f:
            f.writelines(new_lines)
    else:
        print(f"Version already set to {version} in {init_path}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Update rerun_py __version__ variable.")
    parser.add_argument("VERSION", help="Version to use")
    args = parser.parse_args()

    # check that the version is valid
    try:
        semver.VersionInfo.parse(args.VERSION)
    except ValueError:
        print(f"Invalid semver version: {args.VERSION}", file=sys.stderr, flush=True)
        sys.exit(1)

    project_path = Path(__file__).parent.parent.parent.absolute()

    set_rerun_py_version(project_path / "rerun_py" / "rerun_sdk" / "rerun" / "__init__.py", args.VERSION)


if __name__ == "__main__":
    main()
