#!/usr/bin/env python3
from __future__ import annotations

import os
from pathlib import Path
from typing import Final

import requests

DATASET_URL_BASE: Final = "https://storage.googleapis.com/rerun-example-datasets/raw_mesh"

# TODO(cmc): re-enable obj meshes when we support those on the viewer's side
AVAILABLE_MESHES: Final = [
    "lantern",
    "avocado",
    "buggy",
    "brain_stem",
    # "buddha",
    # "bunny",
    # "dragon",
    # "mori_knob",
]

# Maps mesh name to list of (remote_filename, local_filename) pairs.
MESH_FILES: Final = {
    "avocado": [("avocado.glb", "avocado.glb")],
    "brain_stem": [("brain_stem.glb", "brain_stem.glb")],
    "buddha": [("buddha.obj", "buddha.obj")],
    "buggy": [("buggy.glb", "buggy.glb")],
    "bunny": [("bunny.obj", "bunny.obj")],
    "dragon": [("dragon.obj", "dragon.obj")],
    "lantern": [("lantern.glb", "lantern.glb")],
    "mori_knob": [
        ("testObj.obj", "testObj.obj"),
        ("testObj.mtl", "testObj.mtl"),
    ],
}

DOWNLOADED_DIR: Final = Path(__file__).parent.parent / "dataset"


def ensure_mesh_downloaded(mesh_name: str) -> Path:
    path = find_mesh_path_if_downloaded(mesh_name)
    if path is not None:
        return path
    return download_mesh(mesh_name)


def find_mesh_path_if_downloaded(name: str) -> Path | None:
    for mesh_format in ("obj", "glb"):
        for path in DOWNLOADED_DIR.glob(f"{name}/**/*.{mesh_format}"):
            return path
    return None


def download_mesh(name: str) -> Path:
    """Downloads a mesh from the Rerun example datasets bucket and returns the path to the main mesh file."""
    if name not in MESH_FILES:
        raise RuntimeError(f"Unknown mesh named: {name}")

    mesh_dir = DOWNLOADED_DIR / name
    os.makedirs(mesh_dir, exist_ok=True)

    main_path = None
    for remote_name, local_name in MESH_FILES[name]:
        local_path = mesh_dir / local_name
        if not local_path.exists():
            url = f"{DATASET_URL_BASE}/{name}/{remote_name}"
            print(f"downloading {url}…")
            resp = requests.get(url)
            resp.raise_for_status()
            with open(local_path, "wb") as f:
                f.write(resp.content)

        # The main mesh file is the first one listed (obj or glb).
        if main_path is None:
            main_path = local_path

    assert main_path is not None
    return main_path
