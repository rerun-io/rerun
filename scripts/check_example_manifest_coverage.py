"""Check if all examples are listed (or explicitly ignored) in our example manifest."""
from __future__ import annotations

from pathlib import Path
from typing import Iterable

import tomli


def gather_example_in_repo() -> Iterable[Path]:
    example_dir = Path(__file__).parent.parent / "examples"
    langs = ["c", "cpp", "python", "rust"]

    for lang in langs:
        land_dir = example_dir / lang
        for child in land_dir.glob("*"):
            if child.is_dir() and (child / "README.md").exists():
                yield child


def gather_example_in_manifest() -> Iterable[str]:
    manifest_path = Path(__file__).parent.parent / "examples" / "manifest.toml"
    manifest = tomli.loads(manifest_path.read_text())
    for cat in manifest["categories"].values():
        yield from cat["examples"]

    if "ignored" in manifest and "examples" in manifest["ignored"]:
        yield from manifest["ignored"]["examples"]


def main():
    listed_examples = set(gather_example_in_manifest())

    all_examples = list(gather_example_in_repo())

    print("Unlisted examples:")
    for example_path in all_examples:
        if example_path.name not in listed_examples:
            print(f"- {example_path.parent.name}/{example_path.name}")

    print(f"({len(all_examples)} checked)")


if __name__ == "__main__":
    main()
