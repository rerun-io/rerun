#!/usr/bin/env python3

"""Check docs frontmatter order values."""

from __future__ import annotations

import argparse
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

import yaml


def parse_frontmatter(path: Path) -> dict[str, Any]:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---"):
        return {}

    end = text.find("\n---", 3)
    if end == -1:
        raise ValueError(f"{path}: unterminated YAML frontmatter")

    frontmatter = yaml.safe_load(text[3:end].strip()) or {}
    if not isinstance(frontmatter, dict):
        raise ValueError(f"{path}: frontmatter is not a mapping")

    return frontmatter


def check(root: Path) -> bool:
    docs_by_order: dict[Path, dict[Any, list[Path]]] = defaultdict(lambda: defaultdict(list))

    for path in sorted(root.rglob("*.md")):
        frontmatter = parse_frontmatter(path)
        if frontmatter.get("redirect") is not None:
            continue

        if "order" in frontmatter:
            docs_by_order[path.parent][frontmatter["order"]].append(path)

    duplicates = []
    for parent, paths_by_order in docs_by_order.items():
        for order, paths in paths_by_order.items():
            if len(paths) > 1:
                duplicates.append((parent, order, paths))

    for parent, order, paths in sorted(duplicates, key=lambda item: (item[0].as_posix(), str(item[1]))):
        print(f"{parent.relative_to(root)} has multiple docs with order {order}:")
        for path in paths:
            print(f"  {path.relative_to(root)}")
        print()

    if duplicates:
        print("Docs in the same directory must have unique `order` values.")
        return False

    return True


def main() -> None:
    parser = argparse.ArgumentParser(description="Check docs frontmatter order values")
    parser.add_argument("--root", type=Path, default=Path("docs/content"), help="Docs content root")
    args = parser.parse_args()

    if not check(args.root):
        sys.exit(1)


if __name__ == "__main__":
    main()
