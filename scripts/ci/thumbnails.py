#!/usr/bin/env python3

"""Checks or updates cached thumbnail dimensions in example READMEs."""
from __future__ import annotations

import argparse
from io import BytesIO
from pathlib import Path
from typing import Generator

import requests
from PIL import Image

from ..frontmatter import Frontmatter, load_frontmatter


class Example:
    def __init__(self, path: Path, readme: str, fm: Frontmatter) -> None:
        self.path = path
        self.readme = readme
        self.fm = fm


def get_thumbnail_dimensions(thumbnail: str) -> tuple[int, int]:
    response = requests.get(thumbnail)
    response.raise_for_status()
    size: tuple[int, int] = Image.open(BytesIO(response.content)).size
    return size


def examples_with_thumbnails() -> Generator[Example, None, None]:
    for path in Path("examples/python").iterdir():
        if (path / "README.md").exists():
            readme = (path / "README.md").read_text()
            fm = load_frontmatter(readme)
            if fm.get("thumbnail"):
                yield Example(path, readme, fm)


def update() -> None:
    for example in examples_with_thumbnails():
        width, height = get_thumbnail_dimensions(example.fm["thumbnail"])

        if "thumbnail_dimensions" not in example.fm:
            start = example.readme.find("thumbnail: ")
            assert start != -1
            end = example.readme.find("\n", start)
            assert end != -1
            start = end + 1
        else:
            start = example.readme.find("thumbnail_dimensions: ")
            assert start != -1
            end = example.readme.find("\n", start)
            assert end != -1

        (example.path / "README.md").write_text(
            example.readme[:start] + f"thumbnail_dimensions: [{width}, {height}]" + example.readme[end:]
        )

        print(f"✔ {example.path}")


def check() -> None:
    bad = False
    for example in examples_with_thumbnails():
        if not example.fm.get("thumbnail_dimensions"):
            print(f"{example.path} has no `thumbnail_dimensions`")
            bad = True
            continue

        current = tuple(example.fm["thumbnail_dimensions"])
        actual = get_thumbnail_dimensions(example.fm["thumbnail"])
        if current != actual:
            print(f"{example.path} `thumbnail_dimensions` are incorrect (current: {current}, actual: {actual})")
            bad = True

        print(f"✔ {example.path}")

    if bad:
        print("Please run `scripts/ci/thumbnails.py update`.")
        exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Check example thumbnails")
    cmd_parser = parser.add_subparsers(title="cmd", dest="cmd", required=True)
    cmd_parser.add_parser("check", help="Check that example thumbnails have correct thumbnail_dimensions")
    cmd_parser.add_parser("update", help="Update thumbnail_dimensions for each example")
    args = parser.parse_args()

    if args.cmd == "check":
        check()
    elif args.cmd == "update":
        update()


if __name__ == "__main__":
    main()
