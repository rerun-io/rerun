#!/usr/bin/env python3

"""
Add the redirects for a release's changelog entries.

Every `docs/content/changelog/upcoming/<slug>.md` is a published page, so deleting it at
release time breaks its URL. Each entry ends up as a section of that release's changeset,
which makes the redirect mechanical — this script writes it, instead of leaving it to CI
to catch the missing ones afterwards.

Run it from the repository root, as part of `/assemble-changelog`, after assembling the
changeset but *before* deleting the `upcoming/` entries:

    pixi run python scripts/ci/generate_changelog_redirects.py --version 0.36
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import yaml
from doc_anchors import anchors_in, slugify_heading

DOCS_ROOT = Path(__file__).parent.parent.parent / "docs" / "content"
REDIRECTS_FILE = DOCS_ROOT / "_redirects.yaml"
CHANGELOG_DIR = DOCS_ROOT / "changelog"
UPCOMING_DIR = CHANGELOG_DIR / "upcoming"

# Highlight entries are folded into the `## Highlights` prose, so they keep no heading of
# their own. Point them at that section instead.
HIGHLIGHTS_ANCHOR = "highlights"


def load_frontmatter(path: Path) -> dict[str, str]:
    """Load a docs page's YAML frontmatter."""

    text = path.read_text(encoding="utf-8")
    if not text.startswith("---"):
        return {}

    end = text.find("\n---", 3)
    if end == -1:
        return {}

    return yaml.safe_load(text[3:end]) or {}


def upcoming_entries() -> list[Path]:
    """The per-PR entries waiting to be released, oldest path first."""

    return sorted(path for path in UPCOMING_DIR.glob("*.md") if path.name != "_template.md")


def destination_for(entry: Path, changeset_path: str, changeset_anchors: set[str]) -> tuple[str, str | None]:
    """Redirect destination for one entry, plus a warning if we could not pin it to a section."""

    frontmatter = load_frontmatter(entry)
    title = frontmatter.get("title", "")
    anchor = slugify_heading(title)

    if anchor in changeset_anchors:
        return f"{changeset_path}#{anchor}", None

    if frontmatter.get("type") == "highlight" and HIGHLIGHTS_ANCHOR in changeset_anchors:
        return f"{changeset_path}#{HIGHLIGHTS_ANCHOR}", None

    warning = (
        f"{entry.name}: no section in the changeset matches the title {title!r}. "
        f"Redirecting to the top of the changeset — point it at the right section by hand "
        f"if the entry was renamed."
    )
    return changeset_path, warning


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="Release version, e.g. `0.36` or `0.36.0`")
    parser.add_argument("--dry-run", action="store_true", help="Print the redirects instead of writing them")
    args = parser.parse_args()

    major_minor = ".".join(args.version.split(".")[:2])
    changeset_file = CHANGELOG_DIR / f"changeset-{major_minor.replace('.', '-')}.md"
    changeset_path = f"changelog/changeset-{major_minor.replace('.', '-')}"

    if not changeset_file.exists():
        print(f"ERROR: assemble the changeset first, it does not exist yet.\nFile path: {changeset_file}")
        return 1

    entries = upcoming_entries()
    if not entries:
        print("No entries in `upcoming/`; nothing to redirect.")
        return 0

    changeset_anchors = anchors_in(changeset_file)
    existing = yaml.safe_load(REDIRECTS_FILE.read_text(encoding="utf-8")) or {}

    new_redirects: dict[str, str] = {}
    warnings: list[str] = []
    for entry in entries:
        source = f"changelog/upcoming/{entry.stem}"
        if source in existing:
            continue  # Already redirected by an earlier run

        destination, warning = destination_for(entry, changeset_path, changeset_anchors)
        new_redirects[source] = destination
        if warning:
            warnings.append(warning)

    for warning in warnings:
        print(f"WARNING: {warning}")

    if not new_redirects:
        print("Every entry already has a redirect; nothing to do.")
        return 0

    lines = [f"\n# Changelog entries merged into the {major_minor} changeset"]
    lines += [f"{source}: {destination}" for source, destination in new_redirects.items()]
    block = "\n".join(lines) + "\n"

    if args.dry_run:
        print(block, end="")
        return 0

    with REDIRECTS_FILE.open("a", encoding="utf-8") as f:
        f.write(block)

    print(f"Added {len(new_redirects)} redirects to {REDIRECTS_FILE.relative_to(Path.cwd())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
