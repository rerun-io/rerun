#!/usr/bin/env python3

"""Checks or updates cached thumbnail dimensions in example READMEs."""

from __future__ import annotations

import argparse
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from io import BytesIO
from pathlib import Path
from typing import TYPE_CHECKING, Any

import requests
import tomlkit
from frontmatter import load_frontmatter
from PIL import Image

if TYPE_CHECKING:
    from collections.abc import Generator

Frontmatter = dict[str, Any]


class Example:
    def __init__(self, path: Path, readme: str, fm: Frontmatter) -> None:
        self.path = path
        self.readme = readme
        self.fm = fm


def get_thumbnail_dimensions(thumbnail: str) -> tuple[int, int]:
    max_retries = 3
    timeout = 10  # seconds

    for attempt in range(max_retries):
        try:
            response = requests.get(thumbnail, timeout=timeout)
            response.raise_for_status()
            size: tuple[int, int] = Image.open(BytesIO(response.content)).size
            return size
        except (requests.Timeout, requests.ConnectionError):
            if attempt == max_retries - 1:
                raise
            wait_time = 2**attempt  # exponential backoff: 1, 2, 4 seconds
            print(
                f"Timeout/connection error for {thumbnail}, retrying in {wait_time}s… (attempt {attempt + 1}/{max_retries})"
            )
            time.sleep(wait_time)

    # This should never be reached due to the raise in the loop
    raise RuntimeError("Unexpected retry loop exit")


def load_ignored_examples() -> set[str]:
    manifest_path = Path("examples/manifest.toml")
    manifest = tomlkit.loads(manifest_path.read_text(encoding="utf-8"))
    return set(manifest.get("ignored", {}).get("examples", []))


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

        def work(example: Example) -> None:
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
    ignored = load_ignored_examples()

    # Check that non-ignored examples have thumbnails (only check default language: python > cpp > rust)
    # NOTE: This language priority must match the logic in the rerun-io/landing repo:
    # https://github.com/rerun-io/landing/blob/main/src/lib/lang.ts
    example_names = set()
    for lang in ["python", "rust", "cpp", "c"]:
        lang_dir = Path(f"examples/{lang}")
        if lang_dir.exists():
            for path in lang_dir.iterdir():
                if path.is_dir() and (path / "README.md").exists():
                    example_names.add(path.name)

    for name in sorted(example_names):
        if name in ignored:
            continue

        # Find default language version (python > cpp > rust)
        default_path = None
        for lang in ["python", "cpp", "rust"]:
            path = Path(f"examples/{lang}/{name}")
            if (path / "README.md").exists():
                default_path = path
                break

        if default_path:
            readme = (default_path / "README.md").read_text(encoding="utf-8")
            fm = load_frontmatter(readme)
            if fm is None or not fm.get("thumbnail") or not fm.get("thumbnail_dimensions"):
                print(f"{default_path} is missing `thumbnail` and/or `thumbnail_dimensions`")
                bad = True

    # Check that existing thumbnail dimensions are correct (all languages)
    with ThreadPoolExecutor() as ex:

        def work(example: Example) -> None:
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
        sys.exit(1)


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
