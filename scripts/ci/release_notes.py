#!/usr/bin/env python3

"""
Print the body for a GitHub release draft: the install header followed by the version's section of `CHANGELOG.md`.

Usage:
    python3 scripts/ci/release_notes.py --tag 0.39.0-rc.1 --changelog-version 0.39.0 --changelog CHANGELOG.md
    python3 scripts/ci/release_notes.py --tag 0.39.0 --changelog-version 0.39.0 --without-crates
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

RUST_INSTALL = "`cargo add rerun` and `cargo install rerun-cli --locked`"
RUST_NOT_PUBLISHED = (
    "this version is not published on crates.io. "
    "The `rerun` CLI is available via `pip install rerun-sdk` or the binaries attached to this release."
)

HEADER = """\
[Rerun](https://www.rerun.io/) is an easy-to-use database and visualization toolbox for multimodal and temporal data.

- Python: `pip install -U rerun-sdk`
- Rust: {rust}
- C++ FetchContent: https://github.com/rerun-io/rerun/releases/download/{tag}/rerun_cpp_sdk.zip
- Online demo: https://rerun.io/viewer

---
"""


def changelog_section(changelog: str, version: str) -> str | None:
    """The body of the `## [<version>](…)` section, without its heading."""
    m = re.search(
        rf"^## \[{re.escape(version)}\][^\n]*\n(.*?)(?=^## |\Z)",
        changelog,
        flags=re.MULTILINE | re.DOTALL,
    )
    return None if m is None else m.group(1).strip()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--tag", required=True, help="The release tag, e.g. 0.39.0-rc.1")
    parser.add_argument("--changelog-version", required=True, help="The CHANGELOG.md section to use, e.g. 0.39.0")
    parser.add_argument("--changelog", type=Path, default=Path("CHANGELOG.md"))
    parser.add_argument("--without-crates", action="store_true", help="The release is not published to crates.io")
    args = parser.parse_args()

    section = changelog_section(args.changelog.read_text(encoding="utf-8"), args.changelog_version)
    if section is None:
        print(f"{args.changelog} has no section for {args.changelog_version}", file=sys.stderr)
        return 1

    print(HEADER.format(tag=args.tag, rust=RUST_NOT_PUBLISHED if args.without_crates else RUST_INSTALL))
    print(section)
    return 0


if __name__ == "__main__":
    sys.exit(main())
