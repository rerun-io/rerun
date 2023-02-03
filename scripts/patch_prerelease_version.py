#!/bin/env python3

# This script should only be called by the CI system before building Python wheels.
# This script is used to patch the version number in the Cargo.toml. The git sha is also appended to the version number, and written to the "version" field back into the Cargo.toml.

import re
import subprocess

version_regex = r"^version\s*=\s*\"(?P<version>([0-9]+)\.([0-9]+)\.([0-9]+))\"$"

# Using regex, parse the version number from Cargo.toml
with open("Cargo.toml", "r") as f:
    cargo_toml = f.read()

match = re.search(version_regex, cargo_toml, re.MULTILINE)

if match is None:
    raise Exception("Could not find valid base version number in Cargo.toml")

cargo_version = match.group("version")

# Get the git short sha
git_sha = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"]).decode("utf-8").strip()

# Append the git sha to the version number
pre_version = f"{cargo_version}+{git_sha}"

print(f"Patching version number in Cargo.toml to: {pre_version}")

# Patch the version number in Cargo.toml
(cargo_toml, num_subs) = re.subn(version_regex, f'version = "{pre_version}"', cargo_toml, count=1, flags=re.MULTILINE)

if num_subs != 1:
    raise Exception("Could not patch version number in Cargo.toml")

# Write the patched Cargo.toml back to disk
with open("Cargo.toml", "w") as f:
    f.write(cargo_toml)
