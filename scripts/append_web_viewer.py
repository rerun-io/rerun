#!/usr/bin/env python3
"""
Append web viewer assets to a CLI binary built with __trailing_web_viewer feature.

This script creates a zip archive of the web viewer assets and appends it to the
end of a binary, along with metadata needed to locate and extract the assets at runtime.

Format of trailing data:
    [Original Binary] [ZIP Archive] [ZIP Offset: 8 bytes LE] [Magic: "RERUNWEB"]

Usage:
    python3 scripts/append_web_viewer.py <binary_path> <web_viewer_dir>

Example:
    python3 scripts/append_web_viewer.py target/release/rerun rerun/crates/viewer/re_web_viewer_server/web_viewer
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from create_web_viewer_zip import create_web_viewer_zip

MAGIC = b"RERUNWEB"
MAGIC_LEN = 8
OFFSET_LEN = 8


def append_web_viewer_to_binary(binary_path: Path, web_viewer_dir: Path) -> None:
    """Append web viewer assets to a binary."""
    if not binary_path.exists():
        raise FileNotFoundError(f"Binary not found: {binary_path}")

    print(f"Creating zip archive from {web_viewer_dir}…")
    zip_data = create_web_viewer_zip(web_viewer_dir)
    print(f"Created zip archive ({len(zip_data)} bytes)")

    # Get the current size of the binary (this is where the zip will start)
    binary_size = binary_path.stat().st_size
    zip_offset = binary_size

    print(f"Appending to binary {binary_path} (current size: {binary_size} bytes)…")

    # Append the zip data, offset, and magic to the binary
    with open(binary_path, "ab") as f:
        # Write the zip archive
        f.write(zip_data)

        # Write the zip offset (8 bytes, little-endian)
        f.write(struct.pack("<Q", zip_offset))

        # Write the magic marker
        f.write(MAGIC)

    new_size = binary_path.stat().st_size
    print(f"Done! New binary size: {new_size} bytes (+{new_size - binary_size} bytes)")
    print()
    print("The binary can now be run and will serve the web viewer from the appended assets.")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Append web viewer assets to a CLI binary.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "binary_path",
        type=Path,
        help="Path to the CLI binary (must be built with __trailing_web_viewer feature)",
    )
    parser.add_argument(
        "web_viewer_dir",
        type=Path,
        help="Path to the web viewer directory containing the assets",
    )

    args = parser.parse_args()

    try:
        append_web_viewer_to_binary(args.binary_path, args.web_viewer_dir)
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
