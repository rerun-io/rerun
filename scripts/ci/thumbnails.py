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


def all_examples() -> Generator[Example, None, None]:
    def single_language(lang: str) -> Generator[Example, None, None]:
        for path in Path(f"examples/{lang}").iterdir():
            if (path / "README.md").exists():
                readme = (path / "README.md").read_text(encoding="utf-8")
                fm = load_frontmatter(readme)
                yield Example(path, readme, fm if fm is not None else {})

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

        examples_with_thumbnails = [ex for ex in all_examples() if ex.fm.get("thumbnail")]
        futures = [ex.submit(work, example) for example in examples_with_thumbnails]
        ex.shutdown()
        for future in futures:
            future.result()


def check() -> None:
    ignored = load_ignored_examples()
    missing_frontmatter = []
    missing_thumbnails = []
    missing_dimensions = []
    incorrect_dimensions = []

    # NOTE: This language priority must match the logic in the rerun-io/landing repo:
    # Priority order: Python > C++ > Rust > C.
    # See: https://github.com/rerun-io/landing/blob/main/src/lib/lang.ts

    # Build a map of example names to their default language version
    example_defaults = {}
    for lang in ["python", "cpp", "rust", "c"]:
        lang_dir = Path(f"examples/{lang}")
        if lang_dir.exists():
            for path in lang_dir.iterdir():
                if path.is_dir() and (path / "README.md").exists():
                    # Only set if we haven't seen this example before (priority order)
                    if path.name not in example_defaults and lang in ["python", "cpp", "rust", "c"]:
                        example_defaults[path.name] = path

    # Check all examples in a single pass
    with ThreadPoolExecutor() as ex:

        def work(example: Example) -> None:
            # Skip ignored examples
            if example.path.name in ignored:
                return

            # Only check default language versions for missing frontmatter/thumbnails
            is_default = example_defaults.get(example.path.name) == example.path

            # Check if frontmatter is missing
            if not example.fm:
                if is_default:
                    missing_frontmatter.append(example.path)
                return

            if not example.fm.get("thumbnail"):
                if is_default:
                    missing_thumbnails.append(example.path)
                return

            # If there's a thumbnail, check dimensions
            if not example.fm.get("thumbnail_dimensions"):
                missing_dimensions.append(example.path)
            else:
                current = tuple(example.fm["thumbnail_dimensions"])
                actual = get_thumbnail_dimensions(example.fm["thumbnail"])
                if current != actual:
                    incorrect_dimensions.append((example.path, current, actual))
                else:
                    print(f"✔ {example.path}")

        futures = [ex.submit(work, example) for example in all_examples()]
        ex.shutdown()
        for future in futures:
            future.result()

    # Report errors
    if missing_frontmatter:
        for path in sorted(missing_frontmatter):
            print(f"{path} is missing frontmatter")
        print()

    if missing_thumbnails:
        for path in sorted(missing_thumbnails):
            print(f"{path} is missing `thumbnail`")
        print()

    if missing_dimensions:
        for path in sorted(missing_dimensions):
            print(f"{path} is missing `thumbnail_dimensions`")
        print()

    if incorrect_dimensions:
        for path, current, actual in sorted(incorrect_dimensions):
            print(f"{path} `thumbnail_dimensions` are incorrect (current: {current}, actual: {actual})")
        print()

    if missing_frontmatter or missing_thumbnails:
        print(
            "Some examples are missing frontmatter or thumbnails. Either add those, or add those examples to the `ignored` list in `examples/manifest.toml`."
        )
        print()

    if missing_dimensions or incorrect_dimensions:
        print("Incorrect thumbnail dimensions may be fixed automatically by running `scripts/ci/thumbnails.py update`.")
        print()

    if missing_frontmatter or missing_thumbnails or missing_dimensions or incorrect_dimensions:
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
