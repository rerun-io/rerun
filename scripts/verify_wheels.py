"""Script to confirm wheels have the expected version number."""
import argparse
import sys
from pathlib import Path

from packaging.utils import canonicalize_version


def check_version(folder: str, expected_version: str) -> None:
    wheels = list(Path("wheels").glob("*.whl"))

    for wheel in wheels:
        wheel_version = wheel.stem.split("-")[1]
        if canonicalize_version(wheel_version) != expected_version:
            print(f"Unexpected version: {wheel_version} (expected: {expected_version}) in {wheel.name}")
            sys.exit(1)

    print(f"All wheel versions match the expected version: {expected_version}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate wheels have the specified version")
    parser.add_argument("--version", required=True, help="Version to expect")
    parser.add_argument("--folder", required=True, help="Version to expect")
    args = parser.parse_args()

    check_version(args.folder, canonicalize_version(args.version))


if __name__ == "__main__":
    main()
