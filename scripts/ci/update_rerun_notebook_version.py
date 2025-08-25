#!/usr/bin/env python3

"""
Update the version of the `rerun_notebook`.

This includes:
- the `rerun_notebook` package itself
- the dependency in the `rerun_py/pyproject.toml` file.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Any

import semver
import tomlkit
import tomlkit.container


def run(
    cmd: str,
    *,
    cwd: str | None = None,
    env: dict[str, str] | None = None,
) -> None:
    print(f"{cwd or ''}> {cmd}")
    subprocess.check_output(cmd.split(), cwd=cwd, env=env)


def set_rerun_notebook_version(pyproject_path: Path, version: str) -> None:
    pyproject: dict[str, Any] = tomlkit.parse(pyproject_path.read_text(encoding="utf-8"))
    pyproject["project"]["version"] = version
    pyproject_path.write_text(tomlkit.dumps(pyproject), encoding="utf-8")


def set_dependency_version(pyproject_path: Path, version: str) -> None:
    pyproject: dict[str, Any] = tomlkit.parse(pyproject_path.read_text(encoding="utf-8"))

    notebook_deps = pyproject["project"]["optional-dependencies"]["notebook"]
    new_deps = [dep for dep in notebook_deps if not dep.startswith("rerun-notebook")]
    new_deps.append(f"rerun-notebook=={version}")
    pyproject["project"]["optional-dependencies"]["notebook"] = sorted(new_deps)

    pyproject_path.write_text(tomlkit.dumps(pyproject), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description="Update rerun notebook dependency version")
    parser.add_argument("VERSION", help="Version to use")
    args = parser.parse_args()

    # check that the version is valid
    try:
        semver.VersionInfo.parse(args.VERSION)
    except ValueError:
        print(f"Invalid semver version: '{args.VERSION}'", file=sys.stderr, flush=True)
        sys.exit(1)

    project_path = Path(__file__).parent.parent.parent.absolute()

    # update the version in rerun_notebook
    set_rerun_notebook_version(project_path / "rerun_notebook" / "pyproject.toml", args.VERSION)

    # update the dependency in rerun_py/pyproject.toml
    pyproject_path = project_path / "rerun_py" / "pyproject.toml"
    set_dependency_version(pyproject_path, args.VERSION)


if __name__ == "__main__":
    main()
