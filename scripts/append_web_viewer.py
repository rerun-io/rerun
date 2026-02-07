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
import io
import struct
import sys
import zipfile
from pathlib import Path


MAGIC = b"RERUNWEB"
MAGIC_LEN = 8
OFFSET_LEN = 8


def create_web_viewer_zip(web_viewer_dir: Path) -> bytes:
    """Create a zip archive of the web viewer assets."""
    required_files = [
        "index.html",
        "favicon.svg",
        "sw.js",
        "re_viewer.js",
        "re_viewer_bg.wasm",
    ]

    # Check that all required files exist
    for filename in required_files:
        file_path = web_viewer_dir / filename
        if not file_path.exists():
            raise FileNotFoundError(f"Required file not found: {file_path}")

    # Create the zip archive in memory
    zip_buffer = io.BytesIO()
    with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_DEFLATED) as zip_file:
        for filename in required_files:
            file_path = web_viewer_dir / filename
            zip_file.write(file_path, arcname=filename)

        # Also include signed-in.html from the src directory
        signed_in_path = web_viewer_dir.parent / "src" / "signed-in.html"
        if signed_in_path.exists():
            zip_file.write(signed_in_path, arcname="signed-in.html")

    return zip_buffer.getvalue()


def append_web_viewer_to_binary(binary_path: Path, web_viewer_dir: Path) -> None:
    """Append web viewer assets to a binary."""
    if not binary_path.exists():
        raise FileNotFoundError(f"Binary not found: {binary_path}")

    if not web_viewer_dir.is_dir():
        raise NotADirectoryError(f"Web viewer directory not found: {web_viewer_dir}")

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
