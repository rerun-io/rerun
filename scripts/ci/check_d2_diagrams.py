#!/usr/bin/env python3

"""Checks that no D2 diagrams are stored as fenced code blocks in markdown.

D2 diagrams must be rendered to SVG and embedded as `<img>` elements, not
stored as ```d2 code blocks. Use `scripts/render_d2.py` to render a diagram
and produce a ready-to-paste HTML block.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Matches the opening fence of a ```d2 code block, allowing leading
# whitespace (indented blocks) and trailing info-string after `d2`.
D2_FENCE_RE = re.compile(r"^[ \t]*(`{3,}|~{3,})[ \t]*d2\b", re.IGNORECASE)

SEARCH_ROOTS = ("docs/content", "examples")


def find_d2_code_blocks(text: str) -> list[int]:
    """Return the 1-based line numbers of every ```d2 opening fence."""
    hits: list[int] = []
    for i, line in enumerate(text.splitlines(), start=1):
        if D2_FENCE_RE.match(line):
            hits.append(i)
    return hits


def all_markdown_files() -> list[Path]:
    files: list[Path] = []
    for root in SEARCH_ROOTS:
        files.extend(sorted(Path(root).rglob("*.md")))
    return files


def check() -> None:
    offenders: list[tuple[Path, int]] = []
    for path in all_markdown_files():
        text = path.read_text(encoding="utf-8")
        for line in find_d2_code_blocks(text):
            offenders.append((path, line))

    if offenders:
        for path, line in offenders:
            print(f"{path}:{line}: D2 diagram stored as a code block")
        print()
        print(
            "D2 diagrams must not be stored as ```d2 code blocks. "
            "Render them to SVG with `scripts/render_d2.py` and embed the "
            'resulting `<div class="d2-diagram">` HTML block instead.'
        )
        sys.exit(1)

    print("✔ no D2 code blocks found")


def main() -> None:
    check()


if __name__ == "__main__":
    main()
