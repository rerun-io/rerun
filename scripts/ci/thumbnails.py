#!/usr/bin/env python3

"""Checks or updates cached thumbnail dimensions in example READMEs."""

from __future__ import annotations

import argparse
from collections.abc import Generator
from concurrent.futures import ThreadPoolExecutor
from io import BytesIO
from pathlib import Path
from typing import Any

import requests
from frontmatter import load_frontmatter
from PIL import Image

Frontmatter = dict[str, Any]


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
    def single_language(lang: str) -> Generator[Example, None, None]:
        for path in Path(f"examples/{lang}").iterdir():
            if (path / "README.md").exists():
                readme = (path / "README.md").read_text(encoding="utf-8")
                fm = load_frontmatter(readme)
                if fm is not None and fm.get("thumbnail"):
                    yield Example(path, readme, fm)

    yield from single_language("c")
    yield from single_language("cpp")
    yield from single_language("rust")
    yield from single_language("python")


def update() -> None:
    with ThreadPoolExecutor() as ex:

        def work(example: Example):
            width, height = get_thumbnail_dimensions(example.fm["thumbnail"])

            if "thumbnail_dimensions" not in example.fm:
                start = example.readme.find("thumbnail = ")
                assert start != -1
                end = example.readme.find("\n", start)
                assert end != -1
                start = end + 1
            else:
                start = example.readme.find("thumbnail_dimensions = ")
                assert start != -1
                end = example.readme.find("\n", start)
                assert end != -1

            (example.path / "README.md").write_text(
                example.readme[:start] + f"thumbnail_dimensions = [{width}, {height}]" + example.readme[end:],
                encoding="utf-8",
                newline="\n",
            )

            print(f"✔ {example.path}")

        futures = [ex.submit(work, example) for example in examples_with_thumbnails()]
        ex.shutdown()
        for future in futures:
            future.result()


def check() -> None:
    bad = False
    with ThreadPoolExecutor() as ex:

        def work(example: Example):
            nonlocal bad
            if not example.fm.get("thumbnail_dimensions"):
                print(f"{example.path} has no `thumbnail_dimensions`")
                bad = True
                return

            current = tuple(example.fm["thumbnail_dimensions"])
            actual = get_thumbnail_dimensions(example.fm["thumbnail"])
            if current != actual:
                print(f"{example.path} `thumbnail_dimensions` are incorrect (current: {current}, actual: {actual})")
                bad = True
            else:
                print(f"✔ {example.path}")

        futures = [ex.submit(work, example) for example in examples_with_thumbnails()]
        ex.shutdown()
        for future in futures:
            future.result()

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
