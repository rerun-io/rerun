"""Sanity-check the skills under `skills/*`.

Runs a set of independent checks against every skill directory and reports all
failures at once. Add new checks by writing a `(skill_dir) -> list[str]`
function (returning one message per problem) and appending it to `CHECKS`.
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

import yaml

REQUIRED_KEYS = ("name", "description")


def _parse_frontmatter(path: Path) -> dict[str, Any]:
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---"):
        raise ValueError("missing YAML frontmatter (file must start with `---`)")

    end = text.find("\n---", 3)
    if end == -1:
        raise ValueError("unterminated YAML frontmatter (no closing `---`)")

    frontmatter = yaml.safe_load(text[3:end].strip())
    if not isinstance(frontmatter, dict):
        raise ValueError("frontmatter is not a mapping")

    return frontmatter


def check_frontmatter(skill_dir: Path) -> list[str]:
    """Verify the front matter is valid yaml"""
    skill_md = skill_dir / "SKILL.md"
    if not skill_md.is_file():
        return ["missing SKILL.md"]

    try:
        frontmatter = _parse_frontmatter(skill_md)
    except yaml.YAMLError as err:
        # Flatten the multi-line PyYAML message so the failure is a single grep-able line.
        detail = " ".join(str(err).split())
        return [f"SKILL.md: invalid YAML frontmatter: {detail}"]
    except ValueError as err:
        return [f"SKILL.md: {err}"]

    errors = []
    missing = [key for key in REQUIRED_KEYS if not frontmatter.get(key)]
    if missing:
        errors.append(f"SKILL.md: missing required frontmatter key(s): {', '.join(missing)}")

    if "name" in frontmatter and frontmatter["name"] != skill_dir.name:
        errors.append(f"SKILL.md: `name: {frontmatter['name']}` does not match directory name `{skill_dir.name}`")

    return errors


# Each check takes a skill directory and returns one message per problem found (empty = pass).
CHECKS: list[Callable[[Path], list[str]]] = [
    check_frontmatter,
]


def check(root: Path) -> bool:
    skill_dirs = sorted(p for p in root.glob("*") if p.is_dir())
    if not skill_dirs:
        print(f"No skill directories found under {root}")
        return False

    ok = True
    for skill_dir in skill_dirs:
        errors = [msg for check_fn in CHECKS for msg in check_fn(skill_dir)]
        if errors:
            ok = False
            for msg in errors:
                print(f"FAIL {skill_dir.name}: {msg}")
        else:
            print(f"ok   {skill_dir.name}")

    return ok


def main() -> None:
    parser = argparse.ArgumentParser(description="Sanity-check the skills under skills/*")
    parser.add_argument("--root", type=Path, default=Path("skills"), help="Skills root directory")
    args = parser.parse_args()

    if not check(args.root):
        print("\nSkill checks failed. See errors above.")
        sys.exit(1)


if __name__ == "__main__":
    main()
