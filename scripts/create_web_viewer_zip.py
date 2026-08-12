#!/usr/bin/env python3
"""
Create a zip archive of the web viewer assets.

The archive can be served by `re_web_viewer_server` at runtime (`WebViewerServer::with_archive`).
It is used both for appending to CLI binaries built with `RERUN_TRAILING_WEB_VIEWER=1`
(see `scripts/append_web_viewer.py`) and for shipping inside Python wheels built with
`RERUN_EXTERNAL_WEB_VIEWER=1` (as `rerun_sdk/web_viewer.zip`).

Usage:
    python3 scripts/create_web_viewer_zip.py <web_viewer_dir> <output_zip>

Example:
    python3 scripts/create_web_viewer_zip.py crates/viewer/re_web_viewer_server/web_viewer web_viewer.zip
"""

from __future__ import annotations

import argparse
import io
import sys
import zipfile
from pathlib import Path

# The files `re_web_viewer_server` serves. Must match the list in its `lib.rs`.
REQUIRED_FILES = [
    "index.html",
    "favicon.ico",
    "apple-touch-icon.png",
    "sw.js",
    "re_viewer.js",
    "re_viewer_bg.wasm",
    "signed-in.html",
    "signed-out.html",
]


def create_web_viewer_zip(web_viewer_dir: Path) -> bytes:
    """Create a zip archive of the web viewer assets, in memory."""
    if not web_viewer_dir.is_dir():
        raise NotADirectoryError(f"Web viewer directory not found: {web_viewer_dir}")

    for filename in REQUIRED_FILES:
        file_path = web_viewer_dir / filename
        if not file_path.exists():
            raise FileNotFoundError(f"Required file not found: {file_path}")

    # Zip every file in the directory, not just the required ones,
    # so that newly added assets cannot be silently left out.
    zip_buffer = io.BytesIO()
    with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_DEFLATED) as zip_file:
        for file_path in sorted(web_viewer_dir.iterdir()):
            if file_path.is_file():
                zip_file.write(file_path, arcname=file_path.name)

    return zip_buffer.getvalue()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Create a zip archive of the web viewer assets.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "web_viewer_dir",
        type=Path,
        help="Path to the web viewer directory containing the assets",
    )
    parser.add_argument(
        "output_zip",
        type=Path,
        help="Path to write the zip archive to",
    )

    args = parser.parse_args()

    try:
        zip_data = create_web_viewer_zip(args.web_viewer_dir)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    args.output_zip.write_bytes(zip_data)
    print(f"Wrote {args.output_zip} ({len(zip_data)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
