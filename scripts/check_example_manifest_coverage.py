#!/usr/bin/env python3

"""
Check the example manifest against the example READMEs.

Every example must be listed in a category or explicitly ignored, and its README `steps` must fit the category that
lists it (see `check_steps`).
"""

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


def readme_steps(readme: Path) -> object:
    """The `steps` entry of a README's frontmatter, or `None` if it has none."""
    text = readme.read_text()
    start = text.find("<!--[metadata]")
    if start == -1:
        return None
    start += len("<!--[metadata]")
    end = text.find("-->", start)
    if end == -1:
        return None
    return tomli.loads(text[start:end]).get("steps")


def check_steps(all_examples: list[Path]) -> list[str]:
    """
    Error messages for README `steps` that don't fit the category listing the example.

    In a category that declares `steps`, every example must list a non-empty subset of them, in the category's order.
    Outside such a category, an example must not list any.
    """
    manifest = tomli.loads(manifest_path().read_text())
    declared_by_example: dict[str, list[str]] = {}
    for cat in manifest["categories"].values():
        for example in cat["examples"]:
            declared_by_example[example] = cat.get("steps", [])

    errors = []
    for example_path in all_examples:
        name = f"{example_path.parent.name}/{example_path.name}"
        steps = readme_steps(example_path / "README.md")
        declared = declared_by_example.get(example_path.name, [])
        if steps is None:
            if declared:
                errors.append(f"- {name}: missing `steps`, expected some of {declared}")
            continue
        if not isinstance(steps, list) or not all(isinstance(step, str) for step in steps):
            errors.append(f"- {name}: `steps` must be a list of strings, got {steps!r}")
            continue
        if not declared:
            errors.append(f"- {name}: has `steps`, but its manifest category declares none")
            continue
        expected = [step for step in declared if step in steps]
        if not steps or steps != expected:
            errors.append(f"- {name}: `steps` {steps} must be a non-empty subset of {declared}, in that order")
    return errors


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

    step_errors = check_steps(all_examples)
    if step_errors:
        print("README `steps` that don't fit the example's manifest category:")
        print(*step_errors, sep="\n")
        sys.exit(1)

    print("all ok")


if __name__ == "__main__":
    main()
