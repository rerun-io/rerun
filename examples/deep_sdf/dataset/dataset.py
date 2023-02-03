#!/usr/bin/env python3

import io
import os
import zipfile
from pathlib import Path
from typing import Final, Optional

import requests

# TODO(cmc): re-enable obj meshes when we support those on the viewer's side
AVAILABLE_MESHES: Final = [
    "avocado",
    "brain_stem",
    # "buddha",
    "buggy",
    # "bunny",
    # "dragon",
    "lantern",
    # "mori_knob",
]
DOWNLOADED_DIR: Final = Path(os.path.dirname(__file__)) / "downloaded"


def ensure_mesh_downloaded(mesh_name: str) -> Path:
    path = find_mesh_path_if_downloaded(mesh_name)
    if path is not None:
        return path
    return download_mesh(mesh_name)


def download_mesh(name: str) -> Path:
    if name == "avocado":
        return download_glb_sample("avocado")
    if name == "brain_stem":
        return download_glb_sample("brain_stem")
    if name == "buddha":
        return download_mcguire_sample("research", "buddha")
    if name == "buggy":
        return download_glb_sample("buggy")
    if name == "bunny":
        return download_mcguire_sample("research", "bunny")
    if name == "dragon":
        return download_mcguire_sample("research", "dragon")
    if name == "lantern":
        return download_glb_sample("lantern")
    if name == "mori_knob":
        return download_mcguire_sample("common", "mori_knob")
    raise RuntimeError(f"Unknown mesh named: {name}")


def find_mesh_path_if_downloaded(name: str) -> Optional[Path]:
    for mesh_format in ("obj", "glb"):
        for path in DOWNLOADED_DIR.glob(f"{name}/**/*.{mesh_format}"):
            return path
    return None


def download_mcguire_sample(package: str, name: str) -> Path:
    """Downloads a mcguire sample mesh and returns the path it was downloaded to."""

    # To satisfy Apache mod_security thing.
    headers = {
        "User-Agent": "Wget/1.12 (cygwin)",
        "Accept": "*/*",
        "Connection": "Keep-Alive",
    }
    url = f"https://casual-effects.com/g3d/data10/{package}/model/{name}/{name}.zip"

    dir = Path(os.path.dirname(__file__)) / "downloaded"
    os.makedirs(dir, exist_ok=True)

    def download_and_extract(url: str, path: Path) -> None:
        if not os.path.exists(path):
            print(f"downloading {url}…")
            resp = requests.get(url, stream=True, headers=headers)
            z = zipfile.ZipFile(io.BytesIO(resp.content))
            z.extractall(path)

    download_path = dir / name
    download_and_extract(url, download_path)
    return download_path


def download_glb_sample(name: str) -> Path:
    """Downloads a sample glb mesh and returns the path it was downloaded to."""
    capitalized_name = name.capitalize()
    url = f"https://github.com/KhronosGroup/glTF-Sample-Models/blob/master/2.0/{capitalized_name}/glTF-Binary/{capitalized_name}.glb?raw=true"

    def download(url: str, path: Path) -> None:
        if not os.path.exists(path):
            print(f"downloading {url} …")
            resp = requests.get(url)
            os.makedirs(path.parent, exist_ok=True)
            with open(path, "wb") as f:
                f.write(resp.content)

    name = name.lower()
    sample_dir = DOWNLOADED_DIR / name
    os.makedirs(DOWNLOADED_DIR, exist_ok=True)
    download_path = sample_dir / f"{name}.glb"
    download(url, download_path)
    return download_path
