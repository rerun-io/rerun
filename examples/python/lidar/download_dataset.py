"""Module to download nuScenes minisplit."""
from __future__ import annotations

import os
import pathlib
import tarfile

import requests
import tqdm

MINISPLIT_SCENES = [
    "scene-0061",
    "scene-0103",
    "scene-0553",
    "scene-0655",
    "scene-0757",
    "scene-0796",
    "scene-0916",
    "scene-1077",
    "scene-1094",
    "scene-1100",
]
MINISPLIT_URL = "https://www.nuscenes.org/data/v1.0-mini.tgz"


def download_file(url: str, dst_file_path: pathlib.Path) -> None:
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


def untar_file(tar_file_path: pathlib.Path, dst_path: pathlib.Path, keep_tar: bool = True) -> bool:
    """Untar tar file at tar_file_path to dst."""
    print(f"Untar file {tar_file_path}")
    try:
        with tarfile.open(tar_file_path, "r") as tf:
            tf.extractall(dst_path)
    except Exception as error:
        print(f"Error unzipping {tar_file_path}, error: {error}")
        return False
    if not keep_tar:
        os.remove(tar_file_path)
    return True


def download_minisplit(root_dir: pathlib.Path) -> None:
    """
    Download nuScenes minisplit.

    Adopted from https://colab.research.google.com/github/nutonomy/nuscenes-devkit/blob/master/python-sdk/tutorials/nuscenes_tutorial.ipynb
    """
    zip_file_path = pathlib.Path("./v1.0-mini.tgz")
    if not zip_file_path.is_file():
        download_file(MINISPLIT_URL, zip_file_path)
    untar_file(zip_file_path, root_dir, keep_tar=True)
