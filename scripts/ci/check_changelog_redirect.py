from __future__ import annotations

from pathlib import Path

CHANGELOG_DIR = Path(__file__).parent.parent.parent / "docs" / "content" / "changelog"
CHANGELOG_TOC = Path(__file__).parent.parent.parent / "docs" / "content" / "changelog.md"


def extract_version(path: Path) -> tuple[int, ...]:
    version = path.name.removesuffix(".md").removeprefix("changeset-")
    return tuple(map(int, version.split("-")))


def extract_current_redirect_version() -> tuple[int, ...] | None:
    for line in CHANGELOG_TOC.read_text().splitlines():
        if line.startswith("redirect:"):
            return extract_version(Path(line.removeprefix("redirect: ")))

    return None


def main() -> None:
    assert CHANGELOG_TOC.exists(), "Could not find the `changelog.md` file in the docs"
    assert CHANGELOG_DIR.exists() and CHANGELOG_DIR.is_dir(), "Could not find the `changelog` directory in the docs"

    changesets = list(CHANGELOG_DIR.glob("changeset-*.md"))
    assert changesets, "Could not find any `changeset-*.md` files in the changelog directory"

    max_version = max(extract_version(changeset) for changeset in changesets)

    assert max_version == extract_current_redirect_version(), "The current `changelog.md` redirect is not up to date"


if __name__ == "__main__":
    main()
