#!/usr/bin/env python3
"""
Check that deleted/renamed doc files have corresponding redirects in _redirects.yaml.

This script compares the current branch against the base branch (default: main)
and ensures that any deleted or renamed markdown files in docs/content/ have
corresponding entries in docs/content/_redirects.yaml.
"""

from __future__ import annotations

import sys
from pathlib import Path

import git
import yaml
from doc_anchors import anchors_in

DOCS_ROOT = Path(__file__).parent.parent.parent / "docs" / "content"
REDIRECTS_FILE = DOCS_ROOT / "_redirects.yaml"


def get_deleted_and_renamed_docs(base_branch: str = "main") -> tuple[list[str], list[str]]:
    """Get lists of deleted and renamed doc paths relative to docs/content/."""
    repo = git.Repo(search_parent_directories=True)
    diff_output = repo.git.diff(base_branch, "--name-status", "--", "docs/content/**/*.md")

    deleted = []
    renamed = []

    for line in diff_output.strip().split("\n"):
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


def check_destination(destination: str) -> str | None:
    """Check that a redirect destination exists, and return why it does not if it doesn't."""
    # External URLs are assumed valid
    if destination.startswith(("http://", "https://")):
        return None

    base_path, _, anchor = destination.partition("#")
    if not base_path:
        return None  # Same-page anchor

    # Check if the destination file exists
    dest_file = DOCS_ROOT / f"{base_path}.md"
    dest_dir_index = DOCS_ROOT / base_path / "index.md"

    page = None
    if dest_file.exists():
        page = dest_file
    elif dest_dir_index.exists():
        page = dest_dir_index
    elif not (DOCS_ROOT / base_path).is_dir():
        return "destination does not exist"

    # An anchor that names no heading silently drops the reader at the top of the page,
    # which is the whole failure this redirect exists to avoid.
    if anchor and page is not None and anchor not in anchors_in(page):
        return f"no heading in {base_path} matches the anchor '#{anchor}'"

    return None


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
        reason = check_destination(destination)
        if reason is not None:
            errors.append(f"Broken redirect: {source} -> {destination} ({reason})")

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
