from __future__ import annotations

from pathlib import Path

MIGRATION_DIR = Path(__file__).parent.parent.parent / "docs" / "content" / "reference" / "migration"
MIGRATION_TOC = Path(__file__).parent.parent.parent / "docs" / "content" / "reference" / "migration.md"


def extract_version(path: Path) -> tuple[int, ...]:
    version = path.name.removesuffix(".md").removeprefix("migration-")
    return tuple(map(int, version.split("-")))


def extract_current_redirect_version() -> tuple[int, ...] | None:
    for line in MIGRATION_TOC.read_text().splitlines():
        if line.startswith("redirect:"):
            return extract_version(Path(line.removeprefix("redirect: ")))

    return None


def main() -> None:
    assert MIGRATION_TOC.exists(), "Could not find the `migration.md` file in the docs"
    assert MIGRATION_DIR.exists() and MIGRATION_DIR.is_dir(), "Could not find the `migration` directory in the docs"

    max_version = max(extract_version(guide) for guide in MIGRATION_DIR.glob("*.md"))

    assert max_version == extract_current_redirect_version(), "The current `migration.md` redirect is not up to date"


if __name__ == "__main__":
    main()
