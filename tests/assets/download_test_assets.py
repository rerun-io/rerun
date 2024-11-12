"""
Downloads test assets used by tests.

Usage:
    pixi run python tests/assets/download_test_assets.py
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Final

import requests
import tqdm

test_assets = [
    "video/Big_Buck_Bunny_1080_10s_av1.mp4",
    "video/Big_Buck_Bunny_1080_10s_h264.mp4",
    "video/Big_Buck_Bunny_1080_10s_h265.mp4",
    "video/Big_Buck_Bunny_1080_10s_vp9.mp4",
    "video/Sintel_1080_10s_av1.mp4",
]

test_asset_base_url = "https://storage.googleapis.com/rerun-test-assets/"


def download_file(url: str, dst_file_path: Path) -> None:
    """Download file from url to dst_fpath."""
    dst_file_path.parent.mkdir(parents=True, exist_ok=True)
    print(f"Downloading {url} to {dst_file_path}")
    response = requests.get(url, stream=True)
    with tqdm.tqdm.wrapattr(
        open(dst_file_path, "wb"),
        "write",
        miniters=1,
        total=int(response.headers.get("content-length", 0)),
        desc=f"Downloading {dst_file_path.name}",
    ) as f:
        for chunk in response.iter_content(chunk_size=4096):
            f.write(chunk)


def main() -> None:
    """Downloads all test assets."""

    test_asset_dir: Final = Path(os.path.dirname(__file__))

    for asset in test_assets:
        target_file = test_asset_dir / asset
        if not target_file.exists():
            download_file(test_asset_base_url + asset, target_file)
        else:
            print(f'Skipping "{asset}" because it already exists.')


if __name__ == "__main__":
    main()
