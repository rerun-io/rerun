"""CI helpers for isolated Python example projects.

An "isolated" example is one that has its own uv project (separate pyproject.toml and uv.lock)
because its dependency closure conflicts with the workspace .venv (e.g. LeRobot requiring Python >=3.12).
Each such example opts in by setting `[tool.rerun-example] isolated = true` in its pyproject.toml.

Fails on the first non-zero exit.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

import tomli


def discover_isolated(examples_dir: Path) -> list[Path]:
    found: list[Path] = []
    for pyproject in sorted(examples_dir.glob("*/pyproject.toml")):
        with pyproject.open("rb") as f:
            data = tomli.load(f)
        if data.get("tool", {}).get("rerun-example", {}).get("isolated"):
            found.append(pyproject.parent)
    return found


def _announce(projects: list[Path], repo_root: Path, verb: str) -> None:
    print(f"{verb} {len(projects)} isolated example(s):")
    for project in projects:
        print(f"  - {project.relative_to(repo_root)}")


def cmd_lint(examples_dir: Path, repo_root: Path) -> int:
    projects = discover_isolated(examples_dir)
    if not projects:
        print("No isolated examples found — nothing to lint.", file=sys.stderr)
        return 1
    _announce(projects, repo_root, "Linting")

    for project in projects:
        print(f"\n=== {project.relative_to(repo_root)} ===", flush=True)
        subprocess.run(
            ["uv", "sync", "--frozen", "--no-install-package", "rerun-sdk"],
            cwd=project,
            check=True,
        )
        subprocess.run(
            ["uv", "run", "--no-sync", "ty", "check", "--error", "deprecated"],
            cwd=project,
            check=True,
        )

    return 0


def cmd_check_lock(examples_dir: Path, repo_root: Path) -> int:
    projects = discover_isolated(examples_dir)
    if not projects:
        print("No isolated examples found — nothing to check.", file=sys.stderr)
        return 1
    _announce(projects, repo_root, "Checking lockfile of")

    for project in projects:
        print(f"\n=== {project.relative_to(repo_root)} ===", flush=True)
        # Mirrors the root `uv-lock-check` pixi task: `--dry-run` surfaces
        # what would change (useful diagnostic), `--check` fails the CI step
        # if the lockfile is out of date.
        subprocess.run(["uv", "lock", "--dry-run"], cwd=project, check=True)
        subprocess.run(["uv", "lock", "--check"], cwd=project, check=True)

    return 0


def main() -> int:
    repo_root = Path(__file__).resolve().parents[2]
    examples_dir = repo_root / "examples" / "python"

    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("lint", help="ty-check every isolated example")
    sub.add_parser("check-lock", help="verify every isolated example's uv.lock is up to date")
    args = parser.parse_args()

    if args.command == "lint":
        return cmd_lint(examples_dir, repo_root)
    if args.command == "check-lock":
        return cmd_check_lock(examples_dir, repo_root)
    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    sys.exit(main())
