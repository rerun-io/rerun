#!/bin/env python3

"""
Script should only be called by the CI system.

This script accepts one argument:
    --patch_prerelease: This will patch the version in rerun/Cargo.toml with the current git sha. This is intended to
    create a prerelease version for continuous releases.

    --check_version: This will check that the version in rerun/Cargo.toml matches the version in the tag name from `GITHUB_REF_NAME`. This is
    intended to be used to check that the version number in Cargo.toml is correct before creating a release on PyPI. If
    the versions don't match, an exception will be raised.
"""

import os
import re
import subprocess
import sys
from typing import Final

CARGO_VERSION_REGEX: Final = r"^version\s*=\s*\"(?P<version>([0-9]+)\.([0-9]+)\.([0-9]+))\"$"
VERSION_TAG_REGEX: Final = r"^v(?P<version>([0-9]+)\.([0-9]+)\.([0-9]+))$"


def get_cargo_version(cargo_toml: str) -> str:
    """Using regex, parse the version number from Cargo.toml."""

    match = re.search(CARGO_VERSION_REGEX, cargo_toml, re.MULTILINE)

    if match is None:
        raise Exception("Could not find valid base version number in Cargo.toml")

    return match.group("version")


def get_git_sha() -> str:
    """Return the git short sha of the current commit."""
    return subprocess.check_output(["git", "rev-parse", "--short", "HEAD"]).decode("utf-8").strip()


def get_ref_name_version() -> str:
    """Return the parsed tag version from the GITHUB_REF_NAME environment variable."""

    # This is the branch, or tag name that triggered the workflow.
    ref_name = os.environ.get("GITHUB_REF_NAME")

    if ref_name is None:
        raise Exception("GITHUB_REF_NAME environment variable not set")

    # Extract the version number from the tag name
    match = re.search(VERSION_TAG_REGEX, ref_name)

    if match is None:
        raise Exception("Could not find valid version number in GITHUB_REF_NAME")

    return match.group("version")


def patch_cargo_version(cargo_toml: str, new_version: str) -> str:
    """Patch the version number in Cargo.toml with `new_version`."""

    print(f"Patching version number in Cargo.toml to: {new_version}")

    (cargo_toml, num_subs) = re.subn(
        CARGO_VERSION_REGEX,
        f'version = "{new_version}"',
        cargo_toml,
        count=1,
        flags=re.MULTILINE,
    )

    if num_subs != 1:
        raise Exception("Could not patch version number in Cargo.toml")

    return cargo_toml


def main() -> None:
    if len(sys.argv) != 2:
        raise Exception("Invalid number of arguments")

    with open("Cargo.toml", "r") as f:
        cargo_toml = f.read()

    cargo_version = get_cargo_version(cargo_toml)

    if sys.argv[1] == "--patch_prerelease":
        git_sha = get_git_sha()
        new_version = f"{cargo_version}+{git_sha}"
        new_cargo_toml = patch_cargo_version(cargo_toml, new_version)

        # Write the patched Cargo.toml back to disk
        with open("Cargo.toml", "w") as f:
            f.write(new_cargo_toml)

    elif sys.argv[1] == "--check_version":
        ref_version = get_ref_name_version()
        if cargo_version != ref_version:
            raise Exception(
                f"Version number in Cargo.toml ({cargo_version}) does not match tag version ({ref_version})"
            )
        print(f"Version numbers match: {cargo_version} == {ref_version}")

    else:
        raise Exception("Invalid argument")


if __name__ == "__main__":
    main()
