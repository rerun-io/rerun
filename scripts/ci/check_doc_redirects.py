#!/usr/bin/env python3
"""
Check that deleted/renamed doc files have corresponding redirects in _redirects.yaml.

This script compares the current branch against the base branch (default: main)
and ensures that any deleted or renamed markdown files in docs/content/ have
corresponding entries in docs/content/_redirects.yaml.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import yaml

DOCS_ROOT = Path(__file__).parent.parent.parent / "docs" / "content"
REDIRECTS_FILE = DOCS_ROOT / "_redirects.yaml"


def get_deleted_and_renamed_docs(base_branch: str = "main") -> tuple[list[str], list[str]]:
    """Get lists of deleted and renamed doc paths relative to docs/content/."""
    result = subprocess.run(
        ["git", "diff", base_branch, "--name-status", "--", "docs/content/**/*.md"],
        capture_output=True,
        text=True,
        check=True,
    )

    deleted = []
    renamed = []

    for line in result.stdout.strip().split("\n"):
        if not line:
            continue
        parts = line.split("\t")
        status = parts[0]

        if status == "D":
            # Deleted file
            path = parts[1]
            doc_path = path.removeprefix("docs/content/").removesuffix(".md")
            deleted.append(doc_path)
        elif status.startswith("R"):
            # Renamed file (R followed by similarity percentage)
            old_path = parts[1]
            doc_path = old_path.removeprefix("docs/content/").removesuffix(".md")
            renamed.append(doc_path)

    return deleted, renamed


def load_redirects() -> dict[str, str]:
    """Load redirects from _redirects.yaml as source -> destination mapping."""
    if not REDIRECTS_FILE.exists():
        return {}

    with open(REDIRECTS_FILE) as f:
        redirects = yaml.safe_load(f)

    return redirects or {}


def check_destination_exists(destination: str) -> bool:
    """Check if the redirect destination exists (internal paths only)."""
    # External URLs are assumed valid
    if destination.startswith("http://") or destination.startswith("https://"):
        return True

    # Handle anchor links
    base_path = destination.split("#")[0]
    if not base_path:
        return True  # Same-page anchor

    # Check if the destination file exists
    dest_file = DOCS_ROOT / f"{base_path}.md"
    dest_dir_index = DOCS_ROOT / base_path / "index.md"

    return dest_file.exists() or dest_dir_index.exists() or (DOCS_ROOT / base_path).is_dir()


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--base",
        default="main",
        help="Base branch to compare against (default: main)",
    )
    args = parser.parse_args()

    deleted, renamed = get_deleted_and_renamed_docs(args.base)
    redirects = load_redirects()

    errors = []

    # Check that all deleted/renamed files have redirects
    for path in deleted + renamed:
        if path not in redirects:
            errors.append(f"Missing redirect: {path}")

    # Check that destinations exist
    for source, destination in redirects.items():
        if not check_destination_exists(destination):
            errors.append(f"Broken redirect: {source} -> {destination}")

    if errors:
        print("ERROR: Found redirect issues:")
        print()
        for error in sorted(errors):
            print(f"  - {error}")
        print()
        print(f"Fix these issues in {REDIRECTS_FILE.relative_to(Path.cwd())}")
        return 1

    if deleted or renamed:
        print(f"OK: All {len(deleted)} deleted and {len(renamed)} renamed doc files have redirects")
    else:
        print("OK: No deleted or renamed doc files")

    print(f"OK: All {len(redirects)} redirect destinations are valid")

    return 0


if __name__ == "__main__":
    sys.exit(main())
