#!/usr/bin/env python3

"""
Keep the `rerun-sdk` PyPI project below its total size limit.

PyPI rejects every upload once a project reaches its size limit, which blocks a release halfway through.
Old prereleases make up a large share of that size and are safe to delete once a newer stable release exists.

Subcommands:

    check     Read-only. Fails if the next release would likely not fit, warns when headroom is low.
              Runs in the release workflow before anything is published.
    cleanup   Lists prereleases (`aN`, `bN`, `rcN`, `.devN`) older than the latest stable release.
              Dry run by default. With `--delete`, deletes them via the PyPI web UI using the third-party
              `pypi-cleanup` tool (https://github.com/arcivanov/pypi-cleanup).

PyPI has no API for deleting releases: deletion needs an interactive login as a project owner, including 2FA.
That is why deletion is a maintainer-run command and not a CI job.
Only TOTP 2FA is supported by `pypi-cleanup`; owners with only a security key must delete in the web UI.

Usage:
    pixi run pypi-size-check
    pixi run pypi-prune-prereleases                                  # dry run
    pixi run pypi-prune-prereleases --delete --username <pypi-user>  # prompts for the pypi.org password and TOTP code
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass

import requests
from packaging.version import InvalidVersion, Version

DEFAULT_PROJECT = "rerun-sdk"

# Granted per project by PyPI admins (the default is 10 GiB), and not exposed by any PyPI API.
# Update this if PyPI raises it.
PROJECT_SIZE_LIMIT_GIB = 50

# This tool is handed a PyPI owner's password, so it is pinned to an exact version whose source has been reviewed.
# Versions before 0.1.11 report success without deleting anything, since PyPI started requiring delete acknowledgments.
PYPI_CLEANUP_REQUIREMENT = "pypi-cleanup==0.1.11"

GIB = 1024**3


@dataclass(frozen=True)
class Release:
    version_text: str
    version: Version
    size_bytes: int
    upload_time: str


@dataclass(frozen=True)
class Project:
    name: str
    releases: list[Release]
    invalid_versions: list[str]

    @property
    def total_bytes(self) -> int:
        return sum(r.size_bytes for r in self.releases)

    def latest_stable(self) -> Release | None:
        stable = [r for r in self.releases if not r.version.is_prerelease]
        return max(stable, key=lambda r: r.version, default=None)

    def newest_upload(self) -> Release | None:
        return max(self.releases, key=lambda r: r.upload_time, default=None)

    def deletable_prereleases(self) -> list[Release]:
        """Prereleases strictly older than the latest stable release, oldest first."""
        latest_stable = self.latest_stable()
        if latest_stable is None:
            return []
        return sorted(
            (r for r in self.releases if r.version.is_prerelease and r.version < latest_stable.version),
            key=lambda r: r.version,
        )


def fetch_project(name: str) -> Project:
    """
    Read every release of `name` from the PyPI JSON API.

    The per-project endpoint lists files of all releases, including yanked ones, which also count towards the size limit.
    """
    url = f"https://pypi.org/pypi/{name}/json"
    response = requests.get(url, headers={"Cache-Control": "no-cache"}, timeout=60)
    response.raise_for_status()
    data = response.json()

    releases = []
    invalid = []
    for version_text, files in data["releases"].items():
        try:
            version = Version(version_text)
        except InvalidVersion:
            invalid.append(version_text)
            continue
        if not files:
            continue
        releases.append(
            Release(
                version_text=version_text,
                version=version,
                size_bytes=sum(f["size"] for f in files),
                upload_time=min(f["upload_time_iso_8601"] for f in files),
            )
        )
    return Project(name=data["info"]["name"], releases=releases, invalid_versions=invalid)


def gib(num_bytes: int) -> str:
    return f"{num_bytes / GIB:.2f} GiB"


def github_annotation(level: str, message: str) -> None:
    print(f"::{level}::{message}")


def cmd_check(args: argparse.Namespace) -> int:
    project = fetch_project(args.project)
    limit = args.limit_gib * GIB
    headroom = limit - project.total_bytes
    newest = project.newest_upload()
    release_size = newest.size_bytes if newest is not None else 0
    reclaimable = sum(r.size_bytes for r in project.deletable_prereleases())

    print(f"{project.name}: {gib(project.total_bytes)} of {gib(limit)} used, {gib(headroom)} free")
    print(f"Newest release ({newest.version_text if newest else '–'}) is {gib(release_size)}")
    print(f"Old prereleases take up {gib(reclaimable)}")

    fix = f"Run `pixi run pypi-prune-prereleases` in `rerun/` (see RELEASES.md) to delete old prereleases of {project.name}."

    # The release itself plus a margin, since a new release is usually somewhat larger than the last one.
    if headroom < release_size * 1.2:
        github_annotation("error", f"PyPI project {project.name} is too full for another release. {fix}")
        return 1
    if headroom < args.warn_gib * GIB:
        github_annotation("warning", f"PyPI project {project.name} only has {gib(headroom)} left. {fix}")
    return 0


def cmd_cleanup(args: argparse.Namespace) -> int:
    project = fetch_project(args.project)
    latest_stable = project.latest_stable()
    if latest_stable is None:
        print(f"{project.name} has no stable release, so nothing is deleted.")
        return 0

    to_delete = project.deletable_prereleases()
    reclaim = sum(r.size_bytes for r in to_delete)

    print(f"{project.name}: {gib(project.total_bytes)} of {PROJECT_SIZE_LIMIT_GIB} GiB used")
    print(f"Latest stable release: {latest_stable.version_text}")
    if project.invalid_versions:
        print(f"Ignoring versions with an unrecognized format: {', '.join(sorted(project.invalid_versions))}")
    print()

    if not to_delete:
        print("No prereleases older than the latest stable release.")
        return 0

    print(f"{len(to_delete)} prereleases older than {latest_stable.version_text}:")
    for r in to_delete:
        print(f"  {r.version_text:<16} {r.upload_time[:10]}  {gib(r.size_bytes)}")
    print()
    print(f"Deleting them frees {gib(reclaim)}, leaving {gib(project.total_bytes - reclaim)}.")

    if not args.delete:
        print()
        print("Dry run: nothing was deleted. Pass `--delete --username <pypi-user>` to delete them.")
        return 0

    if args.username is None:
        print("error: `--delete` requires `--username`", file=sys.stderr)
        return 2
    uv = shutil.which("uv")
    if uv is None:
        print("error: `uv` not found; run this through `pixi run pypi-prune-prereleases`", file=sys.stderr)
        return 2

    print()
    print("Deleted releases can never be re-uploaded, not even with different files.")
    confirmation = input(f"Type the project name ({project.name}) to delete these {len(to_delete)} releases: ")
    if confirmation.strip() != project.name:
        print("Aborted.")
        return 1

    version_regex = "^(?:" + "|".join(re.escape(r.version_text) for r in to_delete) + ")$"
    print()
    print(
        f"Logging in to pypi.org as `{args.username}` (must be a user account with the Owner role on {project.name})."
    )
    print("`pypi-cleanup` will now ask for:")
    print(f"  1. Password: the pypi.org login password of `{args.username}` (not an API token)")
    print("  2. TOTP code: the 6-digit code from that account's authenticator app")
    print("pypi.org may also email you a link to confirm the login.")
    print()
    returncode = subprocess.call([
        uv,
        "tool",
        "run",
        "--from",
        PYPI_CLEANUP_REQUIREMENT,
        "pypi-cleanup",
        "--package",
        project.name,
        "--username",
        args.username,
        "--version-regex",
        version_regex,
        "--do-it",
        "--yes",
    ])

    remaining = {r.version_text for r in fetch_project(args.project).releases}
    not_deleted = [r.version_text for r in to_delete if r.version_text in remaining]
    if not_deleted:
        print(
            f"Still listed on PyPI (possibly a stale cache, re-run the dry run to verify): {', '.join(not_deleted)}",
            file=sys.stderr,
        )
        return returncode or 1
    return returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    subparsers = parser.add_subparsers(dest="command", required=True)
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--project", default=DEFAULT_PROJECT, help="PyPI project name")

    check = subparsers.add_parser(
        "check", parents=[common], help="fail or warn if the project is close to its size limit"
    )
    check.add_argument("--limit-gib", type=float, default=PROJECT_SIZE_LIMIT_GIB, help="project size limit")
    check.add_argument("--warn-gib", type=float, default=5.0, help="warn when less than this is free")
    check.set_defaults(func=cmd_check)

    cleanup = subparsers.add_parser("cleanup", parents=[common], help="list, and optionally delete, old prereleases")
    cleanup.add_argument("--delete", action="store_true", help="actually delete (default is a dry run)")
    cleanup.add_argument("--username", help="PyPI username of a project owner, required with --delete")
    cleanup.set_defaults(func=cmd_cleanup)

    args = parser.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    sys.exit(main())
