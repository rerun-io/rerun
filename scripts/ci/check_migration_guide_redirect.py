from __future__ import annotations

from pathlib import Path

import yaml

DOCS_ROOT = Path(__file__).parent.parent.parent / "docs" / "content"
MIGRATION_DIR = DOCS_ROOT / "reference" / "migration"
REDIRECTS_FILE = DOCS_ROOT / "_redirects.yaml"


def extract_version(name: str) -> tuple[int, ...]:
    """Extract version tuple from a migration guide name like 'migration-0-29'."""
    version = name.removeprefix("migration-")
    return tuple(map(int, version.split("-")))


def extract_current_redirect_version() -> tuple[int, ...] | None:
    """Get the version from the reference/migration redirect in _redirects.yaml."""
    with open(REDIRECTS_FILE) as f:
        redirects = yaml.safe_load(f)

    destination = redirects.get("reference/migration")
    if destination is None:
        return None

    # destination is like "reference/migration/migration-0-29"
    name = destination.split("/")[-1]
    return extract_version(name)


def main() -> None:
    assert REDIRECTS_FILE.exists(), "Could not find _redirects.yaml"
    assert MIGRATION_DIR.exists() and MIGRATION_DIR.is_dir(), "Could not find the `migration` directory in the docs"

    max_version = max(extract_version(guide.stem) for guide in MIGRATION_DIR.glob("*.md"))

    assert max_version == extract_current_redirect_version(), (
        "The `reference/migration` redirect in _redirects.yaml is not up to date"
    )


if __name__ == "__main__":
    main()
