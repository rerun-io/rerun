"""
Anchors (`#fragment`s) in the docs.

The website turns each markdown heading into an anchor. This module reproduces that
slug so that scripts can generate anchors and check them against a real heading.

The rule below is the common GitHub-flavored one. If the website ever diverges from it,
a link can still point at a heading that exists but resolve to the top of the page, which
no script in this repository can detect — only self-consistency is checked here.
"""

from __future__ import annotations

import re
from pathlib import Path

# `[text](url)` → `text`
_MARKDOWN_LINK = re.compile(r"\[([^\]]*)\]\([^)]*\)")

# Emphasis and code markers, which the heading text keeps but the slug drops.
_MARKDOWN_MARKERS = re.compile(r"[`*_~]")

# Everything a slug may not contain. Spaces survive this pass and become hyphens after it.
_NON_SLUG = re.compile(r"[^\w\- ]", re.UNICODE)


def slugify_heading(heading: str) -> str:
    """Anchor for a markdown heading, without the leading `#` characters."""

    text = _MARKDOWN_LINK.sub(r"\1", heading.strip())
    text = _MARKDOWN_MARKERS.sub("", text)
    text = _NON_SLUG.sub("", text.lower())
    return text.strip().replace(" ", "-")


def headings_in(path: Path) -> list[str]:
    """Every markdown heading in a file, in order, with the leading `#` characters stripped."""

    headings = []
    in_code_block = False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.lstrip().startswith("```"):
            in_code_block = not in_code_block
        elif not in_code_block and line.startswith("#"):
            headings.append(line.lstrip("#").strip())
    return headings


def anchors_in(path: Path) -> set[str]:
    """Every anchor a markdown file offers."""

    return {slugify_heading(heading) for heading in headings_in(path)}
