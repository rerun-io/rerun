#!/usr/bin/env python3

"""Check if all examples are listed (or explicitly ignored) in our example manifest."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import TYPE_CHECKING

import tomli

if TYPE_CHECKING:
    from collections.abc import Iterable


def gather_example_in_repo() -> Iterable[Path]:
    example_dir = Path(__file__).parent.parent / "examples"
    langs = ["c", "cpp", "python", "rust"]

    for lang in langs:
        land_dir = example_dir / lang
        for child in land_dir.glob("*"):
            if child.is_dir() and (child / "README.md").exists():
                yield child


def manifest_path() -> Path:
    return Path(__file__).parent.parent / "examples" / "manifest.toml"


def gather_example_in_manifest() -> Iterable[str]:
    manifest = tomli.loads(manifest_path().read_text())
    for cat in manifest["categories"].values():
        yield from cat["examples"]

    if "ignored" in manifest and "examples" in manifest["ignored"]:
        yield from manifest["ignored"]["examples"]


def main() -> None:
    listed_examples = set(gather_example_in_manifest())

    all_examples = list(gather_example_in_repo())

    print(*[f"- {example}\n" for example in listed_examples])
    print(*[f"- {example.name}\n" for example in all_examples])

    unlisted_examples: list[Path] = []
    for example_path in all_examples:
        if example_path.name not in listed_examples:
            unlisted_examples.append(example_path)

    print(f"({len(all_examples)} checked)")
    if len(unlisted_examples) > 0:
        print("Unlisted examples:")
        for example_path in unlisted_examples:
            print(f"- {example_path.parent.name}/{example_path.name}")
        print(f"Please add them to {manifest_path()}")
        sys.exit(1)
    else:
        print("all ok")


if __name__ == "__main__":
    main()
