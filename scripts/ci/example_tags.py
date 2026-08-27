#!/usr/bin/env python3

"""Checks the `tags` in example READMEs against the master list in `examples/tags.toml`."""

from __future__ import annotations

import argparse
import re
import sys
from collections import defaultdict
from difflib import SequenceMatcher
from pathlib import Path
from typing import TYPE_CHECKING

import tomlkit
from frontmatter import load_frontmatter

if TYPE_CHECKING:
    from collections.abc import Generator

EXAMPLES_DIR = Path("examples")
TAGS_PATH = EXAMPLES_DIR / "tags.toml"

# Two tags are near-matches when their normalized forms are this similar.
# Lower it to catch more, at the cost of more false positives.
SIMILARITY_THRESHOLD = 0.85

# Short tags are usually acronyms, where one character apart means two different things
# ("SAM" and "SLAM"). Only compare those for an exact match.
MIN_LENGTH_FOR_FUZZY_MATCH = 5


def normalize(tag: str) -> str:
    """Case, spacing and punctuation are not meaningful differences between tags."""
    return re.sub(r"[^a-z0-9]", "", tag.lower())


def singularize(normalized: str) -> str:
    return normalized.removesuffix("s")


def similarity(lhs: str, rhs: str) -> float:
    """How likely two tags are to mean the same thing, from 0 to 1."""
    lhs, rhs = normalize(lhs), normalize(rhs)
    if lhs == rhs or singularize(lhs) == singularize(rhs):
        return 1.0
    if min(len(lhs), len(rhs)) < MIN_LENGTH_FOR_FUZZY_MATCH:
        return 0.0
    return SequenceMatcher(None, lhs, rhs).ratio()


def suggestions(tag: str, known: list[str]) -> list[str]:
    """The known tags that `tag` was most likely meant to be, best first."""
    scored = [(similarity(tag, other), other) for other in known]
    scored = [(score, other) for score, other in scored if score >= SIMILARITY_THRESHOLD]
    return [other for _, other in sorted(scored, key=lambda it: -it[0])]


def load_known_tags() -> list[str]:
    tags: list[str] = tomlkit.loads(TAGS_PATH.read_text(encoding="utf-8")).unwrap()["tags"]
    return tags


def load_distinct_pairs() -> set[frozenset[str]]:
    """The near-matching pairs that a human has already looked at and kept apart."""
    doc = tomlkit.loads(TAGS_PATH.read_text(encoding="utf-8")).unwrap()
    return {frozenset(pair) for pair in doc.get("distinct_pairs", [])}


def iter_examples() -> Generator[tuple[Path, list[str]]]:
    """Yields the path and tags of every example that has frontmatter."""
    for readme in sorted(EXAMPLES_DIR.rglob("README.md")):
        # The templates hold placeholder tags for people to replace.
        if "template" in readme.parts:
            continue
        fm = load_frontmatter(readme.read_text(encoding="utf-8"))
        if fm is None:
            continue
        yield readme, fm.get("tags", [])


def collect_used_tags() -> dict[str, list[Path]]:
    """Maps each tag in use to the examples using it."""
    used: dict[str, list[Path]] = defaultdict(list)
    for readme, tags in iter_examples():
        for tag in tags:
            used[tag].append(readme)
    return used


def near_matches(tags: list[str]) -> list[tuple[float, str, str]]:
    """Every pair of tags that is similar enough to be suspicious, most similar first."""
    known_distinct = load_distinct_pairs()
    pairs = [
        (similarity(lhs, rhs), lhs, rhs)
        for i, lhs in enumerate(tags)
        for rhs in tags[i + 1 :]
        if similarity(lhs, rhs) >= SIMILARITY_THRESHOLD and frozenset((lhs, rhs)) not in known_distinct
    ]
    return sorted(pairs, key=lambda it: (-it[0], it[1].lower()))


def check() -> None:
    known = load_known_tags()
    used = collect_used_tags()

    errors: list[str] = []

    for tag, readmes in sorted(used.items()):
        if tag in known:
            continue
        error = f"Unknown tag {tag!r} in {', '.join(str(p) for p in readmes)}"
        if did_you_mean := suggestions(tag, known):
            error += f"\n    Did you mean {' or '.join(repr(t) for t in did_you_mean)}?"
        errors.append(error)

    # The master list must not grow near-duplicates of its own.
    for score, lhs, rhs in near_matches(known):
        errors.append(f"{TAGS_PATH} lists both {lhs!r} and {rhs!r}, which are {score:.0%} similar. Keep only one.")

    if errors:
        print("\n".join(errors))
        print()
        print(
            f"Every tag must appear in {TAGS_PATH}. Reuse an existing tag where one fits, "
            "or add the new tag to that list."
        )
        sys.exit(1)

    unused = sorted(set(known) - set(used), key=str.lower)
    print(f"{len(used)} tags in use across {len(list(iter_examples()))} examples, all listed in {TAGS_PATH}.")
    if unused:
        print(f"{len(unused)} listed but unused: {', '.join(unused)}")


def report_similar() -> None:
    """Reports near-matches among the tags actually in use, for cleaning up the list."""
    used = collect_used_tags()
    pairs = near_matches(sorted(used))

    if not pairs:
        print("No near-matching tags found.")
        return

    for score, lhs, rhs in pairs:
        print(f"{score:.0%}  {lhs!r} ({len(used[lhs])}) vs {rhs!r} ({len(used[rhs])})")
        for tag in (lhs, rhs):
            for readme in used[tag]:
                print(f"        {tag!r}: {readme}")
    print()
    print(f"{len(pairs)} near-matching pairs at or above {SIMILARITY_THRESHOLD:.0%} similarity.")


def list_tags(show_paths: bool) -> None:
    used = collect_used_tags()
    for tag, readmes in sorted(used.items(), key=lambda it: (-len(it[1]), it[0].lower())):
        print(f"{len(readmes):4}  {tag}")
        if show_paths:
            for readme in readmes:
                print(f"          {readme}")

    unused = sorted(set(load_known_tags()) - set(used), key=str.lower)
    for tag in unused:
        print(f"{0:4}  {tag}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    cmd_parser = parser.add_subparsers(title="cmd", dest="cmd", required=True)
    cmd_parser.add_parser("check", help=f"Check that every tag in use appears in {TAGS_PATH}")
    cmd_parser.add_parser("similar", help="Report tags that are similar enough to be duplicates")
    list_parser = cmd_parser.add_parser("list", help="List every tag with its usage count, most used first")
    list_parser.add_argument("--paths", action="store_true", help="Also list the examples using each tag")
    args = parser.parse_args()

    if args.cmd == "check":
        check()
    elif args.cmd == "similar":
        report_similar()
    elif args.cmd == "list":
        list_tags(args.paths)


if __name__ == "__main__":
    main()
